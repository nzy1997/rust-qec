use rstim::ir::StimInstr;
use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::{MeasurementTransform, MeasurementTransformLimits};
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sample_archive::format::SampleArchiveErrorCode;
use rstim::sample_archive::format::{
    ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, GLOBAL_HEADER_LEN,
    STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1,
};
use rstim::sample_archive::{
    ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter,
};
use rstim::sim::bit_table::BitTable;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const SMALL_SEMANTIC_CASES: [&str; 6] = [
    "nonzero_reference",
    "rank_zero",
    "dependent_detectors",
    "repeat_records",
    "observable_recovery",
    "loss_visible_measurements",
];

#[test]
fn rsmp_dense_archive_contract() {
    verify_zero_shot_no_writer_call();
    verify_transform_limit_mapping();
    let valid_cases = verify_six_small_semantic_cases();
    assert_eq!(valid_cases, 6);

    let negative_cases = verify_negative_cases();
    assert_eq!(negative_cases, 15);

    println!("PASS rsmp dense archive valid_cases=6 negative_cases=15");
}

fn verify_zero_shot_no_writer_call() {
    let circuit = parse("M 0\nDETECTOR rec[-1]\n");
    let transform = MeasurementTransform::from_circuit(&circuit).expect("zero-shot transform");
    let writer_calls = 0usize;
    let archive = finish_archive(transform, 0, |writer| {
        assert_eq!(writer_calls, 0);
        writer.finish()
    });

    let mut reader = SampleArchiveReader::open(ShortRead::new(&archive, 3), &circuit, limits())
        .expect("open zero-shot archive");
    assert!(reader.next_block().expect("zero-shot next_block").is_none());
    reader.finish().expect("zero-shot finish");
}

fn verify_six_small_semantic_cases() -> usize {
    let mut consumed = 0;
    for id in SMALL_SEMANTIC_CASES {
        let circuit_text = read_fixture(&format!("{id}.stim"));
        let writer_circuit = parse(&circuit_text);
        let reader_circuit = if id == "nonzero_reference" {
            parse("# comment-only change\n\n  X 0\n   M 0\nDETECTOR rec[-1]\n")
        } else {
            writer_circuit.clone()
        };
        let transform =
            MeasurementTransform::from_circuit(&writer_circuit).expect("semantic transform");
        let shots = 4;
        let measurements = patterned_table(transform.num_measurements(), shots, id.as_bytes());
        let archive = archive_from_measurements(&writer_circuit, &measurements);

        let mut reader =
            SampleArchiveReader::open(ShortRead::new(&archive, 2), &reader_circuit, limits())
                .unwrap_or_else(|err| panic!("{id}: open failed: {err}"));
        let decoded = reader
            .next_block()
            .unwrap_or_else(|err| panic!("{id}: next_block failed: {err}"))
            .unwrap_or_else(|| panic!("{id}: expected one block"));
        reader
            .finish()
            .unwrap_or_else(|err| panic!("{id}: finish failed: {err}"));

        assert_tables_eq(&measurements, &decoded.measurements, id, "measurements");
        let m2d = measurements_to_detections(&reader_circuit, &measurements)
            .unwrap_or_else(|err| panic!("{id}: m2d failed: {err}"));
        assert_tables_eq(&m2d.detections, &decoded.detections, id, "detections");
        assert_tables_eq(
            &m2d.observable_flips,
            &decoded.observable_flips,
            id,
            "observables",
        );

        if id == "rank_zero" {
            assert_eq!(transform.rank(), 0);
        }
        if id == "repeat_records" {
            assert_eq!(transform.rank(), transform.num_measurements());
        }

        consumed += 1;
    }
    consumed
}

