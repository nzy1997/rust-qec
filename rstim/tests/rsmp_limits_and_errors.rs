use rstim::ir::StimInstr;
use rstim::measurement_transform::{
    MeasurementTransform, MeasurementTransformError, MeasurementTransformLimits,
};
use rstim::parser::parse_lines;
use rstim::sample_archive::format::{
    BLOCK_HEADER_LEN, FORMAT_MAJOR, GLOBAL_HEADER_LEN, SampleArchiveErrorCode,
    checked_dense_bit_bytes,
};
use rstim::sample_archive::{
    ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter,
};
use rstim::sim::bit_table::BitTable;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[test]
fn rsmp_limits_and_error_contract() {
    let overflow = verify_numeric_negative_controls();
    assert_eq!(overflow, 1);
    let fields = verify_all_limit_fields();
    assert_eq!(fields, 20);
    let prepayload = verify_oversized_declarations_do_not_read_payload();
    assert_eq!(prepayload, 1);
    let zstd_window = verify_zstd_window_and_frame_multiplicity();
    assert_eq!(zstd_window, 1);
    let aggregate = verify_aggregate_limits();
    assert_eq!(aggregate, 1);
    let public_codes = verify_public_codes();
    assert_eq!(public_codes, 14);
    let cli_snapshots = verify_cli_snapshots();
    assert_eq!(cli_snapshots, 14);
    verify_precedence_controls();
    println!(
        "PASS rsmp limits fields=20 overflow=1 prepayload=1 zstd_window=1 aggregate=1 public_codes=14 cli_snapshots=14"
    );
}

fn verify_all_limit_fields() -> usize {
    let circuit =
        parse("REPEAT 2 {\n    M 0\n}\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n");
    let transform =
        MeasurementTransform::from_circuit_with_limits(&circuit, generous_transform_limits())
            .expect("limit-field transform");
    let measurements = patterned_table(transform.num_measurements(), 2);
    let exact_transform_limits = exact_transform_limits(&transform, measurements.num_minor());
    let exact_limits = ArchiveLimits {
        transform: exact_transform_limits,
        max_archive_bytes: 512,
        max_block_count: 1,
        max_total_shots: measurements.num_minor() as u64,
        max_detector_rank: transform.rank() as u64,
        max_free_measurements: (transform.num_measurements() - transform.rank()) as u64,
        max_compressed_bytes_per_frame: 256,
        max_decompressed_bytes_per_frame: 8,
        max_compressed_bytes_per_archive: 512,
        max_decompressed_bytes_per_archive: 16,
        max_zstd_window_bytes: 8,
        max_zstd_decoder_memory_bytes: 2 * 1024 * 1024,
    };

    let archive = {
        let mut writer = SampleArchiveWriter::new(
            NonSeekableWriter::default(),
            transform.clone(),
            measurements.num_minor() as u64,
            SampleArchiveOptions::default(),
            exact_limits,
        )
        .expect("writer accepts exact nested limits");
        writer
            .write_measurements(&measurements)
            .expect("write exact");
        writer.finish().expect("finish exact").into_inner()
    };
    let mut reader = SampleArchiveReader::open(ShortRead::new(&archive, 3), &circuit, exact_limits)
        .expect("reader receives exact nested limits");
    assert!(reader.next_block().expect("read exact block").is_some());
    reader.finish().expect("finish exact reader");

    let mut checked = 0usize;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_measurements),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_detectors),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_observables),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_repeat_depth),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_expanded_instructions),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_parity_terms),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        zero_nested(exact_limits, |t| &mut t.max_shots_per_block),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_transform_working_bytes),
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        lower_nested(exact_limits, |t| &mut t.max_block_working_bytes),
    );
    checked += 1;

    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_archive_bytes = (GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN) as u64;
    });
    checked += 1;
    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_block_count = 0;
    });
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        ArchiveLimits {
            max_total_shots: 1,
            ..exact_limits
        },
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        ArchiveLimits {
            max_detector_rank: 0,
            ..exact_limits
        },
    );
    checked += 1;
    expect_writer_new_limit(
        &circuit,
        &measurements,
        ArchiveLimits {
            max_free_measurements: 0,
            ..exact_limits
        },
    );
    checked += 1;
    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_compressed_bytes_per_frame = 1;
    });
    checked += 1;
    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_decompressed_bytes_per_frame = 0;
    });
    checked += 1;
    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_compressed_bytes_per_archive = 1;
    });
    checked += 1;
    expect_writer_or_reader_limit(&circuit, &measurements, exact_limits, |limits| {
        limits.max_decompressed_bytes_per_archive = 0;
    });
    checked += 1;
    expect_reader_next_limit(
        &archive,
        &circuit,
        ArchiveLimits {
            max_zstd_window_bytes: 0,
            ..exact_limits
        },
    );
    checked += 1;
    expect_reader_next_limit(
        &archive,
        &circuit,
        ArchiveLimits {
            max_zstd_decoder_memory_bytes: 1,
            ..exact_limits
        },
    );
    checked += 1;

    checked
}

