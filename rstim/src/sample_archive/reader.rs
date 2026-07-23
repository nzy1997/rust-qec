use crate::ir::StimInstr;
use crate::measurement_transform::{
    DecodedSampleBlock, EncodedMeasurementBlock, MeasurementTransform,
};
use crate::sample_archive::dense::unpack_dense;
use crate::sample_archive::format::{
    ARCHIVE_TRAILER_LEN, ArchiveTrailer, BLOCK_HEADER_LEN, BLOCK_MAGIC, BlockHeader,
    CODEC_SUITE_ZSTD_FRAMES_V1, GLOBAL_HEADER_LEN, GlobalHeader, STREAM_CODEC_EMPTY,
    STREAM_CODEC_FREE_DENSE_V1, STREAM_CODEC_SYNDROME_DENSE_V1,
    STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1, SampleArchiveError, SampleArchiveErrorCode,
    TRAILER_MAGIC, checked_dense_bit_bytes,
};
use crate::sample_archive::integrity::{header_digest, trailer_prefix};
use crate::sample_archive::limits::ArchiveLimits;
use crate::sample_archive::syndrome::{decode_syndrome_raw, update_dense_syndrome_hash};
use crate::sample_archive::telemetry::{
    bit_table_bytes, checked_sum, record_reader_decoded_blocks, record_reader_live_bytes,
    record_transform_payloads, record_transform_retained,
};
use crate::sample_archive::writer::{
    checked_archive_byte_total, limit, map_transform_error, shape,
};
use crate::sample_archive::zstd_frame::decompress_frame;
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveSummary {
    pub block_count: u64,
    pub total_shots: u64,
    pub measurement_count: u64,
    pub detector_count: u64,
    pub observable_count: u64,
}

pub struct SampleArchiveReader<R: Read> {
    input: R,
    header: GlobalHeader,
    transform: MeasurementTransform,
    limits: ArchiveLimits,
    archive_hasher: Sha256,
    next_block_index: u64,
    next_first_shot: u64,
    archive_bytes: u64,
    decompressed_archive_bytes: u64,
    compressed_archive_bytes: u64,
    trailer: Option<ArchiveTrailer>,
}

impl<R: Read> SampleArchiveReader<R> {
    pub fn open(
        mut input: R,
        circuit: &[StimInstr],
        limits: ArchiveLimits,
    ) -> Result<Self, SampleArchiveError> {
        let mut header_bytes = [0u8; GLOBAL_HEADER_LEN];
        read_exact_or_truncated(&mut input, &mut header_bytes)?;
        let header = GlobalHeader::from_bytes_before_checksum(&header_bytes)?;
        if header.header_sha256 != header_digest(&header_bytes) {
            return Err(checksum("global header digest mismatch"));
        }
        header.validate_after_checksum()?;
        validate_header_limits(&header, limits)?;
        let transform = MeasurementTransform::from_circuit_with_limits(circuit, limits.transform)
            .map_err(map_transform_error)?;
        compare_identity(&header, &transform)?;
        record_transform_retained(transform.transform_working_bytes());
        let mut archive_hasher = Sha256::new();
        archive_hasher.update(header_bytes);
        Ok(Self {
            input,
            header,
            transform,
            limits,
            archive_hasher,
            next_block_index: 0,
            next_first_shot: 0,
            archive_bytes: GLOBAL_HEADER_LEN as u64,
            decompressed_archive_bytes: 0,
            compressed_archive_bytes: 0,
            trailer: None,
        })
    }

    pub fn total_shots(&self) -> u64 {
        self.header.total_shots
    }