fn verify_transform_limit_mapping() {
    let circuit =
        parse("REPEAT 2 {\n    M 0\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let permissive = MeasurementTransformLimits {
        max_measurements: 64,
        max_detectors: 64,
        max_observables: 16,
        max_repeat_depth: 8,
        max_expanded_instructions: 1_000,
        max_parity_terms: 1_000,
        max_shots_per_block: 16,
        max_transform_working_bytes: 1 << 20,
        max_block_working_bytes: 1 << 20,
    };
    let transform = MeasurementTransform::from_circuit_with_limits(&circuit, permissive)
        .expect("mapping transform");
    let measurements = patterned_table(transform.num_measurements(), 2, b"mapping");
    let block_working = transform
        .estimate_block_working_bytes(measurements.num_minor())
        .expect("block working estimate");

    let exact_transform_limits = MeasurementTransformLimits {
        max_measurements: transform.num_measurements() as u64,
        max_detectors: transform.num_detectors() as u64,
        max_observables: transform.num_observables() as u64,
        max_repeat_depth: transform.max_repeat_depth() as u64,
        max_expanded_instructions: transform.expanded_instructions(),
        max_parity_terms: transform.parity_terms(),
        max_shots_per_block: measurements.num_minor() as u64,
        max_transform_working_bytes: transform.transform_working_bytes(),
        max_block_working_bytes: block_working,
    };
    let exact_limits = ArchiveLimits {
        transform: exact_transform_limits,
        ..limits()
    };
    let archive = {
        let mut writer = SampleArchiveWriter::new(
            NonSeekableWriter::default(),
            transform.clone(),
            measurements.num_minor() as u64,
            SampleArchiveOptions::default(),
            exact_limits,
        )
        .expect("mapping exact writer limits");
        writer
            .write_measurements(&measurements)
            .expect("mapping write");
        writer.finish().expect("mapping finish writer").into_inner()
    };
    let mut reader = SampleArchiveReader::open(ShortRead::new(&archive, 3), &circuit, exact_limits)
        .expect("mapping exact reader limits");
    assert!(reader.next_block().expect("mapping block").is_some());
    reader.finish().expect("mapping finish");

    let mut checks = 0;
    for lower in [
        lower_limit(exact_limits, |t| &mut t.max_measurements),
        lower_limit(exact_limits, |t| &mut t.max_detectors),
        lower_limit(exact_limits, |t| &mut t.max_observables),
        lower_limit(exact_limits, |t| &mut t.max_repeat_depth),
        lower_limit(exact_limits, |t| &mut t.max_expanded_instructions),
        lower_limit(exact_limits, |t| &mut t.max_parity_terms),
        zero_limit(exact_limits, |t| &mut t.max_shots_per_block),
        lower_limit(exact_limits, |t| &mut t.max_transform_working_bytes),
        lower_limit(exact_limits, |t| &mut t.max_block_working_bytes),
    ] {
        let transform =
            MeasurementTransform::from_circuit_with_limits(&circuit, permissive).unwrap();
        let writer_err = SampleArchiveWriter::new(
            NonSeekableWriter::default(),
            transform,
            measurements.num_minor() as u64,
            SampleArchiveOptions::default(),
            lower,
        )
        .and_then(|mut writer| {
            writer.write_measurements(&measurements)?;
            writer.finish()
        })
        .unwrap_err();
        assert_eq!(writer_err.code(), SampleArchiveErrorCode::LimitExceeded);
        checks += 1;
    }
    assert_eq!(checks, 9);
}