fn verify_numeric_negative_controls() -> usize {
    assert_code(
        checked_dense_bit_bytes(u64::MAX, 2),
        SampleArchiveErrorCode::LimitExceeded,
    );

    let repeated = parse("REPEAT 2 {\n    REPEAT 2 {\n        M 0\n    }\n}\n");
    let mut limits = generous_transform_limits();
    limits.max_repeat_depth = 1;
    assert_transform_limit(MeasurementTransform::from_circuit_with_limits(
        &repeated, limits,
    ));

    let expanded = parse("REPEAT 5 {\n    M 0\n}\n");
    let mut limits = generous_transform_limits();
    limits.max_expanded_instructions = 4;
    assert_transform_limit(MeasurementTransform::from_circuit_with_limits(
        &expanded, limits,
    ));

    let parity = parse("M 0 1 2\nDETECTOR rec[-1] rec[-2] rec[-3]\n");
    let mut limits = generous_transform_limits();
    limits.max_parity_terms = 2;
    assert_transform_limit(MeasurementTransform::from_circuit_with_limits(
        &parity, limits,
    ));

    let circuit = parse("M 0\nDETECTOR rec[-1]\n");
    let transform = MeasurementTransform::from_circuit(&circuit).expect("working transform");
    assert_transform_limit(transform.validate_actual_usage(
        MeasurementTransformLimits {
            max_block_working_bytes: 1,
            ..generous_transform_limits()
        },
        Some(usize::MAX),
    ));

    1
}

fn verify_oversized_declarations_do_not_read_payload() -> usize {
    let circuit = parse("M 0\n");
    let measurements = patterned_table(1, 1);
    let mut archive = archive_from_measurements(&circuit, &measurements, limits());
    put_u64(&mut archive, GLOBAL_HEADER_LEN + 68, u64::MAX);
    let prefix_len = GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN;
    let mut reader = SampleArchiveReader::open(
        PayloadPanicRead::new(&archive[..prefix_len]),
        &circuit,
        limits(),
    )
    .expect("open structural prefix");
    let err = reader
        .next_block()
        .expect_err("oversized declaration fails before payload");
    assert_eq!(err.code(), SampleArchiveErrorCode::LimitExceeded);
    1
}

fn verify_zstd_window_and_frame_multiplicity() -> usize {
    let circuit = parse("M 0\n");
    let measurements = patterned_table(1, 1);
    let archive = archive_from_measurements(&circuit, &measurements, limits());
    let mut tiny_window = limits();
    tiny_window.max_zstd_window_bytes = 0;
    expect_reader_next_limit(&archive, &circuit, tiny_window);

    let mut concatenated = archive.clone();
    let free = free_stream_range(&concatenated);
    let original = concatenated[free.clone()].to_vec();
    concatenated.splice(free.end..free.end, original.iter().copied());
    put_u64(
        &mut concatenated,
        GLOBAL_HEADER_LEN + 68,
        (original.len() * 2) as u64,
    );
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(&concatenated, 5), &circuit, limits())
            .expect("open concatenated frame archive");
    let err = reader
        .next_block()
        .expect_err("concatenated frames are malformed");
    assert_eq!(err.code(), SampleArchiveErrorCode::MalformedArchive);
    1
}

