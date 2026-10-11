//! Compare the reached miss path with the former unconditional projection load.
use super::*;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use std::cell::Cell;

thread_local! {
    static FORCE_RELOAD: Cell<bool> = const { Cell::new(false) };
    static LOADS: Cell<usize> = const { Cell::new(0) };
    static REUSES: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn force_reload() -> bool {
    FORCE_RELOAD.get()
}
pub(super) fn record_load() {
    LOADS.set(LOADS.get() + 1);
}
pub(super) fn record_reuse() {
    REUSES.set(REUSES.get() + 1);
}

fn measured<T>(force: bool, f: impl FnOnce() -> T) -> (T, usize, usize) {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCE_RELOAD.set(self.0);
        }
    }
    let _restore = Restore(FORCE_RELOAD.replace(force));
    LOADS.set(0);
    REUSES.set(0);
    let result = f();
    (result, LOADS.get(), REUSES.get())
}

fn policies() -> [CompiledRotationArithmetic; 2] {
    [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ]
}

fn first_active(plan: &CompiledNearCliffordExecutor) -> (usize, Measurement) {
    plan.operations
        .iter()
        .enumerate()
        .skip(plan.prefix_len)
        .find_map(|(n, op)| match op {
            PlanOp::Measure(m) if matches!(m.projection, Projection::Active { .. }) => {
                Some((n, m.clone()))
            }
            _ => None,
        })
        .unwrap()
}

fn project_branch(
    sampler: &mut CompiledNearCliffordSampler<'_>,
    node: usize,
    m: &Measurement,
    bit: bool,
    state: &mut Option<usize>,
    materialized: &mut Option<usize>,
) -> Result<(), String> {
    let Projection::Active {
        index, y, offset, ..
    } = m.projection
    else {
        unreachable!()
    };
    sampler.cached_project(
        node,
        &m.pauli,
        index,
        y,
        bit ^ offset,
        bit,
        state,
        materialized,
    )
}

#[test]
fn probability_miss_reuses_only_the_unmodified_input_for_both_projection_branches() {
    for policy in policies() {
        // A wide prefix plus different compact projection axes covers both the
        // copy cost and the unchanged projection arithmetic.
        for measurement in [
            "MPP X0*X1*X2*X3*X4*X5*X6*X7",
            "MPP Y0*Y1*Y2*Y3*Y4*Y5*Y6*Y7",
            "MPP X0*Y1*X2*Y3*X4*Y5*X6*Y7",
        ] {
            let text = format!("H 0 1 2 3 4 5 6 7\nT 0 1 2 3 4 5 6 7\n{measurement}\n");
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy).unwrap();
            assert!(
                plan.initial_coefficients.len() >= 64,
                "{measurement}: prefix coefficients={}",
                plan.initial_coefficients.len()
            );
            let (node, m) = first_active(&plan);
            for first in [false, true] {
                let mut actual = plan.prepare_sampler().unwrap();
                let mut old = plan.prepare_sampler().unwrap();
                let id = actual.cache.as_ref().unwrap().start;
                let mut probability = actual
                    .cached_probability_zero(node, &m.pauli, Some(id))
                    .unwrap();
                let mut expected = old
                    .cached_probability_zero(node, &m.pauli, Some(id))
                    .unwrap();
                assert_eq!(probability.zero.to_bits(), expected.zero.to_bits());
                assert!(probability.zero > 0. && probability.zero < 1.);
                for (i, bit) in [first, !first].into_iter().enumerate() {
                    let mut a = Some(id);
                    let mut b = Some(id);
                    let (ar, loads, reuses) = measured(false, || {
                        project_branch(
                            &mut actual,
                            node,
                            &m,
                            bit,
                            &mut a,
                            &mut probability.materialized,
                        )
                    });
                    let (br, old_loads, _) = measured(true, || {
                        project_branch(&mut old, node, &m, bit, &mut b, &mut expected.materialized)
                    });
                    assert_eq!(ar, br);
                    ar.unwrap();
                    assert_eq!(a, b);
                    assert_eq!(reuses, usize::from(i == 0));
                    assert_eq!(old_loads, 1);
                    assert_eq!(loads, i);
                    assert!(probability.materialized.is_none());
                    assert_eq!(
                        lazy_uniform_reference_tests::snapshot(&actual),
                        lazy_uniform_reference_tests::snapshot(&old)
                    );
                }
            }
        }
    }
}

