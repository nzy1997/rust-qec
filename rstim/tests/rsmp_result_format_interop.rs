use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::{DecodedSampleBlock, MeasurementTransform};
use rstim::output::{
    OutputFormat, read_shots_b8, write_shots_01, write_shots_b8, write_shots_dets,
    write_shots_hits, write_shots_ptb64, write_shots_r8,
};
use rstim::parser::parse_lines;
use rstim::result_stream::{ResultBlockReader, ResultBlockWriter, ResultOutputKind};
use rstim::sample_archive::{
    ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter,
};
use rstim::sim::bit_table::BitTable;
use serde_json::Value;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const PACK_FORMATS: [&str; 3] = ["01", "b8", "ptb64"];
const RESULT_FORMATS: [&str; 5] = ["01", "b8", "r8", "hits", "ptb64"];
const DETECTOR_FORMATS: [&str; 6] = ["01", "b8", "r8", "hits", "ptb64", "dets"];

#[test]
fn rsmp_result_format_interop_contract() {
    let pack_formats = verify_pack_formats();
    assert_eq!(pack_formats, 3);
    let measurement_formats = verify_measurement_outputs();
    assert_eq!(measurement_formats, 5);
    let detector_formats = verify_detector_outputs();
    assert_eq!(detector_formats, 6);
    let observable_formats = verify_observable_outputs();
    assert_eq!(observable_formats, 5);
    verify_simultaneous_outputs_and_dets_labels();
    verify_zero_detector_observable_dets_cli();
    let ptb64_cross_block = verify_ptb64_cross_block();
    assert_eq!(ptb64_cross_block, 1);
    let guarded_read = verify_guarded_read();
    assert_eq!(guarded_read, 1);
    let negative_cases = verify_negative_cases();
    assert_eq!(negative_cases, 14);
    println!(
        "PASS rsmp result formats pack_formats=3 measurement_formats=5 detector_formats=6 observable_formats=5 ptb64_cross_block=1 guarded_read=1 negative_cases=14"
    );
}

fn verify_pack_formats() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let measurements = repeated_table(&fixture.measurements, 65);
    let mut cases = 0;

    for format in PACK_FORMATS {
        let dir = tempfile::tempdir().expect("tempdir");
        let input = encode_table(&measurements, format, None);
        let archive = dir.path().join(format!("pack-{format}.rsmp"));
        let output = run_cli(
            &pack_args(&fixture.circuit, 65, Path::new("-"), &archive, format),
            Some(&input),
        );
        assert_success(&output, &format!("pack_samples accepts {format}"));
        assert_archive_measurements(&archive, &fixture.instructions, &measurements);
        cases += 1;
    }
    verify_zero_width_pack_inputs();
    cases
}

fn verify_zero_width_pack_inputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = dir.path().join("zero-width.stim");
    fs::write(&circuit, b"").expect("write zero-width circuit");
    let instructions = parse_lines("").expect("parse zero-width circuit");
    let shots = 3;
    let measurements = BitTable::try_new(0, shots).expect("allocate zero-width measurements");

    for format in PACK_FORMATS {
        let input = encode_table(&measurements, format, None);
        match format {
            "01" => assert_eq!(input, vec![b'\n'; shots], "zero-width 01 rows"),
            "b8" | "ptb64" => assert!(input.is_empty(), "zero-width {format} input"),
            _ => unreachable!("pack format list is fixed"),
        }
        let archive = dir.path().join(format!("zero-width-{format}.rsmp"));
        let output = run_cli(
            &pack_args(&circuit, shots as u64, Path::new("-"), &archive, format),
            Some(&input),
        );
        assert_success(&output, &format!("pack zero-width {format}"));
        assert_archive_measurements(&archive, &instructions, &measurements);
    }

    for (tag, format) in ["b8", "ptb64"].into_iter().enumerate() {
        let destination = dir.path().join(format!("zero-width-extra-{format}.rsmp"));
        write_sentinel(&destination, 0xe0 + tag as u8);
        let output = run_cli(
            &pack_args(&circuit, shots as u64, Path::new("-"), &destination, format),
            Some(&[0]),
        );
        assert_failure(&output, &format!("zero-width-extra-{format}"));
        assert_sentinel(&destination, 0xe0 + tag as u8);
    }
}

