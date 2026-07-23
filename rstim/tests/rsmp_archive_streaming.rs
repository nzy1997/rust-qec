use rstim::ir::StimInstr;
use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::{MeasurementTransform, MeasurementTransformLimits};
use rstim::output::{read_shots_b8, write_shots_b8};
use rstim::parser::parse_lines;
use rstim::sample_archive::format::{
    ARCHIVE_TRAILER_LEN, ArchiveTrailer, BLOCK_FIELDS, BLOCK_HEADER_LEN, BLOCK_MAGIC, BlockHeader,
    CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1, CODEC_SUITE_ZSTD_FRAMES_V1,
    FINGERPRINT_SHA256_CANONICAL_CIRCUIT, GLOBAL_HEADER_LEN, GlobalHeader,
    REFERENCE_SIMULATE_NOISELESS, STREAM_CODEC_EMPTY, STREAM_CODEC_FREE_DENSE_V1,
    SampleArchiveErrorCode, TRAILER_FIELDS, TRAILER_MAGIC,
    TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
};
use rstim::sample_archive::syndrome::encode_syndrome;
use rstim::sample_archive::telemetry::{
    archive_telemetry, diagnostic_lines, reset_archive_telemetry,
};
use rstim::sample_archive::{
    ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter,
};
use rstim::sim::bit_table::BitTable;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

const MAX_BLOCK_SHOTS: usize = 4096;

#[test]
fn rsmp_archive_streaming_contract() {
    verify_default_block_limit();
    verify_zero_shot_archive();
    verify_writer_emits_full_blocks_before_finish();
    verify_reader_accepts_short_interior_blocks();
    let boundary_cases = verify_boundary_cases();
    assert_eq!(boundary_cases, 4);
    let partition_invariant = verify_partition_invariant();
    assert_eq!(partition_invariant, 1);
    let malformed_cases = verify_malformed_cases();
    assert_eq!(malformed_cases, 10);
    let memory = verify_bounded_memory();
    assert_eq!(memory.max_buffered_shots, MAX_BLOCK_SHOTS as u64);
    assert_eq!(memory.max_live_decoded_blocks, 1);
    assert_eq!(memory.max_transform_payloads, 2);
    assert_eq!(memory.total_block_growth_bytes, 0);
    for line in diagnostic_lines() {
        println!("DIAG rsmp memory {line}");
    }
    println!(
        "PASS rsmp streaming boundary_cases=4 partition_invariant=1 malformed_cases=10 max_buffered_shots=4096 max_live_decoded_blocks=1 max_transform_payloads=2 total_block_growth_bytes=0"
    );
}

fn verify_default_block_limit() {
    assert_eq!(
        ArchiveLimits::default().transform.max_shots_per_block,
        MAX_BLOCK_SHOTS as u64
    );
}

fn verify_boundary_cases() -> usize {
    let fixture = CatalogFixture::load("known_mpad_multi");
    let mut cases = 0;
    for (shots, expected_blocks) in [(4095, 1), (4096, 1), (4097, 2), (8193, 3)] {
        let measurements = repeated_table(&fixture.measurements, shots);
        let expected_detections = repeated_table(&fixture.detections, shots);
        let expected_observables = repeated_table(&fixture.observables, shots);
        let m2d = measurements_to_detections(&fixture.circuit, &measurements)
            .expect("catalog measurements convert through m2d");
        assert_tables_eq(
            &expected_detections,
            &m2d.detections,
            "catalog",
            "detections",
        );
        assert_tables_eq(
            &expected_observables,
            &m2d.observable_flips,
            "catalog",
            "observables",
        );
        let archive = archive_from_partitions(&fixture.circuit, &measurements, &[shots]);
        assert_eq!(
            block_layouts(&archive).len(),
            expected_blocks,
            "{shots} shot block count"
        );
        let decoded = decode_archive(&archive, &fixture.circuit, shots, expected_blocks as u64);
        assert_tables_eq(
            &measurements,
            &decoded.measurements,
            "boundary",
            "measurements",
        );
        assert_tables_eq(
            &expected_detections,
            &decoded.detections,
            "boundary",
            "detections",
        );
        assert_tables_eq(
            &expected_observables,
            &decoded.observable_flips,
            "boundary",
            "observables",
        );
        cases += 1;
    }
    cases
}