#[test]
fn cached_first_branch_keeps_buffer_untouched_and_missing_second_branch_loads_input() {
    for policy in policies() {
        let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
            "H 0 1\nT 0 1\nMY 0\nMX 1\n",
            policy,
        )
        .unwrap();
        let (node, m) = first_active(&plan);
        for first in [false, true] {
            let mut sampler = plan.prepare_sampler().unwrap();
            let id = sampler.cache.as_ref().unwrap().start;
            let mut probability = sampler
                .cached_probability_zero(node, &m.pauli, Some(id))
                .unwrap();
            project_branch(
                &mut sampler,
                node,
                &m,
                first,
                &mut Some(id),
                &mut probability.materialized,
            )
            .unwrap();
            let mut hit = sampler
                .cached_probability_zero(node, &m.pauli, Some(id))
                .unwrap();
            assert!(hit.materialized.is_none());
            let before = sampler
                .coefficients
                .iter()
                .map(|a| (a.re.to_bits(), a.im.to_bits()))
                .collect::<Vec<_>>();
            let (result, loads, reuses) = measured(false, || {
                project_branch(
                    &mut sampler,
                    node,
                    &m,
                    first,
                    &mut Some(id),
                    &mut hit.materialized,
                )
            });
            result.unwrap();
            assert_eq!((loads, reuses), (0, 0));
            assert_eq!(
                before,
                sampler
                    .coefficients
                    .iter()
                    .map(|a| (a.re.to_bits(), a.im.to_bits()))
                    .collect::<Vec<_>>()
            );
            let (result, loads, reuses) = measured(false, || {
                project_branch(
                    &mut sampler,
                    node,
                    &m,
                    !first,
                    &mut Some(id),
                    &mut hit.materialized,
                )
            });
            result.unwrap();
            assert_eq!((loads, reuses), (1, 0));
        }
    }
}

#[test]
fn projection_error_invalidates_loaded_input_and_preserves_the_old_error_state() {
    for policy in policies() {
        for fail_second in [false, true] {
            let (plan, node, _) =
                lazy_uniform_reference_tests::error_plan(policy, false, fail_second, false);
            let (_, m) = first_active(&plan);
            let mut actual = plan.prepare_sampler().unwrap();
            let mut old = plan.prepare_sampler().unwrap();
            let id = actual.cache.as_ref().unwrap().start;
            let mut probability = actual
                .cached_probability_zero(node, &m.pauli, Some(id))
                .unwrap();
            let mut expected = old
                .cached_probability_zero(node, &m.pauli, Some(id))
                .unwrap();
            for bit in [false, true] {
                let mut a = Some(id);
                let mut b = Some(id);
                let (ar, _, _) = measured(false, || {
                    project_branch(
                        &mut actual,
                        node,
                        &m,
                        bit,
                        &mut a,
                        &mut probability.materialized,
                    )
                });
                let (br, _, _) = measured(true, || {
                    project_branch(&mut old, node, &m, bit, &mut b, &mut expected.materialized)
                });
                assert_eq!(ar, br);
                assert_eq!(a, b);
                assert!(probability.materialized.is_none());
                assert_eq!(
                    lazy_uniform_reference_tests::snapshot(&actual),
                    lazy_uniform_reference_tests::snapshot(&old)
                );
                if bit == fail_second {
                    assert_eq!(
                        ar.unwrap_err(),
                        "compiled near-Clifford zero-probability measurement"
                    );
                    break;
                } else {
                    ar.unwrap();
                }
            }
        }
    }
}

#[test]
fn default_mixed_calls_match_old_reload_state_records_counts_and_rng() {
    let text = "REPEAT 5 {\nR 0 1 2\nH 0 1 2\nT 0 1 2\nCX 0 1\nDEPOLARIZE2(0.01) 1 2\nMY 0\nCX rec[-1] 2\nT_DAG 2\nMX 1\nMY 2\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-3]\n}\n";
    for policy in policies() {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, policy).unwrap();
        for seed in [583, 1739] {
            let mut actual = plan.prepare_sampler().unwrap();
            let mut old = plan.prepare_sampler().unwrap();
            let mut a = StdRng::seed_from_u64(seed);
            let mut b = a.clone();
            let mut total_reuses = 0;
            for shots in [1, 31, 32, 63, 64, 65, 1024, 64] {
                let (rows, loads, reuses) = measured(false, || actual.sample(shots, &mut a));
                let (expected, old_loads, _) = measured(true, || old.sample(shots, &mut b));
                assert_eq!(rows, expected);
                rows.unwrap();
                assert_eq!(loads + reuses, old_loads);
                total_reuses += reuses;
                assert_eq!(
                    lazy_uniform_reference_tests::snapshot(&actual),
                    lazy_uniform_reference_tests::snapshot(&old)
                );
                let (counts, loads, reuses) = measured(false, || {
                    actual.sample_postselected_counts(shots, 7, &mut a)
                });
                let (expected, old_loads, _) =
                    measured(true, || old.sample_postselected_counts(shots, 7, &mut b));
                assert_eq!(counts, expected);
                counts.unwrap();
                assert_eq!(loads + reuses, old_loads);
                total_reuses += reuses;
                assert_eq!(
                    lazy_uniform_reference_tests::snapshot(&actual),
                    lazy_uniform_reference_tests::snapshot(&old)
                );
                let (flat, _, reuses) =
                    measured(false, || actual.sample_measurements_u8(shots, &mut a));
                let (expected, _, _) = measured(true, || old.sample_measurements_u8(shots, &mut b));
                assert_eq!(flat, expected);
                flat.unwrap();
                total_reuses += reuses;
                assert_eq!(
                    lazy_uniform_reference_tests::snapshot(&actual),
                    lazy_uniform_reference_tests::snapshot(&old)
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert!(
                total_reuses > 0,
                "the original default must actually skip loads"
            );
        }
    }
}