fn verify_measurement_outputs() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let archive = archive_from_measurements(&fixture.instructions, &fixture.measurements);
    let mut cases = 0;

    for format in RESULT_FORMATS {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive_path = dir.path().join("measurements.rsmp");
        let output_path = dir.path().join(format!("measurements.{format}"));
        fs::write(&archive_path, &archive).expect("write archive");
        let output = run_cli(
            &unpack_args(
                &fixture.circuit,
                &archive_path,
                Some(("--measurements_out", &output_path, format)),
            ),
            None,
        );
        assert_success(&output, &format!("unpack measurements as {format}"));
        assert_eq!(
            fs::read(&output_path).expect("read measurement output"),
            encode_table(&fixture.measurements, format, None),
            "measurement {format} output"
        );
        cases += 1;
    }
    cases
}

fn verify_detector_outputs() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let archive = archive_from_measurements(&fixture.instructions, &fixture.measurements);
    let decoded = measurements_to_detections(&fixture.instructions, &fixture.measurements)
        .expect("derive expected detector results");
    let mut cases = 0;

    for format in DETECTOR_FORMATS {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive_path = dir.path().join("detectors.rsmp");
        let output_path = dir.path().join(format!("detectors.{format}"));
        fs::write(&archive_path, &archive).expect("write archive");
        let output = run_cli(
            &unpack_args(
                &fixture.circuit,
                &archive_path,
                Some(("--detectors_out", &output_path, format)),
            ),
            None,
        );
        assert_success(&output, &format!("unpack detectors as {format}"));
        assert_eq!(
            fs::read(&output_path).expect("read detector output"),
            encode_table(
                &decoded.detections,
                format,
                if format == "dets" {
                    Some(&decoded.observable_flips)
                } else {
                    None
                },
            ),
            "detector {format} output"
        );
        cases += 1;
    }
    cases
}

fn verify_observable_outputs() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let archive = archive_from_measurements(&fixture.instructions, &fixture.measurements);
    let decoded = measurements_to_detections(&fixture.instructions, &fixture.measurements)
        .expect("derive expected observable results");
    let mut cases = 0;

    for format in RESULT_FORMATS {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive_path = dir.path().join("observables.rsmp");
        let output_path = dir.path().join(format!("observables.{format}"));
        fs::write(&archive_path, &archive).expect("write archive");
        let output = run_cli(
            &unpack_args(
                &fixture.circuit,
                &archive_path,
                Some(("--obs_out", &output_path, format)),
            ),
            None,
        );
        assert_success(&output, &format!("unpack observables as {format}"));
        assert_eq!(
            fs::read(&output_path).expect("read observable output"),
            encode_table(&decoded.observable_flips, format, None),
            "observable {format} output"
        );
        cases += 1;
    }
    cases
}

fn verify_simultaneous_outputs_and_dets_labels() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = dir.path().join("three-output-dets.stim");
    fs::write(
        &circuit,
        b"M 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write three-output circuit");
    let instructions = parse_lines(
        "M 0\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("parse three-output circuit");
    let measurements = bit_table_from_shots(&[&[false, true]]);
    let archive = archive_from_measurements(&instructions, &measurements);
    let archive_path = dir.path().join("three-output.rsmp");
    fs::write(&archive_path, archive).expect("write three-output archive");

    let measurements_path = dir.path().join("measurements.01");
    let detectors_path = dir.path().join("detectors.dets");
    let observables_path = dir.path().join("observables.01");
    let output = run_cli(
        &unpack_args(&circuit, &archive_path, None)
            .into_iter()
            .chain([
                "--measurements_out".to_owned(),
                measurements_path.display().to_string(),
                "--measurements_out_format".to_owned(),
                "01".to_owned(),
                "--detectors_out".to_owned(),
                detectors_path.display().to_string(),
                "--detectors_out_format".to_owned(),
                "dets".to_owned(),
                "--obs_out".to_owned(),
                observables_path.display().to_string(),
                "--obs_out_format".to_owned(),
                "01".to_owned(),
            ])
            .collect::<Vec<_>>(),
        None,
    );
    assert_success(&output, "unpack all three outputs with dets");
    assert_eq!(fs::read(measurements_path).unwrap(), b"01\n");
    assert_eq!(fs::read(detectors_path).unwrap(), b"shot D1 L0\n");
    assert_eq!(fs::read(observables_path).unwrap(), b"1\n");
}

