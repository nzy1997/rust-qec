use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::compiled::{
    CompiledBlock, CompiledOp, SamplerPathDecision, SamplingFallbackReason, choose_sampler_path,
    compile_circuit, sample_compiled_batch,
};
use rstim::ir::{StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};
use rstim::sim::bit_table::BitTable;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);

#[derive(Default)]
struct VariantCounts {
    cx: usize,
    depolarize1: usize,
    depolarize2: usize,
    measure: usize,
    measure_reset: usize,
    detector: usize,
    observable: usize,
    noop: usize,
    unsupported: usize,
}

fn count_variants(blocks: &[CompiledBlock], counts: &mut VariantCounts) {
    for block in blocks {
        match block {
            CompiledBlock::Ops(ops) => {
                for op in ops {
                    match op {
                        CompiledOp::Cx { .. } => counts.cx += 1,
                        CompiledOp::Depolarize1 { .. } => counts.depolarize1 += 1,
                        CompiledOp::Depolarize2 { .. } => counts.depolarize2 += 1,
                        CompiledOp::Measure { .. } => counts.measure += 1,
                        CompiledOp::MeasureReset { .. } => counts.measure_reset += 1,
                        CompiledOp::Detector { .. } => counts.detector += 1,
                        CompiledOp::ObservableInclude { .. } => counts.observable += 1,
                        CompiledOp::NoOp => counts.noop += 1,
                        CompiledOp::UnsupportedSamplerOp { .. } => counts.unsupported += 1,
                        _ => {}
                    }
                }
            }
            CompiledBlock::Repeat(region) => count_variants(&region.body, counts),
        }
    }
}

fn bit_table_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|major| {
            (0..table.num_minor())
                .map(|minor| table.get(major, minor))
                .collect()
        })
        .collect()
}

#[test]
fn selected_surface_fixture_lowers_to_typed_sampler_ops() {
    let instrs = parse_lines(SURFACE_D11_R100).expect("parse selected fixture");
    let compiled = compile_circuit(&instrs).expect("compile selected fixture");

    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);

    assert!(counts.depolarize1 > 0, "fixture should lower DEPOLARIZE1");
    assert!(counts.depolarize2 > 0, "fixture should lower DEPOLARIZE2");
    assert!(counts.cx > 0, "fixture should lower CX");
    assert!(counts.measure_reset > 0, "fixture should lower MR");
    assert!(counts.measure > 0, "fixture should lower M");
    assert!(counts.detector > 0, "fixture should lower DETECTOR");
    assert!(
        counts.observable > 0,
        "fixture should lower OBSERVABLE_INCLUDE"
    );
    assert_eq!(
        counts.unsupported, 0,
        "selected fixture must not contain fallback markers"
    );
}

#[test]
fn compiled_sampler_ir_preserves_sample_bits_on_smoke_fixture() {
    let instrs = parse_lines(
        "R 0 1\n\
         X_ERROR(0.125) 0\n\
         H 0\n\
         CX 0 1\n\
         DEPOLARIZE1(0.125) 0\n\
         DEPOLARIZE2(0.125) 0 1\n\
         MR 1\n\
         X_ERROR(0.125) 0\n\
         M 0\n\
         DETECTOR rec[-1] rec[-2]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("parse smoke circuit");

    let compiled = compile_circuit(&instrs).expect("compile smoke circuit");
    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let mut interpreted_rng = StdRng::seed_from_u64(20260709);
    let mut compiled_rng = StdRng::seed_from_u64(20260709);

    let interpreted = sample_batch_with_options(
        &instrs,
        32,
        &mut interpreted_rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            ..SampleOptions::default()
        },
    )
    .expect("interpreted sample");
    let compiled = sample_batch_with_options(
        &instrs,
        32,
        &mut compiled_rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            ..SampleOptions::default()
        },
    )
    .expect("compiled sample");

    assert_eq!(
        bit_table_rows(&compiled.measurements),
        bit_table_rows(&interpreted.measurements)
    );
    assert_eq!(
        bit_table_rows(&compiled.detections),
        bit_table_rows(&interpreted.detections)
    );
    assert_eq!(
        bit_table_rows(&compiled.observable_flips),
        bit_table_rows(&interpreted.observable_flips)
    );
}