    pub fn next_block(&mut self) -> Result<Option<DecodedSampleBlock>, SampleArchiveError> {
        if self.trailer.is_some() {
            return Ok(None);
        }
        if self.next_first_shot == self.header.total_shots {
            self.read_trailer()?;
            return Ok(None);
        }
        let mut magic = [0u8; 8];
        read_exact_or_truncated(&mut self.input, &mut magic)?;
        if magic == *TRAILER_MAGIC {
            self.read_trailer_after_magic(magic)?;
            return Ok(None);
        }
        if magic != *BLOCK_MAGIC {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::BadMagic,
                "invalid block magic",
            ));
        }
        let mut block_bytes = [0u8; BLOCK_HEADER_LEN];
        block_bytes[..8].copy_from_slice(&magic);
        read_exact_or_truncated(&mut self.input, &mut block_bytes[8..])?;
        self.archive_hasher.update(block_bytes);
        let block = BlockHeader::from_bytes(&block_bytes)?;
        self.validate_block_header(&block)?;
        let next_archive_bytes = self.validate_archive_stream_totals(&block)?;

        let syndrome_frame = self.read_stream(block.syndrome_compressed_len)?;
        let free_frame = self.read_stream(block.free_compressed_len)?;
        let syndrome = decode_stream(
            &syndrome_frame,
            block.syndrome_uncompressed_len,
            self.limits,
        )?;
        let free = decode_stream(&free_frame, block.free_uncompressed_len, self.limits)?;
        let shot_count =
            usize::try_from(block.shot_count).map_err(|_| limit("block shot count too large"))?;
        let expected_free_len =
            checked_dense_bit_bytes(self.transform.free_columns().len() as u64, block.shot_count)?;
        if block.free_uncompressed_len != expected_free_len {
            return Err(shape("block free stream shape does not match transform"));
        }

        let selected = decode_syndrome_raw(
            block.syndrome_codec_id,
            block.syndrome_uncompressed_len,
            &syndrome,
            self.transform.rank(),
            shot_count,
        )?;
        let free_measurements =
            unpack_dense(&free, self.transform.free_columns().len(), shot_count)?;
        let mut logical_hasher = Sha256::new();
        update_dense_syndrome_hash(&selected, &mut logical_hasher)?;
        logical_hasher.update(&free);
        let digest: [u8; 32] = logical_hasher.finalize().into();
        if digest != block.logical_payload_sha256 {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::LogicalDigestMismatch,
                "logical payload digest mismatch",
            ));
        }
        let decoded = self
            .transform
            .decode_block(&EncodedMeasurementBlock {
                selected_detectors: selected,
                free_measurements,
            })
            .map_err(map_transform_error)?;
        record_transform_payloads(2);
        record_reader_decoded_blocks(1);
        let rank = self.transform.rank() as u64;
        let free_columns = self.transform.free_columns().len() as u64;
        let measurement_rows = decoded.measurements.num_major() as u64;
        let detection_rows = decoded.detections.num_major() as u64;
        let observable_rows = decoded.observable_flips.num_major() as u64;
        let selected_bytes = bit_table_bytes("reader.selected", rank, block.shot_count)?;
        let free_table_bytes =
            bit_table_bytes("reader.free_measurements", free_columns, block.shot_count)?;
        let decoded_measurements = bit_table_bytes(
            "reader.decoded_measurements",
            measurement_rows,
            block.shot_count,
        )?;
        let decoded_detections = bit_table_bytes(
            "reader.decoded_detections",
            detection_rows,
            block.shot_count,
        )?;
        let decoded_observables = bit_table_bytes(
            "reader.decoded_observables",
            observable_rows,
            block.shot_count,
        )?;
        let transform_scratch_x = bit_table_bytes(
            "reader.transform_scratch_x",
            measurement_rows,
            block.shot_count,
        )?;
        let transform_scratch_rhs =
            bit_table_bytes("reader.transform_scratch_rhs", rank, block.shot_count)?;
        let transform_scratch_row =
            bit_table_bytes("reader.transform_scratch_row", 1, block.shot_count)?;
        let transform_scratch_parts = [
            ("x", transform_scratch_x),
            ("rhs", transform_scratch_rhs),
            ("row", transform_scratch_row),
        ];
        let transform_scratch = checked_sum("reader.transform_scratch", &transform_scratch_parts)?;
        let raw_parts = [
            ("syndrome_raw", syndrome.len() as u64),
            ("free_raw", free.len() as u64),
        ];
        let raw_bytes = checked_sum("reader.raw_codec_buffers", &raw_parts)?;
        let compressed_parts = [
            ("syndrome_frame", syndrome_frame.len() as u64),
            ("free_frame", free_frame.len() as u64),
        ];
        let compressed_bytes = checked_sum("reader.compressed_frames", &compressed_parts)?;
        let reader_live_parts = [
            ("selected", selected_bytes),
            ("free_measurements", free_table_bytes),
            ("decoded_measurements", decoded_measurements),
            ("decoded_detections", decoded_detections),
            ("decoded_observables", decoded_observables),
            ("transform_scratch", transform_scratch),
            ("raw_codec_buffers", raw_bytes),
            ("compressed_frames", compressed_bytes),
            ("zstd_state", self.limits.max_zstd_window_bytes),
        ];
        record_reader_live_bytes(&reader_live_parts)?;
        self.next_first_shot = self
            .next_first_shot
            .checked_add(block.shot_count)
            .ok_or_else(|| limit("block first-shot sum overflow"))?;
        self.next_block_index = self
            .next_block_index
            .checked_add(1)
            .ok_or_else(|| limit("block count overflow"))?;
        self.archive_bytes = next_archive_bytes;
        Ok(Some(decoded))
    }

    pub fn finish(mut self) -> Result<ArchiveSummary, SampleArchiveError> {
        while self.trailer.is_none() {
            if self.next_block()?.is_none() {
                break;
            }
        }
        let trailer = self
            .trailer
            .as_ref()
            .expect("finish loop reads archive trailer");
        if trailer.block_count > self.limits.max_block_count {
            return Err(limit("archive block count exceeds limit"));
        }
        if self.archive_bytes > self.limits.max_archive_bytes {
            return Err(limit("archive bytes exceed limit"));
        }
        if trailer.block_count != self.next_block_index {
            return Err(shape("trailer block count does not match decoded blocks"));
        }
        if trailer.total_shots != self.header.total_shots
            || trailer.total_shots != self.next_first_shot
        {
            return Err(shape("trailer shot count does not match decoded blocks"));
        }
        let prefix = trailer_prefix(trailer.block_count, trailer.total_shots)?;
        self.archive_hasher.update(prefix);
        let digest: [u8; 32] = self.archive_hasher.finalize().into();
        if digest != trailer.archive_sha256 {
            return Err(checksum("archive digest mismatch"));
        }
        let mut extra = [0u8; 1];
        match self.input.read(&mut extra) {
            Ok(0) => Ok(ArchiveSummary {
                block_count: trailer.block_count,
                total_shots: trailer.total_shots,
                measurement_count: self.header.measurement_count,
                detector_count: self.header.detector_count,
                observable_count: self.header.observable_count,
            }),
            Ok(_) => Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::TrailingData,
                "archive has trailing data",
            )),
            Err(_) => Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::Io,
                "archive I/O failed",
            )),
        }
    }

    fn read_trailer(&mut self) -> Result<(), SampleArchiveError> {
        self.validate_trailer_byte_limit()?;
        let mut trailer_bytes = [0u8; ARCHIVE_TRAILER_LEN];
        read_exact_or_truncated(&mut self.input, &mut trailer_bytes)?;
        if trailer_bytes[0..8] == BLOCK_MAGIC[..] {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "data block appears after declared total shots",
            ));
        }
        if trailer_bytes[0..8] != TRAILER_MAGIC[..] {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::BadMagic,
                "invalid trailer magic",
            ));
        }
        let trailer = ArchiveTrailer::from_bytes(&trailer_bytes)?;
        self.archive_bytes =
            checked_archive_byte_total(self.archive_bytes, ARCHIVE_TRAILER_LEN as u64)?;
        self.trailer = Some(trailer);
        Ok(())
    }

    fn read_trailer_after_magic(&mut self, magic: [u8; 8]) -> Result<(), SampleArchiveError> {
        self.validate_trailer_byte_limit()?;
        let mut trailer_bytes = [0u8; ARCHIVE_TRAILER_LEN];
        trailer_bytes[..8].copy_from_slice(&magic);
        read_exact_or_truncated(&mut self.input, &mut trailer_bytes[8..])?;
        let trailer = ArchiveTrailer::from_bytes(&trailer_bytes)?;
        self.archive_bytes =
            checked_archive_byte_total(self.archive_bytes, ARCHIVE_TRAILER_LEN as u64)?;
        self.trailer = Some(trailer);
        Ok(())
    }

    fn validate_block_header(&self, block: &BlockHeader) -> Result<(), SampleArchiveError> {
        if block.block_index != self.next_block_index {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "invalid block sequence number",
            ));
        }
        if block.first_shot != self.next_first_shot {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "invalid block first-shot index",
            ));
        }
        if block.shot_count > self.header.max_shots_per_block
            || block.shot_count > self.limits.transform.max_shots_per_block
        {
            return Err(limit("block shot count exceeds limit"));
        }
        if self.next_block_index >= self.limits.max_block_count {
            return Err(limit("archive block count exceeds limit"));
        }
        let end = block
            .first_shot
            .checked_add(block.shot_count)
            .ok_or_else(|| limit("block first-shot sum overflow"))?;
        if end > self.header.total_shots {
            return Err(shape("block shot count exceeds archive total"));
        }
        validate_stream_lengths(
            block.syndrome_uncompressed_len,
            block.syndrome_compressed_len,
            self.limits,
        )?;
        validate_stream_lengths(
            block.free_uncompressed_len,
            block.free_compressed_len,
            self.limits,
        )?;
        if block
            .syndrome_uncompressed_len
            .checked_add(block.free_uncompressed_len)
            .ok_or_else(|| limit("decompressed archive bytes overflow"))?
            > self.limits.max_decompressed_bytes_per_archive
        {
            return Err(limit("decompressed archive bytes exceed limit"));
        }
        if block
            .syndrome_compressed_len
            .checked_add(block.free_compressed_len)
            .ok_or_else(|| limit("compressed archive bytes overflow"))?
            > self.limits.max_compressed_bytes_per_archive
        {
            return Err(limit("compressed archive bytes exceed limit"));
        }
        if block.syndrome_codec_id != STREAM_CODEC_EMPTY
            && block.syndrome_codec_id != STREAM_CODEC_SYNDROME_DENSE_V1
            && block.syndrome_codec_id != STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1
        {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "unknown syndrome codec",
            ));
        }
        if block.free_codec_id != STREAM_CODEC_EMPTY
            && block.free_codec_id != STREAM_CODEC_FREE_DENSE_V1
        {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "unknown free codec",
            ));
        }
        Ok(())
    }

    fn validate_archive_stream_totals(
        &mut self,
        block: &BlockHeader,
    ) -> Result<u64, SampleArchiveError> {
        let block_decompressed_bytes = block
            .syndrome_uncompressed_len
            .checked_add(block.free_uncompressed_len)
            .ok_or_else(|| limit("decompressed archive bytes overflow"))?;
        let next_decompressed_archive_bytes = self
            .decompressed_archive_bytes
            .checked_add(block_decompressed_bytes)
            .ok_or_else(|| limit("decompressed archive bytes overflow"))?;
        if next_decompressed_archive_bytes > self.limits.max_decompressed_bytes_per_archive {
            return Err(limit("decompressed archive bytes exceed limit"));
        }

        let block_compressed_bytes = block
            .syndrome_compressed_len
            .checked_add(block.free_compressed_len)
            .ok_or_else(|| limit("compressed archive bytes overflow"))?;
        let next_compressed_archive_bytes = self
            .compressed_archive_bytes
            .checked_add(block_compressed_bytes)
            .ok_or_else(|| limit("compressed archive bytes overflow"))?;
        if next_compressed_archive_bytes > self.limits.max_compressed_bytes_per_archive {
            return Err(limit("compressed archive bytes exceed limit"));
        }

        let next_archive_bytes = checked_archive_byte_total(
            self.archive_bytes,
            (BLOCK_HEADER_LEN as u64)
                .checked_add(block_compressed_bytes)
                .ok_or_else(|| limit("archive byte count overflow"))?,
        )?;
        let archive_with_trailer =
            checked_archive_byte_total(next_archive_bytes, ARCHIVE_TRAILER_LEN as u64)?;
        if archive_with_trailer > self.limits.max_archive_bytes {
            return Err(limit("archive bytes exceed limit"));
        }

        self.decompressed_archive_bytes = next_decompressed_archive_bytes;
        self.compressed_archive_bytes = next_compressed_archive_bytes;
        Ok(next_archive_bytes)
    }

    fn read_stream(&mut self, len: u64) -> Result<Vec<u8>, SampleArchiveError> {
        let len = usize::try_from(len).map_err(|_| limit("compressed frame too large"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| limit("compressed frame reservation failed"))?;
        bytes.resize(len, 0);
        read_exact_or_truncated(&mut self.input, &mut bytes)?;
        self.archive_hasher.update(&bytes);
        Ok(bytes)
    }

    fn validate_trailer_byte_limit(&self) -> Result<(), SampleArchiveError> {
        if checked_archive_byte_total(self.archive_bytes, ARCHIVE_TRAILER_LEN as u64)?
            > self.limits.max_archive_bytes
        {
            return Err(limit("archive bytes exceed limit"));
        }
        Ok(())
    }
}

