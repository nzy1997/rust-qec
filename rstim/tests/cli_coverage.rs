use rstim::cli;
use rstim::output::OutputFormat;
use rstim::sim::bit_table::BitTable;

// ---------- write_format ----------

#[test]
fn write_format_01() {
    let mut table = BitTable::new(2, 1);
    table.set(0, 0, true);
    let mut buf = Vec::new();
    cli::write_format(OutputFormat::Format01, &table, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "10\n");
}

#[test]
fn write_format_b8() {
    let mut table = BitTable::new(8, 1);
    table.set(0, 0, true);
    table.set(1, 0, true);
    let mut buf = Vec::new();
    cli::write_format(OutputFormat::B8, &table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x03]);
}

#[test]
fn write_format_r8() {
    let mut table = BitTable::new(3, 1);
    table.set(2, 0, true);
    let mut buf = Vec::new();
    cli::write_format(OutputFormat::R8, &table, &mut buf).unwrap();
    assert_eq!(buf, vec![2, 0]);
}

#[test]
fn write_format_hits() {
    let mut table = BitTable::new(4, 1);
    table.set(1, 0, true);
    table.set(3, 0, true);
    let mut buf = Vec::new();
    cli::write_format(OutputFormat::Hits, &table, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "1,3\n");
}

#[test]
fn write_format_dets_error() {
    let table = BitTable::new(1, 1);
    let mut buf = Vec::new();
    let err = cli::write_format(OutputFormat::Dets, &table, &mut buf);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("write_shots_dets"));
}

// ---------- merge_detections_observables ----------

#[test]
fn merge_empty() {
    let dets = BitTable::new(0, 2);
    let obs = BitTable::new(0, 2);
    let merged = cli::merge_detections_observables(&dets, &obs);
    assert_eq!(merged.num_major(), 0);
    assert_eq!(merged.num_minor(), 2);
}

#[test]
fn merge_dets_only() {
    let mut dets = BitTable::new(2, 1);
    dets.set(0, 0, true);
    let obs = BitTable::new(0, 1);
    let merged = cli::merge_detections_observables(&dets, &obs);
    assert_eq!(merged.num_major(), 2);
    assert!(merged.get(0, 0));
    assert!(!merged.get(1, 0));
}

#[test]
fn merge_obs_only() {
    let dets = BitTable::new(0, 1);
    let mut obs = BitTable::new(2, 1);
    obs.set(1, 0, true);
    let merged = cli::merge_detections_observables(&dets, &obs);
    assert_eq!(merged.num_major(), 2);
    assert!(!merged.get(0, 0));
    assert!(merged.get(1, 0));
}

#[test]
fn merge_both() {
    let mut dets = BitTable::new(2, 2);
    dets.set(0, 0, true);
    dets.set(1, 1, true);
    let mut obs = BitTable::new(1, 2);
    obs.set(0, 0, true);
    let merged = cli::merge_detections_observables(&dets, &obs);
    assert_eq!(merged.num_major(), 3);
    assert!(merged.get(0, 0));
    assert!(!merged.get(1, 0));
    assert!(merged.get(2, 0));
    assert!(!merged.get(0, 1));
    assert!(merged.get(1, 1));
    assert!(!merged.get(2, 1));
}

// ---------- make_rng ----------

#[test]
fn make_rng_with_seed_deterministic() {
    use rand::Rng;
    let mut rng1 = cli::make_rng(Some(123));
    let mut rng2 = cli::make_rng(Some(123));
    let v1: u64 = rng1.r#gen();
    let v2: u64 = rng2.r#gen();
    assert_eq!(v1, v2);
}

#[test]
fn make_rng_without_seed() {
    let _rng = cli::make_rng(None);
}

// ---------- read_input ----------

#[test]
fn read_input_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.stim");
    std::fs::write(&path, "R 0\nM 0").unwrap();
    let text = cli::read_input(Some(path.to_str().unwrap())).unwrap();
    assert_eq!(text, "R 0\nM 0");
}

#[test]
fn read_input_file_not_found() {
    let err = cli::read_input(Some("/nonexistent/path/file.stim"));
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("failed to read"));
}

