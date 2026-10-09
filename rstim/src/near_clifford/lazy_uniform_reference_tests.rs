//! Independent literal S2 bookkeeping oracle for the lazy uniform-state change.
//! This module is test-only; shared packet physics retains its separate scalar/dense oracles.
use super::*;
use rand::{RngCore, SeedableRng, rngs::StdRng};
include!("lazy_uniform_noise_error_tests.rs");

type AmpBits = Vec<(u64, u64)>;
type SidecarSnapshot = (Vec<u64>, usize, Vec<u64>, usize, usize);

#[derive(Debug, PartialEq, Eq)]
enum EntrySnapshot {
    None,
    Rotate([Option<usize>; 2]),
    Measure(u64, [Option<usize>; 2]),
}

#[derive(Debug, PartialEq, Eq)]
struct CacheSnapshot {
    start: usize,
    reserved: usize,
    budget: usize,
    nodes: Vec<EntrySnapshot>,
    // Preserve insertion order: each vector index is the actual cache ID.
    states: Vec<(AmpBits, Option<usize>, EntrySnapshot)>,
}

#[derive(Debug, PartialEq, Eq)]
struct SamplerSnapshot {
    cache: Option<CacheSnapshot>,
    coefficients: (AmpBits, usize),
    reduced: (AmpBits, usize),
    x: Vec<u64>,
    z: Vec<u64>,
    pack_enabled: bool,
    live: usize,
    live_mask: u64,
    scalar_prepared: bool,
    // Capacities are included because retained buffers affect later admission.
    packets: Vec<(Vec<u64>, usize)>,
    independent: Option<SidecarSnapshot>,
}

fn amp_bits(values: &[ComplexAmp]) -> AmpBits {
    values
        .iter()
        .map(|a| (a.re.to_bits(), a.im.to_bits()))
        .collect()
}

fn entry_snapshot(entry: CachedOp) -> EntrySnapshot {
    match entry {
        CachedOp::None => EntrySnapshot::None,
        CachedOp::Rotate(next) => EntrySnapshot::Rotate(next),
        CachedOp::Measure(m) => EntrySnapshot::Measure(m.probability_zero.to_bits(), m.next),
    }
}

fn snapshot(sampler: &CompiledNearCliffordSampler<'_>) -> SamplerSnapshot {
    SamplerSnapshot {
        cache: sampler.core.cache.as_ref().map(|cache| CacheSnapshot {
            start: cache.start,
            reserved: cache.reserved,
            budget: cache.budget,
            nodes: cache.nodes.iter().copied().map(entry_snapshot).collect(),
            states: cache
                .states
                .iter()
                .map(|state| {
                    (
                        amp_bits(&state.coefficients),
                        state.next_node,
                        entry_snapshot(state.transition),
                    )
                })
                .collect(),
        }),
        coefficients: (
            amp_bits(&sampler.core.coefficients),
            sampler.core.coefficients.capacity(),
        ),
        reduced: (
            amp_bits(&sampler.core.reduced_coefficients),
            sampler.core.reduced_coefficients.capacity(),
        ),
        x: sampler.core.x.clone(),
        z: sampler.core.z.clone(),
        pack_enabled: sampler.core.pack_enabled,
        live: sampler.core.last_packet_live,
        live_mask: sampler.core.last_packet_live_mask,
        scalar_prepared: sampler.core.last_scalar_prepared,
        packets: [
            &sampler.core.packet_x,
            &sampler.core.packet_z,
            &sampler.core.packet_tape,
            &sampler.core.packet_records,
            &sampler.core.packet_noise_masks,
        ]
        .into_iter()
        .map(|v| (v.to_vec(), v.capacity()))
        .collect(),
        independent: sampler
            .core
            .packet_independent
            .as_ref()
            .map(IndependentPacket::snapshot),
    }
}