fn verify_zero_detector_observable_dets_cli() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = dir.path().join("zero-detector-observable.stim");
    fs::write(&circuit, b"M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
        .expect("write zero-detector circuit");
    let instructions =
        parse_lines("M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").expect("parse zero-detector circuit");
    let measurements = bit_table_from_shots(&[&[true]]);
    let archive = archive_from_measurements(&instructions, &measurements);
    let archive_path = dir.path().join("zero-detector.rsmp");
    fs::write(&archive_path, archive).expect("write zero-detector archive");

    let dets_path = dir.path().join("detectors.dets");
    let output = run_cli(
        &unpack_args(
            &circuit,
            &archive_path,
            Some(("--detectors_out", &dets_path, "dets")),
        ),
        None,
    );
    assert_success(&output, "zero-detector dets output");
    assert_eq!(fs::read(dets_path).unwrap(), b"shot L0\n");

    let detector_01_path = dir.path().join("detectors.01");
    let output = run_cli(
        &unpack_args(
            &circuit,
            &archive_path,
            Some(("--detectors_out", &detector_01_path, "01")),
        ),
        None,
    );
    assert_success(&output, "zero-detector 01 output");
    assert_eq!(fs::read(detector_01_path).unwrap(), b"\n");
}

fn verify_ptb64_cross_block() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let measurements = repeated_table(&fixture.measurements, 195);
    let decoded = measurements_to_detections(&fixture.instructions, &measurements)
        .expect("derive cross-block detector results");
    let outputs = [
        ("measurements", "--measurements_out", &measurements),
        ("detectors", "--detectors_out", &decoded.detections),
        ("observables", "--obs_out", &decoded.observable_flips),
    ];
    let dir = tempfile::tempdir().expect("tempdir");

    for (block_shots, expected_blocks) in [(65, vec![65, 65, 65]), (130, vec![130, 65])] {
        let archive = archive_from_measurements_with_block_shots(
            &fixture.instructions,
            &measurements,
            block_shots,
        );
        let archive_path = dir.path().join(format!("cross-block-{block_shots}.rsmp"));
        fs::write(&archive_path, archive).expect("write archive");

        assert_eq!(
            archive_block_shots(
                &fs::read(&archive_path).expect("read cross-block archive"),
                &fixture.instructions,
            ),
            expected_blocks,
            "ptb64 cross-block archive uses {block_shots}-shot blocks"
        );

        for (kind, flag, expected) in outputs {
            let output_path = dir
                .path()
                .join(format!("cross-block-{block_shots}-{kind}.ptb64"));
            let output = run_cli(
                &unpack_args(
                    &fixture.circuit,
                    &archive_path,
                    Some((flag, &output_path, "ptb64")),
                ),
                None,
            );
            assert_success(
                &output,
                &format!("ptb64 {kind} output across {block_shots}-shot archive blocks"),
            );
            assert_eq!(
                fs::read(&output_path).expect("read cross-block ptb64"),
                ptb64_bytes(expected),
                "ptb64 {kind} bytes carry across {block_shots}-shot archive blocks"
            );
        }
    }
    1
}

