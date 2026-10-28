use rmagna::{
    clifford::CliffordGate as C,
    gf2::BitVector,
    pauli::PauliWord,
    phase::{
        NativePhaseOptions, NativeTransportMode, NativeTransportReceipt, PreparedPhaseResource,
    },
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::{CompilerStrategy, DensityLayout, UnitaryCircuit, UnitaryGate as G},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Meter;
thread_local! {
    static REQUESTS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}
fn charge(bytes: usize) {
    let _ = REQUESTS.try_with(|counter| {
        if let Some((calls, total)) = counter.get() {
            counter.set(Some((calls + 1, total + bytes)));
        }
    });
}
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        charge(bytes);
        unsafe { System.realloc(pointer, layout, bytes) }
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;

fn measure<T>(f: impl FnOnce() -> T) -> (T, (usize, usize)) {
    REQUESTS.with(|counter| counter.set(Some((0, 0))));
    let result = f();
    let requests = REQUESTS.with(|counter| counter.replace(None).unwrap());
    (result, requests)
}
fn zero_cap(cap: usize) -> NativePhaseOptions {
    let mut options = NativePhaseOptions::default();
    options.transport.mode = NativeTransportMode::Prepared;
    match cap {
        0 => options.transport.limits.max_terms = 0,
        1 => options.transport.limits.max_geometry_words = 0,
        _ => options.transport.limits.max_construction_work = 0,
    }
    options
}
fn check_skip(off: &PreparedPhaseResource, skipped: &PreparedPhaseResource) {
    assert_eq!(off.native_transport_receipt(), None);
    let Some(NativeTransportReceipt::BudgetSkipped {
        option,
        limit,
        required,
    }) = skipped.native_transport_receipt()
    else {
        panic!("a zero cap must retain the selected resource and report a skip");
    };
    assert_eq!(limit, 0);
    assert!(required > 0);
    assert!(option.starts_with("catalogue_boundary."));
    assert_eq!(off.num_variables(), skipped.num_variables());
    assert_eq!(off.quotient_rank(), skipped.quotient_rank());
    assert_eq!(off.num_terms(), skipped.num_terms());
    assert_eq!(
        off.native_resource().unwrap().receipt(),
        skipped.native_resource().unwrap().receipt()
    );
}
fn bits(width: usize, indices: impl IntoIterator<Item = usize>) -> BitVector {
    BitVector::from_indices(width, indices).unwrap()
}

#[test]
fn each_zero_cap_avoids_transport_allocations_across_scalar_and_labelled_families() {
    let a = ScalarArithmetic::new(2048).unwrap();
    let empty = BitVector::zeros(0).unwrap();
    for q in [3, 8, 17] {
        let gates = (0..q).flat_map(|j| {
            [
                G::Clifford(C::H(j)),
                G::T { qubit: j, turns: 1 },
                G::Clifford(C::H(j)),
            ]
        });
        let state = UnitaryCircuit::new(q, gates)
            .unwrap()
            .compile_with_strategy(
                &BitVector::zeros(q).unwrap(),
                &a,
                CompilerStrategy::SharedCommutingLayer,
            )
            .unwrap();
        let word = PauliWord::new(
            BitVector::zeros(q).unwrap(),
            BitVector::zeros(q).unwrap(),
            1,
        )
        .unwrap();
        let layout = DensityLayout {
            outputs: &[],
            event_rows: &[],
        };
        let before = state.prepare_probability(&[], &a).unwrap();
        let (probability, probability_allocations) = measure(|| {
            before
                .with_native_phase_resource(256, NativePhaseOptions::default(), None, &a)
                .unwrap()
        });
        let before = state.prepare_pauli_moment(&word, &[], &a).unwrap();
        let (moment, moment_allocations) = measure(|| {
            before
                .with_native_phase_resource(256, NativePhaseOptions::default(), None, &a)
                .unwrap()
        });
        let before = state
            .prepare_density_overlap(layout, &state, layout, &a)
            .unwrap();
        let (density, density_allocations) = measure(|| {
            before
                .with_native_phase_resource(256, NativePhaseOptions::default(), None, &a)
                .unwrap()
        });
        let rows = [
            bits(q, [0]),
            bits(q, [1]),
            bits(q, [2]),
            bits(q, [0, 1]),
            bits(q, []),
        ];
        let (support, support_allocations) = measure(|| {
            state
                .prepare_probability_support_with_native(
                    &rows,
                    256,
                    NativePhaseOptions::default(),
                    None,
                    &a,
                )
                .unwrap()
        });
        assert!(
            support.fixed_phase_resource().quotient_rank() > 0,
            "exclude the distinct rank-zero Clifford shortcut from the allocation comparison"
        );
        for cap in 0..3 {
            let options = zero_cap(cap);
            let before = state.prepare_probability(&[], &a).unwrap();
            let (skipped, allocations) = measure(|| {
                before
                    .with_native_phase_resource(256, options, None, &a)
                    .unwrap()
            });
            assert_eq!(
                allocations, probability_allocations,
                "probability cap={cap} q={q}"
            );
            check_skip(
                probability.fixed_phase_resource().unwrap(),
                skipped.fixed_phase_resource().unwrap(),
            );
            assert_eq!(
                skipped.evaluate(&state, &empty, 256, &a).unwrap().value,
                ExactScalar::one()
            );
            let before = state.prepare_pauli_moment(&word, &[], &a).unwrap();
            let (skipped, allocations) = measure(|| {
                before
                    .with_native_phase_resource(256, options, None, &a)
                    .unwrap()
            });
            assert_eq!(allocations, moment_allocations, "moment cap={cap} q={q}");
            check_skip(
                moment.fixed_phase_resource().unwrap(),
                skipped.fixed_phase_resource().unwrap(),
            );
            assert_eq!(
                skipped.evaluate(&state, &empty, 256, &a).unwrap().value,
                a.omega(1, 0).unwrap()
            );
            let before = state
                .prepare_density_overlap(layout, &state, layout, &a)
                .unwrap();
            let (skipped, allocations) = measure(|| {
                before
                    .with_native_phase_resource(256, options, None, &a)
                    .unwrap()
            });
            assert_eq!(allocations, density_allocations, "density cap={cap} q={q}");
            check_skip(
                density.fixed_phase_resource().unwrap(),
                skipped.fixed_phase_resource().unwrap(),
            );
            assert_eq!(
                skipped
                    .evaluate(&state, &state, &empty, &empty, 256, &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
            let (skipped, allocations) = measure(|| {
                state
                    .prepare_probability_support_with_native(&rows, 256, options, None, &a)
                    .unwrap()
            });
            assert_eq!(allocations, support_allocations, "support cap={cap} q={q}");
            check_skip(
                support.fixed_phase_resource(),
                skipped.fixed_phase_resource(),
            );
            let terms = skipped
                .evaluate(&state, 256, &a)
                .unwrap()
                .iter()
                .map(|ket| ket.prepare().unwrap())
                .collect::<Vec<_>>();
            for x in 0_usize..32 {
                let label = bits(5, (0..5).filter(|j| x >> j & 1 != 0));
                let actual = terms.iter().fold(ExactScalar::zero(), |sum, ket| {
                    a.add(&sum, &ket.amplitude(&label, &a).unwrap()).unwrap()
                });
                let mut expected = ExactScalar::zero();
                if ((x >> 3) & 1) == ((x & 1) ^ ((x >> 1) & 1)) && x & 16 == 0 {
                    expected = ExactScalar::one();
                    for j in 0..3 {
                        let mass = a
                            .scalar(
                                (if x >> j & 1 == 0 {
                                    [2, 1, 0, -1]
                                } else {
                                    [2, -1, 0, 1]
                                })
                                .map(Into::into),
                                -2,
                            )
                            .unwrap();
                        expected = a.multiply(&expected, &mass).unwrap();
                    }
                }
                assert_eq!(actual, expected, "dependent label={x} cap={cap} q={q}");
            }
        }
    }
}
