use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use rstim::measurement_transform::MeasurementTransform;
use rstim::parser::parse_lines;
use rstim::sample_archive::format::{
    GLOBAL_HEADER_LEN, STREAM_CODEC_EMPTY, STREAM_CODEC_SYNDROME_DENSE_V1,
    STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1, SampleArchiveError, SampleArchiveErrorCode,
};
use rstim::sample_archive::syndrome::{
    decode_syndrome_raw, encode_syndrome, for_each_sparse_syndrome_hit,
    max_materialized_candidates, reset_materialization_telemetry,
};
use rstim::sample_archive::{
    ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter,
};
use rstim::sim::bit_table::BitTable;

#[test]
fn rsmp_adaptive_codec_contract() {
    let known_cases = verify_known_cases();
    assert_eq!(known_cases, 3);
    let uleb_boundaries = verify_uleb_boundaries();
    assert_eq!(uleb_boundaries, 4);
    let malformed_cases = verify_malformed_cases();
    assert_eq!(malformed_cases, 11);
    let property_cases = verify_property_cases();
    assert_eq!(property_cases, 4096);
    let max_materialized_candidates = max_materialized_candidates();
    assert_eq!(max_materialized_candidates, 1);
    println!(
        "PASS rsmp adaptive codec known_cases=3 uleb_boundaries=4 malformed_cases=11 property_cases=4096 max_materialized_candidates=1"
    );
}

fn verify_known_cases() -> usize {
    reset_materialization_telemetry();

    let zero = BitTable::new(12_000, 1);
    assert_eq!(
        encode_syndrome(&zero).unwrap().codec_id,
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1
    );
    verify_sparse_archive_round_trip();
    verify_writer_limit_rejects_before_syndrome_materialization();

    let mut all_one = BitTable::new(8, 1);
    for detector in 0..8 {
        all_one.set(detector, 0, true);
    }
    let dense = encode_syndrome(&all_one).unwrap();
    assert_eq!(dense.codec_id, STREAM_CODEC_SYNDROME_DENSE_V1);
    assert_eq!(dense.raw, [0xff]);

    let tie = BitTable::new(8, 1);
    let tie_encoding = encode_syndrome(&tie).unwrap();
    assert_eq!(tie_encoding.codec_id, STREAM_CODEC_SYNDROME_DENSE_V1);
    assert_eq!(tie_encoding.raw, [0x00]);

    let mut shots = BitTable::new(3, 3);
    for (shot, hits) in [&[0, 2][..], &[1][..], &[0, 1][..]].into_iter().enumerate() {
        for detector in hits {
            shots.set(*detector, shot, true);
        }
    }
    let shot_encoding = encode_syndrome(&shots).unwrap();
    assert_eq!(shot_encoding.codec_id, STREAM_CODEC_SYNDROME_DENSE_V1);
    assert_eq!(shot_encoding.raw, [0xd5, 0x00]);

    let mut sparse = BitTable::new(200, 1);
    for detector in [0, 128, 199] {
        sparse.set(detector, 0, true);
    }
    let sparse_encoding = encode_syndrome(&sparse).unwrap();
    assert_eq!(
        sparse_encoding.codec_id,
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1
    );
    assert_eq!(sparse_encoding.raw, [0x03, 0x00, 0x7f, 0x46]);
    3
}

