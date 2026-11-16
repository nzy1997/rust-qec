// SPDX-License-Identifier: MPL-2.0
// Attributed source scenarios from tests/native/compact_suffix.cpp at pinned
// itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f.
// Full license: fixtures/original_many_record_input.LICENSE.
#[allow(dead_code)]
#[path = "support/original_gf2.rs"]
mod original_gf2;
use original_gf2::Mt19937;
use rmagna::compact::{CompactBitVector as CompactBits, CompactPauliWord as CompactFrame};
use rmagna::{gf2::BitVector, pauli::PauliWord};
use std::collections::{BTreeMap as Map, BTreeSet as Set};
const WIDTH: usize = 100_100;
fn bits(values: &Set<usize>) -> BitVector {
    BitVector::from_indices(WIDTH, values.iter().copied()).unwrap()
}
fn set(values: impl IntoIterator<Item = usize>) -> Set<usize> {
    values.into_iter().collect()
}
fn xor(a: &Set<usize>, b: &Set<usize>) -> Set<usize> {
    a.symmetric_difference(b).copied().collect()
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Oracle {
    x: Set<usize>,
    z: Set<usize>,
    p: u8,
}
impl Oracle {
    fn new(x: &[usize], z: &[usize], p: u8) -> Self {
        Self {
            x: set(x.iter().copied()),
            z: set(z.iter().copied()),
            p,
        }
    }
    fn product(&self, b: &Self) -> Self {
        let sign = self.z.intersection(&b.x).count() % 2;
        Self {
            x: xor(&self.x, &b.x),
            z: xor(&self.z, &b.z),
            p: (self.p + b.p + 2 * sign as u8) % 4,
        }
    }
    fn dense(&self) -> PauliWord {
        PauliWord::new(bits(&self.x), bits(&self.z), 2 * self.p).unwrap()
    }
    fn saved(&self) -> CompactFrame {
        CompactFrame::from_dense(&self.dense())
    }
    fn assert_frame(&self, actual: &CompactFrame) {
        let word = actual.materialize().unwrap();
        assert_eq!(set(word.x().ones()), self.x);
        assert_eq!(set(word.z().ones()), self.z);
        assert_eq!(word.phase(), 2 * self.p);
    }
}
fn roundtrip(values: Set<usize>) {
    let mut original = bits(&values);
    let saved = CompactBits::from_dense(&original);
    assert_eq!(saved.materialize().unwrap(), original);
    let base = set([1, 63, 65, 100_003]);
    let target = CompactBits::from_dense(&bits(&base));
    let changed = saved.xor(&target).unwrap();
    assert_eq!(
        set(changed.materialize().unwrap().ones()),
        xor(&base, &values)
    );
    assert_eq!(
        changed.xor(&saved).unwrap().materialize().unwrap(),
        bits(&base)
    );
    assert_eq!(saved.xor(&saved).unwrap().word_limit(), 0);
    let copy = saved.clone();
    let moved = copy;
    assert_eq!(moved.materialize().unwrap(), original);
    original = BitVector::zeros(WIDTH).unwrap();
    assert!(original.is_zero());
    assert_eq!(set(saved.materialize().unwrap().ones()), values);
}
#[test]
fn complete_original_storage_and_ranges() {
    let mut cases = 1;
    roundtrip(Set::new());
    for bit in [0, 63, 64, 65, 127, 128, 129, 65_535, 65_536, 100_000] {
        for values in [set([bit]), set([0, bit]), set([bit, bit + 1, bit + 64])] {
            roundtrip(values);
            cases += 1;
        }
    }
    for first in [0, 61, 64, 99_328] {
        roundtrip(set(first..first + 257));
        roundtrip(set((first..first + 257).step_by(64)));
        cases += 2;
    }
    let distant = CompactBits::from_dense(&bits(&set([0, 100_000])));
    let high = CompactBits::from_dense(&bits(&set([100_000])));
    assert!(distant.is_sparse());
    assert!(!high.is_sparse());
    assert!(distant.heap_payload_bytes() < 128 && high.heap_payload_bytes() < 128);
    let mut rng = Mt19937::new(2026100501);
    for trial in 0..64 {
        let sparse = set((0..1 + trial % 19).map(|_| (rng.next() % 100_001) as usize));
        let dense = set((0..1 + trial * 4).filter_map(|j| (rng.next() & 1 != 0).then_some(63 + j)));
        roundtrip(sparse.clone());
        roundtrip(dense.clone());
        cases += 2;
        for p in 0..4 {
            let value = Oracle {
                x: sparse.clone(),
                z: dense.clone(),
                p,
            };
            let saved = value.saved();
            value.assert_frame(&saved);
            let initial = Oracle::new(&[0, 65, 100_000], &[63, 129], 3);
            Oracle {
                x: xor(&initial.x, &sparse),
                z: xor(&initial.z, &dense),
                p: 3,
            }
            .assert_frame(&saved.xor_coordinates(&initial.saved()).unwrap());
        }
    }
    let mut ranges = 0;
    for high in [63, 64, 65, 127, 128, 65_535, 65_536, 100_000] {
        for distant in [false, true] {
            let source = if distant { set([0, high]) } else { set([high]) };
            let input = BitVector::from_indices(high + 1, source.iter().copied()).unwrap();
            let saved = CompactBits::from_dense(&input);
            assert_eq!(saved.word_limit(), high / 64 + 1);
            assert!(saved.word_limit() <= input.words().len());
            let padded =
                BitVector::from_indices((input.words().len() + 65) * 64, source.iter().copied())
                    .unwrap();
            let padded = CompactBits::from_dense(&padded);
            assert_eq!(padded.word_limit(), high / 64 + 1);
            assert_eq!(set(padded.materialize().unwrap().ones()), source);
            ranges += 1;
        }
    }
    let zero = CompactBits::from_dense(&BitVector::zeros(100 * 64).unwrap());
    assert_eq!(zero.word_limit(), 0);
    assert!(zero.materialize().unwrap().is_zero());
    println!(
        "source cases: {cases} storage cases, {ranges} padded ranges, 256 phase/XOR cases, original MT stream draws={}",
        rng.draws
    );
}
#[derive(Clone)]
struct Row {
    position: usize,
    x: Oracle,
    z: Oracle,
}
type Fault = (usize, i32, u8);
fn rows() -> Map<i32, Vec<Row>> {
    let mut out = Map::<i32, Vec<Row>>::new();
    let mut add = |wire, position, x, z| out.entry(wire).or_default().push(Row { position, x, z });
    add(
        10,
        1,
        Oracle::new(&[0, 100_000], &[65], 1),
        Oracle::new(&[63], &[0], 2),
    );
    add(
        10,
        4,
        Oracle::new(&[65], &[0, 100_000], 2),
        Oracle::new(&[0], &[63, 100_000], 3),
    );
    add(
        10,
        6,
        Oracle::new(&[], &[], 3),
        Oracle::new(&[100_000], &[100_000], 1),
    );
    add(
        11,
        8,
        Oracle::new(&[129], &[65], 0),
        Oracle::new(&[65], &[129], 1),
    );
    add(
        11,
        12,
        Oracle::new(&[0, 64, 100_000], &[63], 2),
        Oracle::new(&[63], &[0, 64, 100_000], 3),
    );
    for at in [1, 4, 8, 12] {
        add(
            20,
            at,
            Oracle::new(&[at, 64], &[100_000], (at % 4) as u8),
            Oracle::new(&[100_000], &[at, 64], ((at + 1) % 4) as u8),
        );
    }
    out
}
const LOCATIONS: [usize; 6] = [0, 2, 5, 6, 9, 12];
fn histories() -> Map<i32, Vec<(i32, i32)>> {
    Map::from([(5, vec![(-1, 10), (3, 11), (5, -1)]), (7, vec![(-1, 20)])])
}
fn expected(rows: &Map<i32, Vec<Row>>, input: &[Fault]) -> Result<Oracle, ()> {
    let histories = histories();
    let mut out = Oracle::default();
    for (location, position) in LOCATIONS.into_iter().enumerate() {
        for &(loc, wire, pauli) in input {
            if loc != location || pauli == 0 {
                continue;
            }
            let mut internal = -1;
            for &(first, next) in histories.get(&wire).ok_or(())? {
                if first <= location as i32 {
                    internal = next;
                }
            }
            if internal < 0 {
                return Err(());
            }
            let row = rows
                .get(&internal)
                .ok_or(())?
                .iter()
                .find(|r| r.position >= position)
                .ok_or(())?;
            let mut image = if pauli == 1 {
                row.x.clone()
            } else {
                row.z.clone()
            };
            if pauli == 3 {
                image = row.x.product(&row.z);
                image.p = (image.p + 1) % 4;
            }
            out = image.product(&out);
        }
    }
    Ok(out)
}
// The source manually supplies arbitrary suffix rows, not necessarily Clifford
// images. This adapter exercises public compact operations over those rows;
// separate tests below check real compiled storage and physical wire lifetimes.
fn transported(
    rows: &Map<i32, Vec<Row>>,
    input: &[Fault],
    phase: bool,
) -> Result<CompactFrame, ()> {
    let histories = histories();
    let mut input = input.to_vec();
    input.sort_by_key(|f| f.0);
    let mut out = Oracle::default().saved();
    for (loc, wire, pauli) in input {
        let position = *LOCATIONS.get(loc).ok_or(())?;
        if pauli == 0 {
            continue;
        }
        let history = histories.get(&wire).ok_or(())?;
        let h = history
            .partition_point(|&(first, _)| first <= loc as i32)
            .checked_sub(1)
            .ok_or(())?;
        let internal = history[h].1;
        if internal < 0 {
            return Err(());
        }
        let values = rows.get(&internal).ok_or(())?;
        let row = values
            .get(values.partition_point(|r| r.position < position))
            .ok_or(())?;
        if !phase {
            if pauli == 1 || pauli == 3 {
                out = row.x.saved().xor_coordinates(&out).unwrap();
            }
            if pauli == 2 || pauli == 3 {
                out = row.z.saved().xor_coordinates(&out).unwrap();
            }
        } else {
            let image = if pauli == 3 {
                let value = row
                    .x
                    .saved()
                    .multiply(&row.z.saved())
                    .unwrap()
                    .materialize()
                    .unwrap();
                CompactFrame::from_dense(
                    &PauliWord::new(
                        value.x().clone(),
                        value.z().clone(),
                        (value.phase() + 2) & 7,
                    )
                    .unwrap(),
                )
            } else if pauli == 1 {
                row.x.saved()
            } else {
                row.z.saved()
            };
            out = image.multiply(&out).unwrap();
        }
    }
    Ok(out)
}
#[test]
fn complete_original_manual_order_phase_lifetime_oracle() {
    let rows = rows();
    for row in rows.values().flatten() {
        row.x.assert_frame(&row.x.saved());
        row.z.assert_frame(&row.z.saved());
    }
    let mut cases = vec![
        vec![(4, 5, 3), (0, 5, 1), (4, 5, 2), (1, 7, 3)],
        vec![(1, 5, 1), (1, 5, 2)],
        vec![(1, 5, 2), (1, 5, 1)],
        vec![(0, 5, 3), (0, 5, 3)],
        vec![(5, 7, 3), (2, 5, 2), (3, 5, 1), (4, 7, 2)],
        vec![(0, 999, 0)],
        vec![],
    ];
    let mut rng = Mt19937::new(2026100502);
    for trial in 0..128 {
        let mut faults = vec![];
        for _ in 0..trial % 11 {
            let loc = (rng.next() % 6) as usize;
            let wire = if loc < 5 && rng.next() & 1 != 0 { 5 } else { 7 };
            faults.push((loc, wire, (rng.next() % 4) as u8));
        }
        cases.push(faults);
    }
    for faults in &cases {
        let mut oracle = expected(&rows, faults).unwrap();
        oracle.assert_frame(&transported(&rows, faults, true).unwrap());
        oracle.assert_frame(&transported(&rows.clone(), faults, true).unwrap());
        oracle.p = 0;
        oracle.assert_frame(&transported(&rows, faults, false).unwrap());
    }
    for faults in [vec![(5, 5, 1)], vec![(0, 999, 1)]] {
        assert!(transported(&rows, &faults, true).is_err());
        assert!(expected(&rows, &faults).is_err());
    }
    println!(
        "source cases: {} manual assignments, tracked/untracked/copy modes, 2 lifetime rejections, original MT draws={}",
        cases.len(),
        rng.draws
    );
}

use rmagna::{
    clifford::CliffordCircuit,
    instrument::*,
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::{
        CompilerStrategy, InsertionPreparationOptions, PauliInsertion, UnitaryCircuit, UnitaryError,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
const COMPILERS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
fn bit(n: usize, one: bool) -> BitVector {
    BitVector::from_indices(n, one.then_some(0)).unwrap()
}
fn source(inverted: bool, callback: Option<Arc<AtomicUsize>>) -> OriginalInstrument {
    let a = ScalarArithmetic::default();
    let input = rmagna::stabilizer::AffineKet::new(
        bit(1, false),
        vec![bit(1, true)],
        rmagna::quadratic::QuadraticForm::new(0, vec![0], vec![bit(1, false)]).unwrap(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let mut ops = vec![
        OriginalOperation {
            id: "t".into(),
            gate: OriginalGate::Phase {
                wires: vec![0],
                turns: 1,
            },
        },
        OriginalOperation {
            id: "m".into(),
            gate: OriginalGate::Measure {
                factors: vec![(0, OriginalPauli::X)],
                inverted,
                record: "m".into(),
            },
        },
    ];
    let controls = if let Some(calls) = callback {
        ops.push(OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        });
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["m".into()],
            version: "1".into(),
            callback: Arc::new(move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![OriginalGate::Identity(vec![0])])
            }),
        }]
    } else {
        vec![]
    };
    OriginalInstrument::new(
        vec![0],
        vec![0],
        input,
        ops,
        controls,
        OriginalLimits::default(),
        &a,
    )
    .unwrap()
}
fn fault(p: OriginalPauli) -> OriginalFault {
    OriginalFault {
        after: "t".into(),
        wire: 0,
        pauli: p,
    }
}
fn assignment(value: bool, p: OriginalPauli) -> OriginalAssignment {
    OriginalAssignment {
        true_values: vec![("m".into(), value)],
        faults: vec![fault(p)],
        report_flips: vec![],
    }
}
fn low_high() -> [ExactScalar; 2] {
    let a = ScalarArithmetic::default();
    let half = a.omega(0, -2).unwrap();
    let quarter_root = a.omega(0, -3).unwrap();
    [
        a.subtract(&half, &quarter_root).unwrap(),
        a.add(&half, &quarter_root).unwrap(),
    ]
}
#[test]
fn real_compiler_retains_compact_high_offset_suffix_and_exact_ordered_phase() {
    let a = ScalarArithmetic::default();
    let width = 100_065;
    for compiler in COMPILERS {
        let zero = BitVector::zeros(width).unwrap();
        let base = UnitaryCircuit::new(width, vec![])
            .unwrap()
            .compile_with_strategy(&zero, &a, compiler)
            .unwrap();
        let high = BitVector::from_indices(width, [100_000]).unwrap();
        let mut catalog = vec![
            PauliInsertion {
                before_gate: 0,
                word: PauliWord::new(high.clone(), zero.clone(), 3).unwrap(),
            },
            PauliInsertion {
                before_gate: 0,
                word: PauliWord::new(zero.clone(), high, 4).unwrap(),
            },
        ];
        let required =
            match base.insertion_preparation_requirement(catalog.len(), Default::default()) {
                Err(UnitaryError::InsertionResourceLimit {
                    option: "max_table_bytes",
                    required,
                    ..
                }) => required,
                other => panic!("expected conservative worst-case admission, got {other:?}"),
            };
        // Admission remains a worst-case bound, not a claim that this sparse
        // source needs that many bytes. Raise it explicitly for this diagnostic.
        let options = InsertionPreparationOptions {
            max_catalog_entries: 2,
            max_table_bytes: usize::try_from(required).unwrap(),
        };
        let prepared = base.prepare_insertions(&catalog, options).unwrap();
        let stats = prepared.storage_statistics();
        assert_eq!(stats.catalog_entries, 2);
        assert_eq!(stats.retained_mask_payload_bytes, 0);
        assert_eq!(stats.retained_heap_buffers, 2); // entry and lookup vectors.
        assert_eq!(stats.final_suffix_mask_payload_bytes, 0);
        assert_eq!(stats.final_suffix_heap_buffers, 2); // generator-row vectors.
        assert!(stats.final_suffix_inline_and_table_bytes < 32 * 1024 * 1024);
        assert!(stats.retained_inline_and_table_bytes < 4096);
        let original = [catalog[0].clone(), catalog[1].clone(), catalog[0].clone()];
        catalog.clear();
        let frame = prepared.transported_frame(&[0, 1, 0]).unwrap();
        assert_eq!(frame.phase(), 6);
        assert!(frame.x().ones().next().is_none());
        assert_eq!(frame.z().ones().collect::<Vec<_>>(), [100_000]);
        assert_eq!(frame.heap_payload_bytes(), 0);
        let packed = base
            .with_prepared_insertions(&prepared, &[0, 1, 0], &a)
            .unwrap();
        let direct = base.with_insertions(&original, &a).unwrap();
        assert_eq!(packed.width(), direct.width());
        assert_eq!(packed.phase_legs(), direct.phase_legs());
    }
    let extreme = CompactBits::basis(usize::MAX, usize::MAX - 1).unwrap();
    assert_eq!(extreme.word_limit(), usize::MAX.div_ceil(64));
    assert_eq!(extreme.heap_payload_bytes(), 0);
    assert!(CompactBits::basis(usize::MAX, usize::MAX).is_err());
    let high_word = CompactBits::from_indices(4_194_369, [0, 4_194_304]).unwrap();
    assert_eq!(high_word.word_limit(), 65_537);
    assert_eq!(
        set(high_word.materialize().unwrap().ones()),
        set([0, 4_194_304])
    );
    assert!(high_word.is_sparse() && high_word.heap_payload_bytes() < 128);
    println!(
        "2 actual 100065-wire compilers, exact X/Z/X phase, zero mask heap payload and only2 suffix table buffers; additional word65536 and usize endpoint guards"
    );
}
#[test]
fn original_shared_immutable_faults_pack_once_with_distinct_records_and_each_batch_restarts() {
    let a = ScalarArithmetic::default();
    let expected = low_high();
    let mut values = 0;
    for compiler in COMPILERS {
        let plan = OriginalAssignmentPlan::from_owned(source(false, None), compiler, a);
        let layout = OriginalCompactNoiseLayout::new(
            &plan,
            vec![
                OriginalCompactNoiseChannel::pauli_site("t", &[0], &[vec![OriginalPauli::Z]])
                    .unwrap(),
            ],
        )
        .unwrap();
        let assignments = layout
            .assignments(
                vec![vec![(serde_json::json!(0), serde_json::json!(1))]],
                vec![bit(1, false), bit(1, true)],
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            assignments[0]
                .shared_faults()
                .shares_storage_with(assignments[1].shared_faults())
        );
        for (j, input) in assignments.iter().enumerate() {
            assert_eq!(plan.probability(Arc::clone(input)).unwrap(), expected[j]);
        }
        assert_ne!(expected[0], expected[1]);
        let packed = plan
            .prepare_packed_probabilities(Default::default())
            .unwrap();
        for _ in 0..2 {
            let mut batch = packed.probabilities(assignments.iter().cloned());
            assert_eq!(
                batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
                expected
            );
            assert!(batch.next().is_none());
            assert_eq!(
                batch.statistics(),
                OriginalPackedBatchStatistics {
                    inputs: 2,
                    packed_fault_groups: 1,
                    prepared_record_laws: 1,
                    values: 2
                }
            );
            values += 2;
        }
        // A changed group in the next batch cannot inherit the previous Z law.
        let clean = plan
            .compact(bit(1, false), OriginalSharedFaults::empty())
            .unwrap();
        let mut batch = packed.probabilities([clean]);
        assert_eq!(batch.next().unwrap().unwrap(), expected[1]);
        assert_eq!(batch.statistics().packed_fault_groups, 1);
        values += 1;
    }
    println!(
        "{values} actual packed values across6 batches and2 compilers, with literal reference comparisons; same immutable fault group gives distinct exact record masses and packs once afresh per batch"
    );
}
#[test]
fn packed_batches_repack_mutable_inputs_and_reject_foreign_owners_before_empty_group_reuse() {
    let a = ScalarArithmetic::default();
    let expected = low_high();
    for compiler in COMPILERS {
        let plan = OriginalAssignmentPlan::from_owned(source(false, None), compiler, a);
        let packed = plan
            .prepare_packed_probabilities(Default::default())
            .unwrap();
        let mut ordinary = Arc::new(assignment(false, OriginalPauli::I));
        let mut index = 0;
        let inputs = std::iter::from_fn(move || {
            if index == 2 {
                return None;
            }
            assert_eq!(Arc::strong_count(&ordinary), 1);
            Arc::get_mut(&mut ordinary).unwrap().faults[0].pauli = if index == 0 {
                OriginalPauli::I
            } else {
                OriginalPauli::Z
            };
            index += 1;
            Some(Arc::clone(&ordinary))
        });
        let mut batch = packed.probabilities(inputs);
        assert_eq!(
            batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
            [expected[1].clone(), expected[0].clone()]
        );
        assert_eq!(batch.statistics().packed_fault_groups, 2);
        let z = plan
            .shared_faults(vec![fault(OriginalPauli::Z)], vec![])
            .unwrap();
        let inputs = [z.clone(), OriginalSharedFaults::empty(), z]
            .into_iter()
            .map(|group| plan.compact(bit(1, false), group).unwrap())
            .collect::<Vec<_>>();
        let mut transitions = packed.probabilities(inputs);
        assert_eq!(
            transitions.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
            [
                expected[0].clone(),
                expected[1].clone(),
                expected[0].clone()
            ]
        );
        assert_eq!(transitions.statistics().packed_fault_groups, 3);
        let foreign = OriginalAssignmentPlan::from_owned(source(false, None), compiler, a);
        let first = plan
            .compact(bit(1, false), OriginalSharedFaults::empty())
            .unwrap();
        let second = foreign
            .compact(bit(1, false), OriginalSharedFaults::empty())
            .unwrap();
        assert!(
            first
                .shared_faults()
                .shares_storage_with(second.shared_faults())
        );
        let mut batch = packed.probabilities([first, second]);
        assert_eq!(batch.next().unwrap().unwrap(), expected[1]);
        assert!(
            batch
                .next()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("different IR snapshot")
        );
        assert!(batch.next().is_none());
        assert_eq!(
            batch.statistics(),
            OriginalPackedBatchStatistics {
                inputs: 2,
                packed_fault_groups: 1,
                prepared_record_laws: 1,
                values: 1
            }
        );
    }
    println!(
        "4 actual mutable-input packings and2 foreign-owner refusals before shared-empty reuse; consumed ordinary inputs are not retained"
    );
}