fn verify_guarded_read() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let measurements = repeated_table(&fixture.measurements, 65_537);
    let input = encode_table(&measurements, "b8", None);
    assert!(input.len() > 64 * 1024, "guarded input exceeds 64 KiB");

    let mut input_reader = ResultBlockReader::new(
        GuardedRead::new(&input, 64 * 1024, 7),
        measurements.num_major(),
        measurements.num_minor() as u64,
        OutputFormat::B8,
        127,
    )
    .expect("open guarded result input");
    let transform = MeasurementTransform::from_circuit(&fixture.instructions)
        .expect("build guarded archive transform");
    let mut limits = ArchiveLimits::default();
    limits.transform.max_shots_per_block = 257;
    let mut writer = SampleArchiveWriter::new(
        Vec::new(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits,
    )
    .expect("open archive writer for guarded input");
    while let Some(block) = input_reader
        .next_block()
        .expect("read guarded result input block")
    {
        writer
            .write_measurements(&block)
            .expect("write guarded result input block");
    }
    let archive = writer
        .finish()
        .expect("finish guarded result input archive");
    assert!(
        archive_block_shots(&archive, &fixture.instructions).len() > 1,
        "guarded input produces a many-block archive"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("guarded-input.rsmp");
    fs::write(&archive_path, archive).expect("write guarded input archive");
    assert_archive_measurements(&archive_path, &fixture.instructions, &measurements);
    1
}

fn verify_negative_cases() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let archive = archive_from_measurements(&fixture.instructions, &fixture.measurements);
    let dir = tempfile::tempdir().expect("tempdir");
    let valid_archive = dir.path().join("valid.rsmp");
    fs::write(&valid_archive, archive).expect("write valid archive");
    let b8 = encode_table(&fixture.measurements, "b8", None);
    let format01 = encode_table(&fixture.measurements, "01", None);
    let ptb64 = encode_table(&fixture.measurements, "ptb64", None);
    let row_len = format01.len() / fixture.shots;
    let mut cases = 0;

    verify_result_block_writer_rejects_mismatched_shots();
    cases += 1;

    verify_pack_failure_preserves_destination(
        dir.path(),
        &fixture.circuit,
        fixture.shots as u64,
        "non-01-byte",
        "01",
        b"10x\n000\n111\n000\n".to_vec(),
        cases,
    );
    cases += 1;

    verify_01_framing_rejection(
        dir.path(),
        &fixture.circuit,
        fixture.shots as u64,
        &format01,
    );
    cases += 1;

    verify_pack_failure_preserves_destination(
        dir.path(),
        &fixture.circuit,
        fixture.shots as u64,
        "fewer-shots",
        "01",
        format01[..format01.len() - row_len].to_vec(),
        cases,
    );
    cases += 1;

    verify_pack_failure_preserves_destination(
        dir.path(),
        &fixture.circuit,
        fixture.shots as u64,
        "more-shots",
        "01",
        [format01.as_slice(), &format01[..row_len]].concat(),
        cases,
    );
    cases += 1;

    let invalid_pack_inputs = [
        ("partial-b8-row", "b8", b8[..b8.len() - 1].to_vec()),
        ("padding-b8", "b8", vec![0xf8; fixture.shots]),
        (
            "invalid-ptb64-byte-length",
            "ptb64",
            ptb64[..ptb64.len() - 1].to_vec(),
        ),
        (
            "nonzero-ptb64-shot-padding",
            "ptb64",
            vec![0xff; 8 * fixture.measurements.num_major()],
        ),
        (
            "extra-ptb64-group",
            "ptb64",
            [ptb64.as_slice(), &ptb64].concat(),
        ),
    ];
    for (name, format, input) in invalid_pack_inputs {
        verify_pack_failure_preserves_destination(
            dir.path(),
            &fixture.circuit,
            fixture.shots as u64,
            name,
            format,
            input,
            cases,
        );
        cases += 1;
    }

    for (name, flag, format) in [
        ("measurement-dets", "--measurements_out", "dets"),
        ("observable-dets", "--obs_out", "dets"),
    ] {
        let destination = dir.path().join(format!("{name}.out"));
        write_sentinel(&destination, cases as u8);
        let output = run_cli(
            &unpack_args(
                &fixture.circuit,
                &valid_archive,
                Some((flag, &destination, format)),
            ),
            None,
        );
        assert_failure(&output, name);
        assert_sentinel(&destination, cases as u8);
        cases += 1;
    }

    let output = run_cli(
        &unpack_args(&fixture.circuit, &valid_archive, None)
            .into_iter()
            .chain([
                "--measurements_out".to_owned(),
                "-".to_owned(),
                "--measurements_out_format".to_owned(),
                "b8".to_owned(),
                "--detectors_out".to_owned(),
                "-".to_owned(),
                "--detectors_out_format".to_owned(),
                "b8".to_owned(),
            ])
            .collect::<Vec<_>>(),
        None,
    );
    assert_failure(&output, "two stdout destinations");
    cases += 1;

    let duplicate = dir.path().join("duplicate.b8");
    write_sentinel(&duplicate, cases as u8);
    let output = run_cli(
        &unpack_args(
            &fixture.circuit,
            &valid_archive,
            Some(("--measurements_out", &duplicate, "b8")),
        )
        .into_iter()
        .chain([
            "--detectors_out".to_owned(),
            duplicate.display().to_string(),
            "--detectors_out_format".to_owned(),
            "b8".to_owned(),
        ])
        .collect::<Vec<_>>(),
        None,
    );
    assert_failure(&output, "duplicate unpack output");
    assert_sentinel(&duplicate, cases as u8);
    cases += 1;

    cases
}

