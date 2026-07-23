use crate::measurement_transform::{MeasurementTransform, MeasurementTransformError};
use crate::sample_archive::dense::pack_dense;
use crate::sample_archive::format::{
    ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, BlockHeader, CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
    CODEC_SUITE_ZSTD_FRAMES_V1, FINGERPRINT_SHA256_CANONICAL_CIRCUIT, GLOBAL_HEADER_LEN,
    GlobalHeader, REFERENCE_SIMULATE_NOISELESS, STREAM_CODEC_EMPTY, STREAM_CODEC_FREE_DENSE_V1,
    SampleArchiveError, SampleArchiveErrorCode, TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
    checked_dense_bit_bytes,
};
use crate::sample_archive::integrity::{finalize_header, finalize_trailer};
use crate::sample_archive::limits::{ArchiveLimits, SampleArchiveOptions};
use crate::sample_archive::syndrome::{
    materialize_syndrome, plan_syndrome, update_dense_syndrome_hash,
};
use crate::sample_archive::telemetry::{
    bit_table_bytes, checked_sum, record_buffered_input, record_transform_payloads,
    record_transform_retained, record_writer_live_bytes,
};
use crate::sample_archive::zstd_frame::compress_frame;
use crate::sim::bit_table::BitTable;
use sha2::{Digest, Sha256};
use std::io::Write;

const DECOMP_LIMIT: &str = "decompressed archive bytes exceed limit";
const COMP_LIMIT: &str = "compressed archive bytes exceed limit";

pub struct SampleArchiveWriter<W: Write> {
    output: W,
    transform: MeasurementTransform,
    total_shots: u64,
    options: SampleArchiveOptions,
    limits: ArchiveLimits,
    archive_hasher: Sha256,
    next_block_index: u64,
    written_shots: u64,
    buffered_shots: usize,
    buffer: Option<BitTable>,
    archive_bytes: u64,
    decompressed_archive_bytes: u64,
    compressed_archive_bytes: u64,
}

impl<W: Write> SampleArchiveWriter<W> {
    pub fn new(
        mut output: W,
        transform: MeasurementTransform,
        total_shots: u64,
        options: SampleArchiveOptions,
        limits: ArchiveLimits,
    ) -> Result<Self, SampleArchiveError> {
        validate_transform_and_archive_shape(&transform, total_shots, limits)?;
        record_transform_retained(transform.transform_working_bytes());
        let identity = transform.identity();
        let mut header = GlobalHeader {
            required_flags: 0,
            optional_flags: 0,
            canonicalization_id: CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
            fingerprint_id: FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
            transform_id: TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
            reference_id: REFERENCE_SIMULATE_NOISELESS,
            codec_suite_id: CODEC_SUITE_ZSTD_FRAMES_V1,
            max_shots_per_block: limits.transform.max_shots_per_block,
            measurement_count: identity.measurement_count,
            detector_count: identity.detector_count,
            observable_count: identity.observable_count,
            detector_rank: identity.detector_rank,
            total_shots,
            circuit_sha256: identity.circuit_sha256,
            header_sha256: [0; 32],
        };
        let header_bytes = finalize_header(&mut header)?;
        output.write_all(&header_bytes).map_err(map_io)?;
        let mut archive_hasher = Sha256::new();
        archive_hasher.update(header_bytes);
        Ok(Self {
            output,
            transform,
            total_shots,
            options,
            limits,
            archive_hasher,
            next_block_index: 0,
            written_shots: 0,
            buffered_shots: 0,
            buffer: None,
            archive_bytes: GLOBAL_HEADER_LEN as u64,
            decompressed_archive_bytes: 0,
            compressed_archive_bytes: 0,
        })
    }

