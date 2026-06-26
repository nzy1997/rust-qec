use rbposd::{OsdVariant, ParityCheckMatrix};
use serde::Deserialize;

use rsinter::bb_circuit_memory::{
    EffectiveDecoderModel, ProfileReplayBasis, SimulationConfig, SyndromeReplayDiagnostic,
    build_code, build_effective_models, build_syndrome_cycle, profile_syndrome_replay,
    profile_syndrome_replay_for_basis, profile_syndrome_replay_with_candidate_limit,
    profile_syndrome_replay_with_candidate_limit_for_basis,
    profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant,
    replay_syndrome_diagnostic, replay_syndrome_diagnostic_with_osd_variant, sample_seeded_trial,
};

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/bb_circuit_bposd/bb90_hard_syndrome.json"
);
const PROFILE_CANDIDATE_LIMIT: usize = 16;

#[derive(Debug, Clone, Deserialize)]
struct HardSyndromeFixture {
    case_id: String,
    code_id: String,
    physical_error_rate: f64,
    num_cycles: usize,
    seed: u64,
    max_bp_iterations: usize,
    osd_order: usize,
    basis: FixtureBasis,
    syndrome_support: Vec<usize>,
    expected_sampled_logical: Vec<bool>,
    syndrome_weight: usize,
    expected_osd0_logical_prediction: Vec<bool>,
    expected_logical_prediction_decode_order: usize,
    expected_osd_use: bool,
    expected_bp_converged: bool,
    expected_bp_iterations: usize,
    expected_residual_syndrome_weight: usize,
    expected_free_column_count: usize,
    expected_candidate_search_frontier_size: usize,
    expected_max_candidate_order: usize,
    expected_planned_candidate_count: u128,
    why_hard: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum FixtureBasis {
    X,
    Z,
}

struct ComputedFixtureReplay {
    sampled_syndrome: Vec<bool>,
    sampled_support: Vec<usize>,
    sampled_logical: Vec<bool>,
    model: EffectiveDecoderModel,
    diagnostic: SyndromeReplayDiagnostic,
}

#[test]
fn bb90_hard_syndrome_fixture_replays_osd_order7() {
    let fixture = load_fixture();
    println!(
        "case_id={} code_id={} p={} num_cycles={} seed={} osd_order={}",
        fixture.case_id,
        fixture.code_id,
        fixture.physical_error_rate,
        fixture.num_cycles,
        fixture.seed,
        fixture.osd_order
    );
    validate_fixture(&fixture).unwrap();
}

#[test]
fn bb90_hard_syndrome_fixture_rejects_low_p_control() {
    let fixture = load_fixture();

    let mut low_p = fixture.clone();
    low_p.physical_error_rate = 1.0e-12;
    assert!(
        validate_fixture(&low_p)
            .unwrap_err()
            .contains("fixture physical_error_rate mismatch")
    );

    let mut osd0 = fixture.clone();
    osd0.osd_order = 0;
    assert!(
        validate_fixture(&osd0)
            .unwrap_err()
            .contains("fixture osd_order mismatch")
    );

    let mut wrong_seed = fixture.clone();
    wrong_seed.seed += 1;
    assert!(
        validate_fixture(&wrong_seed)
            .unwrap_err()
            .contains("fixture seed mismatch")
    );

    let mut wrong_code = fixture.clone();
    wrong_code.code_id = "bb144".to_owned();
    assert!(
        validate_fixture(&wrong_code)
            .unwrap_err()
            .contains("expected BB90 shape")
    );

    let computed = compute_fixture_replay(&fixture).unwrap();

    let mut wrong_support = fixture.clone();
    wrong_support.syndrome_support[0] += 1;
    assert!(
        validate_computed_fixture(&wrong_support, &computed)
            .unwrap_err()
            .contains("sampled syndrome support mismatch")
    );

    let mut wrong_prediction = fixture.clone();
    wrong_prediction.expected_osd0_logical_prediction[0] ^= true;
    assert!(
        validate_computed_fixture(&wrong_prediction, &computed)
            .unwrap_err()
            .contains("OSD-0 logical prediction mismatch")
    );

    let mut wrong_prediction_order = fixture.clone();
    wrong_prediction_order.expected_logical_prediction_decode_order = 7;
    assert!(
        validate_computed_fixture(&wrong_prediction_order, &computed)
            .unwrap_err()
            .contains("fixture expected_logical_prediction_decode_order mismatch")
    );

    let mut wrong_frontier = fixture.clone();
    wrong_frontier.expected_candidate_search_frontier_size += 1;
    assert!(
        validate_computed_fixture(&wrong_frontier, &computed)
            .unwrap_err()
            .contains("candidate_search_frontier_size mismatch")
    );
}

#[test]
fn bb90_hard_syndrome_reports_osd_profile_counters() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let profile = profile_syndrome_replay_with_candidate_limit_for_basis(
        profile_replay_basis(fixture.basis),
        &computed.model,
        &computed.sampled_syndrome,
        fixture.max_bp_iterations,
        fixture.osd_order,
        PROFILE_CANDIDATE_LIMIT,
    )
    .unwrap();