fn pair<'a>(
    plan: &'a CompiledNearCliffordExecutor,
    budget: usize,
) -> (
    CompiledNearCliffordSampler<'a>,
    CompiledNearCliffordSampler<'a>,
) {
    let lazy = plan.prepare_sampler_with_cache_budget(budget).unwrap();
    let mut reference = plan.prepare_sampler_with_cache_budget(budget).unwrap();
    reference.core.materialized_packet_reference = true;
    (lazy, reference)
}

fn policies() -> [CompiledRotationArithmetic; 2] {
    [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ]
}

fn witness_text(compact: bool) -> String {
    let mut text = String::from("H 0 1\nT 0 1\nCX 0 1\nMY 0\nMX 1\n");
    if compact {
        text.push_str("REPEAT 129 {\nR 2\nH 2\nM 2\n}\n");
    }
    text
}

// Derive the cold one-path charge schedule from actual compiled coefficient
// lengths. This accounts for scheduler-moved prefix boundaries and expansions;
// an independent-event prefix can leave the initial coherent rank at zero.
// It does not predict which branches win admission or assert their ID ordering.
fn scheduled_cache_charges(plan: &CompiledNearCliffordExecutor) -> Vec<(usize, usize, usize)> {
    let mut len = plan.initial_coefficients.len();
    let mut result = Vec::new();
    for (node, op) in plan.operations.iter().enumerate().skip(plan.prefix_len) {
        let transition = match op {
            PlanOp::Rotate { pauli, expand, .. } if pauli.x != 0 || pauli.z != 0 => {
                if *expand {
                    len = len.checked_mul(2).unwrap();
                }
                true
            }
            PlanOp::Measure(Measurement {
                projection: Projection::Active { .. },
                ..
            }) => {
                assert!(len >= 2 && len.is_power_of_two());
                len /= 2;
                true
            }
            _ => false,
        };
        if transition {
            // Literal current S2 reservation contract; scalar ID 0 costs zero.
            let charge = if len == 1 {
                0
            } else {
                len * size_of::<ComplexAmp>() + 256
            };
            result.push((node, len, charge));
        }
    }
    result
}

fn first_projection_path_charge(plan: &CompiledNearCliffordExecutor) -> usize {
    let first = plan
        .operations
        .iter()
        .enumerate()
        .skip(plan.prefix_len)
        .find_map(|(node, op)| {
            matches!(
                op,
                PlanOp::Measure(Measurement {
                    projection: Projection::Active { .. },
                    ..
                })
            )
            .then_some(node)
        })
        .expect("fixture needs an Active projection");
    scheduled_cache_charges(plan)
        .into_iter()
        .take_while(|(node, _, _)| *node <= first)
        .map(|(_, _, charge)| charge)
        .sum()
}

fn adjacent_budgets(plan: &CompiledNearCliffordExecutor, initial: usize) -> Vec<usize> {
    let mut boundaries = vec![initial];
    let charges = scheduled_cache_charges(plan);
    // Individual allocations and each cold-path accumulation through the first
    // projection provide both immediate and combined admission boundaries.
    for &(_, _, charge) in &charges {
        if charge != 0 {
            boundaries.push(initial.checked_add(charge).unwrap());
        }
    }
    let first = plan
        .operations
        .iter()
        .enumerate()
        .skip(plan.prefix_len)
        .find_map(|(node, op)| {
            matches!(
                op,
                PlanOp::Measure(Measurement {
                    projection: Projection::Active { .. },
                    ..
                })
            )
            .then_some(node)
        })
        .unwrap();
    let mut cumulative = initial;
    for &(node, _, charge) in &charges {
        if node > first {
            break;
        }
        cumulative = cumulative.checked_add(charge).unwrap();
        boundaries.push(cumulative);
    }
    let mut budgets = vec![0, DEFAULT_CACHE_BYTE_BUDGET];
    for boundary in boundaries {
        for budget in [
            boundary.saturating_sub(1),
            boundary,
            boundary.checked_add(1).unwrap(),
        ] {
            assert!(
                budget <= DEFAULT_CACHE_BYTE_BUDGET,
                "bounded rank-two fixtures"
            );
            budgets.push(budget);
        }
    }
    budgets.sort_unstable();
    budgets.dedup();
    budgets
}