fn verify_pack_failure_preserves_destination(
    dir: &Path,
    circuit: &Path,
    shots: u64,
    name: &str,
    format: &str,
    input: Vec<u8>,
    tag: usize,
) {
    let destination = dir.join(format!("{name}.rsmp"));
    write_sentinel(&destination, tag as u8);
    let output = run_cli(
        &pack_args(circuit, shots, Path::new("-"), &destination, format),
        Some(&input),
    );
    assert_failure(&output, name);
    assert_sentinel(&destination, tag as u8);
}

fn verify_01_framing_rejection(dir: &Path, circuit: &Path, shots: u64, format01: &[u8]) {
    let row_width = format01.len() / shots as usize;
    let mut misplaced_newline = format01.to_vec();
    misplaced_newline.swap(row_width - 2, row_width - 1);
    assert_eq!(misplaced_newline.len(), format01.len());

    for (tag, (name, input)) in [
        (
            "missing-newline-01",
            format01[..format01.len() - 1].to_vec(),
        ),
        ("misplaced-newline-01", misplaced_newline),
    ]
    .into_iter()
    .enumerate()
    {
        let destination = dir.join(format!("{name}.rsmp"));
        write_sentinel(&destination, 0xc0 + tag as u8);
        let output = run_cli(
            &pack_args(circuit, shots, Path::new("-"), &destination, "01"),
            Some(&input),
        );
        assert_failure(&output, name);
        assert_sentinel(&destination, 0xc0 + tag as u8);
    }
}

fn verify_result_block_writer_rejects_mismatched_shots() {
    let fixture = CatalogFixture::known_mpad_multi();
    let decoded = measurements_to_detections(&fixture.instructions, &fixture.measurements)
        .expect("derive decoded block");
    let shorter_shots = fixture.shots - 1;
    let blocks = [
        (
            "measurements",
            table_prefix(&fixture.measurements, shorter_shots),
            decoded.detections.clone(),
            decoded.observable_flips.clone(),
        ),
        (
            "detections",
            fixture.measurements.clone(),
            table_prefix(&decoded.detections, shorter_shots),
            decoded.observable_flips.clone(),
        ),
        (
            "observables",
            fixture.measurements.clone(),
            decoded.detections.clone(),
            table_prefix(&decoded.observable_flips, shorter_shots),
        ),
    ];

    for (block_tag, (name, measurements, detections, observable_flips)) in
        blocks.into_iter().enumerate()
    {
        let block = DecodedSampleBlock {
            measurements,
            detections,
            observable_flips,
        };
        for (kind_tag, kind) in [
            ResultOutputKind::Measurements,
            ResultOutputKind::Detectors,
            ResultOutputKind::Observables,
        ]
        .into_iter()
        .enumerate()
        {
            let sentinel = vec![0xd0, block_tag as u8, kind_tag as u8, 0x0d];
            let mut output = sentinel.clone();
            let mut writer = ResultBlockWriter::new(
                &mut output,
                kind,
                OutputFormat::B8,
                block.measurements.num_minor() as u64,
                ArchiveLimits::default(),
            )
            .expect("create result block writer");
            assert!(
                writer.write_block(&block).is_err(),
                "{name} shot-count mismatch must fail for {kind:?}"
            );
            drop(writer);
            assert_eq!(
                output, sentinel,
                "{name} mismatch must not write {kind:?} output"
            );
        }
    }
}

fn archive_from_measurements(circuit: &[rstim::ir::StimInstr], measurements: &BitTable) -> Vec<u8> {
    archive_from_measurements_with_limits(circuit, measurements, ArchiveLimits::default())
}

fn archive_from_measurements_with_block_shots(
    circuit: &[rstim::ir::StimInstr],
    measurements: &BitTable,
    block_shots: u64,
) -> Vec<u8> {
    let mut limits = ArchiveLimits::default();
    limits.transform.max_shots_per_block = block_shots;
    archive_from_measurements_with_limits(circuit, measurements, limits)
}