fn verify_negative_cases() -> usize {
    let circuit = parse("M 0 1 2\nDETECTOR rec[-3] rec[-2]\nDETECTOR rec[-2] rec[-1]\n");
    let measurements = patterned_table(3, 5, b"negative");
    let archive = archive_from_measurements(&circuit, &measurements);
    let mut cases = 0;

    expect_open_error(
        &archive,
        &parse("M 0 1 2 3\n"),
        SampleArchiveErrorCode::CircuitMismatch,
    );
    cases += 1;

    let mut bad_header_digest = archive.clone();
    bad_header_digest[GLOBAL_HEADER_LEN - 1] ^= 0x55;
    expect_open_error(
        &bad_header_digest,
        &circuit,
        SampleArchiveErrorCode::ChecksumMismatch,
    );
    let mut bad_archive_digest = archive.clone();
    let digest_offset = bad_archive_digest.len() - 1;
    bad_archive_digest[digest_offset] ^= 0x55;
    expect_finish_error(
        &bad_archive_digest,
        &circuit,
        SampleArchiveErrorCode::ChecksumMismatch,
        true,
    );
    cases += 1;

    let mut malformed_zstd = archive.clone();
    let syndrome = stream_range(&malformed_zstd, StreamKind::Syndrome);
    malformed_zstd[syndrome.start + 5] ^= 0x80;
    recompute_archive_digest(&mut malformed_zstd);
    expect_next_block_error(
        &malformed_zstd,
        &circuit,
        SampleArchiveErrorCode::DecompressionFailed,
    );
    let mut extra_frame = archive.clone();
    let syndrome = stream_range(&extra_frame, StreamKind::Syndrome);
    let skippable_empty_frame = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
    extra_frame.splice(syndrome.end..syndrome.end, skippable_empty_frame);
    put_u64(
        &mut extra_frame,
        GLOBAL_HEADER_LEN + 52,
        (syndrome.len() + skippable_empty_frame.len()) as u64,
    );
    recompute_archive_digest(&mut extra_frame);
    expect_next_block_error(
        &extra_frame,
        &circuit,
        SampleArchiveErrorCode::DecompressionFailed,
    );
    cases += 1;

    let syndrome = stream_range(&archive, StreamKind::Syndrome);
    let cut_frame = archive[..syndrome.end - 2].to_vec();
    expect_next_block_error(&cut_frame, &circuit, SampleArchiveErrorCode::Truncated);
    cases += 1;

    let truncated_trailer = archive[..archive.len() - 3].to_vec();
    expect_finish_error(
        &truncated_trailer,
        &circuit,
        SampleArchiveErrorCode::Truncated,
        true,
    );
    cases += 1;

    let mut changed_total = archive.clone();
    put_u64(&mut changed_total, 80, 6);
    recompute_header_digest(&mut changed_total);
    recompute_archive_digest(&mut changed_total);
    expect_next_block_error(
        &changed_total,
        &circuit,
        SampleArchiveErrorCode::ShapeMismatch,
    );
    cases += 1;

    let mut bad_block_order = archive.clone();
    put_u64(&mut bad_block_order, GLOBAL_HEADER_LEN + 12, 1);
    recompute_archive_digest(&mut bad_block_order);
    expect_next_block_error(
        &bad_block_order,
        &circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut high_length = archive.clone();
    put_u64(
        &mut high_length,
        GLOBAL_HEADER_LEN + 52,
        limits().max_compressed_bytes_per_stream + 1,
    );
    recompute_archive_digest(&mut high_length);
    expect_next_block_error(
        &high_length,
        &circuit,
        SampleArchiveErrorCode::LimitExceeded,
    );
    let mut window_limits = limits();
    window_limits.max_zstd_window_bytes = 1;
    expect_next_block_error_with_limits(
        &archive,
        &circuit,
        window_limits,
        SampleArchiveErrorCode::LimitExceeded,
    );
    cases += 1;

    let mut nonzero_padding = archive.clone();
    replace_stream(&mut nonzero_padding, StreamKind::Free, |bytes| {
        let bit_count = 5u64;
        set_first_padding_bit(bytes, bit_count);
    });
    expect_next_block_error(
        &nonzero_padding,
        &circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut sparse_codec = archive.clone();
    put_u16(
        &mut sparse_codec,
        GLOBAL_HEADER_LEN + 36,
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1,
    );
    recompute_archive_digest(&mut sparse_codec);
    expect_next_block_error(
        &sparse_codec,
        &circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut changed_payload = archive.clone();
    replace_stream(&mut changed_payload, StreamKind::Free, |bytes| {
        bytes[0] ^= 0x01
    });
    expect_next_block_error(
        &changed_payload,
        &circuit,
        SampleArchiveErrorCode::LogicalDigestMismatch,
    );
    cases += 1;

    let mut trailing = archive.clone();
    trailing.push(0);
    expect_finish_error(
        &trailing,
        &circuit,
        SampleArchiveErrorCode::TrailingData,
        true,
    );
    cases += 1;

    let wrong_width = BitTable::try_new(2, 5).expect("wrong-width table");
    expect_writer_error(
        &circuit,
        5,
        &wrong_width,
        SampleArchiveErrorCode::ShapeMismatch,
    );
    cases += 1;

    let too_many_shots = BitTable::try_new(3, 6).expect("too-many-shots table");
    expect_writer_error(
        &circuit,
        5,
        &too_many_shots,
        SampleArchiveErrorCode::ShapeMismatch,
    );
    cases += 1;

    let transform = MeasurementTransform::from_circuit(&circuit).expect("finish-before transform");
    let writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        5,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("writer");
    assert_eq!(
        writer.finish().unwrap_err().code(),
        SampleArchiveErrorCode::ShapeMismatch
    );
    cases += 1;

    cases
}

fn archive_from_measurements(circuit: &[StimInstr], measurements: &BitTable) -> Vec<u8> {
    let transform = MeasurementTransform::from_circuit(circuit).expect("archive transform");
    let mut calls = 0usize;
    finish_archive(transform, measurements.num_minor() as u64, |mut writer| {
        calls += 1;
        writer
            .write_measurements(measurements)
            .expect("write measurements");
        assert_eq!(calls, 1);
        writer.finish()
    })
}

fn finish_archive(
    transform: MeasurementTransform,
    total_shots: u64,
    finish: impl FnOnce(
        SampleArchiveWriter<NonSeekableWriter>,
    )
        -> Result<NonSeekableWriter, rstim::sample_archive::format::SampleArchiveError>,
) -> Vec<u8> {
    let writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        total_shots,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("archive writer");
    finish(writer).expect("finish archive").into_inner()
}

fn expect_open_error(archive: &[u8], circuit: &[StimInstr], code: SampleArchiveErrorCode) {
    let err = match SampleArchiveReader::open(ShortRead::new(archive, 3), circuit, limits()) {
        Ok(_) => panic!("expected open error {code:?}"),
        Err(err) => err,
    };
    assert_eq!(err.code(), code);
}

fn expect_next_block_error(archive: &[u8], circuit: &[StimInstr], code: SampleArchiveErrorCode) {
    expect_next_block_error_with_limits(archive, circuit, limits(), code);
}

fn expect_next_block_error_with_limits(
    archive: &[u8],
    circuit: &[StimInstr],
    limits: ArchiveLimits,
    code: SampleArchiveErrorCode,
) {
    let mut reader = SampleArchiveReader::open(ShortRead::new(archive, 3), circuit, limits)
        .expect("open before next_block error");
    let err = reader.next_block().unwrap_err();
    assert_eq!(err.code(), code);
}

fn expect_finish_error(
    archive: &[u8],
    circuit: &[StimInstr],
    code: SampleArchiveErrorCode,
    expect_block: bool,
) {
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 4), circuit, limits()).expect("open");
    if expect_block {
        assert!(
            reader
                .next_block()
                .expect("block before finish error")
                .is_some()
        );
    }
    let err = reader.finish().unwrap_err();
    assert_eq!(err.code(), code);
}

fn expect_writer_error(
    circuit: &[StimInstr],
    total_shots: u64,
    measurements: &BitTable,
    code: SampleArchiveErrorCode,
) {
    let transform = MeasurementTransform::from_circuit(circuit).expect("writer-error transform");
    let mut writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        total_shots,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("writer");
    let err = writer.write_measurements(measurements).unwrap_err();
    assert_eq!(err.code(), code);
}

fn replace_stream(archive: &mut Vec<u8>, kind: StreamKind, mutate: impl FnOnce(&mut Vec<u8>)) {
    let range = stream_range(archive, kind);
    let expected_len = match kind {
        StreamKind::Syndrome => get_u64(archive, GLOBAL_HEADER_LEN + 44),
        StreamKind::Free => get_u64(archive, GLOBAL_HEADER_LEN + 60),
    } as usize;
    let mut decoded = zstd::bulk::decompress(&archive[range.clone()], expected_len)
        .expect("test helper decompress");
    mutate(&mut decoded);
    let compressed = compress_frame_for_test(&decoded);
    archive.splice(range.clone(), compressed.iter().copied());
    let block_base = GLOBAL_HEADER_LEN;
    match kind {
        StreamKind::Syndrome => {
            put_u64(archive, block_base + 52, compressed.len() as u64);
        }
        StreamKind::Free => {
            put_u64(archive, block_base + 68, compressed.len() as u64);
        }
    }
    recompute_archive_digest(archive);
}

fn compress_frame_for_test(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 3).expect("test zstd encoder");
    encoder
        .include_checksum(true)
        .expect("test zstd checksum flag");
    encoder
        .include_contentsize(true)
        .expect("test zstd content-size flag");
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .expect("test zstd content size");
    encoder.write_all(bytes).expect("test zstd write");
    encoder.finish().expect("test zstd finish")
}