fn verify_writer_emits_full_blocks_before_finish() {
    let fixture = CatalogFixture::load("known_mpad_multi");
    let measurements = repeated_table(&fixture.measurements, 8193);
    let sink = SharedWriter::default();
    let observed = sink.bytes();
    let transform = MeasurementTransform::from_circuit(&fixture.circuit).expect("emit transform");
    let mut writer = SampleArchiveWriter::new(
        sink,
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("emit writer");
    assert_eq!(observed.borrow().len(), GLOBAL_HEADER_LEN);

    writer
        .write_measurements(&slice_table(&measurements, 0, 4095))
        .expect("write partial block");
    assert_eq!(
        complete_blocks_in_prefix(&observed.borrow()),
        0,
        "partial chunk must not emit a short interior block"
    );

    writer
        .write_measurements(&slice_table(&measurements, 4095, 1))
        .expect("write chunk crossing first boundary");
    assert_eq!(
        complete_blocks_in_prefix(&observed.borrow()),
        1,
        "first full block must be emitted before finish"
    );

    writer
        .write_measurements(&slice_table(&measurements, 4096, 4096))
        .expect("write second full block");
    assert_eq!(
        complete_blocks_in_prefix(&observed.borrow()),
        2,
        "second full block must be emitted before finish"
    );

    writer
        .write_measurements(&slice_table(&measurements, 8192, 1))
        .expect("write final short carry");
    assert_eq!(
        complete_blocks_in_prefix(&observed.borrow()),
        2,
        "final short block must wait for finish"
    );
    writer.finish().expect("finish emitted archive");
    assert_eq!(block_layouts(&observed.borrow()).len(), 3);
}

fn verify_partition_invariant() -> usize {
    let fixture = CatalogFixture::load("known_mpad_multi");
    let measurements = repeated_table(&fixture.measurements, 8193);
    let whole = archive_from_partitions(&fixture.circuit, &measurements, &[8193]);
    let canonical = archive_from_partitions(&fixture.circuit, &measurements, &[4096, 4096, 1]);
    let crossed = archive_from_partitions(&fixture.circuit, &measurements, &[1, 4095, 4097]);
    assert_eq!(whole, canonical, "single chunk and canonical chunks");
    assert_eq!(whole, crossed, "crossed caller boundary chunks");
    1
}

fn verify_reader_accepts_short_interior_blocks() {
    let fixture = CatalogFixture::load("known_mpad_multi");
    let measurements = repeated_table(&fixture.measurements, 8193);
    let archive = archive_from_explicit_blocks(&fixture.circuit, &measurements, &[1, 4096, 4096]);
    assert_eq!(block_layouts(&archive).len(), 3);
    let decoded = decode_archive(&archive, &fixture.circuit, 8193, 3);
    assert_tables_eq(
        &measurements,
        &decoded.measurements,
        "short-interior",
        "measurements",
    );
}

fn verify_zero_shot_archive() {
    let circuit = parse("M 0\nDETECTOR rec[-1]\n");
    let transform = MeasurementTransform::from_circuit(&circuit).expect("zero-shot transform");
    let writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        0,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("zero-shot writer");
    let archive = writer.finish().expect("zero-shot finish").into_inner();
    assert!(block_layouts(&archive).is_empty());

    let mut reader =
        SampleArchiveReader::open(ShortRead::new(&archive, 3), &circuit, limits()).expect("open");
    assert!(reader.next_block().expect("zero-shot next").is_none());
    let summary = reader.finish().expect("zero-shot reader finish");
    assert_eq!(summary.block_count, 0);
    assert_eq!(summary.total_shots, 0);
}

fn verify_malformed_cases() -> usize {
    let fixture = CatalogFixture::load("known_mpad_multi");
    let measurements = repeated_table(&fixture.measurements, 8193);
    let archive = archive_from_partitions(&fixture.circuit, &measurements, &[8193]);
    assert_eq!(block_layouts(&archive).len(), 3);
    let mut cases = 0;

    let mut wrong_first_block = archive.clone();
    put_u64(
        &mut wrong_first_block,
        block_field_offset(GLOBAL_HEADER_LEN, "block_index"),
        1,
    );
    expect_next_block_error(
        &wrong_first_block,
        &fixture.circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let second_block = block_layouts(&archive)[1].header.start;
    let mut repeated_block = archive.clone();
    put_u64(
        &mut repeated_block,
        block_field_offset(second_block, "block_index"),
        0,
    );
    expect_second_next_block_error(
        &repeated_block,
        &fixture.circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut skipped_block = archive.clone();
    put_u64(
        &mut skipped_block,
        block_field_offset(second_block, "block_index"),
        2,
    );
    expect_second_next_block_error(
        &skipped_block,
        &fixture.circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut incorrect_first_shot = archive.clone();
    put_u64(
        &mut incorrect_first_shot,
        block_field_offset(second_block, "first_shot"),
        MAX_BLOCK_SHOTS as u64 + 1,
    );
    expect_second_next_block_error(
        &incorrect_first_shot,
        &fixture.circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut first_shot_overflow = archive.clone();
    put_u64(
        &mut first_shot_overflow,
        block_field_offset(second_block, "first_shot"),
        u64::MAX,
    );
    expect_second_next_block_error(
        &first_shot_overflow,
        &fixture.circuit,
        SampleArchiveErrorCode::LimitExceeded,
    );
    cases += 1;

    let mut zero_shot_interior = archive.clone();
    put_u64(
        &mut zero_shot_interior,
        block_field_offset(second_block, "shot_count"),
        0,
    );
    expect_second_next_block_error(
        &zero_shot_interior,
        &fixture.circuit,
        SampleArchiveErrorCode::MalformedArchive,
    );
    cases += 1;

    let mut oversized_block = archive.clone();
    put_u64(
        &mut oversized_block,
        block_field_offset(second_block, "shot_count"),
        MAX_BLOCK_SHOTS as u64 + 1,
    );
    expect_second_next_block_error(
        &oversized_block,
        &fixture.circuit,
        SampleArchiveErrorCode::LimitExceeded,
    );
    cases += 1;

    let mut wrong_free_shape = archive.clone();
    replace_free_stream(&mut wrong_free_shape, 0, |decoded| {
        decoded.push(0);
    });
    expect_next_block_error(
        &wrong_free_shape,
        &fixture.circuit,
        SampleArchiveErrorCode::ShapeMismatch,
    );
    cases += 1;

    let after_first_block = block_layouts(&archive)[0].free.end;
    let eof_between_blocks = archive[..after_first_block].to_vec();
    expect_second_next_block_error(
        &eof_between_blocks,
        &fixture.circuit,
        SampleArchiveErrorCode::Truncated,
    );
    cases += 1;

    let trailer = trailer_offset(&archive);
    let mut bad_trailer_totals = archive.clone();
    put_u64(
        &mut bad_trailer_totals,
        trailer_field_offset(trailer, "block_count"),
        2,
    );
    put_u64(
        &mut bad_trailer_totals,
        trailer_field_offset(trailer, "total_shots"),
        8192,
    );
    expect_finish_error(
        &bad_trailer_totals,
        &fixture.circuit,
        SampleArchiveErrorCode::ShapeMismatch,
    );
    cases += 1;

    let mut late_checksum = archive.clone();
    let digest_offset = late_checksum.len() - 1;
    late_checksum[digest_offset] ^= 0x55;
    let mut reader = SampleArchiveReader::open(
        ShortRead::new(&late_checksum, 7),
        &fixture.circuit,
        limits(),
    )
    .expect("open late-checksum archive");
    assert!(
        reader
            .next_block()
            .expect("first block before late checksum")
            .is_some()
    );
    assert_eq!(
        reader.finish().unwrap_err().code(),
        SampleArchiveErrorCode::ChecksumMismatch
    );

    cases
}

fn verify_bounded_memory() -> MemoryResult {
    let circuit = parse("M 0 1 2\nDETECTOR rec[-3] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let three_blocks = memory_case(&circuit, 8193);
    let twenty_one_blocks = memory_case(&circuit, 81930);
    assert!(three_blocks.max_buffered_shots <= MAX_BLOCK_SHOTS as u64);
    assert!(twenty_one_blocks.max_buffered_shots <= MAX_BLOCK_SHOTS as u64);
    assert_eq!(three_blocks.max_buffered_shots, MAX_BLOCK_SHOTS as u64);
    assert_eq!(twenty_one_blocks.max_buffered_shots, MAX_BLOCK_SHOTS as u64);
    assert_eq!(three_blocks.max_live_decoded_blocks, 1);
    assert_eq!(twenty_one_blocks.max_live_decoded_blocks, 1);
    assert_eq!(three_blocks.max_transform_payloads, 2);
    assert_eq!(twenty_one_blocks.max_transform_payloads, 2);

    let three_peak = three_blocks
        .max_writer_live_bytes
        .max(three_blocks.max_reader_live_bytes);
    let twenty_one_peak = twenty_one_blocks
        .max_writer_live_bytes
        .max(twenty_one_blocks.max_reader_live_bytes);
    assert_eq!(
        three_peak, twenty_one_peak,
        "per-block high-water mark changed from {three_peak} to {twenty_one_peak}"
    );
    let diagnostics: Vec<String> = diagnostic_lines();
    assert!(
        diagnostics
            .iter()
            .any(|line: &String| line.contains("checked_mul")),
        "memory diagnostics must expose checked multiplication formulas: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|line: &String| line.contains("checked_add")),
        "memory diagnostics must expose checked addition formulas: {diagnostics:?}"
    );
    MemoryResult {
        max_buffered_shots: twenty_one_blocks.max_buffered_shots,
        max_live_decoded_blocks: twenty_one_blocks.max_live_decoded_blocks,
        max_transform_payloads: twenty_one_blocks.max_transform_payloads,
        total_block_growth_bytes: twenty_one_peak - three_peak,
    }
}

fn memory_case(
    circuit: &[StimInstr],
    shots: usize,
) -> rstim::sample_archive::telemetry::ArchiveTelemetrySnapshot {
    let transform = MeasurementTransform::from_circuit(circuit).expect("memory transform");
    let measurements = patterned_table(transform.num_measurements(), shots, b"memory");
    reset_archive_telemetry();
    let archive = archive_from_partitions(circuit, &measurements, &[shots]);
    let decoded = decode_archive(
        &archive,
        circuit,
        shots,
        block_layouts(&archive).len() as u64,
    );
    assert_tables_eq(
        &measurements,
        &decoded.measurements,
        "memory",
        "measurements",
    );
    archive_telemetry()
}

fn archive_from_partitions(
    circuit: &[StimInstr],
    measurements: &BitTable,
    partitions: &[usize],
) -> Vec<u8> {
    assert_eq!(partitions.iter().sum::<usize>(), measurements.num_minor());
    let transform = MeasurementTransform::from_circuit(circuit).expect("archive transform");
    let mut writer = SampleArchiveWriter::new(
        NonSeekableWriter::default(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits(),
    )
    .expect("archive writer");
    let mut offset = 0usize;
    for &shots in partitions {
        let chunk = slice_table(measurements, offset, shots);
        writer
            .write_measurements(&chunk)
            .expect("write measurement chunk");
        offset += shots;
    }
    writer.finish().expect("finish archive").into_inner()
}

fn archive_from_explicit_blocks(
    circuit: &[StimInstr],
    measurements: &BitTable,
    block_shots: &[usize],
) -> Vec<u8> {
    assert_eq!(block_shots.iter().sum::<usize>(), measurements.num_minor());
    let transform = MeasurementTransform::from_circuit(circuit).expect("manual transform");
    let identity = transform.identity();
    let mut header = GlobalHeader {
        required_flags: 0,
        optional_flags: 0,
        canonicalization_id: CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
        fingerprint_id: FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
        transform_id: TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
        reference_id: REFERENCE_SIMULATE_NOISELESS,
        codec_suite_id: CODEC_SUITE_ZSTD_FRAMES_V1,
        max_shots_per_block: MAX_BLOCK_SHOTS as u64,
        measurement_count: identity.measurement_count,
        detector_count: identity.detector_count,
        observable_count: identity.observable_count,
        detector_rank: identity.detector_rank,
        total_shots: measurements.num_minor() as u64,
        circuit_sha256: identity.circuit_sha256,
        header_sha256: [0; 32],
    };
    let mut header_bytes = header.to_bytes().expect("serialize manual header");
    header.header_sha256 = Sha256::digest(&header_bytes[..GLOBAL_HEADER_LEN - 32]).into();
    header_bytes[GLOBAL_HEADER_LEN - 32..GLOBAL_HEADER_LEN].copy_from_slice(&header.header_sha256);

    let mut archive = Vec::new();
    archive.extend_from_slice(&header_bytes);
    let mut archive_hasher = Sha256::new();
    archive_hasher.update(header_bytes);
    let mut first_shot = 0u64;
    let mut offset = 0usize;
    for (block_index, &shots) in block_shots.iter().enumerate() {
        let chunk = slice_table(measurements, offset, shots);
        let encoded = transform.encode_block(&chunk).expect("manual encode block");
        let syndrome = encode_syndrome(&encoded.selected_detectors).expect("manual syndrome");
        let free = pack_dense_for_test(&encoded.free_measurements);
        let dense_syndrome = pack_dense_for_test(&encoded.selected_detectors);
        let mut logical_hasher = Sha256::new();
        logical_hasher.update(&dense_syndrome);
        logical_hasher.update(&free);
        let syndrome_frame = if syndrome.raw.is_empty() {
            Vec::new()
        } else {
            compress_frame_for_test(&syndrome.raw)
        };
        let free_frame = if free.is_empty() {
            Vec::new()
        } else {
            compress_frame_for_test(&free)
        };
        let block_header = BlockHeader {
            block_index: block_index as u64,
            first_shot,
            shot_count: shots as u64,
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
            logical_payload_sha256: logical_hasher.finalize().into(),
        };
        let block_bytes = block_header.to_bytes().expect("serialize manual block");
        archive_hasher.update(block_bytes);
        archive_hasher.update(&syndrome_frame);
        archive_hasher.update(&free_frame);
        archive.extend_from_slice(&block_bytes);
        archive.extend_from_slice(&syndrome_frame);
        archive.extend_from_slice(&free_frame);
        first_shot = first_shot
            .checked_add(shots as u64)
            .expect("manual first-shot sum");
        offset += shots;
    }

    let prefix = ArchiveTrailer {
        block_count: block_shots.len() as u64,
        total_shots: measurements.num_minor() as u64,
        archive_sha256: [0; 32],
    }
    .to_bytes()
    .expect("serialize manual trailer prefix");
    archive_hasher.update(&prefix[..32]);
    let trailer = ArchiveTrailer {
        block_count: block_shots.len() as u64,
        total_shots: measurements.num_minor() as u64,
        archive_sha256: archive_hasher.finalize().into(),
    }
    .to_bytes()
    .expect("serialize manual trailer");
    archive.extend_from_slice(&trailer);
    archive
}

fn decode_archive(
    archive: &[u8],
    circuit: &[StimInstr],
    shots: usize,
    expected_blocks: u64,
) -> DecodedTables {
    let transform = MeasurementTransform::from_circuit(circuit).expect("decode transform");
    let mut decoded = DecodedTables {
        measurements: BitTable::try_new(transform.num_measurements(), shots)
            .expect("measurement output allocates"),
        detections: BitTable::try_new(transform.num_detectors(), shots)
            .expect("detection output allocates"),
        observable_flips: BitTable::try_new(transform.num_observables(), shots)
            .expect("observable output allocates"),
    };
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 11), circuit, limits()).expect("open");
    let mut offset = 0usize;
    let mut blocks = 0u64;
    while let Some(block) = reader.next_block().expect("read streaming block") {
        let block_shots = block.measurements.num_minor();
        copy_table_columns(
            &block.measurements,
            0,
            &mut decoded.measurements,
            offset,
            block_shots,
        );
        copy_table_columns(
            &block.detections,
            0,
            &mut decoded.detections,
            offset,
            block_shots,
        );
        copy_table_columns(
            &block.observable_flips,
            0,
            &mut decoded.observable_flips,
            offset,
            block_shots,
        );
        offset += block_shots;
        blocks += 1;
    }
    assert_eq!(offset, shots);
    let summary = reader.finish().expect("finish streaming archive");
    assert_eq!(summary.block_count, expected_blocks);
    assert_eq!(summary.total_shots, shots as u64);
    assert_eq!(blocks, expected_blocks);
    decoded
}

fn expect_next_block_error(archive: &[u8], circuit: &[StimInstr], code: SampleArchiveErrorCode) {
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 5), circuit, limits()).expect("open");
    assert_eq!(reader.next_block().unwrap_err().code(), code);
}

fn expect_second_next_block_error(
    archive: &[u8],
    circuit: &[StimInstr],
    code: SampleArchiveErrorCode,
) {
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 5), circuit, limits()).expect("open");
    assert!(reader.next_block().expect("first block").is_some());
    assert_eq!(reader.next_block().unwrap_err().code(), code);
}

fn expect_finish_error(archive: &[u8], circuit: &[StimInstr], code: SampleArchiveErrorCode) {
    let mut reader =
        SampleArchiveReader::open(ShortRead::new(archive, 5), circuit, limits()).expect("open");
    while reader.next_block().expect("drain block").is_some() {}
    assert_eq!(reader.finish().unwrap_err().code(), code);
}

fn replace_free_stream(
    archive: &mut Vec<u8>,
    block_index: usize,
    mutate: impl FnOnce(&mut Vec<u8>),
) {
    let layout = block_layouts(archive)[block_index].clone();
    let range = layout.free;
    let header = layout.header.start;
    let expected_len =
        get_u64(archive, block_field_offset(header, "free_uncompressed_len")) as usize;
    let mut decoded =
        zstd::bulk::decompress(&archive[range.clone()], expected_len).expect("decompress stream");
    mutate(&mut decoded);
    let compressed = compress_frame_for_test(&decoded);
    archive.splice(range.clone(), compressed.iter().copied());
    put_u64(
        archive,
        block_field_offset(header, "free_uncompressed_len"),
        decoded.len() as u64,
    );
    put_u64(
        archive,
        block_field_offset(header, "free_compressed_len"),
        compressed.len() as u64,
    );
    recompute_archive_digest(archive);
}

fn block_layouts(archive: &[u8]) -> Vec<BlockLayout> {
    let mut layouts = Vec::new();
    let mut offset = GLOBAL_HEADER_LEN;
    loop {
        if archive[offset..offset + 8] == TRAILER_MAGIC[..] {
            break;
        }
        assert_eq!(archive[offset..offset + 8], BLOCK_MAGIC[..]);
        let syndrome_start = offset + BLOCK_HEADER_LEN;
        let syndrome_end = syndrome_start
            + get_u64(
                archive,
                block_field_offset(offset, "syndrome_compressed_len"),
            ) as usize;
        let free_end = syndrome_end
            + get_u64(archive, block_field_offset(offset, "free_compressed_len")) as usize;
        layouts.push(BlockLayout {
            header: offset..offset + BLOCK_HEADER_LEN,
            free: syndrome_end..free_end,
        });
        offset = free_end;
    }
    layouts
}

fn complete_blocks_in_prefix(bytes: &[u8]) -> usize {
    let mut blocks = 0usize;
    let mut offset = GLOBAL_HEADER_LEN;
    while bytes.len() >= offset + 8 && bytes[offset..offset + 8] == BLOCK_MAGIC[..] {
        if bytes.len() < offset + BLOCK_HEADER_LEN {
            break;
        }
        let syndrome_len =
            get_u64(bytes, block_field_offset(offset, "syndrome_compressed_len")) as usize;
        let free_len = get_u64(bytes, block_field_offset(offset, "free_compressed_len")) as usize;
        let Some(end) = offset
            .checked_add(BLOCK_HEADER_LEN)
            .and_then(|value| value.checked_add(syndrome_len))
            .and_then(|value| value.checked_add(free_len))
        else {
            break;
        };
        if bytes.len() < end {
            break;
        }
        blocks += 1;
        offset = end;
    }
    blocks
}

fn trailer_offset(archive: &[u8]) -> usize {
    archive.len() - ARCHIVE_TRAILER_LEN
}

fn block_field_offset(block_start: usize, name: &str) -> usize {
    block_start + field_offset(&BLOCK_FIELDS, name)
}

fn trailer_field_offset(trailer_start: usize, name: &str) -> usize {
    trailer_start + field_offset(&TRAILER_FIELDS, name)
}

fn trailer_field_width(name: &str) -> usize {
    field_width(&TRAILER_FIELDS, name)
}

fn field_offset(fields: &[rstim::sample_archive::format::FieldSpec], name: &str) -> usize {
    fields
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("missing RSMP field {name}"))
        .offset
}