fn verify_aggregate_limits() -> usize {
    let circuit = parse("M 0\n");
    let measurements = patterned_table(1, 3);
    let mut aggregate_limits = limits();
    aggregate_limits.transform.max_shots_per_block = 1;
    let archive = archive_from_measurements(&circuit, &measurements, aggregate_limits);
    let first_block = block_compressed_bytes(&archive);
    let mut low = aggregate_limits;
    low.max_compressed_bytes_per_archive = first_block * 2 - 1;
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(&archive, 7), &circuit, low).expect("open");
    assert!(
        reader
            .next_block()
            .expect("first aggregate block")
            .is_some()
    );
    let err = reader
        .next_block()
        .expect_err("aggregate compressed bytes fail on second block");
    assert_eq!(err.code(), SampleArchiveErrorCode::LimitExceeded);

    let mut shot_limits = aggregate_limits;
    shot_limits.max_total_shots = 2;
    expect_writer_new_limit(&circuit, &measurements, shot_limits);
    1
}

fn verify_public_codes() -> usize {
    let codes = [
        SampleArchiveErrorCode::BadMagic,
        SampleArchiveErrorCode::UnsupportedVersion,
        SampleArchiveErrorCode::UnsupportedFeature,
        SampleArchiveErrorCode::UnsupportedSweep,
        SampleArchiveErrorCode::CircuitMismatch,
        SampleArchiveErrorCode::ShapeMismatch,
        SampleArchiveErrorCode::LimitExceeded,
        SampleArchiveErrorCode::Truncated,
        SampleArchiveErrorCode::MalformedArchive,
        SampleArchiveErrorCode::DecompressionFailed,
        SampleArchiveErrorCode::ChecksumMismatch,
        SampleArchiveErrorCode::LogicalDigestMismatch,
        SampleArchiveErrorCode::TrailingData,
        SampleArchiveErrorCode::Io,
    ];
    let names = codes.map(SampleArchiveErrorCode::as_str);
    assert_eq!(
        names,
        [
            "RSMP_BAD_MAGIC",
            "RSMP_UNSUPPORTED_VERSION",
            "RSMP_UNSUPPORTED_FEATURE",
            "RSMP_UNSUPPORTED_SWEEP",
            "RSMP_CIRCUIT_MISMATCH",
            "RSMP_SHAPE_MISMATCH",
            "RSMP_LIMIT_EXCEEDED",
            "RSMP_TRUNCATED",
            "RSMP_MALFORMED_ARCHIVE",
            "RSMP_DECOMPRESSION_FAILED",
            "RSMP_CHECKSUM_MISMATCH",
            "RSMP_LOGICAL_DIGEST_MISMATCH",
            "RSMP_TRAILING_DATA",
            "RSMP_IO",
        ]
    );
    codes.len()
}