    pub fn write_measurements(
        &mut self,
        measurements: &BitTable,
    ) -> Result<(), SampleArchiveError> {
        if measurements.num_minor() == 0 {
            return Err(shape("measurement chunk must contain at least one shot"));
        }
        if measurements.num_major() != self.transform.num_measurements() {
            return Err(shape(
                "measurement table shape does not match archive header",
            ));
        }
        let chunk_shots = measurements.num_minor() as u64;
        let supplied = self
            .written_shots
            .checked_add(self.buffered_shots as u64)
            .ok_or_else(|| limit("supplied shot count overflow"))?;
        let after_chunk = supplied
            .checked_add(chunk_shots)
            .ok_or_else(|| limit("supplied shot count overflow"))?;
        if after_chunk > self.total_shots {
            return Err(shape("measurement chunk exceeds declared total shots"));
        }
        self.ensure_buffer()?;
        let max_block_shots = max_block_shots_usize(self.limits)?;
        let mut chunk_offset = 0usize;
        while chunk_offset < measurements.num_minor() {
            let space = max_block_shots - self.buffered_shots;
            let copied = space.min(measurements.num_minor() - chunk_offset);
            copy_measurement_columns(
                measurements,
                chunk_offset,
                self.buffer
                    .as_mut()
                    .expect("buffer initialized before streaming copy"),
                self.buffered_shots,
                copied,
            );
            self.buffered_shots += copied;
            let measurement_rows = self.transform.num_measurements() as u64;
            record_buffered_input(measurement_rows, self.buffered_shots as u64)?;
            chunk_offset += copied;
            if self.buffered_shots == max_block_shots {
                self.emit_buffered_block(max_block_shots)?;
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, SampleArchiveError> {
        let supplied = self
            .written_shots
            .checked_add(self.buffered_shots as u64)
            .ok_or_else(|| limit("supplied shot count overflow"))?;
        if supplied != self.total_shots {
            return Err(shape(
                "supplied measurement shots do not match declared total",
            ));
        }
        if self.buffered_shots > 0 {
            self.emit_buffered_block(self.buffered_shots)?;
        }
        if checked_archive_byte_total(self.archive_bytes, ARCHIVE_TRAILER_LEN as u64)?
            > self.limits.max_archive_bytes
        {
            return Err(limit("archive bytes exceed limit"));
        }
        let archive_hasher = self.archive_hasher.clone();
        let trailer = finalize_trailer(self.next_block_index, self.written_shots, archive_hasher)?;
        self.output.write_all(&trailer).map_err(map_io)?;
        self.output.flush().map_err(map_io)?;
        Ok(self.output)
    }

    fn ensure_buffer(&mut self) -> Result<(), SampleArchiveError> {
        if self.buffer.is_none() {
            let max_block_shots = max_block_shots_usize(self.limits)?;
            let buffer = BitTable::try_new(self.transform.num_measurements(), max_block_shots)
                .map_err(|_| limit("measurement buffer allocation failed"))?;
            self.buffer = Some(buffer);
        }
        Ok(())
    }

    fn emit_buffered_block(&mut self, shots: usize) -> Result<(), SampleArchiveError> {
        if shots == 0 {
            return Err(shape("archive block must contain at least one shot"));
        }
        if self.next_block_index >= self.limits.max_block_count {
            return Err(limit("archive block count exceeds limit"));
        }
        let max_block_shots = max_block_shots_usize(self.limits)?;
        if shots > max_block_shots {
            return Err(limit("block shot count exceeds limit"));
        }
        let buffered_bytes =
            record_buffered_input(self.transform.num_measurements() as u64, shots as u64)?;
        let buffer = self
            .buffer
            .as_ref()
            .expect("buffer exists before block emission");
        let encoded = self
            .transform
            .encode_block_prefix(buffer, shots)
            .map_err(map_transform_error)?;
        record_transform_payloads(2);
        let free_len =
            checked_dense_bit_bytes(encoded.free_measurements.num_major() as u64, shots as u64)?;
        let syndrome_plan = plan_syndrome(&encoded.selected_detectors)?;
        validate_decompressed_streams(syndrome_plan.raw_len, free_len, self.limits)?;
        let block_decompressed_bytes = syndrome_plan
            .raw_len
            .checked_add(free_len)
            .ok_or_else(|| limit("decompressed archive bytes overflow"))?;
        let next_decompressed_archive_bytes = self
            .decompressed_archive_bytes
            .checked_add(block_decompressed_bytes)
            .ok_or_else(|| limit("decompressed archive bytes overflow"))?;
        if next_decompressed_archive_bytes > self.limits.max_decompressed_bytes_per_archive {
            return Err(limit("decompressed archive bytes exceed limit"));
        }
        let syndrome = materialize_syndrome(&encoded.selected_detectors, syndrome_plan)?;
        let free = pack_dense(&encoded.free_measurements)?;
        let mut logical_hasher = Sha256::new();
        update_dense_syndrome_hash(&encoded.selected_detectors, &mut logical_hasher)?;
        logical_hasher.update(&free);
        let logical_payload_sha256: [u8; 32] = logical_hasher.finalize().into();

        let syndrome_frame = if syndrome.is_empty() {
            Vec::new()
        } else {
            compress_frame(&syndrome, self.options.compression_level)?
        };
        let free_frame = if free.is_empty() {
            Vec::new()
        } else {
            compress_frame(&free, self.options.compression_level)?
        };
        validate_compressed_streams(
            syndrome_frame.len() as u64,
            free_frame.len() as u64,
            self.limits,
        )?;
        let block_compressed_bytes = (syndrome_frame.len() as u64)
            .checked_add(free_frame.len() as u64)
            .ok_or_else(|| limit("compressed archive bytes overflow"))?;
        let next_compressed_archive_bytes = self
            .compressed_archive_bytes
            .checked_add(block_compressed_bytes)
            .ok_or_else(|| limit("compressed archive bytes overflow"))?;
        if next_compressed_archive_bytes > self.limits.max_compressed_bytes_per_archive {
            return Err(limit("compressed archive bytes exceed limit"));
        }
        let block_archive_bytes = checked_block_archive_byte_delta(block_compressed_bytes)?;
        let next_archive_bytes =
            checked_archive_byte_total(self.archive_bytes, block_archive_bytes)?;
        let archive_with_trailer =
            checked_archive_byte_total(next_archive_bytes, ARCHIVE_TRAILER_LEN as u64)?;
        if archive_with_trailer > self.limits.max_archive_bytes {
            return Err(limit("archive bytes exceed limit"));
        }

        let block_header = BlockHeader {
            block_index: self.next_block_index,
            first_shot: self.written_shots,
            shot_count: shots as u64,
            syndrome_codec_id: syndrome_plan.codec_id,
            free_codec_id: if free.is_empty() {
                STREAM_CODEC_EMPTY
            } else {
                STREAM_CODEC_FREE_DENSE_V1
            },
            syndrome_uncompressed_len: syndrome_plan.raw_len,
            syndrome_compressed_len: syndrome_frame.len() as u64,
            free_uncompressed_len: free.len() as u64,
            free_compressed_len: free_frame.len() as u64,
            logical_payload_sha256,
        };
        let block_bytes = block_header.to_bytes()?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &block_bytes)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &syndrome_frame)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &free_frame)?;
        let selected_rows = encoded.selected_detectors.num_major() as u64;
        let free_rows = encoded.free_measurements.num_major() as u64;
        let encoded_selected_bytes =
            bit_table_bytes("writer.encoded_selected", selected_rows, shots as u64)?;
        let encoded_free_bytes = bit_table_bytes("writer.encoded_free", free_rows, shots as u64)?;
        let raw_parts = [
            ("syndrome_raw", syndrome.len() as u64),
            ("free_raw", free.len() as u64),
        ];
        let raw_bytes = checked_sum("writer.raw_codec_buffers", &raw_parts)?;
        let compressed_parts = [
            ("syndrome_frame", syndrome_frame.len() as u64),
            ("free_frame", free_frame.len() as u64),
        ];
        let compressed_bytes = checked_sum("writer.compressed_frames", &compressed_parts)?;
        let writer_live_parts = [
            ("buffered_input", buffered_bytes),
            ("encoded_selected", encoded_selected_bytes),
            ("encoded_free", encoded_free_bytes),
            ("raw_codec_buffers", raw_bytes),
            ("compressed_frames", compressed_bytes),
            ("zstd_state", self.limits.max_zstd_window_bytes),
        ];
        record_writer_live_bytes(&writer_live_parts)?;
        self.written_shots = self
            .written_shots
            .checked_add(shots as u64)
            .ok_or_else(|| limit("written shot count overflow"))?;
        self.next_block_index = self
            .next_block_index
            .checked_add(1)
            .ok_or_else(|| limit("block count overflow"))?;
        self.buffered_shots = 0;
        self.archive_bytes = next_archive_bytes;
        self.decompressed_archive_bytes = next_decompressed_archive_bytes;
        self.compressed_archive_bytes = next_compressed_archive_bytes;
        Ok(())
    }
}