fn archive_from_measurements_with_limits(
    circuit: &[rstim::ir::StimInstr],
    measurements: &BitTable,
    limits: ArchiveLimits,
) -> Vec<u8> {
    let transform = MeasurementTransform::from_circuit(circuit).expect("build archive transform");
    let mut writer = SampleArchiveWriter::new(
        Vec::new(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        limits,
    )
    .expect("open archive writer");
    if measurements.num_minor() > 0 {
        writer
            .write_measurements(measurements)
            .expect("write archive measurements");
    }
    writer.finish().expect("finish archive")
}

fn assert_archive_measurements(
    archive: &Path,
    circuit: &[rstim::ir::StimInstr],
    expected: &BitTable,
) {
    let bytes = fs::read(archive).expect("read packed archive");
    let mut reader = SampleArchiveReader::open(&bytes[..], circuit, ArchiveLimits::default())
        .expect("open packed archive");
    let mut first_shot = 0;
    while let Some(block) = reader.next_block().expect("read packed block") {
        assert_table_slice_eq(
            &block.measurements,
            expected,
            first_shot,
            "packed measurements",
        );
        first_shot += block.measurements.num_minor();
    }
    assert_eq!(first_shot, expected.num_minor());
    reader.finish().expect("finish packed archive");
}

fn archive_block_shots(archive: &[u8], circuit: &[rstim::ir::StimInstr]) -> Vec<usize> {
    let mut reader = SampleArchiveReader::open(archive, circuit, ArchiveLimits::default())
        .expect("open archive for block count");
    let mut blocks = Vec::new();
    while let Some(block) = reader
        .next_block()
        .expect("read archive block for block count")
    {
        blocks.push(block.measurements.num_minor());
    }
    let summary = reader.finish().expect("finish archive block count");
    assert_eq!(
        summary.block_count,
        blocks.len() as u64,
        "archive block count summary"
    );
    blocks
}

fn assert_table_slice_eq(actual: &BitTable, expected: &BitTable, first_shot: usize, label: &str) {
    assert_eq!(actual.num_major(), expected.num_major(), "{label} rows");
    for bit in 0..actual.num_major() {
        for shot in 0..actual.num_minor() {
            assert_eq!(
                actual.get(bit, shot),
                expected.get(bit, first_shot + shot),
                "{label} bit={bit} shot={}",
                first_shot + shot
            );
        }
    }
}

fn encode_table(table: &BitTable, format: &str, observables: Option<&BitTable>) -> Vec<u8> {
    let mut bytes = Vec::new();
    match format {
        "01" => write_shots_01(table, &mut bytes),
        "b8" => write_shots_b8(table, &mut bytes),
        "r8" => write_shots_r8(table, &mut bytes),
        "hits" => write_shots_hits(table, &mut bytes),
        "ptb64" => write_shots_ptb64(table, &mut bytes),
        "dets" => write_shots_dets(table, observables.expect("dets observables"), &mut bytes),
        _ => panic!("unknown expected format {format}"),
    }
    .expect("encode expected result format");
    bytes
}

fn ptb64_bytes(table: &BitTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_shots_ptb64(table, &mut bytes).expect("encode whole-table ptb64");
    bytes
}

fn bit_table_from_shots(shots: &[&[bool]]) -> BitTable {
    let width = shots.first().map_or(0, |shot| shot.len());
    let mut table = BitTable::try_new(width, shots.len()).expect("allocate bit table");
    for (shot_index, shot) in shots.iter().enumerate() {
        assert_eq!(shot.len(), width);
        for (bit, value) in shot.iter().copied().enumerate() {
            table.set(bit, shot_index, value);
        }
    }
    table
}

fn repeated_table(source: &BitTable, shots: usize) -> BitTable {
    let mut output = BitTable::try_new(source.num_major(), shots).expect("allocate repeated table");
    for shot in 0..shots {
        for bit in 0..source.num_major() {
            output.set(bit, shot, source.get(bit, shot % source.num_minor()));
        }
    }
    output
}

fn table_prefix(source: &BitTable, shots: usize) -> BitTable {
    let mut output = BitTable::try_new(source.num_major(), shots).expect("allocate table prefix");
    for shot in 0..shots {
        for bit in 0..source.num_major() {
            output.set(bit, shot, source.get(bit, shot));
        }
    }
    output
}

fn pack_args(circuit: &Path, shots: u64, input: &Path, output: &Path, format: &str) -> Vec<String> {
    vec![
        "pack_samples".to_owned(),
        "--circuit".to_owned(),
        circuit.display().to_string(),
        "--shots".to_owned(),
        shots.to_string(),
        "--in".to_owned(),
        input.display().to_string(),
        "--in_format".to_owned(),
        format.to_owned(),
        "--out".to_owned(),
        output.display().to_string(),
    ]
}

fn unpack_args(circuit: &Path, archive: &Path, output: Option<(&str, &Path, &str)>) -> Vec<String> {
    let mut args = vec![
        "unpack_samples".to_owned(),
        "--circuit".to_owned(),
        circuit.display().to_string(),
        "--in".to_owned(),
        archive.display().to_string(),
    ];
    if let Some((flag, path, format)) = output {
        args.extend([
            flag.to_owned(),
            path.display().to_string(),
            format!("{flag}_format"),
            format.to_owned(),
        ]);
    }
    args
}

fn run_cli(args: &[String], stdin: Option<&[u8]>) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;

    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rstim"));
    command.args(args).stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn rstim");
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .expect("open stdin")
            .write_all(bytes)
            .expect("write stdin");
    }
    child.wait_with_output().expect("wait rstim")
}

fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_sentinel(path: &Path, tag: u8) {
    fs::write(path, [tag, tag.wrapping_add(1), tag.wrapping_add(2)]).expect("write sentinel");
}

fn assert_sentinel(path: &Path, tag: u8) {
    assert_eq!(
        fs::read(path).expect("read sentinel"),
        [tag, tag.wrapping_add(1), tag.wrapping_add(2)]
    );
}

struct CatalogFixture {
    circuit: PathBuf,
    instructions: Vec<rstim::ir::StimInstr>,
    measurements: BitTable,
    shots: usize,
}

impl CatalogFixture {
    fn known_mpad_multi() -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let catalog_path = workspace.join("rstim/tests/fixtures/rsmp/catalog.json");
        let catalog: Value =
            serde_json::from_slice(&fs::read(&catalog_path).expect("read catalog"))
                .expect("parse catalog");
        let case = catalog["cases"]
            .as_array()
            .expect("catalog cases")
            .iter()
            .find(|case| case["id"].as_str() == Some("known_mpad_multi"))
            .expect("known_mpad_multi catalog case");
        assert!(
            case["consumers"]
                .as_array()
                .expect("catalog consumers")
                .iter()
                .any(|consumer| consumer.as_str() == Some("rsmp_archive")),
            "catalog fixture must remain an rsmp archive consumer"
        );
        let circuit = workspace.join(case["circuit_path"].as_str().expect("catalog circuit path"));
        let instructions =
            parse_lines(&fs::read_to_string(&circuit).expect("read fixture circuit"))
                .expect("parse fixture circuit");
        let shots = case["shots"].as_u64().expect("catalog shots") as usize;
        let measurement_path = workspace.join(
            case["measurement_input"]["path"]
                .as_str()
                .expect("catalog measurement input path"),
        );
        let measurement_count = case["measurement_count"]
            .as_u64()
            .expect("catalog measurement count") as usize;
        let measurements = read_shots_b8(
            &fs::read(measurement_path).expect("read fixture measurements"),
            measurement_count,
        )
        .expect("decode fixture measurements");
        assert_eq!(measurements.num_minor(), shots);
        Self {
            circuit,
            instructions,
            measurements,
            shots,
        }
    }
}

struct GuardedRead<'a> {
    input: &'a [u8],
    max_request: usize,
    max_yield: usize,
}

impl<'a> GuardedRead<'a> {
    fn new(input: &'a [u8], max_request: usize, max_yield: usize) -> Self {
        Self {
            input,
            max_request,
            max_yield,
        }
    }
}

impl Read for GuardedRead<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.len() > self.max_request {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("read request {} exceeds {}", buffer.len(), self.max_request),
            ));
        }
        let yielded = buffer.len().min(self.max_yield);
        self.input.read(&mut buffer[..yielded])
    }
}
