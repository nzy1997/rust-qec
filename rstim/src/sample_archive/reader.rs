use crate::ir::StimInstr;
use crate::measurement_transform::{
    DecodedSampleBlock, EncodedMeasurementBlock, MeasurementTransform,
};
use crate::sample_archive::dense::unpack_dense;
use crate::sample_archive::format::{
    ArchiveTrailer, BLOCK_HEADER_LEN, BLOCK_MAGIC, BlockHeader, CODEC_SUITE_ZSTD_FRAMES_V1,
    GLOBAL_HEADER_LEN, GlobalHeader, STREAM_CODEC_EMPTY, STREAM_CODEC_FREE_DENSE_V1,
    STREAM_CODEC_SYNDROME_DENSE_V1, STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1, SampleArchiveError,
    SampleArchiveErrorCode, TRAILER_MAGIC, checked_dense_bit_bytes,
};
use crate::sample_archive::integrity::{header_digest, trailer_prefix};
use crate::sample_archive::limits::ArchiveLimits;
use crate::sample_archive::syndrome::{decode_syndrome_raw, update_dense_syndrome_hash};
use crate::sample_archive::writer::{limit, map_transform_error, shape};
use crate::sample_archive::zstd_frame::decompress_frame;
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read};

pub struct SampleArchiveReader<R: Read> {
    input: R,
    header: GlobalHeader,
    transform: MeasurementTransform,
    limits: ArchiveLimits,
    archive_hasher: Sha256,
    returned_block: bool,
}

impl<R: Read> SampleArchiveReader<R> {
    pub fn open(
        mut input: R,
        circuit: &[StimInstr],
        limits: ArchiveLimits,
    ) -> Result<Self, SampleArchiveError> {
        let mut header_bytes = [0u8; GLOBAL_HEADER_LEN];
        read_exact_or_truncated(&mut input, &mut header_bytes)?;
        let header = GlobalHeader::from_bytes(&header_bytes)?;
        if header.header_sha256 != header_digest(&header_bytes) {
            return Err(checksum("global header digest mismatch"));
        }
        validate_header_limits(&header, limits)?;
        let transform = MeasurementTransform::from_circuit_with_limits(circuit, limits.transform)
            .map_err(map_transform_error)?;
        compare_identity(&header, &transform)?;
        let mut archive_hasher = Sha256::new();
        archive_hasher.update(header_bytes);
        Ok(Self {
            input,
            header,
            transform,
            limits,
            archive_hasher,
            returned_block: false,
        })
    }

    pub fn next_block(&mut self) -> Result<Option<DecodedSampleBlock>, SampleArchiveError> {
        if self.header.total_shots == 0 || self.returned_block {
            return Ok(None);
        }
        let mut block_bytes = [0u8; BLOCK_HEADER_LEN];
        read_exact_or_truncated(&mut self.input, &mut block_bytes)?;
        self.archive_hasher.update(block_bytes);
        let block = BlockHeader::from_bytes(&block_bytes)?;
        self.validate_block_header(&block)?;

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
        self.returned_block = true;
        Ok(Some(decoded))
    }