fn validate_transform_and_archive_shape(
    transform: &MeasurementTransform,
    total_shots: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if total_shots > limits.max_total_shots {
        return Err(limit("archive total shots exceed limit"));
    }
    let minimum_archive_bytes =
        checked_archive_byte_total(GLOBAL_HEADER_LEN as u64, ARCHIVE_TRAILER_LEN as u64)?;
    if minimum_archive_bytes > limits.max_archive_bytes {
        return Err(limit("archive bytes exceed limit"));
    }
    reject_limit_when(
        limits.transform.max_shots_per_block == 0,
        "archive block-shot limit is zero",
    )?;
    let expected_blocks = expected_block_count(total_shots, limits.transform.max_shots_per_block)?;
    if expected_blocks > limits.max_block_count {
        return Err(limit("archive block count exceeds limit"));
    }
    let validation_shots = total_shots.min(limits.transform.max_shots_per_block);
    let validation_shots =
        usize::try_from(validation_shots).map_err(|_| limit("archive shot count exceeds usize"))?;
    transform
        .validate_actual_usage(limits.transform, Some(validation_shots))
        .map_err(map_transform_error)?;
    let rank = transform.rank() as u64;
    if rank > limits.max_detector_rank {
        return Err(limit("detector rank exceeds archive limit"));
    }
    let free = transform
        .num_measurements()
        .checked_sub(transform.rank())
        .ok_or_else(|| shape("rank exceeds measurement count"))? as u64;
    if free > limits.max_free_measurements {
        return Err(limit("free measurement width exceeds archive limit"));
    }
    Ok(())
}

