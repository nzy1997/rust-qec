use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::MeasurementTransform;
use rstim::output::{
    read_shots_b8, write_shots_01, write_shots_b8, write_shots_dets, write_shots_hits,
    write_shots_ptb64, write_shots_r8,
};
use rstim::parser::parse_lines;
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
    cases
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

fn verify_ptb64_cross_block() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let measurements = repeated_table(&fixture.measurements, 4097);
    let archive = archive_from_measurements(&fixture.instructions, &measurements);
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("cross-block.rsmp");
    let output_path = dir.path().join("cross-block.ptb64");
    fs::write(&archive_path, archive).expect("write archive");

    let output = run_cli(
        &unpack_args(
            &fixture.circuit,
            &archive_path,
            Some(("--measurements_out", &output_path, "ptb64")),
        ),
        None,
    );
    assert_success(&output, "ptb64 output across archive blocks");
    assert_eq!(
        fs::read(&output_path).expect("read cross-block ptb64"),
        encode_table(&measurements, "ptb64", None)
    );
    1
}

fn verify_guarded_read() -> usize {
    let fixture = CatalogFixture::known_mpad_multi();
    let measurements = repeated_table(&fixture.measurements, 8193);
    let archive = archive_from_measurements(&fixture.instructions, &measurements);
    let mut reader = SampleArchiveReader::open(
        GuardedRead::new(&archive, 64 * 1024),
        &fixture.instructions,
        ArchiveLimits::default(),
    )
    .expect("open archive through guarded reader");
    let mut shots = 0;
    while let Some(block) = reader.next_block().expect("read guarded archive block") {
        shots += block.measurements.num_minor();
    }
    let summary = reader.finish().expect("finish guarded archive read");
    assert_eq!(shots, 8193);
    assert_eq!(summary.total_shots, 8193);
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
    let mut cases = 0;

    let invalid_pack_inputs = [
        ("unsupported-r8", "r8", b8.clone()),
        ("unsupported-dets", "dets", b8.clone()),
        ("short-b8", "b8", b8[..b8.len() - 1].to_vec()),
        ("extra-b8", "b8", [b8.clone(), vec![0]].concat()),
        ("padding-b8", "b8", vec![0xf8; fixture.shots]),
        ("bad-01", "01", b"10x\n000\n111\n000\n".to_vec()),
        (
            "missing-newline-01",
            "01",
            format01[..format01.len() - 1].to_vec(),
        ),
        ("short-ptb64", "ptb64", ptb64[..ptb64.len() - 1].to_vec()),
        (
            "padding-ptb64",
            "ptb64",
            vec![0xff; 8 * fixture.measurements.num_major()],
        ),
    ];
    for (name, format, input) in invalid_pack_inputs {
        let destination = dir.path().join(format!("{name}.rsmp"));
        write_sentinel(&destination, cases as u8);
        let output = run_cli(
            &pack_args(
                &fixture.circuit,
                fixture.shots as u64,
                Path::new("-"),
                &destination,
                format,
            ),
            Some(&input),
        );
        assert_failure(&output, name);
        assert_sentinel(&destination, cases as u8);
        cases += 1;
    }

    for (name, flag, format) in [
        ("measurement-dets", "--measurements_out", "dets"),
        ("observable-dets", "--obs_out", "dets"),
        ("unknown-detector-format", "--detectors_out", "unknown"),
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

    let output = run_cli(&unpack_args(&fixture.circuit, &valid_archive, None), None);
    assert_failure(&output, "unpack without outputs");
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

fn archive_from_measurements(circuit: &[rstim::ir::StimInstr], measurements: &BitTable) -> Vec<u8> {
    let transform = MeasurementTransform::from_circuit(circuit).expect("build archive transform");
    let mut writer = SampleArchiveWriter::new(
        Vec::new(),
        transform,
        measurements.num_minor() as u64,
        SampleArchiveOptions::default(),
        ArchiveLimits::default(),
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

fn repeated_table(source: &BitTable, shots: usize) -> BitTable {
    let mut output = BitTable::try_new(source.num_major(), shots).expect("allocate repeated table");
    for shot in 0..shots {
        for bit in 0..source.num_major() {
            output.set(bit, shot, source.get(bit, shot % source.num_minor()));
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
}

impl<'a> GuardedRead<'a> {
    fn new(input: &'a [u8], max_request: usize) -> Self {
        Self { input, max_request }
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
        self.input.read(buffer)
    }
}