// ---------- open_output ----------

#[test]
fn open_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.txt");
    {
        use std::io::Write;
        let mut w = cli::open_output(Some(path.to_str().unwrap())).unwrap();
        w.write_all(b"hello").unwrap();
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
}

#[test]
fn open_output_to_stdout() {
    let w = cli::open_output(None);
    assert!(w.is_ok());
}

#[test]
fn open_output_bad_path() {
    let result = cli::open_output(Some("/nonexistent/dir/file.txt"));
    assert!(result.is_err());
}

// ---------- run() dispatch ----------

#[test]
fn run_no_command_prints_version() {
    use clap::Parser;
    let cli = cli::Cli::parse_from(["rstim"]);
    let result = cli::run(cli);
    assert!(result.is_ok());
}

#[test]
fn run_sample_via_dispatch() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    std::fs::write(&circuit_path, "R 0\nX 0\nM 0").unwrap();
    let out_path = dir.path().join("out.txt");
    let cli = cli::Cli::parse_from([
        "rstim", "sample", "--shots", "1",
        "--in", circuit_path.to_str().unwrap(),
        "--out", out_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    assert_eq!(std::fs::read_to_string(&out_path).unwrap().trim(), "1");
}

#[test]
fn run_detect_via_dispatch() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    std::fs::write(&circuit_path, "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let out_path = dir.path().join("out.txt");
    let cli = cli::Cli::parse_from([
        "rstim", "detect", "--shots", "1",
        "--in", circuit_path.to_str().unwrap(),
        "--out", out_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    assert_eq!(std::fs::read_to_string(&out_path).unwrap().trim(), "1");
}

#[test]
fn run_analyze_errors_via_dispatch() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    std::fs::write(&circuit_path, "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let out_path = dir.path().join("out.dem");
    let cli = cli::Cli::parse_from([
        "rstim", "analyze_errors",
        "--in", circuit_path.to_str().unwrap(),
        "--out", out_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    let dem = std::fs::read_to_string(&out_path).unwrap();
    assert!(dem.contains("error(0.1)"));
    assert!(dem.contains("D0"));
}

#[test]
fn run_sample_dem_via_dispatch() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let dem_path = dir.path().join("test.dem");
    std::fs::write(&dem_path, "error(1) D0 L0").unwrap();
    let out_path = dir.path().join("out.txt");
    let cli = cli::Cli::parse_from([
        "rstim", "sample_dem", "--shots", "1",
        "--in", dem_path.to_str().unwrap(),
        "--out", out_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    assert_eq!(std::fs::read_to_string(&out_path).unwrap().trim(), "1");
}

#[test]
fn run_sample_dem_with_obs_via_dispatch() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let dem_path = dir.path().join("test.dem");
    std::fs::write(&dem_path, "error(1) D0 L0").unwrap();
    let out_path = dir.path().join("out.txt");
    let obs_path = dir.path().join("obs.txt");
    let cli = cli::Cli::parse_from([
        "rstim", "sample_dem", "--shots", "1",
        "--in", dem_path.to_str().unwrap(),
        "--out", out_path.to_str().unwrap(),
        "--obs_out", obs_path.to_str().unwrap(),
    ]);
    cli::run(cli).unwrap();
    assert_eq!(std::fs::read_to_string(&out_path).unwrap().trim(), "1");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "1");
}

// ---------- run_sample ----------

#[test]
fn run_sample_01() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nX 0\nM 0", 2, "01", Some(42), &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "1\n1\n");
}

#[test]
fn run_sample_b8() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nX 0\nM 0", 1, "b8", Some(42), &mut buf).unwrap();
    assert_eq!(buf, vec![0x01]);
}

#[test]
fn run_sample_r8() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nX 0\nM 0", 1, "r8", Some(42), &mut buf).unwrap();
    assert_eq!(buf, vec![0, 0]);
}

#[test]
fn run_sample_hits() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nX 0\nM 0", 1, "hits", Some(42), &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "0\n");
}