fn verify_cli_snapshots() -> usize {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit_path = dir.path().join("base.stim");
    fs::write(&circuit_path, b"M 0\n").expect("write circuit");
    let circuit = parse("M 0\n");
    let measurements = patterned_table(1, 1);
    let archive = archive_from_measurements(&circuit, &measurements, limits());
    let archive_path = dir.path().join("base.rsmp");
    fs::write(&archive_path, &archive).expect("write archive");

    let mut snapshots = 0usize;
    let cases = cli_case_archives(&archive);
    for (code, bytes) in cases {
        let path = dir.path().join(format!("{}.rsmp", code.as_str()));
        fs::write(&path, bytes).expect("write mutated archive");
        let stderr = unpack_stderr(&circuit_path, &path);
        assert_eq!(
            stderr,
            format!("rsmp error [{}]: {}", code.as_str(), expected_detail(code))
        );
        snapshots += 1;
    }

    let missing = dir.path().join("missing.rsmp");
    let stderr = unpack_stderr(&circuit_path, &missing);
    assert_eq!(
        stderr,
        format!(
            "rsmp error [{}]: {}",
            SampleArchiveErrorCode::Io.as_str(),
            expected_detail(SampleArchiveErrorCode::Io)
        )
    );
    snapshots += 1;

    let sweep_circuit = dir.path().join("sweep.stim");
    fs::write(&sweep_circuit, b"M sweep[0]\n").expect("write sweep circuit");
    let input = dir.path().join("sweep.b8");
    fs::write(&input, b"\0").expect("write sweep measurements");
    let output = dir.path().join("sweep.rsmp");
    let stderr = pack_stderr(&sweep_circuit, &input, &output);
    assert_eq!(
        stderr,
        format!(
            "rsmp error [{}]: {}",
            SampleArchiveErrorCode::UnsupportedSweep.as_str(),
            expected_detail(SampleArchiveErrorCode::UnsupportedSweep)
        )
    );
    snapshots += 1;

    let other_circuit = dir.path().join("other.stim");
    fs::write(&other_circuit, b"M 0\nM 1\n").expect("write mismatch circuit");
    let stderr = unpack_stderr(&other_circuit, &archive_path);
    assert_eq!(
        stderr,
        format!(
            "rsmp error [{}]: {}",
            SampleArchiveErrorCode::CircuitMismatch.as_str(),
            expected_detail(SampleArchiveErrorCode::CircuitMismatch)
        )
    );
    snapshots += 1;

    snapshots
}

fn verify_precedence_controls() {
    let circuit = parse("M 0\n");
    let measurements = patterned_table(1, 1);
    let archive = archive_from_measurements(&circuit, &measurements, limits());

    let mut bad_digest_and_bad_dimension = archive.clone();
    put_u64(&mut bad_digest_and_bad_dimension, 40, 0);
    let bad_header_err = match SampleArchiveReader::open(
        ShortRead::new(&bad_digest_and_bad_dimension, 3),
        &circuit,
        limits(),
    ) {
        Ok(_) => panic!("header checksum precedes dimensions"),
        Err(err) => err,
    };
    assert_eq!(
        bad_header_err.code(),
        SampleArchiveErrorCode::ChecksumMismatch
    );

    let mut multi_frame_and_wrong_content_size = archive.clone();
    let free = free_stream_range(&multi_frame_and_wrong_content_size);
    let original = multi_frame_and_wrong_content_size[free.clone()].to_vec();
    multi_frame_and_wrong_content_size.splice(free.end..free.end, original.iter().copied());
    put_u64(&mut multi_frame_and_wrong_content_size, GLOBAL_HEADER_LEN + 60, 2);
    put_u64(
        &mut multi_frame_and_wrong_content_size,
        GLOBAL_HEADER_LEN + 68,
        (original.len() * 2) as u64,
    );
    let mut reader = SampleArchiveReader::open(
        ShortRead::new(&multi_frame_and_wrong_content_size, 7),
        &circuit,
        limits(),
    )
    .expect("open multi-frame precedence archive");
    assert_eq!(
        reader
            .next_block()
            .expect_err("frame multiplicity precedes content size mismatch")
            .code(),
        SampleArchiveErrorCode::MalformedArchive
    );

    let mut multi_frame_and_bad_checksum = archive.clone();
    let free = free_stream_range(&multi_frame_and_bad_checksum);
    let original = multi_frame_and_bad_checksum[free.clone()].to_vec();
    multi_frame_and_bad_checksum.splice(free.end..free.end, original.iter().copied());
    put_u64(
        &mut multi_frame_and_bad_checksum,
        GLOBAL_HEADER_LEN + 68,
        (original.len() * 2) as u64,
    );
    multi_frame_and_bad_checksum[free.end - 1] ^= 0x80;
    let mut reader = SampleArchiveReader::open(
        ShortRead::new(&multi_frame_and_bad_checksum, 7),
        &circuit,
        limits(),
    )
    .expect("open multi-frame checksum precedence archive");
    assert_eq!(
        reader
            .next_block()
            .expect_err("frame multiplicity precedes checksum failure")
            .code(),
        SampleArchiveErrorCode::MalformedArchive
    );

    let mut window_and_wrong_content_size = archive.clone();
    put_u64(&mut window_and_wrong_content_size, GLOBAL_HEADER_LEN + 60, 2);
    let mut tiny_window = limits();
    tiny_window.max_zstd_window_bytes = 0;
    let mut reader = SampleArchiveReader::open(
        ShortRead::new(&window_and_wrong_content_size, 7),
        &circuit,
        tiny_window,
    )
    .expect("open zstd window precedence archive");
    assert_eq!(
        reader
            .next_block()
            .expect_err("zstd window limit precedes content size mismatch")
            .code(),
        SampleArchiveErrorCode::LimitExceeded
    );

    let mut memory_and_wrong_content_size = archive.clone();
    put_u64(&mut memory_and_wrong_content_size, GLOBAL_HEADER_LEN + 60, 2);
    let mut tiny_memory = limits();
    tiny_memory.max_zstd_decoder_memory_bytes = 1;
    let mut reader = SampleArchiveReader::open(
        ShortRead::new(&memory_and_wrong_content_size, 7),
        &circuit,
        tiny_memory,
    )
    .expect("open zstd memory precedence archive");
    assert_eq!(
        reader
            .next_block()
            .expect_err("zstd memory limit precedes content size mismatch")
            .code(),
        SampleArchiveErrorCode::LimitExceeded
    );
}

