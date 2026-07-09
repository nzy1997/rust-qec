use std::fs;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::Deserialize;

use rstim::compiled::{choose_sampler_path, compile_circuit, CompiledPathDecision};
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch_with_options, BatchOutput, SampleOptions, SamplingBackend};
use rstim::sim::bit_table::BitTable;
use rstim::stats::summarize;

const DETERMINISTIC_SEEDS: &[u64] = &[0, 1, 7, 0x5eed_387];

#[derive(Debug, Deserialize)]
struct Manifest {
    cases: Vec<CatalogCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogCase {
    case_id: String,
    tier: String,
    canonical_input_path: String,
    shots: usize,
    expected_qubits: usize,
    expected_measurements: usize,
    expected_detectors: usize,
    expected_observables: usize,
}

#[derive(Debug)]
struct ParsedCase {
    case: CatalogCase,
    instrs: Vec<StimInstr>,
}

struct SamplePair {
    interpreted: BatchOutput,
    compiled: BatchOutput,
}

#[derive(Debug, PartialEq, Eq)]
enum CaseOutcome {
    Checked,
    Fallback(String),
}

#[test]
fn compiled_and_interpreted_sample_paths_agree_on_catalog() {
    let manifest_path = smoke_manifest_path();
    let manifest = load_manifest(&manifest_path).expect("smoke manifest");
    let manifest_dir = manifest_path.parent().expect("manifest directory");
    let mut compiled_checked = 0usize;
    let mut fallback_recorded = 0usize;

    for case in &manifest.cases {
        let parsed = validate_case_metadata(case, manifest_dir).expect("fixture metadata");
        if parsed.case.tier == "documentation-only" {
            continue;
        }

        match compare_sample_paths(&parsed).expect("sample path comparison") {
            CaseOutcome::Checked => compiled_checked += 1,
            CaseOutcome::Fallback(reason) => {
                assert!(
                    !reason.trim().is_empty(),
                    "{} fallback reason must be explicit",
                    parsed.case.case_id
                );
                fallback_recorded += 1;
            }
        }
    }

    assert!(
        compiled_checked > 0,
        "no compiled-capable catalog cases were checked"
    );
    println!(
        "checked {compiled_checked} compiled-capable catalog cases; recorded {fallback_recorded} fallback cases"
    );
}

#[test]
fn statistical_contract_rejects_injected_detector_or_observable_mismatch() {
    let parsed = first_compiled_capable_case().expect("compiled-capable catalog case");
    let mut pair = sample_pair(&parsed, DETERMINISTIC_SEEDS[0]).expect("sample pair");
    inject_comparison_row_mismatch(&pair.interpreted, &mut pair.compiled)
        .expect("comparison row");

    let err = assert_streams_agree(
        &parsed.case.case_id,
        DETERMINISTIC_SEEDS[0],
        &pair.interpreted,
        &pair.compiled,
    )
    .expect_err("injected bit flip should be rejected");

    assert!(
        err.contains("statistical mismatch"),
        "expected statistical mismatch, got {err}"
    );
}

#[test]
fn metadata_contract_rejects_mismatched_detector_counts() {
    let manifest_path = smoke_manifest_path();
    let mut manifest = load_manifest(&manifest_path).expect("smoke manifest");
    manifest.cases[0].expected_detectors += 1;

    let err = validate_case_metadata(
        &manifest.cases[0],
        manifest_path.parent().expect("manifest directory"),
    )
    .expect_err("bad detector count should be rejected");

    assert!(
        err.contains("metadata mismatch"),
        "expected metadata mismatch, got {err}"
    );
}

fn smoke_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("benchmarks/rstim_vs_stim_simulator/cases.smoke.toml")
}

fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn validate_case_metadata(case: &CatalogCase, manifest_dir: &Path) -> Result<ParsedCase, String> {
    let input_path = manifest_dir.join(&case.canonical_input_path);
    let text = fs::read_to_string(&input_path).map_err(|err| {
        format!(
            "metadata mismatch: {} canonical input {} could not be read: {err}",
            case.case_id,
            input_path.display()
        )
    })?;
    let instrs = parse_lines(&text).map_err(|err| {
        format!(
            "metadata mismatch: {} canonical input {} could not be parsed by rstim: {err}",
            case.case_id,
            input_path.display()
        )
    })?;
    let summary = summarize(&instrs);

    assert_metadata_count(
        case,
        "expected_qubits",
        case.expected_qubits,
        summary.num_qubits,
    )?;
    assert_metadata_count(
        case,
        "expected_measurements",
        case.expected_measurements,
        summary.num_measurements,
    )?;
    assert_metadata_count(
        case,
        "expected_detectors",
        case.expected_detectors,
        summary.num_detectors,
    )?;
    assert_metadata_count(
        case,
        "expected_observables",
        case.expected_observables,
        summary.num_observables,
    )?;

    Ok(ParsedCase {
        case: case.clone(),
        instrs,
    })
}

fn assert_metadata_count(
    case: &CatalogCase,
    field: &str,
    expected: usize,
    actual: usize,
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    Err(format!(
        "metadata mismatch: {} {field} manifest expected {expected}, rstim observed {actual}",
        case.case_id
    ))
}

fn compare_sample_paths(parsed: &ParsedCase) -> Result<CaseOutcome, String> {
    let compiled = compile_circuit(&parsed.instrs)?;
    match choose_sampler_path(&compiled) {
        CompiledPathDecision::FastPath => {
            for &seed in DETERMINISTIC_SEEDS {
                let pair = sample_pair(parsed, seed)?;
                assert_output_metadata(
                    &parsed.case,
                    seed,
                    SamplingBackend::Interpreted,
                    &pair.interpreted,
                )?;
                assert_output_metadata(
                    &parsed.case,
                    seed,
                    SamplingBackend::Compiled,
                    &pair.compiled,
                )?;
                assert_streams_agree(
                    &parsed.case.case_id,
                    seed,
                    &pair.interpreted,
                    &pair.compiled,
                )?;
            }
            Ok(CaseOutcome::Checked)
        }
        CompiledPathDecision::Fallback(reason) => Ok(CaseOutcome::Fallback(reason.to_string())),
    }
}

fn sample_pair(parsed: &ParsedCase, seed: u64) -> Result<SamplePair, String> {
    let mut interpreted_rng = StdRng::seed_from_u64(seed);
    let interpreted = sample_batch_with_options(
        &parsed.instrs,
        parsed.case.shots,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )?;

    let mut compiled_rng = StdRng::seed_from_u64(seed);
    let compiled = sample_batch_with_options(
        &parsed.instrs,
        parsed.case.shots,
        &mut compiled_rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )?;

    Ok(SamplePair {
        interpreted,
        compiled,
    })
}

fn assert_output_metadata(
    case: &CatalogCase,
    seed: u64,
    backend: SamplingBackend,
    output: &BatchOutput,
) -> Result<(), String> {
    assert_table_shape(
        case,
        seed,
        backend,
        "measurements",
        &output.measurements,
        case.expected_measurements,
    )?;
    assert_table_shape(
        case,
        seed,
        backend,
        "detections",
        &output.detections,
        case.expected_detectors,
    )?;
    assert_table_shape(
        case,
        seed,
        backend,
        "observable_flips",
        &output.observable_flips,
        case.expected_observables,
    )
}

fn assert_table_shape(
    case: &CatalogCase,
    seed: u64,
    backend: SamplingBackend,
    label: &str,
    table: &BitTable,
    expected_major: usize,
) -> Result<(), String> {
    if table.num_major() != expected_major {
        return Err(format!(
            "metadata mismatch: {} seed {seed} {backend:?} {label} rows {}, expected {expected_major}",
            case.case_id,
            table.num_major()
        ));
    }
    if table.num_minor() != case.shots {
        return Err(format!(
            "metadata mismatch: {} seed {seed} {backend:?} {label} shots {}, expected {}",
            case.case_id,
            table.num_minor(),
            case.shots
        ));
    }
    Ok(())
}