fn verify_sparse_archive_round_trip() {
    let circuit_text = sparse_archive_circuit(12_000);
    let circuit = parse_lines(&circuit_text).expect("parse sparse archive circuit");
    let transform = MeasurementTransform::from_circuit(&circuit).expect("sparse archive transform");
    assert_eq!(transform.rank(), 12_000);
    let measurements = BitTable::new(transform.num_measurements(), 1);

    let mut writer = SampleArchiveWriter::new(
        Vec::new(),
        transform,
        1,
        SampleArchiveOptions::default(),
        ArchiveLimits::default(),
    )
    .expect("sparse archive writer");
    writer
        .write_measurements(&measurements)
        .expect("write sparse archive measurements");
    let archive = writer.finish().expect("finish sparse archive");
    assert_eq!(
        get_u16(&archive, GLOBAL_HEADER_LEN + 36),
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1
    );

    let mut reader = SampleArchiveReader::open(
        std::io::Cursor::new(&archive),
        &circuit,
        ArchiveLimits::default(),
    )
    .expect("open sparse archive");
    let decoded = reader
        .next_block()
        .expect("read sparse archive block")
        .expect("sparse archive block exists");
    reader.finish().expect("finish sparse archive reader");
    assert_tables_eq(&decoded.measurements, &measurements);
}

fn verify_writer_limit_rejects_before_syndrome_materialization() {
    let circuit_text = sparse_archive_circuit(9);
    let circuit = parse_lines(&circuit_text).expect("parse limit circuit");
    let transform = MeasurementTransform::from_circuit(&circuit).expect("limit transform");
    let measurements = BitTable::new(transform.num_measurements(), 1);
    let mut limits = ArchiveLimits::default();
    limits.max_decompressed_bytes_per_frame = 0;
    limits.max_decompressed_bytes_per_archive = 0;
    reset_materialization_telemetry();

    let mut writer = SampleArchiveWriter::new(
        Vec::new(),
        transform,
        1,
        SampleArchiveOptions::default(),
        limits,
    )
    .expect("limit writer");
    writer
        .write_measurements(&measurements)
        .expect("buffer final short block before limit check");
    let err = writer.finish().unwrap_err();
    assert_eq!(err.code(), SampleArchiveErrorCode::LimitExceeded);
    assert_eq!(max_materialized_candidates(), 0);
}

fn verify_uleb_boundaries() -> usize {
    for detector in [127, 128, 16_383, 16_384] {
        let mut table = BitTable::new(detector + 1, 1);
        table.set(detector, 0, true);
        let encoded = encode_syndrome(&table).unwrap();
        assert_eq!(encoded.codec_id, STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1);
        let decoded = decode_syndrome_raw(
            encoded.codec_id,
            encoded.raw_len,
            &encoded.raw,
            detector + 1,
            1,
        )
        .unwrap();
        assert_tables_eq(&decoded, &table);
    }
    4
}

fn verify_malformed_cases() -> usize {
    let mut delta_overflow = vec![0x02];
    delta_overflow.extend(uleb(u64::MAX - 1));
    delta_overflow.push(0x01);

    let sparse_malformed = [
        ("noncanonical ULEB zero", vec![0x80, 0x00], 1u64, 1u64),
        ("unterminated ULEB", vec![0x80], 1, 1),
        (
            "ULEB overflow",
            {
                let mut bytes = vec![0xff; 9];
                bytes.push(0x02);
                bytes
            },
            1,
            1,
        ),
        ("count greater than R", vec![0x02], 1, 1),
        ("incomplete hit list", vec![0x01], 2, 1),
        (
            "checked delta-addition overflow",
            delta_overflow,
            u64::MAX,
            1,
        ),
        ("reconstructed index equal to R", vec![0x01, 0x02], 2, 1),
        ("fewer than S shot records", vec![0x00], 1, 2),
        ("bytes after the Sth shot", vec![0x00, 0x00], 1, 1),
    ];
    for (name, raw, rows, shots) in sparse_malformed {
        expect_sparse_malformed_without_callback(name, &raw, rows, shots);
    }

    expect_malformed(
        "declared raw-length mismatch",
        decode_syndrome_raw(STREAM_CODEC_SYNDROME_DENSE_V1, 2, &[0x00], 8, 1),
    );
    expect_malformed(
        "nonzero dense final padding",
        decode_syndrome_raw(STREAM_CODEC_SYNDROME_DENSE_V1, 1, &[0x08], 3, 1),
    );
    11
}