fn cli_case_archives(archive: &[u8]) -> Vec<(SampleArchiveErrorCode, Vec<u8>)> {
    let mut cases = Vec::new();

    let mut bad_magic = archive.to_vec();
    bad_magic[0] = b'X';
    cases.push((SampleArchiveErrorCode::BadMagic, bad_magic));

    let mut bad_version = archive.to_vec();
    put_u16(&mut bad_version, 8, FORMAT_MAJOR + 1);
    cases.push((SampleArchiveErrorCode::UnsupportedVersion, bad_version));

    let mut unsupported_feature = archive.to_vec();
    put_u32(&mut unsupported_feature, 16, 1);
    cases.push((
        SampleArchiveErrorCode::UnsupportedFeature,
        unsupported_feature,
    ));

    let mut shape = archive.to_vec();
    put_u64(&mut shape, 48, 2);
    recompute_header_digest(&mut shape);
    cases.push((SampleArchiveErrorCode::ShapeMismatch, shape));

    let mut limit = archive.to_vec();
    put_u64(&mut limit, GLOBAL_HEADER_LEN + 68, u64::MAX);
    cases.push((SampleArchiveErrorCode::LimitExceeded, limit));

    let truncated = archive[..archive.len() - 3].to_vec();
    cases.push((SampleArchiveErrorCode::Truncated, truncated));

    let mut malformed = archive.to_vec();
    put_u64(&mut malformed, GLOBAL_HEADER_LEN + 12, 1);
    cases.push((SampleArchiveErrorCode::MalformedArchive, malformed));

    let mut decompression = archive.to_vec();
    let free = free_stream_range(&decompression);
    decompression[free.end - 1] ^= 0x80;
    cases.push((SampleArchiveErrorCode::DecompressionFailed, decompression));

    let mut checksum = archive.to_vec();
    checksum[GLOBAL_HEADER_LEN - 1] ^= 0x55;
    cases.push((SampleArchiveErrorCode::ChecksumMismatch, checksum));

    let mut logical = archive.to_vec();
    logical[GLOBAL_HEADER_LEN + 76] ^= 0x55;
    cases.push((SampleArchiveErrorCode::LogicalDigestMismatch, logical));

    let mut trailing = archive.to_vec();
    trailing.push(0);
    cases.push((SampleArchiveErrorCode::TrailingData, trailing));

    cases
}