#[test]
fn run_sample_dets_rejected() {
    let mut buf = Vec::new();
    let err = cli::run_sample("R 0\nM 0", 1, "dets", Some(42), &mut buf);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("dets"));
}

#[test]
fn run_sample_invalid_format() {
    let mut buf = Vec::new();
    let err = cli::run_sample("R 0\nM 0", 1, "bad", Some(42), &mut buf);
    assert!(err.is_err());
}

#[test]
fn run_sample_noiseless() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nM 0", 3, "01", Some(42), &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "0\n0\n0\n");
}

#[test]
fn run_sample_seed_deterministic() {
    let mut buf1 = Vec::new();
    let mut buf2 = Vec::new();
    cli::run_sample("H 0\nM 0", 10, "01", Some(99), &mut buf1).unwrap();
    cli::run_sample("H 0\nM 0", 10, "01", Some(99), &mut buf2).unwrap();
    assert_eq!(buf1, buf2);
}

#[test]
fn run_sample_no_seed() {
    let mut buf = Vec::new();
    cli::run_sample("R 0\nM 0", 1, "01", None, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.trim() == "0" || s.trim() == "1");
}

// ---------- run_detect ----------

#[test]
fn run_detect_01_noiseless() {
    let mut buf = Vec::new();
    cli::run_detect("R 0\nM 0\nDETECTOR rec[-1]", 3, "01", Some(42), false, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "0\n0\n0\n");
}

#[test]
fn run_detect_dets_with_error() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
        1, "dets", Some(42), false, &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "shot D0");
}

#[test]
fn run_detect_dets_with_observable() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        1, "dets", Some(42), false, &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn run_detect_append_observables() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        1, "01", Some(42), true, &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "11");
}

#[test]
fn run_detect_no_append() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        1, "01", Some(42), false, &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "1");
}

#[test]
fn run_detect_b8() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
        1, "b8", Some(42), false, &mut buf,
    ).unwrap();
    assert_eq!(buf, vec![0x01]);
}

#[test]
fn run_detect_r8() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
        1, "r8", Some(42), false, &mut buf,
    ).unwrap();
    assert_eq!(buf, vec![0, 0]);
}

#[test]
fn run_detect_hits() {
    let mut buf = Vec::new();
    cli::run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
        1, "hits", Some(42), false, &mut buf,
    ).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap().trim(), "0");
}

#[test]
fn run_detect_seed_deterministic() {
    let mut buf1 = Vec::new();
    let mut buf2 = Vec::new();
    cli::run_detect("H 0\nM 0\nDETECTOR rec[-1]", 10, "01", Some(42), false, &mut buf1).unwrap();
    cli::run_detect("H 0\nM 0\nDETECTOR rec[-1]", 10, "01", Some(42), false, &mut buf2).unwrap();
    assert_eq!(buf1, buf2);
}

// ---------- run_analyze_errors ----------

#[test]
fn run_analyze_errors_basic() {
    let mut buf = Vec::new();
    cli::run_analyze_errors(
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
        &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("error(0.1)"));
    assert!(s.contains("D0"));
}