#[test]
fn compact_mutation_widths_and_phase_errors_preserve_exact_coordinates() {
    let mut saved = CompactBits::zeros(100_065);
    let mut dense = BitVector::zeros(100_065).unwrap();
    for q in [0, 63, 64, 65, 65_535, 65_536, 100_000] {
        saved.set(q, true).unwrap();
        dense.set(q, true).unwrap();
        assert!(saved.get(q).unwrap());
        assert_eq!(saved.materialize().unwrap(), dense);
    }
    let before = saved.clone();
    assert!(saved.set(100_065, true).is_err());
    assert_eq!(saved, before);
    assert!(saved.resized(100_000).is_err());
    let larger = saved.resized(100_130).unwrap();
    assert_eq!(set(larger.ones()), set(dense.ones()));
    assert!(saved.xor(&CompactBits::zeros(100_064)).is_err());
    for q in [0, 63, 64, 65, 65_535, 65_536, 100_000] {
        saved.set(q, false).unwrap();
        assert!(!saved.get(q).unwrap());
    }
    assert_eq!(saved.word_limit(), 0);
    assert_eq!(saved.heap_payload_bytes(), 0);
    assert_eq!(saved.statistics().heap_buffers, 0);
    assert_eq!(saved.resized(0).unwrap(), CompactBits::zeros(0));
    assert!(matches!(
        CompactFrame::new(CompactBits::zeros(1), CompactBits::zeros(2), 0),
        Err(rmagna::pauli::PauliError::DimensionMismatch)
    ));
    assert!(matches!(
        CompactFrame::new(CompactBits::zeros(1), CompactBits::zeros(1), 8),
        Err(rmagna::pauli::PauliError::InvalidPhase)
    ));
    assert!(matches!(
        CompactFrame::identity(1).multiply(&CompactFrame::identity(2)),
        Err(rmagna::pauli::PauliError::DimensionMismatch)
    ));
}
#[test]
fn actual_prepared_selection_matches_complex_direct_queries_with_shared_phase_updates() {
    use rmagna::{clifford::CliffordGate as C, unitary::UnitaryGate as U};
    let a = ScalarArithmetic::default();
    let mut pairs = 0;
    let circuit = UnitaryCircuit::new(
        3,
        vec![
            U::Clifford(C::H(0)),
            U::T { qubit: 0, turns: 1 },
            U::Clifford(C::H(1)),
            U::T { qubit: 1, turns: 1 },
            U::Clifford(C::Cx {
                control: 0,
                target: 1,
            }),
            U::T { qubit: 1, turns: 3 },
            U::Clifford(C::H(2)),
            U::Clifford(C::Cz { a: 1, b: 2 }),
            U::T { qubit: 0, turns: 5 },
            U::Clifford(C::H(0)),
        ],
    )
    .unwrap();
    for compiler in COMPILERS {
        let base = circuit
            .compile_with_strategy(&BitVector::zeros(3).unwrap(), &a, compiler)
            .unwrap();
        for phase in 0..8 {
            let catalog = vec![
                PauliInsertion {
                    before_gate: 0,
                    word: PauliWord::new(bit(3, true), BitVector::zeros(3).unwrap(), phase)
                        .unwrap(),
                },
                PauliInsertion {
                    before_gate: 3,
                    word: PauliWord::new(
                        BitVector::from_indices(3, [1]).unwrap(),
                        BitVector::from_indices(3, [1]).unwrap(),
                        2,
                    )
                    .unwrap(),
                },
                PauliInsertion {
                    before_gate: 3,
                    word: PauliWord::new(
                        BitVector::from_indices(3, [0, 2]).unwrap(),
                        BitVector::from_indices(3, [1, 2]).unwrap(),
                        5,
                    )
                    .unwrap(),
                },
                PauliInsertion {
                    before_gate: 8,
                    word: PauliWord::new(
                        BitVector::zeros(3).unwrap(),
                        BitVector::from_indices(3, [2]).unwrap(),
                        7,
                    )
                    .unwrap(),
                },
            ];
            let prepared = base
                .prepare_insertions(&catalog, Default::default())
                .unwrap();
            for indices in [
                vec![],
                vec![0],
                vec![1, 2],
                vec![2, 1],
                vec![3, 2, 1, 0],
                vec![0, 1, 0, 2],
            ] {
                let selected = indices
                    .iter()
                    .map(|&j| catalog[j].clone())
                    .collect::<Vec<_>>();
                let direct = base.with_insertions(&selected, &a).unwrap();
                let packed = base
                    .with_prepared_insertions(&prepared, &indices, &a)
                    .unwrap();
                for output in [0usize, 1, 5, 7] {
                    let word =
                        BitVector::from_indices(3, (0..3).filter(|&q| output & (1 << q) != 0))
                            .unwrap();
                    assert_eq!(
                        packed.amplitude(&word, 1024, &a).unwrap().value,
                        direct.amplitude(&word, 1024, &a).unwrap().value
                    );
                    pairs += 1;
                }
            }
            assert!(matches!(
                prepared.transported_frame(&[usize::MAX]),
                Err(UnitaryError::InvalidPreparedInsertion {
                    index: usize::MAX,
                    count: 4
                })
            ));
        }
    }
    println!(
        "{pairs} exact complex packed/direct query pairs across2 compilers,8 scalar phases, duplicate/equal-time/reversed selections and shared observation updates"
    );
}
#[test]
fn actual_packed_lifetimes_and_named_validation_preserve_error_prefixes() {
    let a = ScalarArithmetic::default();
    for compiler in COMPILERS {
        let input = CliffordCircuit::computational(bit(1, false)).unwrap();
        let ir = OriginalInstrument::new(
            vec![5],
            vec![],
            input,
            vec![
                OriginalOperation {
                    id: "initial".into(),
                    gate: OriginalGate::Identity(vec![5]),
                },
                OriginalOperation {
                    id: "reset".into(),
                    gate: OriginalGate::Reset {
                        wire: 5,
                        plus: true,
                    },
                },
                OriginalOperation {
                    id: "measure".into(),
                    gate: OriginalGate::Measure {
                        factors: vec![(5, OriginalPauli::X)],
                        inverted: false,
                        record: "m".into(),
                    },
                },
                OriginalOperation {
                    id: "drop".into(),
                    gate: OriginalGate::Discard(5),
                },
            ],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        let plan = OriginalAssignmentPlan::from_owned(ir, compiler, a);
        let packed = plan
            .prepare_packed_probabilities(Default::default())
            .unwrap();
        for (anchor, wire, pauli, value, expected) in [
            ("initial", 5, OriginalPauli::X, false, ExactScalar::one()),
            ("reset", 5, OriginalPauli::Z, true, ExactScalar::one()),
            ("reset", 5, OriginalPauli::Z, false, ExactScalar::zero()),
            ("drop", 999, OriginalPauli::I, false, ExactScalar::one()),
        ] {
            let assignment = OriginalAssignment {
                true_values: vec![("m".into(), value)],
                faults: vec![OriginalFault {
                    after: anchor.into(),
                    wire,
                    pauli,
                }],
                report_flips: vec![],
            };
            assert_eq!(plan.probability(assignment.clone()).unwrap(), expected);
            assert_eq!(
                packed.probabilities([assignment]).next().unwrap().unwrap(),
                expected
            );
        }
        for (anchor, wire) in [("drop", 5), ("initial", 999)] {
            let bad = OriginalAssignment {
                true_values: vec![("m".into(), false)],
                faults: vec![OriginalFault {
                    after: anchor.into(),
                    wire,
                    pauli: OriginalPauli::X,
                }],
                report_flips: vec![],
            };
            let mut batch = packed.probabilities([bad]);
            assert!(
                matches!(batch.next().unwrap(),Err(OriginalError::Lifetime{wire:w,expected_live:true})if w==wire)
            );
            assert!(batch.next().is_none());
            assert_eq!(batch.statistics().packed_fault_groups, 0);
        }
        for values in [
            vec![],
            vec![("extra".into(), false)],
            vec![("m".into(), false), ("m".into(), true)],
        ] {
            let good = OriginalAssignment {
                true_values: vec![("m".into(), false)],
                ..Default::default()
            };
            let bad = OriginalAssignment {
                true_values: values,
                ..Default::default()
            };
            let mut batch = packed.probabilities([good, bad]);
            assert_eq!(batch.next().unwrap().unwrap(), ExactScalar::one());
            assert!(batch.next().unwrap().is_err());
            assert!(batch.next().is_none());
            assert_eq!(
                batch.statistics(),
                OriginalPackedBatchStatistics {
                    inputs: 2,
                    packed_fault_groups: 1,
                    prepared_record_laws: 1,
                    values: 1
                }
            );
        }
    }
    println!(
        "8 actual reset/historical/retired-identity values,4 lifetime refusals and6 named-record rejection prefixes; checked against literal original compilation"
    );
}
#[test]
fn true_and_reported_words_inversions_and_controls_have_explicit_static_boundaries() {
    let a = ScalarArithmetic::default();
    let expected = low_high();
    for inverted in [false, true] {
        let plan = OriginalAssignmentPlan::from_owned(
            source(inverted, None),
            CompilerStrategy::SharedCommutingLayer,
            a,
        );
        let packed = plan
            .prepare_packed_probabilities(Default::default())
            .unwrap();
        let group = plan
            .shared_faults(vec![fault(OriginalPauli::Z)], vec!["m".into()])
            .unwrap();
        let compact = plan.compact(bit(1, false), group).unwrap();
        let found = packed.probabilities([compact]).next().unwrap().unwrap();
        assert_eq!(found, expected[usize::from(inverted)]);
        let ordinary = OriginalAssignment {
            report_flips: vec!["m".into()],
            ..assignment(false, OriginalPauli::Z)
        };
        assert_eq!(plan.probability(ordinary.clone()).unwrap(), found);
        assert_eq!(
            packed.probabilities([ordinary]).next().unwrap().unwrap(),
            found
        );
        let dynamic = OriginalRecordAssignment {
            records: vec![("m".into(), serde_json::json!(0))],
            faults: vec![fault(OriginalPauli::Z)],
            report_flips: vec!["m".into()],
        };
        assert_eq!(
            packed.probabilities([dynamic]).next().unwrap().unwrap(),
            found
        );
        let layout = OriginalCompactNoiseLayout::new(
            &plan,
            vec![
                OriginalCompactNoiseChannel::pauli_site("t", &[0], &[vec![OriginalPauli::Z]])
                    .unwrap(),
                vec![OriginalCompactNoiseChannel::report("m".into()).unwrap()],
            ],
        )
        .unwrap();
        let inputs = layout
            .assignments(
                vec![vec![
                    (serde_json::json!(0), serde_json::json!(1)),
                    (serde_json::json!(1), serde_json::json!(1)),
                ]],
                vec![bit(1, false), bit(1, true)],
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(inputs[0].word(), &bit(1, true));
        assert_eq!(inputs[1].word(), &bit(1, false));
        assert_eq!(
            packed
                .probabilities(inputs)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [
                expected[usize::from(!inverted)].clone(),
                expected[usize::from(inverted)].clone()
            ]
        );
        for value in [
            serde_json::json!(true),
            serde_json::json!(0.0),
            serde_json::json!(2),
        ] {
            let mut batch = packed.probabilities([OriginalRecordAssignment {
                records: vec![("m".into(), value)],
                ..Default::default()
            }]);
            assert!(batch.next().unwrap().is_err());
            assert_eq!(batch.statistics().packed_fault_groups, 0);
        }
        let bad_flip = OriginalAssignment {
            report_flips: vec!["missing".into()],
            ..assignment(false, OriginalPauli::Z)
        };
        let mut rejected = packed.probabilities([bad_flip]);
        assert!(rejected.next().unwrap().is_err());
        assert_eq!(rejected.statistics().packed_fault_groups, 0);
        assert!(matches!(
            plan.prepare_packed_probabilities(OriginalPackedBatchOptions {
                max_record_occurrences: 0,
                ..Default::default()
            }),
            Err(OriginalError::ResourceLimit {
                option: "max_record_occurrences",
                ..
            })
        ));
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let plan = OriginalAssignmentPlan::from_owned(
        source(false, Some(Arc::clone(&calls))),
        CompilerStrategy::ParityCopies,
        a,
    );
    assert!(matches!(
        plan.prepare_packed_probabilities(Default::default()),
        Err(OriginalError::Metadata {
            kind: "packed static assignments do not support controls",
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