fn expected_detail(code: SampleArchiveErrorCode) -> &'static str {
    match code {
        SampleArchiveErrorCode::BadMagic => "invalid record magic",
        SampleArchiveErrorCode::UnsupportedVersion => "unsupported format version",
        SampleArchiveErrorCode::UnsupportedFeature => "unknown required feature",
        SampleArchiveErrorCode::UnsupportedSweep => "sweep-bit circuits are not supported",
        SampleArchiveErrorCode::CircuitMismatch => {
            "archive circuit fingerprint does not match supplied circuit"
        }
        SampleArchiveErrorCode::ShapeMismatch => {
            "archive transform identity does not match supplied circuit"
        }
        SampleArchiveErrorCode::LimitExceeded => "compressed frame exceeds limit",
        SampleArchiveErrorCode::Truncated => "truncated archive",
        SampleArchiveErrorCode::MalformedArchive => "invalid block sequence number",
        SampleArchiveErrorCode::DecompressionFailed => "zstd decompression failed",
        SampleArchiveErrorCode::ChecksumMismatch => "global header digest mismatch",
        SampleArchiveErrorCode::LogicalDigestMismatch => "logical payload digest mismatch",
        SampleArchiveErrorCode::TrailingData => "archive has trailing data",
        SampleArchiveErrorCode::Io => "archive I/O failed",
    }
}

fn expect_writer_new_limit(circuit: &[StimInstr], measurements: &BitTable, limits: ArchiveLimits) {
    let transform =
        MeasurementTransform::from_circuit_with_limits(circuit, generous_transform_limits())
            .expect("writer limit transform");
    let result = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits,
    )
    .and_then(|mut writer| {
        writer.write_measurements(measurements)?;
        writer.finish()
    });
    assert_eq!(
        result.expect_err("expected writer limit").code(),
        SampleArchiveErrorCode::LimitExceeded
    );
}

fn expect_writer_or_reader_limit(
    circuit: &[StimInstr],
    measurements: &BitTable,
    base: ArchiveLimits,
    mutate: impl FnOnce(&mut ArchiveLimits),
) {
    let mut limits = base;
    mutate(&mut limits);
    let transform =
        MeasurementTransform::from_circuit_with_limits(circuit, generous_transform_limits())
            .expect("limit transform");
    let result = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits,
    )
    .and_then(|mut writer| {
        writer.write_measurements(measurements)?;
        writer.finish()
    });
    if let Err(err) = result {
        assert_eq!(err.code(), SampleArchiveErrorCode::LimitExceeded);
        return;
    }
    let archive = archive_from_measurements(circuit, measurements, base);
    expect_reader_next_limit(&archive, circuit, limits);
}

fn expect_reader_next_limit(archive: &[u8], circuit: &[StimInstr], limits: ArchiveLimits) {
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 3), circuit, limits).expect("open");
    let err = reader.next_block().expect_err("expected next_block limit");
    assert_eq!(err.code(), SampleArchiveErrorCode::LimitExceeded);
}

fn exact_transform_limits(
    transform: &MeasurementTransform,
    shots: usize,
) -> MeasurementTransformLimits {
    MeasurementTransformLimits {
        max_measurements: transform.num_measurements() as u64,
        max_detectors: transform.num_detectors() as u64,
        max_observables: transform.num_observables() as u64,
        max_repeat_depth: transform.max_repeat_depth(),
        max_expanded_instructions: transform.expanded_instructions(),
        max_parity_terms: transform.parity_terms(),
        max_shots_per_block: shots as u64,
        max_transform_working_bytes: transform.transform_working_bytes(),
        max_block_working_bytes: transform
            .estimate_block_working_bytes(shots)
            .expect("block working estimate"),
    }
}

fn lower_nested(
    mut limits: ArchiveLimits,
    select: impl FnOnce(&mut MeasurementTransformLimits) -> &mut u64,
) -> ArchiveLimits {
    let value = select(&mut limits.transform);
    *value = value.saturating_sub(1);
    limits
}

fn zero_nested(
    mut limits: ArchiveLimits,
    select: impl FnOnce(&mut MeasurementTransformLimits) -> &mut u64,
) -> ArchiveLimits {
    *select(&mut limits.transform) = 0;
    limits
}