fn validate_header_limits(
    header: &GlobalHeader,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if header.total_shots > limits.max_total_shots {
        return Err(limit("archive total shots exceed limit"));
    }
    if checked_archive_byte_total(GLOBAL_HEADER_LEN as u64, ARCHIVE_TRAILER_LEN as u64)?
        > limits.max_archive_bytes
    {
        return Err(limit("archive bytes exceed limit"));
    }
    if header.max_shots_per_block == 0
        || header.max_shots_per_block > limits.transform.max_shots_per_block
    {
        return Err(limit("header max shots per block exceeds limit"));
    }
    let minimum_blocks = minimum_block_count(header.total_shots, header.max_shots_per_block)?;
    if minimum_blocks > limits.max_block_count {
        return Err(limit("archive block count exceeds limit"));
    }
    if header.measurement_count > limits.transform.max_measurements
        || header.detector_count > limits.transform.max_detectors
        || header.observable_count > limits.transform.max_observables
    {
        return Err(limit("header dimensions exceed transform limits"));
    }
    if header.detector_rank > limits.max_detector_rank {
        return Err(limit("header detector rank exceeds limit"));
    }
    let free = header
        .measurement_count
        .checked_sub(header.detector_rank)
        .ok_or_else(|| shape("header rank exceeds measurement count"))?;
    if free > limits.max_free_measurements {
        return Err(limit("header free width exceeds limit"));
    }
    Ok(())
}