#[test]
fn loss_and_feedback_circuits_still_choose_fallback() {
    let loss = compile_circuit(&parse_lines("LOSS(1) 0\nMRL 0\n").unwrap()).unwrap();
    let feedback = compile_circuit(&parse_lines("M 0\nCX rec[-1] 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&loss),
        SamplerPathDecision::Fallback(SamplingFallbackReason::Loss)
    );
    assert_eq!(
        choose_sampler_path(&feedback),
        SamplerPathDecision::Fallback(SamplingFallbackReason::MeasurementRecordFeedback)
    );
}

#[test]
fn unsupported_sampler_ops_do_not_enter_typed_fast_path() {
    let compiled = compile_circuit(&parse_lines("S 0\nM 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::Fallback(SamplingFallbackReason::UnsupportedOperation(
            "S".to_string()
        ))
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);
    assert_eq!(counts.unsupported, 1);
}

#[test]
fn ideal_noop_sampler_ops_lower_to_typed_fast_path_ops() {
    let compiled = compile_circuit(
        &parse_lines("X 0\nI 0\nI_ERROR(0.25) 0\nII_ERROR(0.125) 0 1\nM 0\n").unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);

    assert_eq!(counts.unsupported, 0);
    assert!(
        counts.noop > 0,
        "ideal sampler no-op instructions should lower to CompiledOp::NoOp"
    );
}

#[test]
fn malformed_sampler_targets_lower_to_unsupported_markers() {
    let instrs = vec![
        StimInstr::new("H", vec![], vec![StimTarget::Sweep(0)]),
        StimInstr::new("H", vec![], vec![StimTarget::Rec(-1)]),
        StimInstr::new("M", vec![], vec![StimTarget::Sweep(0)]),
        StimInstr::new("M", vec![], vec![StimTarget::Rec(-1)]),
        StimInstr::new("CX", vec![], vec![StimTarget::Qubit(0)]),
        StimInstr::new(
            "CX",
            vec![],
            vec![StimTarget::Qubit(0), StimTarget::Rec(-1)],
        ),
        StimInstr::new("DETECTOR", vec![], vec![StimTarget::Qubit(0)]),
    ];
    let compiled = compile_circuit(&instrs).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::Fallback(SamplingFallbackReason::SweepDependent)
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);
    assert_eq!(counts.unsupported, 5);
}

#[test]
fn compiled_sampler_runs_y_basis_measurement_and_reset_variants() {
    let instrs = parse_lines("RY 0\nMY 0\nRY 1\nMRY 1\n").unwrap();
    let compiled = compile_circuit(&instrs).unwrap();
    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let mut rng = StdRng::seed_from_u64(415);
    let out = sample_batch_with_options(
        &instrs,
        16,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            output_mode: SampleOutputMode::MeasurementsOnly,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    assert_eq!(out.measurements.num_major(), 2);
    assert_eq!(out.measurements.num_minor(), 16);
}

#[test]
fn compiled_sampler_zero_probability_depolarize_ops_are_noops() {
    let instrs = parse_lines(
        "R 0 1\n\
         DEPOLARIZE1(0) 0\n\
         DEPOLARIZE2(0) 0 1\n\
         Y_ERROR(0) 0\n\
         Z_ERROR(0) 1\n\
         M 0 1\n",
    )
    .unwrap();
    let compiled = compile_circuit(&instrs).unwrap();
    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let mut rng = StdRng::seed_from_u64(416);
    let out = sample_batch_with_options(
        &instrs,
        8,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            output_mode: SampleOutputMode::MeasurementsOnly,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    assert_eq!(out.measurements.num_major(), 2);
    for measurement in 0..out.measurements.num_major() {
        for shot in 0..out.measurements.num_minor() {
            assert!(!out.measurements.get(measurement, shot));
        }
    }
}

#[test]
fn compiled_sampler_rejects_unsupported_marker_if_path_gate_is_bypassed() {
    let instrs = parse_lines("S 0\nM 0\n").unwrap();
    let compiled = compile_circuit(&instrs).unwrap();

    let mut rng = StdRng::seed_from_u64(417);
    let err = match sample_compiled_batch(&compiled, 4, &mut rng, SampleOptions::default()) {
        Ok(_) => panic!("unsupported marker should fail compiled execution"),
        Err(err) => err,
    };

    assert_eq!(err, "compiled sampler: unsupported instruction S");
}