fn stream_range(archive: &[u8], kind: StreamKind) -> std::ops::Range<usize> {
    let block_base = GLOBAL_HEADER_LEN;
    let syndrome_start = block_base + BLOCK_HEADER_LEN;
    let syndrome_len = get_u64(archive, block_base + 52) as usize;
    let free_len = get_u64(archive, block_base + 68) as usize;
    match kind {
        StreamKind::Syndrome => syndrome_start..syndrome_start + syndrome_len,
        StreamKind::Free => {
            let start = syndrome_start + syndrome_len;
            start..start + free_len
        }
    }
}

#[derive(Clone, Copy)]
enum StreamKind {
    Syndrome,
    Free,
}

fn set_first_padding_bit(bytes: &mut [u8], bit_count: u64) {
    let byte_index = (bit_count / 8) as usize;
    let bit_index = (bit_count % 8) as u8;
    assert!(bit_index != 0);
    bytes[byte_index] |= 1 << bit_index;
}

fn recompute_header_digest(archive: &mut [u8]) {
    let digest: [u8; 32] = Sha256::digest(&archive[..GLOBAL_HEADER_LEN - 32]).into();
    archive[GLOBAL_HEADER_LEN - 32..GLOBAL_HEADER_LEN].copy_from_slice(&digest);
}