fn archive_from_measurements(
    circuit: &[StimInstr],
    measurements: &BitTable,
    limits: ArchiveLimits,
) -> Vec<u8> {
    let transform =
        MeasurementTransform::from_circuit_with_limits(circuit, generous_transform_limits())
            .expect("archive transform");
    let mut writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits,
    )
    .expect("archive writer");
    if measurements.num_minor() > 0 {
        writer
            .write_measurements(measurements)
            .expect("write archive");
    }
    writer.finish().expect("finish archive").into_inner()
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

fn generous_transform_limits() -> MeasurementTransformLimits {
    MeasurementTransformLimits {
        max_measurements: 1_000,
        max_detectors: 1_000,
        max_observables: 128,
        max_repeat_depth: 64,
        max_expanded_instructions: 10_000,
        max_parity_terms: 10_000,
        max_shots_per_block: 1_000,
        max_transform_working_bytes: 1 << 24,
        max_block_working_bytes: 1 << 24,
    }
}

fn patterned_table(bits: usize, shots: usize) -> BitTable {
    let mut table = BitTable::try_new(bits, shots).expect("patterned table");
    for bit in 0..bits {
        for shot in 0..shots {
            if (bit + shot) % 2 == 1 {
                table.set(bit, shot, true);
            }
        }
    }
    table
}

fn assert_code<T: std::fmt::Debug>(
    result: Result<T, rstim::sample_archive::format::SampleArchiveError>,
    code: SampleArchiveErrorCode,
) {
    assert_eq!(
        result.expect_err("expected sample archive error").code(),
        code
    );
}

fn assert_transform_limit<T: std::fmt::Debug>(result: Result<T, MeasurementTransformError>) {
    assert!(matches!(
        result.expect_err("expected transform limit"),
        MeasurementTransformError::LimitExceeded { .. } | MeasurementTransformError::Allocation(_)
    ));
}

fn free_stream_range(archive: &[u8]) -> std::ops::Range<usize> {
    let syndrome_start = GLOBAL_HEADER_LEN + BLOCK_HEADER_LEN;
    let syndrome_len = get_u64(archive, GLOBAL_HEADER_LEN + 52) as usize;
    let free_len = get_u64(archive, GLOBAL_HEADER_LEN + 68) as usize;
    let start = syndrome_start + syndrome_len;
    start..start + free_len
}

fn block_compressed_bytes(archive: &[u8]) -> u64 {
    get_u64(archive, GLOBAL_HEADER_LEN + 52) + get_u64(archive, GLOBAL_HEADER_LEN + 68)
}

fn recompute_header_digest(archive: &mut [u8]) {
    let digest: [u8; 32] = Sha256::digest(&archive[..GLOBAL_HEADER_LEN - 32]).into();
    archive[GLOBAL_HEADER_LEN - 32..GLOBAL_HEADER_LEN].copy_from_slice(&digest);
}

fn parse(text: &str) -> Vec<StimInstr> {
    parse_lines(text).unwrap_or_else(|err| panic!("parse failed: {err}"))
}

fn unpack_stderr(circuit: &Path, archive: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "unpack_samples",
            "--circuit",
            &circuit.display().to_string(),
            "--in",
            &archive.display().to_string(),
            "--measurements_out",
            "-",
            "--measurements_out_format",
            "b8",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run unpack_samples");
    assert_failure(&output);
    stderr_line(&output)
}

fn pack_stderr(circuit: &Path, input: &Path, output_path: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "pack_samples",
            "--circuit",
            &circuit.display().to_string(),
            "--shots",
            "1",
            "--in",
            &input.display().to_string(),
            "--in_format",
            "b8",
            "--out",
            &output_path.display().to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run pack_samples");
    assert_failure(&output);
    stderr_line(&output)
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded with stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn stderr_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
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

struct PayloadPanicRead<'a> {
    bytes: &'a [u8],
}

impl<'a> PayloadPanicRead<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl Read for PayloadPanicRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            panic!("reader touched declared frame payload after oversized header");
        }
        let n = out.len().min(self.bytes.len());
        out[..n].copy_from_slice(&self.bytes[..n]);
        self.bytes = &self.bytes[n..];
        Ok(n)
    }
}