fn diagnostic(
    sampler: &CompiledNearCliffordSampler<'_>,
    policy: CompiledRotationArithmetic,
    seed: u64,
    budget: usize,
    shots: usize,
    call: usize,
) -> String {
    let ids = sampler.core.cache.as_ref().map(|cache| {
        cache
            .states
            .iter()
            .enumerate()
            .map(|(id, state)| (id, state.next_node, entry_snapshot(state.transition)))
            .collect::<Vec<_>>()
    });
    format!(
        "policy={policy:?}; seed={seed}; budget={budget}; shots={shots}; call={call}; live={:#018x}/{}; reserved={}; IDs={ids:?}",
        sampler.core.last_packet_live_mask,
        sampler.core.last_packet_live,
        sampler.coefficient_cache_reserved_bytes(),
    )
}

#[test]
fn lazy_cache_bookkeeping_matches_materialized_s2_cold_warm_and_split_calls() {
    let mixed = "H 0 1\nT 0 1\nX_ERROR(0.07) 0\nDEPOLARIZE2(0.11) 0 1\nMY 0\nT_DAG 1\nMRX(0.19) !1\nCX rec[-1] 0\nCX sweep[1] 1\nMPP(0.13) X0*Y1\nM 0 1\nREPEAT 129 {\nR 2\nH 2\nMR(0.001) !2\n}\n";
    for policy in policies() {
        for text in [witness_text(true), String::from(mixed)] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy).unwrap();
            assert!(
                plan.peak_active_rank < 4,
                "must exercise cached rather than coherent packets"
            );
            assert!(plan.independent_event_count >= 129);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            let path_boundary = initial + first_projection_path_charge(&plan);
            for budget in adjacent_budgets(&plan, initial) {
                // Extra seeds on the exact combined boundary and warm/default
                // path retain a small matrix while adjacent bytes use seed 583.
                let seeds: &[u64] =
                    if budget == path_boundary || budget == DEFAULT_CACHE_BYTE_BUDGET {
                        &[0, 63, 583]
                    } else {
                        &[583]
                    };
                for &seed in seeds {
                    let (mut lazy, mut reference) = pair(&plan, budget);
                    let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                    scalar.core.pack_enabled = false;
                    let mut a = StdRng::seed_from_u64(seed);
                    let mut b = a.clone();
                    let mut c = a.clone();
                    // 32 is the cold first packet; later calls cover scalar/packet
                    // thresholds, tail 63, tail 1 and retained-state split streams.
                    for (call, shots) in [0, 32, 64, 65, 1, 7, 31, 63, 127, 129, 64]
                        .into_iter()
                        .enumerate()
                    {
                        let actual =
                            lazy.sample_measurements_u8_with_sweep(shots, &[false, true], &mut a);
                        let expected = reference.sample_measurements_u8_with_sweep(
                            shots,
                            &[false, true],
                            &mut b,
                        );
                        let context = diagnostic(&lazy, policy, seed, budget, shots, call);
                        assert_eq!(actual, expected, "{context}");
                        assert_eq!(snapshot(&lazy), snapshot(&reference), "{context}");
                        let scalar_rows = scalar
                            .sample_measurements_u8_with_sweep(shots, &[false, true], &mut c)
                            .unwrap_or_else(|err| panic!("scalar Err={err}; {context}"));
                        assert!(actual.is_ok(), "expected successful fixture; {context}");
                        assert_eq!(
                            actual.unwrap(),
                            scalar_rows,
                            "independent scalar rows; {context}"
                        );
                        assert_eq!(a.clone().next_u64(), b.clone().next_u64(), "{context}");
                        assert_eq!(a.clone().next_u64(), c.clone().next_u64(), "{context}");
                    }
                }
            }
        }
    }
}

