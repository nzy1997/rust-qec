use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentQueryLayout as Q, QueryPreparation, compile_affine,
    compile_unitary,
};
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{DensityBoundary, DensityLayout};
use serde_json::json;
type C = [i64; 4];
const PREFIX: &str = "H 0\nH 1\nH 2\nT 0\nT 1\nT 2\nCX 0 1\nH 0\nX_ERROR(1/5) 0\nT_DAG 0\nCX 0 2\nH 0\nH 1\nH 2\nX_ERROR(1/7) 1\nX_ERROR(1/11) 2";
const MEASURE: &str = "\nM 0\nM 1\nM 2\nDETECTOR rec[-3] rec[-1]\nPOSTSELECT det[0]";
const TARGET: &str = "H 0\nT 0\nH 0\nH 1\nS 1\nCX 1 2";
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| x >> j & 1 != 0)).unwrap()
}
fn add(mut a: C, b: C) -> C {
    for i in 0..4 {
        a[i] += b[i]
    }
    a
}
fn mul(a: C, b: C) -> C {
    let mut c = [0; 4];
    for i in 0..4 {
        for j in 0..4 {
            c[(i + j) % 4] += a[i] * b[j] * if i + j < 4 { 1 } else { -1 };
        }
    }
    c
}
fn conj(a: C) -> C {
    [a[0], -a[3], -a[2], -a[1]]
}
fn omega(e: i64) -> C {
    let e = e.rem_euclid(8) as usize;
    let mut c = [0; 4];
    c[e % 4] = if e < 4 { 1 } else { -1 };
    c
}
fn norm(a: C) -> C {
    mul(conj(a), a)
}
fn amplitude(f: usize, y: usize) -> C {
    let mut c = [0; 4];
    let s = y ^ (f & 6);
    for x in 0..8 {
        for r in 0..2 {
            let t = r ^ (f & 1);
            let u1 = ((x >> 1) ^ (x & 1)) & 1;
            let v2 = ((x >> 2) & 1) ^ t;
            let e = x.count_ones() as i64 + 4 * ((x & 1) * r) as i64 - t as i64
                + 4 * (t * (s & 1) + u1 * ((s >> 1) & 1) + v2 * ((s >> 2) & 1)) as i64;
            c = add(c, omega(e));
        }
    }
    c
}
fn target(y: usize) -> C {
    if (y >> 1 & 1) != (y >> 2 & 1) {
        return [0; 4];
    }
    mul(
        add(omega(0), omega(if y & 1 == 0 { 1 } else { 5 })),
        omega(2 * (y >> 1 & 1) as i64),
    )
}
fn accepted(y: usize) -> bool {
    (y & 5).count_ones().is_multiple_of(2)
}
fn scalar(a: &ScalarArithmetic, c: C, power: i64) -> ExactScalar {
    a.scalar(c.map(Into::into), power).unwrap()
}
fn moment(values: &[C; 8], x: usize, z: usize, phase: u8, projected: bool) -> C {
    if projected && x != 0 {
        return [0; 4];
    }
    (0..8)
        .filter(|&y| !projected || accepted(y))
        .fold([0; 4], |s, y| {
            add(
                s,
                mul(
                    mul(conj(values[y ^ x]), values[y]),
                    omega(phase as i64 + 4 * (y & z).count_ones() as i64),
                ),
            )
        })
}

static NATIVE_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PULLBACKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static REPLAYS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RECOMPUTATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

