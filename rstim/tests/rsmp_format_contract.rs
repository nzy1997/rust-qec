use rstim::sample_archive::format::*;

const GLOBAL_VECTOR: [u8; GLOBAL_HEADER_LEN] = [
    0x52, 0x53, 0x54, 0x4d, 0x53, 0x4d, 0x50, 0x00, 0x01, 0x00, 0x00, 0x00, 0x98, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
    0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
];

const BLOCK_VECTOR: [u8; BLOCK_HEADER_LEN] = [
    0x52, 0x53, 0x4d, 0x50, 0x42, 0x4c, 0x4b, 0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x07, 0x06, 0x05,
    0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11, 0x21, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0xc1, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xcb, 0xcc, 0xcd, 0xce, 0xcf, 0xd0, 0xd1, 0xd2, 0xd3,
    0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xdf,
];

const TRAILER_VECTOR: [u8; ARCHIVE_TRAILER_LEN] = [
    0x52, 0x53, 0x4d, 0x50, 0x45, 0x4e, 0x44, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
];

fn expect_code<T>(result: Result<T, SampleArchiveError>, code: SampleArchiveErrorCode) {
    assert_eq!(
        result.err().expect("expected sample archive error").code(),
        code
    );
}

#[test]
fn rsmp_format_contract_known_vectors_and_negative_cases() {
    assert_eq!(
        GlobalHeader::known_vector_v1().to_bytes().unwrap(),
        GLOBAL_VECTOR
    );
    assert_eq!(
        GlobalHeader::from_bytes(&GLOBAL_VECTOR).unwrap(),
        GlobalHeader::known_vector_v1()
    );
    assert_eq!(
        BlockHeader::known_vector_v1().to_bytes().unwrap(),
        BLOCK_VECTOR
    );
    assert_eq!(
        BlockHeader::from_bytes(&BLOCK_VECTOR).unwrap(),
        BlockHeader::known_vector_v1()
    );
    assert_eq!(
        ArchiveTrailer::known_vector_v1().to_bytes().unwrap(),
        TRAILER_VECTOR
    );
    assert_eq!(
        ArchiveTrailer::from_bytes(&TRAILER_VECTOR).unwrap(),
        ArchiveTrailer::known_vector_v1()
    );

    assert_eq!(
        GLOBAL_FIELDS
            .iter()
            .map(|f| (f.name, f.offset, f.width))
            .collect::<Vec<_>>(),
        vec![
            ("magic", 0, 8),
            ("format_major", 8, 2),
            ("format_minor", 10, 2),
            ("header_len", 12, 4),
            ("required_flags", 16, 4),
            ("optional_flags", 20, 4),
            ("reserved_flags", 24, 4),
            ("canonicalization_id", 28, 2),
            ("fingerprint_id", 30, 2),
            ("transform_id", 32, 2),
            ("reference_id", 34, 2),
            ("codec_suite_id", 36, 2),
            ("reserved0", 38, 2),
            ("max_shots_per_block", 40, 8),
            ("measurement_count", 48, 8),
            ("detector_count", 56, 8),
            ("observable_count", 64, 8),
            ("detector_rank", 72, 8),
            ("total_shots", 80, 8),
            ("circuit_sha256", 88, 32),
            ("header_sha256", 120, 32),
        ]
    );
    assert_eq!(
        BLOCK_FIELDS
            .iter()
            .map(|f| (f.name, f.offset, f.width))
            .collect::<Vec<_>>(),
        vec![
            ("magic", 0, 8),
            ("format_major", 8, 2),
            ("format_minor", 10, 2),
            ("block_index", 12, 8),
            ("first_shot", 20, 8),
            ("shot_count", 28, 8),
            ("syndrome_codec_id", 36, 2),
            ("free_codec_id", 38, 2),
            ("reserved0", 40, 4),
            ("syndrome_uncompressed_len", 44, 8),
            ("syndrome_compressed_len", 52, 8),
            ("free_uncompressed_len", 60, 8),
            ("free_compressed_len", 68, 8),
            ("logical_payload_sha256", 76, 32),
        ]
    );
    assert_eq!(
        TRAILER_FIELDS
            .iter()
            .map(|f| (f.name, f.offset, f.width))
            .collect::<Vec<_>>(),
        vec![
            ("magic", 0, 8),
            ("format_major", 8, 2),
            ("format_minor", 10, 2),
            ("reserved0", 12, 4),
            ("block_count", 16, 8),
            ("total_shots", 24, 8),
            ("archive_sha256", 32, 32),
        ]
    );

    assert_eq!(&GLOBAL_VECTOR[40..48], &4096u64.to_le_bytes());
    assert_eq!(&GLOBAL_VECTOR[48..56], &258u64.to_le_bytes());
    assert_eq!(
        &BLOCK_VECTOR[12..20],
        &0x0102_0304_0506_0708u64.to_le_bytes()
    );
    assert_eq!(
        &BLOCK_VECTOR[20..28],
        &0x1112_1314_1516_1718u64.to_le_bytes()
    );

    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 0, 0x00)),
        SampleArchiveErrorCode::BadMagic,
    );
    expect_code(
        BlockHeader::from_bytes(&mutated(&BLOCK_VECTOR, 0, 0x00)),
        SampleArchiveErrorCode::BadMagic,
    );
    expect_code(
        ArchiveTrailer::from_bytes(&mutated(&TRAILER_VECTOR, 0, 0x00)),
        SampleArchiveErrorCode::BadMagic,
    );
    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 8, 0x02)),
        SampleArchiveErrorCode::UnsupportedVersion,
    );
    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 10, 0x01)),
        SampleArchiveErrorCode::UnsupportedVersion,
    );
    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 16, 0x01)),
        SampleArchiveErrorCode::UnsupportedFeature,
    );
    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 24, 0x01)),
        SampleArchiveErrorCode::MalformedArchive,
    );
    expect_code(
        GlobalHeader::from_bytes(&mutated(&GLOBAL_VECTOR, 38, 0x01)),
        SampleArchiveErrorCode::MalformedArchive,
    );
    expect_code(
        BlockHeader::from_bytes(&mutated(&BLOCK_VECTOR, 36, 0xff)),
        SampleArchiveErrorCode::MalformedArchive,
    );
    expect_code(
        GlobalHeader::from_bytes(&GLOBAL_VECTOR[..GLOBAL_HEADER_LEN - 1]),
        SampleArchiveErrorCode::Truncated,
    );
    expect_code(
        BlockHeader::from_bytes(&BLOCK_VECTOR[..BLOCK_HEADER_LEN - 1]),
        SampleArchiveErrorCode::Truncated,
    );
    expect_code(
        ArchiveTrailer::from_bytes(&TRAILER_VECTOR[..ARCHIVE_TRAILER_LEN - 1]),
        SampleArchiveErrorCode::Truncated,
    );

    let mut undersized = GLOBAL_VECTOR;
    undersized[12..16].copy_from_slice(&(GLOBAL_HEADER_LEN as u32 - 1).to_le_bytes());
    expect_code(
        GlobalHeader::from_bytes(&undersized),
        SampleArchiveErrorCode::MalformedArchive,
    );
    let mut impossible = GLOBAL_VECTOR;
    impossible[12..16].copy_from_slice(&(MAX_GLOBAL_HEADER_LEN + 1).to_le_bytes());
    expect_code(
        GlobalHeader::from_bytes(&impossible),
        SampleArchiveErrorCode::MalformedArchive,
    );

    let zero_shot = [GLOBAL_VECTOR.as_slice(), TRAILER_VECTOR.as_slice()].concat();
    assert_eq!(zero_shot.len(), GLOBAL_HEADER_LEN + ARCHIVE_TRAILER_LEN);
    assert!(!zero_shot
        .windows(BLOCK_MAGIC.len())
        .any(|w| w == BLOCK_MAGIC));
    assert_eq!(
        GlobalHeader::from_bytes(&zero_shot[..GLOBAL_HEADER_LEN])
            .unwrap()
            .total_shots,
        0
    );
    assert_eq!(
        ArchiveTrailer::from_bytes(&zero_shot[GLOBAL_HEADER_LEN..])
            .unwrap()
            .block_count,
        0
    );
    assert_eq!(
        ArchiveTrailer::from_bytes(&zero_shot[GLOBAL_HEADER_LEN..])
            .unwrap()
            .total_shots,
        0
    );

    assert_eq!(checked_dense_bit_bytes(0, 33).unwrap(), 0);
    assert_eq!(checked_dense_bit_bytes(9, 3).unwrap(), 4);
    assert_eq!(checked_logical_payload_bytes(5, 4, 3).unwrap(), 4);
    expect_code(
        checked_logical_payload_bytes(u64::MAX, 1, 2),
        SampleArchiveErrorCode::LimitExceeded,
    );

    println!("PASS rsmp format contract v=1.0 known_vectors=3 negative_cases=12");
}

fn mutated<const N: usize>(input: &[u8; N], offset: usize, value: u8) -> [u8; N] {
    let mut bytes = *input;
    bytes[offset] = value;
    bytes
}