#[test]
fn lazy_cache_bookkeeping_matches_actual_partial_admission_and_compact_replay() {
    for policy in policies() {
        for compact in [false, true] {
            let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
                &witness_text(compact),
                policy,
            )
            .unwrap();
            assert!(plan.peak_active_rank < 4);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            let increment = first_projection_path_charge(&plan);
            // Keep strong canonical fixture assertions while computing budgets
            // from its real compiled expansions/projection, not magic bytes.
            assert_eq!(increment, if compact { 896 } else { 288 });
            if compact {
                assert_eq!(plan.initial_coefficients.len(), 1);
                let path: Vec<_> = scheduled_cache_charges(&plan)
                    .into_iter()
                    .filter(|(_, _, charge)| *charge != 0)
                    .take(3)
                    .map(|(_, len, charge)| (len, charge))
                    .collect();
                assert_eq!(path, vec![(2, 288), (4, 320), (2, 288)]);
            }
            for seed in [583, 0, 63] {
                let (mut lazy, mut reference) = pair(&plan, initial + increment);
                let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                scalar.core.pack_enabled = false;
                let mut a = StdRng::seed_from_u64(seed);
                let mut b = a.clone();
                let mut c = a.clone();
                for (call, shots) in [64, 65, 1, 31, 32, 63, 127, 129, 64]
                    .into_iter()
                    .enumerate()
                {
                    let actual = lazy.sample_measurements_u8(shots, &mut a);
                    let expected = reference.sample_measurements_u8(shots, &mut b);
                    let scalar_rows = scalar.sample_measurements_u8(shots, &mut c);
                    let context = diagnostic(&lazy, policy, seed, initial + increment, shots, call);
                    assert!(
                        actual.is_ok(),
                        "expected successful fixture; compact={compact}; {context}"
                    );
                    assert_eq!(actual, expected, "compact={compact}; {context}");
                    assert_eq!(actual, scalar_rows, "compact={compact}; {context}");
                    assert_eq!(
                        snapshot(&lazy),
                        snapshot(&reference),
                        "compact={compact}; {context}"
                    );
                    assert_eq!(a.clone().next_u64(), b.clone().next_u64(), "{context}");
                    assert_eq!(a.clone().next_u64(), c.clone().next_u64(), "{context}");
                    if call == 0 && seed == 583 {
                        assert!(
                            lazy.core.last_packet_live > 0 && lazy.core.last_packet_live < 64,
                            "compact={compact}; fixture must show actual replay; {context}"
                        );
                        assert_eq!(
                            lazy.coefficient_cache_reserved_bytes(),
                            initial + increment,
                            "{context}"
                        );
                        assert_eq!(lazy.core.packet_independent.is_some(), compact, "{context}");
                    }
                }
            }
        }
    }
}

// Literal rank-one projection norm: independent of project(), cache state and
// the new packet representation. It validates each malformed-cache Err witness.
fn rank_one_projection_norm(p: &CompactPauli, y: bool, fixed: bool, values: &[ComplexAmp]) -> f64 {
    assert_eq!(values.len(), 2);
    if p.x == 0 {
        values[usize::from(fixed)].norm_sqr()
    } else {
        assert_eq!(p.x, 1);
        let mut b = values[1];
        if y {
            b = ComplexAmp::new(b.im, -b.re);
        }
        let sign = if fixed { -1. } else { 1. };
        ((values[0] + b * sign) * std::f64::consts::FRAC_1_SQRT_2).norm_sqr()
    }
}

