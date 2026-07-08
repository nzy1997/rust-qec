use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::compiled::{
    choose_sampler_path, compile_circuit, CompiledBlock, CompiledOp, CompiledPathDecision,
};
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch_with_options, SampleOptions, SamplingBackend};
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

    assert_eq!(choose_sampler_path(&compiled), CompiledPathDecision::FastPath);

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);

    assert!(counts.depolarize1 > 0, "fixture should lower DEPOLARIZE1");
    assert!(counts.depolarize2 > 0, "fixture should lower DEPOLARIZE2");
    assert!(counts.cx > 0, "fixture should lower CX");
    assert!(counts.measure_reset > 0, "fixture should lower MR");
    assert!(counts.measure > 0, "fixture should lower M");
    assert!(counts.detector > 0, "fixture should lower DETECTOR");
    assert!(counts.observable > 0, "fixture should lower OBSERVABLE_INCLUDE");
    assert_eq!(counts.unsupported, 0, "selected fixture must not contain fallback markers");
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
    assert_eq!(choose_sampler_path(&compiled), CompiledPathDecision::FastPath);

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
        CompiledPathDecision::Fallback("loss instructions require the interpreted path")
    );
    assert_eq!(
        choose_sampler_path(&feedback),
        CompiledPathDecision::Fallback("feedback instructions require the interpreted path")
    );
}

#[test]
fn unsupported_sampler_ops_do_not_enter_typed_fast_path() {
    let compiled = compile_circuit(&parse_lines("S 0\nM 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        CompiledPathDecision::Fallback(
            "unsupported sampler instructions require the interpreted path",
        )
    );

    let mut counts = VariantCounts::default();
    count_variants(&compiled.blocks, &mut counts);
    assert_eq!(counts.unsupported, 1);
}