static PROBABILITY_NATIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static DENSITY_NATIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MOMENT_NATIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static SUPPORT_NATIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn record_probability(v: rmagna::unitary::NetworkValue) -> ExactScalar {
    if v.native_transport.is_some() {
        PROBABILITY_NATIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    record(v)
}
fn record_density(v: rmagna::unitary::NetworkValue) -> ExactScalar {
    if v.native_transport.is_some() {
        DENSITY_NATIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    record(v)
}
fn record_moment(v: rmagna::unitary::NetworkValue) -> ExactScalar {
    if v.native_transport.is_some() {
        MOMENT_NATIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    record(v)
}
fn stats(s: Option<rmagna::catalogue::CatalogueBoundaryStatistics>) {
    if let Some(s) = s {
        let o = std::sync::atomic::Ordering::Relaxed;
        NATIVE_CALLS.fetch_add(1, o);
        PULLBACKS.fetch_add(s.map_pullbacks, o);
        REPLAYS.fetch_add(s.gaussian_reuses, o);
        RECOMPUTATIONS.fetch_add(s.gaussian_recomputations, o);
    }
}
fn record(v: rmagna::unitary::NetworkValue) -> ExactScalar {
    stats(v.native_transport);
    v.value
}
fn support(v: rmagna::instrument::ReportedSupportExpansion) -> Vec<rmagna::gf2::AffineSupport> {
    if v.native_transport.is_some() {
        SUPPORT_NATIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    stats(v.native_transport);
    v.supports
}

fn exercise_transport(
    run_mode: &str,
    strategy: rmagna::unitary::CompilerStrategy,
) -> serde_json::Value {
    for counter in [
        &NATIVE_CALLS,
        &PULLBACKS,
        &REPLAYS,
        &RECOMPUTATIONS,
        &PROBABILITY_NATIVE,
        &DENSITY_NATIVE,
        &MOMENT_NATIVE,
        &SUPPORT_NATIVE,
    ] {
        counter.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    let a = ScalarArithmetic::default();
    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = 12;
    native.selection.max_core_table_bytes = 1 << 20;
    native.transport.mode = if run_mode == "off" {
        rmagna::phase::NativeTransportMode::Off
    } else {
        rmagna::phase::NativeTransportMode::Prepared
    };
    match run_mode {
        "term_skip" => native.transport.limits.max_terms = 0,
        "geometry_skip" => native.transport.limits.max_geometry_words = 0,
        "work_skip" => native.transport.limits.max_construction_work = 0,
        _ => {}
    }
    let mode = QueryPreparation::Native(native);
    let options = CompileOptions {
        compiler_strategy: strategy,
        probability_preparation: mode,
        density_preparation: mode,
        moment_preparation: mode,
        ..Default::default()
    };
    let parse = |s: &str| parse_circuit(s, &ParseOptions::default()).unwrap();
    let pure = compile_affine(&parse(PREFIX), &options, &a).unwrap();
    let measured = compile_affine(&parse(&(PREFIX.to_string() + MEASURE)), &options, &a).unwrap();
    let ideal = compile_unitary(&parse(TARGET), &CompileOptions::default(), &a).unwrap();
    let sites = [5, 7, 11].map(|d| {
        pure.noise_sites()
            .iter()
            .position(|s| {
                s.alternatives()
                    .iter()
                    .any(|v| v.probability().value().to_string() == format!("1/{d}"))
            })
            .unwrap()
    });
    let measured_sites = [5, 7, 11].map(|d| {
        measured
            .noise_sites()
            .iter()
            .position(|s| {
                s.alternatives()
                    .iter()
                    .any(|v| v.probability().value().to_string() == format!("1/{d}"))
            })
            .unwrap()
    });
    assert_eq!(sites, measured_sites);
    let records = (0..3)
        .map(|j| BitExpr::atom(BitAtom::Record(j)))
        .collect::<Vec<_>>();
    let dependent = [
        records[0].clone(),
        records[1].clone(),
        records[2].clone(),
        records[1].clone(),
        records[0].clone(),
    ];
    let outputs = [0, 1, 2];
    let event = [bits(3, 1)];
    let target_layout = DensityLayout {
        outputs: &outputs,
        event_rows: &event,
    };
    for p in [&pure, &measured] {
        p.prepare_query(Q::Acceptance, 256, &a).unwrap();
        p.prepare_query(
            Q::TargetDensity {
                outputs: &outputs,
                target: &ideal,
                target_layout,
            },
            256,
            &a,
        )
        .unwrap();
    }
    measured
        .prepare_query(Q::AcceptedProbability { parities: &records }, 256, &a)
        .unwrap();
    measured.prepare_query(Q::ReportedSupport, 256, &a).unwrap();
    let words = [(0, 4, 1), (1, 2, 1), (4, 4, 1), (0, 0, 0)];
    for p in [&pure, &measured] {
        for (x, z, phase) in words {
            p.prepare_query(
                Q::PauliMoment {
                    outputs: &outputs,
                    word: &PauliWord::new(bits(3, x), bits(3, z), phase).unwrap(),
                },
                256,
                &a,
            )
            .unwrap();
        }
    }
    let mut cases = Vec::new();
    let (mut probabilities, mut moments, mut densities, mut supports, mut nonreal) =
        (0, 0, 0, 0, 0);
    for f in 0..8 {
        let faults = (0..3)
            .filter(|j| f >> j & 1 != 0)
            .map(|site| FaultSelection {
                site: sites[site],
                alternative: 0,
            })
            .collect::<Vec<_>>();
        let values: [C; 8] = std::array::from_fn(|y| amplitude(f, y));
        let masses: [C; 8] = values.map(norm);
        let acceptance = (0..8)
            .filter(|&y| accepted(y))
            .fold([0; 4], |s, y| add(s, masses[y]));
        assert_eq!(
            pure.acceptance(&faults, 256, &a)
                .map(record_probability)
                .unwrap(),
            ExactScalar::one()
        );
        assert_eq!(
            measured
                .acceptance(&faults, 256, &a)
                .map(record_probability)
                .unwrap(),
            scalar(&a, acceptance, -7)
        );
        for (y, &mass) in masses.iter().enumerate() {
            assert_eq!(
                measured
                    .reported_probability(&records, &bits(3, y), &faults, 256, &a)
                    .map(record_probability)
                    .unwrap(),
                scalar(&a, mass, -7),
                "reported f={f} y={y}"
            );
            assert_eq!(
                measured
                    .accepted_probability(&records, &bits(3, y), &faults, 256, &a)
                    .map(record_probability)
                    .unwrap(),
                scalar(&a, if accepted(y) { mass } else { [0; 4] }, -7)
            );
            probabilities += 2;
        }
        let terms = measured
            .reported_support_terms_with_statistics(&dependent, &faults, 256, &a)
            .map(support)
            .unwrap();
        for label in 0..32 {
            let y = label & 7;
            let want = if (label >> 3 & 1) == (y >> 1 & 1) && (label >> 4 & 1) == (y & 1) {
                masses[y]
            } else {
                [0; 4]
            };
            assert_eq!(
                measured
                    .reported_probability(&dependent, &bits(5, label), &faults, 256, &a)
                    .map(record_probability)
                    .unwrap(),
                scalar(&a, want, -7)
            );
            let covered = terms.iter().any(|term| {
                (0..(1usize << term.dimension())).any(|u| {
                    let mut word = term.offset().ones().fold(0usize, |s, j| s ^ (1 << j));
                    for (j, column) in term.columns().iter().enumerate() {
                        if u >> j & 1 != 0 {
                            word ^= column.ones().fold(0usize, |s, k| s ^ (1 << k));
                        }
                    }
                    word == label
                })
            });
            if want.iter().any(|&c| c != 0) {
                assert!(covered, "physical nonzero omitted by support envelope");
            }
            if (label >> 3 & 1) != (y >> 1 & 1) || (label >> 4 & 1) != (y & 1) {
                assert!(!covered, "dependent label constraint violated");
            }
            supports += 1;
            probabilities += 1;
        }
        for (x, z, phase) in words {
            let word = PauliWord::new(bits(3, x), bits(3, z), phase).unwrap();
            for (projected, p) in [(false, &pure), (true, &measured)] {
                let want = scalar(&a, moment(&values, x, z, phase, projected), -7);
                if !want.is_real() {
                    nonreal += 1;
                }
                assert_eq!(
                    p.pauli_moment(&outputs, &word, &faults, 256, &a)
                        .map(record_moment)
                        .unwrap(),
                    want,
                    "moment f={f} x={x} z={z} p={projected}"
                );
                moments += 1;
            }
        }
        for outcome in 0..2 {
            let overlap = (0..8)
                .filter(|y| y & 1 == outcome)
                .fold([0; 4], |s, y| add(s, mul(conj(target(y)), values[y])));
            let pure_want = norm(overlap);
            let measured_want = (0..8)
                .filter(|&y| y & 1 == outcome && accepted(y))
                .fold([0; 4], |s, y| add(s, mul(masses[y], norm(target(y)))));
            for (p, want) in [(&pure, pure_want), (&measured, measured_want)] {
                assert_eq!(
                    p.target_density_weight(
                        &outputs,
                        &ideal,
                        DensityBoundary {
                            outputs: &outputs,
                            event_rows: &event,
                            outcomes: &bits(1, outcome)
                        },
                        &faults,
                        256,
                        &a
                    )
                    .map(record_density)
                    .unwrap(),
                    scalar(&a, want, -10),
                    "density f={f} outcome={outcome}"
                );
                densities += 1;
            }
        }
        cases.push(json!({"fault_mask":f,"amplitude_numerator":values,"amplitude_sqrt2_power":-7,"probability_numerator":masses,"probability_binary_power":-7,"acceptance_numerator":acceptance}));
    }
    assert!(nonreal > 0);
    let families = [
        pure.prepared_probability_statistics().unwrap(),
        measured.prepared_probability_statistics().unwrap(),
        pure.prepared_density_statistics().unwrap(),
        measured.prepared_density_statistics().unwrap(),
        pure.prepared_moment_statistics().unwrap(),
        measured.prepared_moment_statistics().unwrap(),
    ];
    let support_counts = measured
        .prepared_support_statistics()
        .unwrap()
        .unwrap()
        .native_transports;
    for f in &families {
        if run_mode == "prepared" {
            assert_eq!(f.native_transports.prepared, f.fixed_resources);
            assert_eq!(f.native_transports.budget_skipped, 0);
        } else if run_mode.ends_with("_skip") {
            assert_eq!(f.native_transports.budget_skipped, f.fixed_resources);
            assert_eq!(f.native_transports.prepared, 0);
        } else {
            assert_eq!(f.native_transports, Default::default());
        }
    }
    if run_mode == "prepared" {
        assert_eq!(support_counts.prepared, 1);
        assert_eq!(support_counts.budget_skipped, 0);
        assert!(REPLAYS.load(std::sync::atomic::Ordering::Relaxed) > 0);
        for counter in [
            &PROBABILITY_NATIVE,
            &DENSITY_NATIVE,
            &MOMENT_NATIVE,
            &SUPPORT_NATIVE,
        ] {
            assert!(
                counter.load(std::sync::atomic::Ordering::Relaxed) > 0,
                "each actual query family must use its prepared transport"
            );
        }
    } else if run_mode.ends_with("_skip") {
        assert_eq!(support_counts.prepared, 0);
        assert_eq!(support_counts.budget_skipped, 1);
    } else {
        assert_eq!(support_counts, Default::default());
    }
    if run_mode != "prepared" {
        assert_eq!(NATIVE_CALLS.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
    let construction_prepared = families
        .iter()
        .map(|s| s.native_transports.prepared)
        .sum::<usize>()
        + support_counts.prepared;
    let construction_skips = families
        .iter()
        .map(|s| s.native_transports.budget_skipped)
        .sum::<usize>()
        + support_counts.budget_skipped;
    let selected_routes = families
        .iter()
        .map(|s| {
            [
                s.native_routes.direct,
                s.native_routes.product,
                s.native_routes.walsh,
                s.native_routes.injected,
                s.native_routes.injection_budget_skips,
            ]
        })
        .collect::<Vec<_>>();
    json!({"passed":true,"candidate_transport_mode":run_mode,"constructed_transports":construction_prepared,"budget_skipped_transports":construction_skips,"actual_native_queries":NATIVE_CALLS.load(std::sync::atomic::Ordering::Relaxed),"map_pullbacks":PULLBACKS.load(std::sync::atomic::Ordering::Relaxed),"gaussian_replays":REPLAYS.load(std::sync::atomic::Ordering::Relaxed),"gaussian_recomputations":RECOMPUTATIONS.load(std::sync::atomic::Ordering::Relaxed),"selected_family_routes":selected_routes,"original_literal_path_model":true,"source":PREFIX,"measurement_suffix":MEASURE,"target":TARGET,"source_wires":3,"fault_masks":8,"path_variables":7,"cases":cases,"probability_comparisons":probabilities,"moment_comparisons":moments,"density_comparisons":densities,"dependent_support_envelope_checks":supports,"nonreal_moments":nonreal,"selection_limits":{"max_quotient_rank":12,"max_core_table_bytes":1<<20}})
}

#[test]
fn prepared_native_task_families_preserve_all_three_wire_faulted_exact_values() {
    for strategy in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        let baseline = exercise_transport("off", strategy);
        for mode in ["prepared", "term_skip", "geometry_skip", "work_skip"] {
            let value = exercise_transport(mode, strategy);
            assert_eq!(value["cases"], baseline["cases"]);
            assert_eq!(
                value["selected_family_routes"],
                baseline["selected_family_routes"]
            );
            assert_eq!(value["probability_comparisons"], 384);
            assert_eq!(value["moment_comparisons"], 64);
            assert_eq!(value["density_comparisons"], 32);
            assert_eq!(value["dependent_support_envelope_checks"], 256);
            assert_eq!(value["nonreal_moments"], 32);
            if mode == "prepared" {
                assert!(value["actual_native_queries"].as_u64().unwrap() > 0);
                assert!(value["gaussian_replays"].as_u64().unwrap() > 0);
                assert!(value["gaussian_recomputations"].as_u64().unwrap() > 0);
                assert_eq!(value["budget_skipped_transports"], 0);
            } else {
                assert_eq!(value["constructed_transports"], 0);
                assert!(value["budget_skipped_transports"].as_u64().unwrap() > 0);
                assert_eq!(value["actual_native_queries"], 0);
            }
        }
    }
}