fn max_block_shots_usize(limits: ArchiveLimits) -> Result<usize, SampleArchiveError> {
    if limits.transform.max_shots_per_block == 0 {
        return Err(limit("archive block-shot limit is zero"));
    }
    usize::try_from(limits.transform.max_shots_per_block)
        .map_err(|_| limit("archive block-shot limit exceeds usize"))
}

fn copy_measurement_columns(
    source: &BitTable,
    source_offset: usize,
    target: &mut BitTable,
    target_offset: usize,
    shots: usize,
) {
    debug_assert_eq!(source.num_major(), target.num_major());
    for row in 0..source.num_major() {
        for shot in 0..shots {
            target.set(
                row,
                target_offset + shot,
                source.get(row, source_offset + shot),
            );
        }
    }
}

fn validate_decompressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_decompressed_bytes_per_frame
        || free_len > limits.max_decompressed_bytes_per_frame
    {
        return Err(limit("decompressed frame exceeds limit"));
    }
    let total = syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("decompressed archive bytes overflow"))?;
    reject_limit_when(
        total > limits.max_decompressed_bytes_per_archive,
        DECOMP_LIMIT,
    )?;
    Ok(())
}

fn validate_compressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_compressed_bytes_per_frame
        || free_len > limits.max_compressed_bytes_per_frame
    {
        return Err(limit("compressed frame exceeds limit"));
    }
    let total = syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("compressed archive bytes overflow"))?;
    reject_limit_when(total > limits.max_compressed_bytes_per_archive, COMP_LIMIT)?;
    Ok(())
}

pub(crate) fn map_transform_error(err: MeasurementTransformError) -> SampleArchiveError {
    match err {
        MeasurementTransformError::UnsupportedSweep => SampleArchiveError::with_code(
            SampleArchiveErrorCode::UnsupportedSweep,
            "sweep-bit circuits are not supported",
        ),
        MeasurementTransformError::LimitExceeded { .. }
        | MeasurementTransformError::Allocation(_) => limit("measurement transform limit exceeded"),
        MeasurementTransformError::ShapeMismatch { .. } => {
            shape("measurement transform shape mismatch")
        }
        MeasurementTransformError::InvalidRecordTarget { .. }
        | MeasurementTransformError::Reference { .. } => SampleArchiveError::with_code(
            SampleArchiveErrorCode::MalformedArchive,
            "measurement transform construction failed",
        ),
    }
}

fn write_and_hash(
    output: &mut impl Write,
    hasher: &mut Sha256,
    bytes: &[u8],
) -> Result<(), SampleArchiveError> {
    output.write_all(bytes).map_err(map_io)?;
    hasher.update(bytes);
    Ok(())
}

fn map_io(_err: std::io::Error) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::Io, "archive I/O failed")
}

pub(crate) fn shape(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::ShapeMismatch, detail)
}

pub(crate) fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}

fn reject_limit_when(rejected: bool, detail: &'static str) -> Result<(), SampleArchiveError> {
    (!rejected).then_some(()).ok_or_else(|| limit(detail))
}