    println!(
        "case_id={} basis={:?} syndrome_weight={} candidate_limit={} profile={profile:#?}",
        fixture.case_id,
        fixture.basis,
        computed.sampled_support.len(),
        PROFILE_CANDIDATE_LIMIT,
    );

    assert!(profile.decode_call_count > 0);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    let (expected_z_calls, expected_x_calls) = expected_basis_calls(fixture.basis);
    assert_eq!(profile.z_decode_call_count, expected_z_calls);
    assert_eq!(profile.x_decode_call_count, expected_x_calls);
    assert!(profile.osd_use_count > 0);
    assert_eq!(profile.osd_candidate_count, PROFILE_CANDIDATE_LIMIT);
    assert_eq!(profile.gf2_solve_count, 1);
    assert_eq!(profile.gf2_full_elimination_count, 1);
}

#[test]
fn bb90_hard_syndrome_ldpc_cs_candidate_count_is_bounded() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let diagnostic = replay_syndrome_diagnostic_with_osd_variant(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();

    let expected_pair_count = 21u128;
    println!(
        "case_id={} planner={} free_columns={} planned_candidates={} legacy_candidates={}",
        fixture.case_id,
        diagnostic.osd_planner,
        diagnostic.free_column_count,
        diagnostic.planned_candidate_count,
        fixture.expected_planned_candidate_count
    );

    assert_eq!(diagnostic.osd_planner, "ldpc_osd_cs");
    assert_eq!(diagnostic.osd_order, 7);
    assert!(diagnostic.free_column_count > 0);
    assert_eq!(diagnostic.candidate_search_frontier_size, 7);
    assert_eq!(diagnostic.max_candidate_order, 2);
    assert_eq!(
        diagnostic.planned_candidate_count,
        diagnostic.free_column_count as u128 + expected_pair_count
    );
    assert!(diagnostic.planned_candidate_count < fixture.expected_planned_candidate_count);

    let profile = profile_syndrome_replay_with_candidate_limit_for_basis_and_osd_variant(
        profile_replay_basis(fixture.basis),
        &computed.model,
        &computed.sampled_syndrome,
        fixture.max_bp_iterations,
        fixture.osd_order,
        PROFILE_CANDIDATE_LIMIT,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();
    assert_eq!(profile.osd_candidate_count, PROFILE_CANDIDATE_LIMIT);
    assert_eq!(profile.gf2_solve_count, 1);
}

#[test]
fn bb90_hard_syndrome_legacy_osd_plan_still_reports_exhaustive_frontier() {
    let fixture = load_fixture();
    let computed = compute_fixture_replay(&fixture).unwrap();
    let legacy = replay_syndrome_diagnostic(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
    )
    .unwrap();
    let ldpc = replay_syndrome_diagnostic_with_osd_variant(
        &computed.model,
        &computed.sampled_syndrome,
        fixture.expected_sampled_logical.len(),
        fixture.max_bp_iterations,
        fixture.osd_order,
        OsdVariant::LdpcCombinationSweep,
    )
    .unwrap();

    assert_eq!(legacy.osd_planner, "legacy_combination_sweep");
    assert_eq!(
        legacy.planned_candidate_count,
        fixture.expected_planned_candidate_count
    );
    assert_eq!(legacy.planned_candidate_count, 26_332);
    assert_ne!(legacy.osd_planner, ldpc.osd_planner);
    assert_ne!(legacy.planned_candidate_count, ldpc.planned_candidate_count);
}

#[test]
fn syndrome_profile_replay_routes_x_basis_counts() {
    let fixture = load_fixture();
    let code = build_code(&fixture.code_id).unwrap();
    let cycle = build_syndrome_cycle(&code);
    let config = SimulationConfig {
        physical_error_rate: 1.0e-12,
        num_cycles: 1,
        num_trials: 1,
        seed: Some(1),
        max_bp_iterations: 10,
        osd_order: 0,
    };
    let models = build_effective_models(&code, &cycle, &config).unwrap();
    let syndrome = vec![false; models.x_faults.decoder.num_checks()];
    let profile = profile_syndrome_replay_for_basis(
        ProfileReplayBasis::X,
        &models.x_faults,
        &syndrome,
        config.max_bp_iterations,
        config.osd_order,
    )
    .unwrap();

    assert_eq!(profile.decode_call_count, 1);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    assert_eq!(profile.z_decode_call_count, 0);
    assert_eq!(profile.x_decode_call_count, 1);
    assert_eq!(profile.osd_candidate_count, 0);
}

#[test]
fn syndrome_profile_replay_reports_nontrivial_osd_counts() {
    let model = EffectiveDecoderModel {
        decoder: ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap(),
        augmented_columns: vec![vec![0], vec![0], vec![0]],
        channel_probs: vec![0.49, 0.48, 0.47],
        first_logical_row: 1,
    };
    let profile = profile_syndrome_replay(&model, &[true], 0, 2).unwrap();

    assert_eq!(profile.decode_call_count, 1);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    assert_eq!(profile.z_decode_call_count, 1);
    assert_eq!(profile.x_decode_call_count, 0);
    assert_eq!(profile.osd_use_count, 1);
    assert!(profile.osd_candidate_count > 0);
    assert_eq!(profile.gf2_solve_count, 1);
    assert_eq!(profile.gf2_full_elimination_count, 1);
}

#[test]
fn syndrome_profile_replay_candidate_limit_wrapper_routes_z_basis_counts() {
    let model = EffectiveDecoderModel {
        decoder: ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap(),
        augmented_columns: vec![vec![0], vec![0], vec![0]],
        channel_probs: vec![0.49, 0.48, 0.47],
        first_logical_row: 1,
    };
    let profile = profile_syndrome_replay_with_candidate_limit(&model, &[true], 0, 2, 1).unwrap();

    assert_eq!(profile.decode_call_count, 1);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    assert_eq!(profile.z_decode_call_count, 1);
    assert_eq!(profile.x_decode_call_count, 0);
    assert_eq!(profile.osd_use_count, 1);
    assert_eq!(profile.osd_candidate_count, 1);
    assert_eq!(profile.gf2_solve_count, 1);
    assert_eq!(profile.gf2_full_elimination_count, 1);
}

fn load_fixture() -> HardSyndromeFixture {
    let text = std::fs::read_to_string(FIXTURE_PATH).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn profile_replay_basis(basis: FixtureBasis) -> ProfileReplayBasis {
    match basis {
        FixtureBasis::Z => ProfileReplayBasis::Z,
        FixtureBasis::X => ProfileReplayBasis::X,
    }
}

fn expected_basis_calls(basis: FixtureBasis) -> (usize, usize) {
    match basis {
        FixtureBasis::Z => (1, 0),
        FixtureBasis::X => (0, 1),
    }
}

fn validate_fixture(fixture: &HardSyndromeFixture) -> Result<(), String> {
    let computed = compute_fixture_replay(fixture)?;
    validate_computed_fixture(fixture, &computed)
}

fn compute_fixture_replay(fixture: &HardSyndromeFixture) -> Result<ComputedFixtureReplay, String> {
    validate_fixture_metadata(fixture)?;

    let code = build_code(&fixture.code_id)?;
    if (code.ell(), code.m(), code.n2(), code.n(), code.k()) != (15, 3, 45, 90, 8) {
        return Err(format!(
            "expected BB90 shape (ell=15, m=3, n2=45, n=90, k=8), got (ell={}, m={}, n2={}, n={}, k={})",
            code.ell(),
            code.m(),
            code.n2(),
            code.n(),
            code.k()
        ));
    }

    let cycle = build_syndrome_cycle(&code);
    let config = SimulationConfig {
        physical_error_rate: fixture.physical_error_rate,
        num_cycles: fixture.num_cycles,
        num_trials: 1,
        seed: Some(fixture.seed),
        max_bp_iterations: fixture.max_bp_iterations,
        osd_order: fixture.osd_order,
    };
    let models = build_effective_models(&code, &cycle, &config)?;
    let sampled = sample_seeded_trial(
        &code,
        &cycle,
        fixture.num_cycles,
        fixture.physical_error_rate,
        fixture.seed,
    )?;

    let (sampled_syndrome, sampled_logical, model) = match fixture.basis {
        FixtureBasis::Z => (&sampled.z_syndrome, &sampled.z_logical, &models.z_faults),
        FixtureBasis::X => (&sampled.x_syndrome, &sampled.x_logical, &models.x_faults),
    };

    Ok(ComputedFixtureReplay {
        sampled_syndrome: sampled_syndrome.clone(),
        sampled_support: syndrome_support(sampled_syndrome),
        sampled_logical: sampled_logical.clone(),
        model: model.clone(),
        diagnostic: replay_syndrome_diagnostic(
            model,
            sampled_syndrome,
            code.k(),
            fixture.max_bp_iterations,
            fixture.osd_order,
        )?,
    })
}

fn validate_computed_fixture(
    fixture: &HardSyndromeFixture,
    computed: &ComputedFixtureReplay,
) -> Result<(), String> {
    validate_fixture_metadata(fixture)?;

    if computed.sampled_support != fixture.syndrome_support {
        return Err(format!(
            "sampled syndrome support mismatch: expected {:?}, got {:?}",
            fixture.syndrome_support, computed.sampled_support
        ));
    }
    if computed.sampled_logical.as_slice() != fixture.expected_sampled_logical.as_slice() {
        return Err(format!(
            "sampled logical mismatch: expected {:?}, got {:?}",
            fixture.expected_sampled_logical, computed.sampled_logical
        ));
    }

    let diagnostic = &computed.diagnostic;
    expect_eq(
        "syndrome_weight",
        fixture.syndrome_weight,
        diagnostic.syndrome_weight,
    )?;
    expect_eq(
        "OSD-0 logical prediction",
        fixture.expected_osd0_logical_prediction.as_slice(),
        diagnostic.osd0_logical_prediction.as_slice(),
    )?;
    expect_eq("used_osd", fixture.expected_osd_use, diagnostic.used_osd)?;
    expect_eq(
        "bp_converged",
        fixture.expected_bp_converged,
        diagnostic.bp_converged,
    )?;
    expect_eq(
        "bp_iterations",
        fixture.expected_bp_iterations,
        diagnostic.bp_iterations,
    )?;
    expect_eq(
        "residual_syndrome_weight",
        fixture.expected_residual_syndrome_weight,
        diagnostic.residual_syndrome_weight,
    )?;
    expect_eq("osd_order", fixture.osd_order, diagnostic.osd_order)?;
    expect_eq(
        "free_column_count",
        fixture.expected_free_column_count,
        diagnostic.free_column_count,
    )?;
    expect_eq(
        "candidate_search_frontier_size",
        fixture.expected_candidate_search_frontier_size,
        diagnostic.candidate_search_frontier_size,
    )?;
    expect_eq(
        "max_candidate_order",
        fixture.expected_max_candidate_order,
        diagnostic.max_candidate_order,
    )?;
    expect_eq(
        "planned_candidate_count",
        fixture.expected_planned_candidate_count,
        diagnostic.planned_candidate_count,
    )?;
    if fixture.why_hard.trim().is_empty() {
        return Err("fixture should explain why this case is hard".to_owned());
    }

    Ok(())
}

fn validate_fixture_metadata(fixture: &HardSyndromeFixture) -> Result<(), String> {
    expect_eq(
        "fixture case_id",
        "bb90-p006-c10-seed12345-order7-hard-syndrome",
        fixture.case_id.as_str(),
    )?;
    expect_eq(
        "fixture physical_error_rate",
        0.006f64,
        fixture.physical_error_rate,
    )?;
    expect_eq("fixture num_cycles", 10usize, fixture.num_cycles)?;
    expect_eq("fixture seed", 12345u64, fixture.seed)?;
    expect_eq(
        "fixture max_bp_iterations",
        10_000usize,
        fixture.max_bp_iterations,
    )?;
    expect_eq("fixture osd_order", 7usize, fixture.osd_order)?;
    expect_eq(
        "fixture expected_logical_prediction_decode_order",
        0usize,
        fixture.expected_logical_prediction_decode_order,
    )?;
    expect_eq("fixture basis", FixtureBasis::Z, fixture.basis)?;
    Ok(())
}

fn expect_eq<T>(label: &str, expected: T, actual: T) -> Result<(), String>
where
    T: PartialEq + std::fmt::Debug,
{
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "{label} mismatch: expected {expected:?}, got {actual:?}"
        ))
    }
}

fn syndrome_support(bits: &[bool]) -> Vec<usize> {
    bits.iter()
        .enumerate()
        .filter_map(|(index, bit)| bit.then_some(index))
        .collect()
}