fn error_plan(
    policy: CompiledRotationArithmetic,
    compact: bool,
    fail_second: bool,
    noise: bool,
) -> (CompiledNearCliffordExecutor, usize, bool) {
    let mut text = String::new();
    if noise {
        text.push_str("DEPOLARIZE1(0.37) 2\n");
    }
    text.push_str("H 0\nT 0\nMX 0\n");
    if compact {
        text.push_str("REPEAT 129 {\nR 1\nH 1\nM 1\n}\n");
    }
    let mut plan =
        CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy).unwrap();
    // Spectator measurements are scheduled left of the q0 rotation. A leading
    // Noise event also ends the deterministic prefix. Observe the real shape
    // BEFORE constructing this deliberately malformed numerical/CDF fixture.
    let runtime_expansion = compact || noise;
    assert_eq!(
        plan.initial_coefficients.len(),
        if runtime_expansion { 1 } else { 2 }
    );
    assert_eq!(plan.prefix_len, usize::from(!runtime_expansion));
    assert_eq!(plan.peak_active_rank, 1, "cached path, never coherent");
    assert_eq!(
        plan.num_qubits,
        if noise {
            3
        } else if compact {
            2
        } else {
            1
        }
    );
    assert_eq!(plan.noise_event_count, usize::from(noise));
    assert_eq!(plan.independent_event_count, if compact { 129 } else { 0 });
    assert!(plan.random_runs.is_some());
    let active: Vec<_> = plan
        .operations
        .iter()
        .enumerate()
        .skip(plan.prefix_len)
        .filter_map(|(node, op)| match op {
            PlanOp::Measure(m) if matches!(m.projection, Projection::Active { .. }) => {
                Some((node, m.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(active.len(), 1, "fixture requires one cached Active node");
    let (node, m) = &active[0];
    let rotations: Vec<_> = plan
        .operations
        .iter()
        .enumerate()
        .filter_map(|(n, op)| {
            if let PlanOp::Rotate { pauli, expand, .. } = op {
                Some((n, pauli.x, pauli.z, *expand))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(rotations.len(), 1, "single retained q0 T");
    let (rotation, x, z, expand) = rotations[0];
    assert!(rotation < *node);
    assert_eq!((x, z, expand), (1, 0, true));
    assert_eq!(rotation >= plan.prefix_len, runtime_expansion);
    if runtime_expansion {
        // Move only this no-RNG numerical expansion into the injected initial
        // rank-one buffer. Keep every node/event/optional-table index intact.
        // Original Rotate has no scalar basis program; empty Basis is a no-op.
        if let Some(programs) = &plan.scalar_basis {
            assert!(programs[rotation].is_none());
        }
        plan.operations[rotation] = PlanOp::Basis(Vec::new());
    }
    assert!(
        !plan.operations[plan.prefix_len..*node]
            .iter()
            .any(|op| matches!(
                op,
                PlanOp::Rotate { .. }
                    | PlanOp::Measure(Measurement {
                        projection: Projection::Active { .. },
                        ..
                    })
            )),
        "input ID must remain the cache start until the failing node"
    );
    let Projection::Active {
        index, y, offset, ..
    } = m.projection
    else {
        unreachable!()
    };
    assert_eq!(index, 0, "rank-one fixture");
    assert_eq!((m.pauli.x, m.pauli.z, y, offset), (0, 1, false, false));
    let successful_fixed = if fail_second { offset } else { !offset };
    let mut values = vec![ComplexAmp::default(); 2];
    if m.pauli.x == 0 {
        values[usize::from(successful_fixed)] = ComplexAmp::new(1., 0.);
    } else {
        let a = std::f64::consts::FRAC_1_SQRT_2;
        let b = if successful_fixed { -a } else { a };
        values[0] = ComplexAmp::new(a, 0.);
        values[1] = if y {
            ComplexAmp::new(0., b)
        } else {
            ComplexAmp::new(b, 0.)
        };
    }
    assert!(rank_one_projection_norm(&m.pauli, y, successful_fixed, &values) > 0.);
    assert_eq!(
        rank_one_projection_norm(&m.pauli, y, !successful_fixed, &values),
        0.
    );
    // Inconsistent CDF is injected below; coefficients have an exact zero-norm
    // branch, exercising the existing logical Err without a production hook.
    plan.initial_coefficients = Arc::new(values);
    (plan, *node, offset)
}

fn inject_inconsistent_cdf(sampler: &mut CompiledNearCliffordSampler<'_>, node: usize) {
    let cache = sampler.core.cache.as_mut().unwrap();
    assert!(cache.set_entry(
        cache.start,
        node,
        CachedOp::Measure(CachedMeasurement {
            probability_zero: 0.5,
            next: [None; 2],
        })
    ));
}

#[test]
fn lazy_cache_bookkeeping_matches_first_and_second_cached_projection_errors() {
    for policy in policies() {
        for compact in [false, true] {
            for fail_second in [false, true] {
                let (plan, node, _) = error_plan(policy, compact, fail_second, false);
                let (mut lazy, mut reference) = pair(&plan, DEFAULT_CACHE_BYTE_BUDGET);
                inject_inconsistent_cdf(&mut lazy, node);
                inject_inconsistent_cdf(&mut reference, node);
                let mut a = StdRng::seed_from_u64(583);
                let mut b = a.clone();
                let mut drawn = a.clone();
                let rows = scalar_prepared_tape_tests::draw_frozen_packet_rows(
                    &plan.random_kinds,
                    64,
                    &mut drawn,
                );
                let event = plan
                    .random_kinds
                    .iter()
                    .position(|kind| matches!(kind, RandomKind::Active))
                    .unwrap();
                let ones = rows
                    .iter()
                    .filter(|row| f64::from_bits(row[event]) >= 0.5)
                    .count();
                let before = diagnostic(&lazy, policy, 583, DEFAULT_CACHE_BYTE_BUDGET, 64, 0);
                assert!(
                    ones > 0 && ones < 64,
                    "both cached calls must have a nonempty mask; {before}"
                );
                // Public calls exercise workspace take/restore as well as Err propagation.
                let actual = lazy.sample_measurements_u8(64, &mut a);
                let expected = reference.sample_measurements_u8(64, &mut b);
                let context = diagnostic(&lazy, policy, 583, DEFAULT_CACHE_BYTE_BUDGET, 64, 0);
                let actual = actual.expect_err(&format!("expected project Err; {context}"));
                let expected = expected.expect_err(&format!("expected reference Err; {context}"));
                assert_eq!(
                    actual, "compiled near-Clifford zero-probability measurement",
                    "{context}"
                );
                assert_eq!(actual, expected, "{context}");
                assert_eq!(
                    snapshot(&lazy),
                    snapshot(&reference),
                    "compact={compact}; fail_second={fail_second}; {context}"
                );
                assert_eq!(a.clone().next_u64(), b.clone().next_u64(), "{context}");
                assert_eq!(
                    a.clone().next_u64(),
                    drawn.clone().next_u64(),
                    "no replay, retry or extra draw on Err; {context}"
                );
                assert_eq!(lazy.core.packet_independent.is_some(), compact, "{context}");
                // Inspect actual stored children to distinguish first from second Err.
                let cache = lazy.core.cache.as_ref().unwrap();
                let Some(CachedOp::Measure(entry)) = cache.entry(cache.start, node) else {
                    unreachable!()
                };
                assert_eq!(
                    entry.probability_zero.to_bits(),
                    0.5f64.to_bits(),
                    "{context}"
                );
                assert_eq!(
                    entry.next,
                    if fail_second {
                        [Some(0), None]
                    } else {
                        [None, None]
                    },
                    "{context}"
                );
                for (lane, row) in rows.iter().enumerate() {
                    let mut independent = 0;
                    for (event, kind) in plan.random_kinds.iter().enumerate() {
                        if matches!(kind, RandomKind::Independent) && compact {
                            let sidecar = lazy.core.packet_independent.as_ref().unwrap();
                            assert_eq!(
                                (sidecar.mask(independent) >> lane) & 1,
                                row[event],
                                "lane={lane}; event={event}; {context}"
                            );
                            independent += 1;
                        } else {
                            assert_eq!(
                                lazy.core.packet_tape[lane * plan.random_kinds.len() + event],
                                row[event],
                                "lane={lane}; event={event}; {context}"
                            );
                        }
                    }
                }
                // Returning Err leaves packet live accounting at its pre-call value.
                assert_eq!(lazy.core.last_packet_live, 0, "{context}");
                assert_eq!(lazy.core.last_packet_live_mask, 0, "{context}");
            }
        }
    }
}