pub(crate) fn checked_archive_byte_total(
    current: u64,
    added: u64,
) -> Result<u64, SampleArchiveError> {
    current
        .checked_add(added)
        .ok_or_else(|| limit("archive byte count overflow"))
}

pub(crate) fn checked_block_archive_byte_delta(
    block_compressed_bytes: u64,
) -> Result<u64, SampleArchiveError> {
    block_compressed_bytes
        .checked_add(BLOCK_HEADER_LEN as u64)
        .ok_or_else(|| limit("archive byte count overflow"))
}

fn expected_block_count(
    total_shots: u64,
    max_shots_per_block: u64,
) -> Result<u64, SampleArchiveError> {
    if total_shots == 0 {
        return Ok(0);
    }
    total_shots
        .checked_add(max_shots_per_block - 1)
        .ok_or_else(|| limit("archive block count overflow"))
        .map(|shots| shots / max_shots_per_block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measurement_transform::MeasurementTransformLimits;
    use crate::parser::parse_lines;
    use std::io;

    fn parse_transform(text: &str) -> MeasurementTransform {
        let circuit = parse_lines(text).expect("parse test circuit");
        MeasurementTransform::from_circuit(&circuit).expect("build test transform")
    }

    fn test_limits() -> ArchiveLimits {
        ArchiveLimits {
            transform: MeasurementTransformLimits {
                max_measurements: 64,
                max_detectors: 64,
                max_observables: 16,
                max_repeat_depth: 8,
                max_expanded_instructions: 1_000,
                max_parity_terms: 1_000,
                max_shots_per_block: 16,
                max_transform_working_bytes: 1 << 20,
                max_block_working_bytes: 1 << 20,
            },
            max_archive_bytes: 1 << 22,
            max_block_count: 16,
            max_total_shots: 16,
            max_detector_rank: 64,
            max_free_measurements: 64,
            max_compressed_bytes_per_frame: 1 << 20,
            max_decompressed_bytes_per_frame: 1 << 20,
            max_compressed_bytes_per_archive: 1 << 21,
            max_decompressed_bytes_per_archive: 1 << 21,
            max_zstd_window_bytes: 1 << 20,
            max_zstd_decoder_memory_bytes: 1 << 21,
        }
    }

    fn patterned_table(rows: usize, shots: usize) -> BitTable {
        let mut table = BitTable::try_new(rows, shots).expect("table allocates");
        for row in 0..rows {
            for shot in 0..shots {
                if (row + shot) % 2 == 1 {
                    table.set(row, shot, true);
                }
            }
        }
        table
    }

    fn expect_new_error(
        transform: MeasurementTransform,
        total_shots: u64,
        limits: ArchiveLimits,
    ) -> SampleArchiveError {
        match SampleArchiveWriter::new(
            Vec::new(),
            transform,
            total_shots,
            SampleArchiveOptions::default(),
            limits,
        ) {
            Ok(_) => panic!("expected writer construction error"),
            Err(err) => err,
        }
    }

    fn expect_write_or_finish_error(
        circuit: &str,
        shots: usize,
        limits: ArchiveLimits,
    ) -> SampleArchiveError {
        let transform = parse_transform(circuit);
        let measurements = patterned_table(transform.num_measurements(), shots);
        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            transform,
            shots as u64,
            SampleArchiveOptions::default(),
            limits,
        )
        .expect("writer constructs");
        match writer.write_measurements(&measurements) {
            Ok(()) => writer.finish().unwrap_err(),
            Err(err) => err,
        }
    }

    #[test]
    fn writer_state_machine_rejects_zero_and_second_blocks() {
        let transform = parse_transform("M 0\n");
        let empty = patterned_table(1, 0);
        let measurements = patterned_table(1, 1);
        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            transform.clone(),
            1,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("one-shot writer constructs");
        assert_eq!(
            writer.write_measurements(&empty).unwrap_err().code(),
            SampleArchiveErrorCode::ShapeMismatch
        );

        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            transform.clone(),
            0,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("zero-shot writer constructs");
        assert_eq!(
            writer.write_measurements(&measurements).unwrap_err().code(),
            SampleArchiveErrorCode::ShapeMismatch
        );

        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            transform,
            1,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("one-shot writer constructs");
        writer
            .write_measurements(&measurements)
            .expect("first block writes");
        assert_eq!(
            writer.write_measurements(&measurements).unwrap_err().code(),
            SampleArchiveErrorCode::ShapeMismatch
        );
    }

    #[test]
    fn writer_private_streaming_helpers_cover_limit_edges() {
        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            parse_transform("M 0\n"),
            0,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("writer constructs");
        assert_eq!(
            writer.emit_buffered_block(0).unwrap_err().code(),
            SampleArchiveErrorCode::ShapeMismatch
        );
        assert_eq!(
            writer.emit_buffered_block(17).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            parse_transform("M 0\n"),
            0,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("writer constructs");
        writer.limits.max_block_count = 0;
        assert_eq!(
            writer.emit_buffered_block(1).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            parse_transform("M 0\n"),
            0,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("writer constructs");
        writer.limits.max_archive_bytes = 0;
        assert_eq!(
            writer.finish().unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.transform.max_shots_per_block = 0;
        assert_eq!(
            max_block_shots_usize(limits).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        assert_eq!(
            expected_block_count(u64::MAX, 2).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_frame = 1;
        assert_eq!(
            validate_decompressed_streams(0, 2, limits)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_archive = 1;
        assert_eq!(
            validate_decompressed_streams(1, 1, limits)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_frame = 1;
        assert_eq!(
            validate_compressed_streams(0, 2, limits)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_archive = 1;
        assert_eq!(
            validate_compressed_streams(1, 1, limits)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        assert_eq!(
            checked_block_archive_byte_delta(u64::MAX)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );
    }

    #[test]
    fn writer_construction_enforces_archive_shape_limits() {
        let mut limits = test_limits();
        limits.max_total_shots = 0;
        assert_eq!(
            expect_new_error(parse_transform("M 0\n"), 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_detector_rank = 0;
        assert_eq!(
            expect_new_error(parse_transform("M 0\nDETECTOR rec[-1]\n"), 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_archive_bytes = (GLOBAL_HEADER_LEN + ARCHIVE_TRAILER_LEN - 1) as u64;
        assert_eq!(
            expect_new_error(parse_transform("M 0\n"), 0, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.transform.max_shots_per_block = 1;
        limits.max_block_count = 1;
        assert_eq!(
            expect_new_error(parse_transform("M 0\n"), 2, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.transform.max_shots_per_block = 0;
        assert_eq!(
            expect_new_error(parse_transform("M 0\n"), 0, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_free_measurements = 0;
        assert_eq!(
            expect_new_error(parse_transform("M 0 1\nDETECTOR rec[-2]\n"), 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );
    }

    #[test]
    fn writer_stream_limits_cover_decompressed_and_compressed_accounting() {
        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_frame = 1;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 9, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_archive = 1;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 9, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_frame = 1;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_archive = 1;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.transform.max_shots_per_block = 1;
        limits.max_decompressed_bytes_per_archive = 1;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 2, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.transform.max_shots_per_block = 1;
        limits.max_compressed_bytes_per_archive = 20;
        assert_eq!(
            expect_write_or_finish_error("M 0\n", 2, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );
    }

    #[test]
    fn writer_maps_io_and_transform_errors() {
        let transform = parse_transform("M 0\n");
        let err = match SampleArchiveWriter::new(
            FailingWriter,
            transform,
            0,
            SampleArchiveOptions::default(),
            test_limits(),
        ) {
            Ok(_) => panic!("expected writer I/O error"),
            Err(err) => err,
        };
        assert_eq!(err.code(), SampleArchiveErrorCode::Io);
        let mut failing = FailingWriter;
        assert!(failing.flush().is_err());

        assert_eq!(
            map_transform_error(MeasurementTransformError::UnsupportedSweep).code(),
            SampleArchiveErrorCode::UnsupportedSweep
        );
        assert_eq!(
            map_transform_error(MeasurementTransformError::ShapeMismatch {
                detail: "shape".to_string(),
            })
            .code(),
            SampleArchiveErrorCode::ShapeMismatch
        );
        assert_eq!(
            map_transform_error(MeasurementTransformError::InvalidRecordTarget {
                detail: "record".to_string(),
            })
            .code(),
            SampleArchiveErrorCode::MalformedArchive
        );
        assert_eq!(
            map_transform_error(MeasurementTransformError::Reference {
                detail: "reference".to_string(),
            })
            .code(),
            SampleArchiveErrorCode::MalformedArchive
        );
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("write failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }
}