fn compare_identity(
    header: &GlobalHeader,
    transform: &MeasurementTransform,
) -> Result<(), SampleArchiveError> {
    let identity = transform.identity();
    if header.circuit_sha256 != identity.circuit_sha256 {
        return Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::CircuitMismatch,
            "archive circuit fingerprint does not match supplied circuit",
        ));
    }
    if header.measurement_count != identity.measurement_count
        || header.detector_count != identity.detector_count
        || header.observable_count != identity.observable_count
        || header.detector_rank != identity.detector_rank
        || header.canonicalization_id != identity.canonicalization_id
        || header.fingerprint_id != identity.fingerprint_id
        || header.transform_id != identity.transform_algorithm_id
        || header.reference_id != identity.reference_strategy_id
        || header.codec_suite_id != CODEC_SUITE_ZSTD_FRAMES_V1
    {
        return Err(shape(
            "archive transform identity does not match supplied circuit",
        ));
    }
    Ok(())
}

fn decode_stream(
    frame: &[u8],
    declared_len: u64,
    limits: ArchiveLimits,
) -> Result<Vec<u8>, SampleArchiveError> {
    if declared_len == 0 {
        if frame.is_empty() {
            Ok(Vec::new())
        } else {
            Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "noncanonical empty stream",
            ))
        }
    } else {
        decompress_frame(frame, declared_len, limits)
    }
}