fn recompute_archive_digest(archive: &mut [u8]) {
    let trailer = archive.len() - ARCHIVE_TRAILER_LEN;
    let digest: [u8; 32] = Sha256::digest(&archive[..trailer + 32]).into();
    archive[trailer + 32..trailer + 64].copy_from_slice(&digest);
}

fn parse(text: &str) -> Vec<StimInstr> {
    parse_lines(text).unwrap_or_else(|err| panic!("parse failed: {err}"))
}

fn read_fixture(name: &str) -> String {
    fs::read_to_string(repo_path(&format!("rstim/tests/fixtures/rsmp/{name}")))
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(path)
}

fn limits() -> ArchiveLimits {
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

fn lower_limit(
    mut limits: ArchiveLimits,
    select: impl FnOnce(&mut MeasurementTransformLimits) -> &mut u64,
) -> ArchiveLimits {
    let value = select(&mut limits.transform);
    *value = value.saturating_sub(1);
    limits
}

fn zero_limit(
    mut limits: ArchiveLimits,
    select: impl FnOnce(&mut MeasurementTransformLimits) -> &mut u64,
) -> ArchiveLimits {
    *select(&mut limits.transform) = 0;
    limits
}

fn patterned_table(bits: usize, shots: usize, salt: &[u8]) -> BitTable {
    let mut table = BitTable::try_new(bits, shots).expect("patterned table allocates");
    for bit in 0..bits {
        for shot in 0..shots {
            let salt_bit = salt[(bit + shot) % salt.len()] & 1 == 1;
            if ((bit * 17 + shot * 31 + salt.len()) & 1 == 1) ^ salt_bit {
                table.set(bit, shot, true);
            }
        }
    }
    table
}

fn assert_tables_eq(left: &BitTable, right: &BitTable, case: &str, label: &str) {
    assert_eq!(left.num_major(), right.num_major(), "{case}: {label} rows");
    assert_eq!(left.num_minor(), right.num_minor(), "{case}: {label} shots");
    for row in 0..left.num_major() {
        for shot in 0..left.num_minor() {
            assert_eq!(
                left.get(row, shot),
                right.get(row, shot),
                "{case}: {label}[{row},{shot}]"
            );
        }
    }
    let mut left_bytes = Vec::new();
    let mut right_bytes = Vec::new();
    write_shots_b8(left, &mut left_bytes).unwrap();
    write_shots_b8(right, &mut right_bytes).unwrap();
    assert_eq!(
        hex(&Sha256::digest(&left_bytes)),
        hex(&Sha256::digest(&right_bytes))
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[derive(Default, Debug)]
struct NonSeekableWriter {
    bytes: Vec<u8>,
}

impl NonSeekableWriter {
    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for NonSeekableWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ShortRead<'a> {
    bytes: &'a [u8],
    max_chunk: usize,
}

impl<'a> ShortRead<'a> {
    fn new(bytes: &'a [u8], max_chunk: usize) -> Self {
        Self { bytes, max_chunk }
    }
}

impl Read for ShortRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            return Ok(0);
        }
        let n = out.len().min(self.max_chunk).min(self.bytes.len());
        out[..n].copy_from_slice(&self.bytes[..n]);
        self.bytes = &self.bytes[n..];
        Ok(n)
    }
}