#[test]
fn run_analyze_errors_with_observable() {
    let mut buf = Vec::new();
    cli::run_analyze_errors(
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        &mut buf,
    ).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn run_analyze_errors_empty_circuit() {
    let mut buf = Vec::new();
    cli::run_analyze_errors("", &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.is_empty() || !s.contains("error"));
}

#[test]
fn run_analyze_errors_with_default_options_still_rejects_gauge() {
    let mut buf = Vec::new();
    let err = cli::run_analyze_errors_with_options(
        "R 0\nH 0\nM 0\nDETECTOR rec[-1]",
        false,
        false,
        &mut buf,
    )
    .unwrap_err();
    assert!(err.contains("non-deterministic"));
}

#[test]
fn run_analyze_errors_options_only_change_behavior_when_enabled() {
    let mut strict_buf = Vec::new();
    let strict = cli::run_analyze_errors_with_options(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
        false,
        false,
        &mut strict_buf,
    );
    assert!(strict.is_err());

    let mut relaxed_buf = Vec::new();
    let relaxed = cli::run_analyze_errors_with_options(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
        true,
        false,
        &mut relaxed_buf,
    );
    assert!(relaxed.is_ok());
}

// ---------- run_sample_dem ----------

#[test]
fn run_sample_dem_01() {
    let mut buf = Vec::new();
    cli::run_sample_dem("error(1) D0 L0", 3, "01", Some(42), &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    for line in s.trim().split('\n') {
        assert_eq!(line, "1");
    }
}

#[test]
fn run_sample_dem_dets() {
    let mut buf = Vec::new();
    cli::run_sample_dem("error(1) D0 D1", 1, "dets", Some(42), &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.trim(), "shot D0 D1");
}

#[test]
fn run_sample_dem_b8() {
    let mut buf = Vec::new();
    cli::run_sample_dem("error(1) D0", 1, "b8", Some(42), &mut buf).unwrap();
    assert_eq!(buf, vec![0x01]);
}

#[test]
fn run_sample_dem_r8() {
    let mut buf = Vec::new();
    cli::run_sample_dem("error(1) D0", 1, "r8", Some(42), &mut buf).unwrap();
    assert_eq!(buf, vec![0, 0]);
}

#[test]
fn run_sample_dem_hits() {
    let mut buf = Vec::new();
    cli::run_sample_dem("error(1) D0", 1, "hits", Some(42), &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap().trim(), "0");
}

#[test]
fn run_sample_dem_seed_deterministic() {
    let mut buf1 = Vec::new();
    let mut buf2 = Vec::new();
    cli::run_sample_dem("error(0.5) D0", 10, "01", Some(99), &mut buf1).unwrap();
    cli::run_sample_dem("error(0.5) D0", 10, "01", Some(99), &mut buf2).unwrap();
    assert_eq!(buf1, buf2);
}

#[test]
fn run_sample_dem_invalid_format() {
    let mut buf = Vec::new();
    let err = cli::run_sample_dem("error(1) D0", 1, "bad", Some(42), &mut buf);
    assert!(err.is_err());
}

// ---------- run_sample_dem_with_obs ----------

#[test]
fn run_sample_dem_with_obs_01() {
    let mut det_buf = Vec::new();
    let mut obs_buf = Vec::new();
    cli::run_sample_dem_with_obs(
        "error(1) D0 L0", 1, "01", Some(42),
        &mut det_buf, &mut obs_buf, "01",
    ).unwrap();
    assert_eq!(String::from_utf8(det_buf).unwrap().trim(), "1");
    assert_eq!(String::from_utf8(obs_buf).unwrap().trim(), "1");
}

#[test]
fn run_sample_dem_with_obs_dets_format() {
    let mut det_buf = Vec::new();
    let mut obs_buf = Vec::new();
    cli::run_sample_dem_with_obs(
        "error(1) D0 L0", 1, "dets", Some(42),
        &mut det_buf, &mut obs_buf, "01",
    ).unwrap();
    let det_s = String::from_utf8(det_buf).unwrap();
    assert!(det_s.contains("D0"));
    assert!(det_s.contains("L0"));
    assert_eq!(String::from_utf8(obs_buf).unwrap().trim(), "1");
}

#[test]
fn run_sample_dem_with_obs_b8() {
    let mut det_buf = Vec::new();
    let mut obs_buf = Vec::new();
    cli::run_sample_dem_with_obs(
        "error(1) D0 L0", 1, "b8", Some(42),
        &mut det_buf, &mut obs_buf, "b8",
    ).unwrap();
    assert_eq!(det_buf, vec![0x01]);
    assert_eq!(obs_buf, vec![0x01]);
}

// ---------- pipeline ----------

#[test]
fn pipeline_analyze_then_sample_dem() {
    let mut dem_buf = Vec::new();
    cli::run_analyze_errors(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        &mut dem_buf,
    ).unwrap();
    let dem_text = String::from_utf8(dem_buf).unwrap();

    let mut result_buf = Vec::new();
    cli::run_sample_dem(&dem_text, 1, "dets", Some(42), &mut result_buf).unwrap();
    let s = String::from_utf8(result_buf).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}