fn validate_stream_lengths(
    uncompressed: u64,
    compressed: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if uncompressed > limits.max_decompressed_bytes_per_frame {
        return Err(limit("decompressed frame exceeds limit"));
    }
    if compressed > limits.max_compressed_bytes_per_frame {
        return Err(limit("compressed frame exceeds limit"));
    }
    Ok(())
}

fn minimum_block_count(
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

fn read_exact_or_truncated(
    input: &mut impl Read,
    bytes: &mut [u8],
) -> Result<(), SampleArchiveError> {
    input.read_exact(bytes).map_err(|err| {
        if err.kind() == ErrorKind::UnexpectedEof {
            SampleArchiveError::with_code(SampleArchiveErrorCode::Truncated, "truncated archive")
        } else {
            SampleArchiveError::with_code(SampleArchiveErrorCode::Io, "archive I/O failed")
        }
    })
}

fn checksum(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::ChecksumMismatch, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measurement_transform::MeasurementTransformLimits;
    use crate::parser::parse_lines;
    use crate::sample_archive::limits::SampleArchiveOptions;
    use crate::sample_archive::writer::SampleArchiveWriter;
    use crate::sim::bit_table::BitTable;
    use std::io;

    fn parse(text: &str) -> Vec<StimInstr> {
        parse_lines(text).expect("parse test circuit")
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

    fn archive_for(circuit: &[StimInstr], shots: usize) -> Vec<u8> {
        let transform = MeasurementTransform::from_circuit(circuit).expect("test transform");
        let mut measurements =
            BitTable::try_new(transform.num_measurements(), shots).expect("table allocates");
        for row in 0..measurements.num_major() {
            for shot in 0..measurements.num_minor() {
                if (row + shot) % 2 == 1 {
                    measurements.set(row, shot, true);
                }
            }
        }
        let mut writer = SampleArchiveWriter::new(
            Vec::new(),
            transform,
            shots as u64,
            SampleArchiveOptions::default(),
            test_limits(),
        )
        .expect("writer constructs");
        if shots > 0 {
            writer
                .write_measurements(&measurements)
                .expect("write measurements");
        }
        writer.finish().expect("finish writer")
    }

    fn header_from(archive: &[u8]) -> GlobalHeader {
        GlobalHeader::from_bytes(&archive[..GLOBAL_HEADER_LEN]).expect("parse header")
    }

    fn block_from(archive: &[u8]) -> BlockHeader {
        BlockHeader::from_bytes(&archive[GLOBAL_HEADER_LEN..GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN])
            .expect("parse block")
    }

    fn assert_code<T: std::fmt::Debug>(
        result: Result<T, SampleArchiveError>,
        code: SampleArchiveErrorCode,
    ) {
        assert_eq!(result.unwrap_err().code(), code);
    }

    #[test]
    fn header_limits_and_identity_checks_cover_rejections() {
        let circuit = parse("M 0 1\nDETECTOR rec[-2]\n");
        let archive = archive_for(&circuit, 5);
        let header = header_from(&archive);
        let transform = MeasurementTransform::from_circuit(&circuit).expect("test transform");

        let mut limits = test_limits();
        limits.max_total_shots = 4;
        assert_code(
            validate_header_limits(&header, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = header.clone();
        bad.max_shots_per_block = 0;
        assert_code(
            validate_header_limits(&bad, test_limits()),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = header.clone();
        bad.measurement_count = test_limits().transform.max_measurements + 1;
        assert_code(
            validate_header_limits(&bad, test_limits()),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = header.clone();
        bad.detector_rank = test_limits().max_detector_rank + 1;
        assert_code(
            validate_header_limits(&bad, test_limits()),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut limits = test_limits();
        limits.max_free_measurements = 0;
        assert_code(
            validate_header_limits(&header, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = header.clone();
        bad.max_shots_per_block = 4;
        validate_header_limits(&bad, test_limits())
            .expect("multi-block archive may have total shots above block size");
        bad.max_shots_per_block = test_limits().transform.max_shots_per_block + 1;
        assert_code(
            validate_header_limits(&bad, test_limits()),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = header;
        bad.canonicalization_id ^= 1;
        assert_code(
            compare_identity(&bad, &transform),
            SampleArchiveErrorCode::ShapeMismatch,
        );
    }

    #[test]
    fn block_header_validation_covers_stream_accounting_and_codecs() {
        let circuit = parse("M 0 1\nDETECTOR rec[-2]\n");
        let archive = archive_for(&circuit, 5);
        let mut reader =
            SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
                .expect("open reader");
        let block = block_from(&archive);

        let mut shot_limit_reader =
            SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
                .expect("open reader");
        shot_limit_reader.limits.transform.max_shots_per_block = 4;
        assert_code(
            shot_limit_reader.validate_block_header(&block),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = block.clone();
        bad.free_uncompressed_len = test_limits().max_decompressed_bytes_per_frame + 1;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = block.clone();
        bad.syndrome_uncompressed_len = 6;
        bad.free_uncompressed_len = 6;
        reader.limits.max_decompressed_bytes_per_archive = 10;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut bad = block.clone();
        bad.syndrome_compressed_len = 6;
        bad.free_compressed_len = 6;
        reader.limits.max_compressed_bytes_per_archive = 10;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let block_decompressed_bytes = block
            .syndrome_uncompressed_len
            .checked_add(block.free_uncompressed_len)
            .expect("test decompressed byte sum");
        assert!(block_decompressed_bytes > 0);
        let mut cumulative_reader =
            SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
                .expect("open reader");
        cumulative_reader.limits.max_decompressed_bytes_per_archive =
            block_decompressed_bytes * 2 - 1;
        cumulative_reader
            .validate_archive_stream_totals(&block)
            .expect("first block within decompressed archive limit");
        assert_code(
            cumulative_reader.validate_archive_stream_totals(&block),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let block_compressed_bytes = block
            .syndrome_compressed_len
            .checked_add(block.free_compressed_len)
            .expect("test compressed byte sum");
        assert!(block_compressed_bytes > 0);
        let mut cumulative_reader =
            SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
                .expect("open reader");
        cumulative_reader.limits.max_compressed_bytes_per_archive = block_compressed_bytes * 2 - 1;
        cumulative_reader
            .validate_archive_stream_totals(&block)
            .expect("first block within compressed archive limit");
        assert_code(
            cumulative_reader.validate_archive_stream_totals(&block),
            SampleArchiveErrorCode::LimitExceeded,
        );

        reader.limits = test_limits();
        let mut bad = block.clone();
        bad.shot_count = 6;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::ShapeMismatch,
        );

        let mut bad = block.clone();
        bad.syndrome_codec_id = 999;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::MalformedArchive,
        );

        let mut bad = block;
        bad.free_codec_id = 999;
        assert_code(
            reader.validate_block_header(&bad),
            SampleArchiveErrorCode::MalformedArchive,
        );
    }

    #[test]
    fn decode_stream_and_read_helpers_cover_empty_and_io_errors() {
        assert_code(
            decode_stream(&[0], 0, test_limits()),
            SampleArchiveErrorCode::MalformedArchive,
        );

        let mut limits = test_limits();
        limits.max_decompressed_bytes_per_frame = 1;
        assert_code(
            validate_stream_lengths(2, 0, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut byte = [0u8; 1];
        assert_code(
            read_exact_or_truncated(&mut AlwaysErr, &mut byte),
            SampleArchiveErrorCode::Io,
        );
    }

    #[test]
    fn next_block_rejects_free_stream_shape_after_successful_decode() {
        let circuit = parse("M 0 1\nDETECTOR rec[-2]\n");
        let archive = archive_for(&circuit, 5);
        let mut block = block_from(&archive);
        let new_free = crate::sample_archive::zstd_frame::compress_frame(&[0, 0], 0)
            .expect("compress replacement free stream");
        block.free_uncompressed_len = 2;
        block.free_compressed_len = new_free.len() as u64;
        let block_bytes = block.to_bytes().expect("serialize block");

        let syndrome_start = GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN;
        let free_start = syndrome_start + block.syndrome_compressed_len as usize;
        let old_free_end = free_start + block_from(&archive).free_compressed_len as usize;
        let mut bad = Vec::new();
        bad.extend_from_slice(&archive[..GLOBAL_HEADER_LEN]);
        bad.extend_from_slice(&block_bytes);
        bad.extend_from_slice(&archive[syndrome_start..free_start]);
        bad.extend_from_slice(&new_free);
        bad.extend_from_slice(&archive[old_free_end..]);

        let mut reader = SampleArchiveReader::open(io::Cursor::new(bad), &circuit, test_limits())
            .expect("open reader");
        assert_code(reader.next_block(), SampleArchiveErrorCode::ShapeMismatch);
    }

    #[test]
    fn next_block_covers_post_trailer_and_bad_magic_edges() {
        let zero_circuit = parse("M 0\n");
        let archive = archive_for(&zero_circuit, 0);
        let mut reader =
            SampleArchiveReader::open(io::Cursor::new(&archive), &zero_circuit, test_limits())
                .expect("open zero-shot reader");
        assert!(reader.next_block().expect("read trailer").is_none());
        assert!(
            reader
                .next_block()
                .expect("trailer stays consumed")
                .is_none()
        );

        let circuit = parse("M 0\n");
        let mut bad = archive_for(&circuit, 1);
        bad[GLOBAL_HEADER_LEN..GLOBAL_HEADER_LEN + 8].copy_from_slice(b"BADMAGIC");
        let mut reader = SampleArchiveReader::open(io::Cursor::new(bad), &circuit, test_limits())
            .expect("open bad-magic reader");
        assert_code(reader.next_block(), SampleArchiveErrorCode::BadMagic);
    }

    #[test]
    fn finish_covers_unread_block_and_trailer_structure_edges() {
        let circuit = parse("M 0 1\nDETECTOR rec[-2]\n");
        let archive = archive_for(&circuit, 5);

        let reader = SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
            .expect("open reader");
        let summary = reader.finish().expect("finish drains unread block");
        assert_eq!(summary.block_count, 1);
        assert_eq!(summary.total_shots, 5);

        let trailer_start = archive.len() - crate::sample_archive::format::ARCHIVE_TRAILER_LEN;

        let mut second_block = archive.clone();
        second_block[trailer_start..trailer_start + 8].copy_from_slice(BLOCK_MAGIC);
        expect_finish_error(
            second_block,
            &circuit,
            SampleArchiveErrorCode::MalformedArchive,
        );

        let mut bad_magic = archive.clone();
        bad_magic[trailer_start..trailer_start + 8].copy_from_slice(b"BADMAGIC");
        expect_finish_error(bad_magic, &circuit, SampleArchiveErrorCode::BadMagic);

        let mut bad_block_count = archive.clone();
        put_u64(&mut bad_block_count, trailer_start + 16, 2);
        expect_finish_error(
            bad_block_count,
            &circuit,
            SampleArchiveErrorCode::ShapeMismatch,
        );

        let mut bad_total = archive.clone();
        put_u64(&mut bad_total, trailer_start + 24, 6);
        expect_finish_error(bad_total, &circuit, SampleArchiveErrorCode::ShapeMismatch);

        let mut reader =
            SampleArchiveReader::open(ErrorAfterEof::new(archive), &circuit, test_limits())
                .expect("open reader");
        assert!(reader.next_block().expect("read block").is_some());
        assert_code(reader.finish(), SampleArchiveErrorCode::Io);
    }

    fn expect_finish_error(archive: Vec<u8>, circuit: &[StimInstr], code: SampleArchiveErrorCode) {
        let mut reader =
            SampleArchiveReader::open(io::Cursor::new(archive), circuit, test_limits())
                .expect("open reader");
        assert!(reader.next_block().expect("read block").is_some());
        assert_code(reader.finish(), code);
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    struct AlwaysErr;

    impl Read for AlwaysErr {
        fn read(&mut self, _out: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("read failed"))
        }
    }

    struct ErrorAfterEof {
        bytes: io::Cursor<Vec<u8>>,
    }

    impl ErrorAfterEof {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes: io::Cursor::new(bytes),
            }
        }
    }

    impl Read for ErrorAfterEof {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.bytes.position() == self.bytes.get_ref().len() as u64 {
                Err(io::Error::other("trailing read failed"))
            } else {
                self.bytes.read(out)
            }
        }
    }
}