    pub fn finish(mut self) -> Result<(), SampleArchiveError> {
        if self.header.total_shots > 0 && !self.returned_block {
            return Err(shape("positive-shot archive block was not read"));
        }
        let mut trailer_bytes = [0u8; crate::sample_archive::format::ARCHIVE_TRAILER_LEN];
        read_exact_or_truncated(&mut self.input, &mut trailer_bytes)?;
        if trailer_bytes[0..8] == BLOCK_MAGIC[..] {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "second data block violates one-block archive contract",
            ));
        }
        if trailer_bytes[0..8] != TRAILER_MAGIC[..] {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::BadMagic,
                "invalid trailer magic",
            ));
        }
        let trailer = ArchiveTrailer::from_bytes(&trailer_bytes)?;
        let expected_blocks = u64::from(self.header.total_shots > 0);
        if trailer.block_count != expected_blocks {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "trailer block count does not match one-block contract",
            ));
        }
        if trailer.total_shots != self.header.total_shots {
            return Err(shape("trailer shot count does not match header"));
        }
        let prefix = trailer_prefix(trailer.block_count, trailer.total_shots)?;
        self.archive_hasher.update(prefix);
        let digest: [u8; 32] = self.archive_hasher.finalize().into();
        if digest != trailer.archive_sha256 {
            return Err(checksum("archive digest mismatch"));
        }
        let mut extra = [0u8; 1];
        match self.input.read(&mut extra) {
            Ok(0) => Ok(()),
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

    fn validate_block_header(&self, block: &BlockHeader) -> Result<(), SampleArchiveError> {
        if block.block_index != 0 || block.first_shot != 0 {
            return Err(SampleArchiveError::with_code(
                SampleArchiveErrorCode::MalformedArchive,
                "invalid one-block sequence",
            ));
        }
        if block.shot_count != self.header.total_shots {
            return Err(shape("block shot count does not match header"));
        }
        if block.shot_count > self.header.max_shots_per_block
            || block.shot_count > self.limits.transform.max_shots_per_block
        {
            return Err(limit("block shot count exceeds limit"));
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

    fn read_stream(&mut self, len: u64) -> Result<Vec<u8>, SampleArchiveError> {
        let len = usize::try_from(len).map_err(|_| limit("compressed stream too large"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| limit("compressed stream reservation failed"))?;
        bytes.resize(len, 0);
        read_exact_or_truncated(&mut self.input, &mut bytes)?;
        self.archive_hasher.update(&bytes);
        Ok(bytes)
    }
}

fn validate_header_limits(
    header: &GlobalHeader,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if header.total_shots > limits.max_total_shots {
        return Err(limit("archive total shots exceed limit"));
    }
    if header.max_shots_per_block == 0
        || header.max_shots_per_block > limits.transform.max_shots_per_block
    {
        return Err(limit("header max shots per block exceeds limit"));
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
    if header.total_shots > 0 && header.total_shots > header.max_shots_per_block {
        return Err(shape("one-block archive exceeds header block-shot limit"));
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
    if uncompressed > limits.max_decompressed_bytes_per_stream {
        return Err(limit("decompressed stream exceeds limit"));
    }
    if compressed > limits.max_compressed_bytes_per_stream {
        return Err(limit("compressed stream exceeds limit"));
    }
    Ok(())
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
        writer
            .write_measurements(&measurements)
            .expect("write measurements");
        writer.finish().expect("finish writer")
    }

    fn header_from(archive: &[u8]) -> GlobalHeader {
        GlobalHeader::from_bytes(&archive[..GLOBAL_HEADER_LEN]).expect("parse header")
    }

    fn block_from(archive: &[u8]) -> BlockHeader {
        BlockHeader::from_bytes(&archive[GLOBAL_HEADER_LEN..GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN])
            .expect("parse block")
    }

    fn assert_code<T>(result: Result<T, SampleArchiveError>, code: SampleArchiveErrorCode) {
        match result {
            Ok(_) => panic!("expected {code:?}"),
            Err(err) => assert_eq!(err.code(), code),
        }
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
        assert_code(
            validate_header_limits(&bad, test_limits()),
            SampleArchiveErrorCode::ShapeMismatch,
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
        bad.free_uncompressed_len = test_limits().max_decompressed_bytes_per_stream + 1;
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

        reader.limits = test_limits();
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
        limits.max_decompressed_bytes_per_stream = 1;
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
    fn finish_covers_unread_block_and_trailer_structure_edges() {
        let circuit = parse("M 0 1\nDETECTOR rec[-2]\n");
        let archive = archive_for(&circuit, 5);

        let reader = SampleArchiveReader::open(io::Cursor::new(&archive), &circuit, test_limits())
            .expect("open reader");
        assert_code(reader.finish(), SampleArchiveErrorCode::ShapeMismatch);

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
            SampleArchiveErrorCode::MalformedArchive,
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