fn field_width(fields: &[rstim::sample_archive::format::FieldSpec], name: &str) -> usize {
    fields
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("missing RSMP field {name}"))
        .width
}

fn recompute_archive_digest(archive: &mut [u8]) {
    let trailer = trailer_offset(archive);
    let digest_offset = trailer_field_offset(trailer, "archive_sha256");
    let digest: [u8; 32] = Sha256::digest(&archive[..digest_offset]).into();
    let digest_width = trailer_field_width("archive_sha256");
    archive[digest_offset..digest_offset + digest_width].copy_from_slice(&digest);
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

fn limits() -> ArchiveLimits {
    ArchiveLimits {
        transform: MeasurementTransformLimits {
            max_measurements: 64,
            max_detectors: 64,
            max_observables: 16,
            max_repeat_depth: 8,
            max_expanded_instructions: 1_000,
            max_parity_terms: 1_000,
            max_shots_per_block: MAX_BLOCK_SHOTS as u64,
            max_transform_working_bytes: 1 << 20,
            max_block_working_bytes: 1 << 20,
        },
        max_archive_bytes: 1 << 25,
        max_block_count: 100,
        max_total_shots: 100_000,
        max_detector_rank: 64,
        max_free_measurements: 64,
        max_compressed_bytes_per_frame: 1 << 20,
        max_decompressed_bytes_per_frame: 1 << 20,
        max_compressed_bytes_per_archive: 1 << 24,
        max_decompressed_bytes_per_archive: 1 << 24,
        max_zstd_window_bytes: 1 << 20,
        max_zstd_decoder_memory_bytes: 1 << 21,
    }
}

fn parse(text: &str) -> Vec<StimInstr> {
    parse_lines(text).unwrap_or_else(|err| panic!("parse failed: {err}"))
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(path)
}

fn read_fixture(name: &str) -> Vec<u8> {
    fs::read(repo_path(&format!("rstim/tests/fixtures/rsmp/{name}")))
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn load_catalog() -> Value {
    let bytes = read_fixture("catalog.json");
    serde_json::from_slice(&bytes).expect("parse rsmp catalog")
}

fn verify_catalog_sha(case: &Value, key: &str, bytes: &[u8]) {
    let expected = case["expected_files"][key]["sha256"]
        .as_str()
        .expect("catalog sha256");
    assert_eq!(hex(&Sha256::digest(bytes)), expected, "{key} catalog hash");
}

fn table_from_catalog_file(case: &Value, key: &str, shots: usize) -> BitTable {
    let path = case["expected_files"][key]["path"]
        .as_str()
        .expect("expected file path");
    let bit_count = case["expected_files"][key]["bit_count"]
        .as_u64()
        .expect("expected bit count") as usize;
    let bytes = fs::read(repo_path(path)).unwrap_or_else(|err| panic!("{path}: {err}"));
    verify_catalog_sha(case, key, &bytes);
    let table = read_shots_b8(&bytes, bit_count).expect("read catalog b8");
    assert_eq!(table.num_minor(), shots);
    table
}

fn repeated_table(base: &BitTable, shots: usize) -> BitTable {
    let mut table = BitTable::try_new(base.num_major(), shots).expect("repeated table allocates");
    for row in 0..base.num_major() {
        for shot in 0..shots {
            table.set(row, shot, base.get(row, shot % base.num_minor()));
        }
    }
    table
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

fn pack_dense_for_test(table: &BitTable) -> Vec<u8> {
    let bits = table.num_major() * table.num_minor();
    let mut bytes = vec![0u8; bits.div_ceil(8)];
    for shot in 0..table.num_minor() {
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                let bit = shot * table.num_major() + row;
                bytes[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    bytes
}

fn slice_table(source: &BitTable, offset: usize, shots: usize) -> BitTable {
    let mut table = BitTable::try_new(source.num_major(), shots).expect("chunk table allocates");
    copy_table_columns(source, offset, &mut table, 0, shots);
    table
}

fn copy_table_columns(
    source: &BitTable,
    source_offset: usize,
    target: &mut BitTable,
    target_offset: usize,
    shots: usize,
) {
    assert_eq!(source.num_major(), target.num_major());
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
        hex(&Sha256::digest(&right_bytes)),
        "{case}: {label} b8 digest"
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[derive(Clone)]
struct BlockLayout {
    header: std::ops::Range<usize>,
    free: std::ops::Range<usize>,
}

struct CatalogFixture {
    circuit: Vec<StimInstr>,
    measurements: BitTable,
    detections: BitTable,
    observables: BitTable,
}

impl CatalogFixture {
    fn load(id: &str) -> Self {
        let catalog = load_catalog();
        let case = catalog["cases"]
            .as_array()
            .expect("catalog cases")
            .iter()
            .find(|case| case["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing catalog case {id}"))
            .clone();
        let shots = case["shots"].as_u64().expect("catalog shots") as usize;
        let circuit_path = case["circuit_path"].as_str().expect("circuit path");
        let circuit_text = fs::read_to_string(repo_path(circuit_path))
            .unwrap_or_else(|err| panic!("{circuit_path}: {err}"));
        Self {
            circuit: parse(&circuit_text),
            measurements: table_from_catalog_file(&case, "measurements_b8", shots),
            detections: table_from_catalog_file(&case, "detectors_b8", shots),
            observables: table_from_catalog_file(&case, "observables_b8", shots),
        }
    }
}

struct DecodedTables {
    measurements: BitTable,
    detections: BitTable,
    observable_flips: BitTable,
}

struct MemoryResult {
    max_buffered_shots: u64,
    max_live_decoded_blocks: u64,
    max_transform_payloads: u64,
    total_block_growth_bytes: u64,
}

#[derive(Clone, Default, Debug)]
struct SharedWriter {
    bytes: Rc<RefCell<Vec<u8>>>,
}

impl SharedWriter {
    fn bytes(&self) -> Rc<RefCell<Vec<u8>>> {
        Rc::clone(&self.bytes)
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