fn expect_sparse_malformed_without_callback(name: &str, raw: &[u8], rows: u64, shots: u64) {
    expect_malformed(
        name,
        for_each_sparse_syndrome_hit(raw, raw.len() as u64, rows, shots, |_, _| {
            panic!("{name}: reconstruction callback must not be invoked for malformed sparse data")
        }),
    );
}

fn expect_malformed<T: std::fmt::Debug>(name: &str, result: Result<T, SampleArchiveError>) {
    assert_eq!(
        result.unwrap_err().code(),
        SampleArchiveErrorCode::MalformedArchive,
        "{name}"
    );
}

fn verify_property_cases() -> usize {
    let mut rng = StdRng::seed_from_u64(0x5253_4d50_0525_0001);
    expect_malformed(
        "noncanonical empty stream",
        decode_syndrome_raw(STREAM_CODEC_EMPTY, 1, &[0x00], 0, 1),
    );
    let mut saw_zero_rows = false;
    let mut saw_zero_shots = false;
    let mut saw_unaligned = false;
    let mut saw_sparse = false;
    let mut saw_dense = false;
    for case in 0..4096 {
        let rows = match case % 16 {
            0 => 0,
            1 => 1,
            2 => 3,
            3 => 7,
            4 => 8,
            5 => 9,
            _ => rng.gen_range(0..=40),
        };
        let shots = match case % 16 {
            0 => 0,
            1 => 1,
            2 => 3,
            3 => 7,
            4 => 8,
            5 => 9,
            _ => rng.gen_range(0..=12),
        };
        let mut table = BitTable::new(rows, shots);
        for detector in 0..rows {
            for shot in 0..shots {
                table.set(
                    detector,
                    shot,
                    rng.gen_bool(if case % 3 == 0 { 0.05 } else { 0.5 }),
                );
            }
        }
        let encoded = encode_syndrome(&table).unwrap();
        let decoded =
            decode_syndrome_raw(encoded.codec_id, encoded.raw_len, &encoded.raw, rows, shots)
                .unwrap();
        assert_tables_eq_with_context(&decoded, &table, case);
        saw_zero_rows |= rows == 0;
        saw_zero_shots |= shots == 0;
        saw_unaligned |= rows.checked_mul(shots).is_some_and(|bits| bits % 8 != 0);
        saw_sparse |= encoded.codec_id == STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1;
        saw_dense |= encoded.codec_id == STREAM_CODEC_SYNDROME_DENSE_V1;
        if rows == 0 || shots == 0 {
            assert_eq!(encoded.codec_id, STREAM_CODEC_EMPTY);
            assert!(encoded.raw.is_empty());
        }
    }
    assert!(saw_zero_rows);
    assert!(saw_zero_shots);
    assert!(saw_unaligned);
    assert!(saw_sparse);
    assert!(saw_dense);
    4096
}

fn assert_tables_eq(left: &BitTable, right: &BitTable) {
    assert_tables_eq_with_context(left, right, usize::MAX);
}

fn assert_tables_eq_with_context(left: &BitTable, right: &BitTable, case: usize) {
    assert_eq!(left.num_major(), right.num_major(), "case {case}: rows");
    assert_eq!(left.num_minor(), right.num_minor(), "case {case}: shots");
    for row in 0..left.num_major() {
        for shot in 0..left.num_minor() {
            assert_eq!(
                left.get(row, shot),
                right.get(row, shot),
                "case {case}: bit[{row},{shot}]"
            );
        }
    }
}

fn sparse_archive_circuit(measurements: usize) -> String {
    let mut text = String::new();
    text.push('M');
    for measurement in 0..measurements {
        text.push(' ');
        text.push_str(&measurement.to_string());
    }
    text.push('\n');
    for measurement in 0..measurements {
        text.push_str("DETECTOR rec[-");
        text.push_str(&(measurements - measurement).to_string());
        text.push_str("]\n");
    }
    text
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn uleb(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            return bytes;
        }
    }
}