fn assert_streams_agree(
    case_id: &str,
    seed: u64,
    interpreted: &BatchOutput,
    compiled: &BatchOutput,
) -> Result<(), String> {
    assert_table_agrees(
        case_id,
        seed,
        "measurements",
        &interpreted.measurements,
        &compiled.measurements,
    )?;
    assert_table_agrees(
        case_id,
        seed,
        "detections",
        &interpreted.detections,
        &compiled.detections,
    )?;
    assert_table_agrees(
        case_id,
        seed,
        "observable_flips",
        &interpreted.observable_flips,
        &compiled.observable_flips,
    )
}

fn assert_table_agrees(
    case_id: &str,
    seed: u64,
    label: &str,
    interpreted: &BitTable,
    compiled: &BitTable,
) -> Result<(), String> {
    if interpreted.num_major() != compiled.num_major()
        || interpreted.num_minor() != compiled.num_minor()
    {
        return Err(format!(
            "statistical mismatch: {case_id} seed {seed} {label} shape interpreted {}x{}, compiled {}x{}",
            interpreted.num_major(),
            interpreted.num_minor(),
            compiled.num_major(),
            compiled.num_minor()
        ));
    }

    for major in 0..interpreted.num_major() {
        let interpreted_count = row_true_count(interpreted, major);
        let compiled_count = row_true_count(compiled, major);
        let diff = interpreted_count.abs_diff(compiled_count);
        let tolerance = count_tolerance(interpreted_count, compiled_count, interpreted.num_minor());
        if diff > tolerance {
            return Err(format!(
                "statistical mismatch: {case_id} seed {seed} {label}[{major}] interpreted_count {interpreted_count}, compiled_count {compiled_count}, tolerance {tolerance}"
            ));
        }
    }
    Ok(())
}

fn row_true_count(table: &BitTable, row: usize) -> usize {
    (0..table.num_minor())
        .filter(|&shot| table.get(row, shot))
        .count()
}

fn count_tolerance(interpreted_count: usize, compiled_count: usize, shots: usize) -> usize {
    if shots == 0 {
        return 0;
    }
    let pooled_p = (interpreted_count + compiled_count) as f64 / (2 * shots) as f64;
    let sigma = (2.0 * shots as f64 * pooled_p * (1.0 - pooled_p)).sqrt();
    (5.0 * sigma).ceil().max(4.0) as usize
}

fn first_compiled_capable_case() -> Result<ParsedCase, String> {
    let manifest_path = smoke_manifest_path();
    let manifest = load_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().expect("manifest directory");

    for case in &manifest.cases {
        if case.tier == "documentation-only" {
            continue;
        }
        let parsed = validate_case_metadata(case, manifest_dir)?;
        let compiled = compile_circuit(&parsed.instrs)?;
        if choose_sampler_path(&compiled) == CompiledPathDecision::FastPath {
            return Ok(parsed);
        }
    }

    Err("no compiled-capable catalog case found".to_string())
}

fn inject_comparison_row_mismatch(
    reference: &BatchOutput,
    output: &mut BatchOutput,
) -> Result<(), String> {
    if output.detections.num_major() > 0 && output.detections.num_minor() > 0 {
        force_row_to_farthest_constant(&reference.detections, &mut output.detections, 0);
        return Ok(());
    }
    if output.observable_flips.num_major() > 0 && output.observable_flips.num_minor() > 0 {
        force_row_to_farthest_constant(
            &reference.observable_flips,
            &mut output.observable_flips,
            0,
        );
        return Ok(());
    }
    Err("sample output has no detector or observable comparison bit".to_string())
}

fn force_row_to_farthest_constant(reference: &BitTable, output: &mut BitTable, row: usize) {
    let reference_ones = row_true_count(reference, row);
    let forced_value = reference_ones <= reference.num_minor() / 2;
    for shot in 0..output.num_minor() {
        output.set(row, shot, forced_value);
    }
}
