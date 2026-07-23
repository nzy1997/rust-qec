use crate::measurement_transform::{MeasurementTransform, MeasurementTransformError};
use crate::sample_archive::dense::pack_dense;
use crate::sample_archive::format::{
    BlockHeader, CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1, CODEC_SUITE_ZSTD_FRAMES_V1,
    FINGERPRINT_SHA256_CANONICAL_CIRCUIT, GlobalHeader, REFERENCE_SIMULATE_NOISELESS,
    STREAM_CODEC_EMPTY, STREAM_CODEC_FREE_DENSE_V1, SampleArchiveError, SampleArchiveErrorCode,
    TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1, checked_dense_bit_bytes,
};
use crate::sample_archive::integrity::{finalize_header, finalize_trailer};
use crate::sample_archive::limits::{ArchiveLimits, SampleArchiveOptions};
use crate::sample_archive::syndrome::{encode_syndrome, update_dense_syndrome_hash};
use crate::sample_archive::zstd_frame::compress_frame;
use crate::sim::bit_table::BitTable;
use sha2::{Digest, Sha256};
use std::io::Write;

pub struct SampleArchiveWriter<W: Write> {
    output: W,
    transform: MeasurementTransform,
    total_shots: u64,
    options: SampleArchiveOptions,
    limits: ArchiveLimits,
    archive_hasher: Sha256,
    wrote_block: bool,
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
            wrote_block: false,
        })
    }

    pub fn write_measurements(
        &mut self,
        measurements: &BitTable,
    ) -> Result<(), SampleArchiveError> {
        if self.total_shots == 0 {
            return Err(shape("zero-shot archive cannot contain a block"));
        }
        if self.wrote_block {
            return Err(shape("positive-shot archive already has its one block"));
        }
        if measurements.num_major() != self.transform.num_measurements()
            || measurements.num_minor() as u64 != self.total_shots
        {
            return Err(shape(
                "measurement table shape does not match archive header",
            ));
        }
        self.transform
            .validate_actual_usage(self.limits.transform, Some(measurements.num_minor()))
            .map_err(map_transform_error)?;

        let encoded = self
            .transform
            .encode_block(measurements)
            .map_err(map_transform_error)?;
        let free_len = checked_dense_bit_bytes(
            encoded.free_measurements.num_major() as u64,
            measurements.num_minor() as u64,
        )?;
        let syndrome = encode_syndrome(&encoded.selected_detectors)?;
        validate_decompressed_streams(syndrome.raw_len, free_len, self.limits)?;
        let free = pack_dense(&encoded.free_measurements)?;
        let mut logical_hasher = Sha256::new();
        update_dense_syndrome_hash(&encoded.selected_detectors, &mut logical_hasher)?;
        logical_hasher.update(&free);
        let logical_payload_sha256: [u8; 32] = logical_hasher.finalize().into();

        let syndrome_frame = if syndrome.raw.is_empty() {
            Vec::new()
        } else {
            compress_frame(&syndrome.raw, self.options.compression_level)?
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

        let block_header = BlockHeader {
            block_index: 0,
            first_shot: 0,
            shot_count: self.total_shots,
            syndrome_codec_id: syndrome.codec_id,
            free_codec_id: if free.is_empty() {
                STREAM_CODEC_EMPTY
            } else {
                STREAM_CODEC_FREE_DENSE_V1
            },
            syndrome_uncompressed_len: syndrome.raw_len,
            syndrome_compressed_len: syndrome_frame.len() as u64,
            free_uncompressed_len: free.len() as u64,
            free_compressed_len: free_frame.len() as u64,
            logical_payload_sha256,
        };
        let block_bytes = block_header.to_bytes()?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &block_bytes)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &syndrome_frame)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &free_frame)?;
        self.wrote_block = true;
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, SampleArchiveError> {
        if self.total_shots > 0 && !self.wrote_block {
            return Err(shape("positive-shot archive is missing its block"));
        }
        let block_count = u64::from(self.wrote_block);
        let trailer = finalize_trailer(block_count, self.total_shots, self.archive_hasher.clone())?;
        self.output.write_all(&trailer).map_err(map_io)?;
        self.output.flush().map_err(map_io)?;
        Ok(self.output)
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
    if total_shots > usize::MAX as u64 {
        return Err(limit("archive shot count exceeds usize"));
    }
    transform
        .validate_actual_usage(limits.transform, Some(total_shots as usize))
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

fn validate_decompressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_decompressed_bytes_per_stream
        || free_len > limits.max_decompressed_bytes_per_stream
    {
        return Err(limit("decompressed stream exceeds limit"));
    }
    if syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("decompressed archive bytes overflow"))?
        > limits.max_decompressed_bytes_per_archive
    {
        return Err(limit("decompressed archive bytes exceed limit"));
    }
    Ok(())
}

fn validate_compressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_compressed_bytes_per_stream
        || free_len > limits.max_compressed_bytes_per_stream
    {
        return Err(limit("compressed stream exceeds limit"));
    }
    if syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("compressed archive bytes overflow"))?
        > limits.max_compressed_bytes_per_archive
    {
        return Err(limit("compressed archive bytes exceed limit"));
    }
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
            max_total_shots: 16,
            max_detector_rank: 64,
            max_free_measurements: 64,
            max_compressed_bytes_per_stream: 1 << 20,
            max_decompressed_bytes_per_stream: 1 << 20,
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

    fn expect_write_error(
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
        writer.write_measurements(&measurements).unwrap_err()
    }

    #[test]
    fn writer_state_machine_rejects_zero_and_second_blocks() {
        let transform = parse_transform("M 0\n");
        let measurements = patterned_table(1, 1);
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
        limits.max_free_measurements = 0;
        assert_eq!(
            expect_new_error(parse_transform("M 0 1\nDETECTOR rec[-2]\n"), 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );
    }

    #[test]
    fn writer_stream_limits_cover_decompressed_and_compressed_accounting() {
        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_stream = 1;
        assert_eq!(
            expect_write_error("M 0\n", 9, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_archive = 1;
        assert_eq!(
            expect_write_error("M 0\n", 9, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_stream = 1;
        assert_eq!(
            expect_write_error("M 0\n", 1, limits).code(),
            SampleArchiveErrorCode::LimitExceeded
        );

        let mut limits = test_limits();
        limits.max_compressed_bytes_per_archive = 1;
        assert_eq!(
            expect_write_error("M 0\n", 1, limits).code(),
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
