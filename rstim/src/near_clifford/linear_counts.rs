//! Optional affine counts model for plans without coherent rotations or active projections.
//! Raw detector parities and repeated observable includes remain the public contract.
use super::*;

// Bounds both retained metadata and construction scratch against the remaining plan budget.
// Independently bounds binary-category validation and adjoint execution visits.
// Matrix construction/conversion is additionally bounded by the heap byte quota.
const BUILD_WORK_LIMIT: usize = 16_000_000;

#[derive(Clone, Copy, Debug, Default)]
struct Input {
    first: usize,
    count: usize,
    binary: bool,
}
#[derive(Clone, Copy, Debug, Default)]
struct SweepInput {
    index: u32,
    variable: usize,
}
#[derive(Clone, Debug)]
pub(super) struct LinearCountsPlan {
    constants: Vec<bool>,
    offsets: Vec<usize>,
    edges: Vec<usize>,
    events: Vec<Input>,
    active_events: Vec<u64>,
    sweeps: Vec<SweepInput>,
    detectors: usize,
    observables: Vec<u32>,
}
struct Budget {
    used: usize,
    limit: usize,
}
impl Budget {
    fn zeros<T: Clone + Default>(&mut self, len: usize) -> Option<Vec<T>> {
        let bytes = len.checked_mul(size_of::<T>())?;
        if self.used.checked_add(bytes)? > self.limit {
            return None;
        }
        let mut v = Vec::new();
        v.try_reserve_exact(len).ok()?;
        self.used = self
            .used
            .checked_add(v.capacity().checked_mul(size_of::<T>())?)?;
        if self.used > self.limit {
            return None;
        }
        v.resize(len, T::default());
        Some(v)
    }
}
// for_support visits every packed word and every set-bit callback on both axes.
fn pauli_visit_work(pauli: &PackedPauli) -> Option<usize> {
    pauli.x.len().checked_add(pauli.z.len())?.checked_add(
        pauli
            .x
            .iter()
            .chain(&pauli.z)
            .try_fold(0usize, |n, word| n.checked_add(word.count_ones() as usize))?,
    )
}
fn variable_count(choices: usize, binary: bool) -> usize {
    if binary {
        usize::BITS as usize - choices.leading_zeros() as usize
    } else {
        choices
    }
}
// Noise categories are numbered 1..=N by the typed producer. Use binary columns
// only when every category equals the XOR of its numbered basis categories.
fn binary_channel(choices: &[NoiseChoice]) -> bool {
    for (index, choice) in choices.iter().enumerate() {
        for (word, (&x, &z)) in choice.pauli.x.iter().zip(&choice.pauli.z).enumerate() {
            let (mut expected_x, mut expected_z) = (0, 0);
            let mut bits = index + 1;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let basis = &choices[(1usize << bit) - 1];
                expected_x ^= basis.pauli.x[word];
                expected_z ^= basis.pauli.z[word];
            }
            if x != expected_x || z != expected_z {
                return false;
            }
        }
        // Record flips need XOR multiplicities, including duplicates. No set allocation.
        let contains =
            |records: &[usize], r: usize| records.iter().filter(|v| **v == r).count() % 2 != 0;
        let equal_at = |r: usize| {
            let mut bits = index + 1;
            let mut expected = false;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                expected ^= contains(&choices[(1usize << bit) - 1].record_flips, r);
            }
            contains(&choice.record_flips, r) == expected
        };
        if choice.record_flips.iter().any(|&r| !equal_at(r)) {
            return false;
        }
        let mut bits = index + 1;
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if choices[(1usize << bit) - 1]
                .record_flips
                .iter()
                .any(|&r| !equal_at(r))
            {
                return false;
            }
        }
    }
    true
}
impl LinearCountsPlan {
    pub(super) fn build(plan: &CompiledNearCliffordExecutor, remaining: usize) -> Option<Self> {
        if plan.operations.iter().any(|op| {
            matches!(
                op,
                PlanOp::Rotate { .. }
                    | PlanOp::Measure(Measurement {
                        projection: Projection::Active { .. },
                        ..
                    })
            )
        }) {
            return None;
        }
        let mut budget = Budget {
            used: size_of::<Self>() + 64,
            limit: remaining,
        };
        let mut modes = budget.zeros::<bool>(plan.operations.len())?;
        let annotations = plan.annotation_positions.len();
        let mut observables = budget.zeros::<u32>(annotations)?;
        let (mut detectors, mut observable_count, mut sweep_count, mut variables, mut work) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        let mut validation_work = 0usize;
        for (position, op) in plan.operations.iter().enumerate() {
            // Count conservative primitive visits, including support and record reads.
            work = work.checked_add(1)?;
            match op {
                PlanOp::Basis(gates) => work = work.checked_add(gates.len())?,
                PlanOp::Noise { choices, .. } => {
                    // Read-only validation scans are bounded too; malformed huge channels decline.
                    if choices.len() > 15 {
                        return None;
                    }
                    // At most four numbered basis categories for N<=15. Count
                    // the word scan once per category, not N times. Record parity
                    // checks visit at most five lists, each with at most rho entries.
                    let word_work = choices.iter().try_fold(0usize, |n, c| {
                        n.checked_add(c.pauli.x.len().checked_mul(12)?)
                    })?;
                    let rho = choices
                        .iter()
                        .map(|c| c.record_flips.len())
                        .max()
                        .unwrap_or(0);
                    let tested_records = rho.checked_mul(5)?;
                    let record_work = tested_records
                        .checked_mul(tested_records.checked_add(4)?)?
                        .checked_mul(choices.len())?;
                    validation_work = validation_work
                        .checked_add(word_work)?
                        .checked_add(record_work)?;
                    if validation_work > BUILD_WORK_LIMIT {
                        return None;
                    }
                    modes[position] = binary_channel(choices);
                    let count = variable_count(choices.len(), modes[position]);
                    for column in 0..count {
                        let c = &choices[if modes[position] {
                            (1usize << column) - 1
                        } else {
                            column
                        }];
                        work = work
                            .checked_add(pauli_visit_work(&c.pauli)?)?
                            .checked_add(c.record_flips.len())?;
                    }
                    if work > BUILD_WORK_LIMIT {
                        return None;
                    }
                    variables =
                        variables.checked_add(variable_count(choices.len(), modes[position]))?;
                }
                PlanOp::Measure(m) => {
                    variables = variables
                        .checked_add(usize::from(matches!(
                            m.projection,
                            Projection::Independent { .. }
                        )))?
                        .checked_add(usize::from(m.readout > 0.))?;
                    work = work
                        .checked_add(m.basis.len())?
                        .checked_add(pauli_visit_work(&m.pauli.physical)?)?
                        .checked_add(m.reset.as_ref().map_or(Some(0), pauli_visit_work)?)?;
                }
                PlanOp::Feedback { condition, pauli } => {
                    if matches!(condition, Condition::Sweep(_)) {
                        sweep_count = sweep_count.checked_add(1)?;
                        variables = variables.checked_add(1)?;
                    }
                    work = work.checked_add(pauli_visit_work(pauli)?)?;
                }
                PlanOp::Annotation {
                    offsets,
                    observable,
                } => {
                    work = work.checked_add(offsets.len())?;
                    if let Some(index) = observable {
                        observables[observable_count] = *index;
                        observable_count += 1;
                    } else {
                        detectors += 1;
                    }
                }
                PlanOp::Rotate { .. } => unreachable!(),
            }
        }
        observables.truncate(observable_count);
        observables.sort_unstable();
        observables.dedup();
        let outputs = detectors.checked_add(observables.len())?;
        let blocks = outputs.div_ceil(64);
        // Observable readers perform a binary search in the deduplicated index list.
        // Include its comparisons, including the final equality check.
        let lookup_visits = usize::BITS as usize - observables.len().leading_zeros() as usize + 2;
        work = work.checked_add(observable_count.checked_mul(lookup_visits)?)?;
        // Each adjoint block clears its variable, frame and record scratch first.
        work = work
            .checked_add(variables)?
            .checked_add(plan.num_qubits.checked_mul(2)?)?
            .checked_add(plan.measurement_count)?;
        if work.checked_mul(blocks.max(1))? > BUILD_WORK_LIMIT {
            return None;
        }
        let mut events = budget.zeros::<Input>(plan.random_kinds.len())?;
        let mut sweeps = budget.zeros::<SweepInput>(sweep_count)?;
        let (mut event, mut sweep, mut first) = (0, 0, 0);
        for (position, op) in plan.operations.iter().enumerate() {
            match op {
                PlanOp::Noise { choices, .. } => {
                    let count = variable_count(choices.len(), modes[position]);
                    events[event] = Input {
                        first,
                        count,
                        binary: modes[position],
                    };
                    event += 1;
                    first += count;
                }
                PlanOp::Measure(m) => {
                    for present in [
                        matches!(m.projection, Projection::Independent { .. }),
                        m.readout > 0.,
                    ] {
                        if present {
                            events[event] = Input {
                                first,
                                count: 1,
                                binary: true,
                            };
                            event += 1;
                            first += 1;
                        }
                    }
                }
                PlanOp::Feedback {
                    condition: Condition::Sweep(index),
                    ..
                } => {
                    sweeps[sweep] = SweepInput {
                        index: *index,
                        variable: first,
                    };
                    sweep += 1;
                    first += 1;
                }
                _ => {}
            }
        }
        debug_assert_eq!(event, events.len());
        debug_assert_eq!(first, variables);
        let mut constants = budget.zeros::<bool>(outputs)?;
        let mut matrix = budget.zeros::<u64>(variables.checked_mul(blocks)?)?;
        let mut columns = budget.zeros::<u64>(variables)?;
        let mut x = budget.zeros::<u64>(plan.num_qubits)?;
        let mut z = budget.zeros::<u64>(plan.num_qubits)?;
        let mut records = budget.zeros::<u64>(plan.measurement_count)?;
        for block in 0..blocks {
            let affine = adjoint(
                plan,
                block * 64,
                &modes,
                detectors,
                &observables,
                &mut columns,
                &mut x,
                &mut z,
                &mut records,
            );
            for bit in 0..(outputs - block * 64).min(64) {
                constants[block * 64 + bit] = affine >> bit & 1 != 0;
            }
            for (variable, &value) in columns.iter().enumerate() {
                matrix[variable * blocks + block] = value;
            }
        }
        let edge_count = matrix
            .iter()
            .try_fold(0usize, |n, word| n.checked_add(word.count_ones() as usize))?;
        let mut offsets = budget.zeros::<usize>(variables.checked_add(1)?)?;
        let mut edges = budget.zeros::<usize>(edge_count)?;
        let mut cursor = 0;
        for variable in 0..variables {
            offsets[variable] = cursor;
            for block in 0..blocks {
                let mut bits = matrix[variable * blocks + block];
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    edges[cursor] = block * 64 + bit;
                    cursor += 1;
                }
            }
        }
        offsets[variables] = cursor;
        let mut active_events = budget.zeros::<u64>(events.len().div_ceil(64))?;
        // Dead inputs still consume their typed RNG events but require no scatter work.
        for (event, input) in events.iter_mut().enumerate() {
            if offsets[input.first] == offsets[input.first + input.count] {
                input.count = 0;
            } else {
                active_events[event / 64] |= 1u64 << (event % 64);
            }
        }
        Some(Self {
            constants,
            offsets,
            edges,
            events,
            active_events,
            sweeps,
            detectors,
            observables,
        })
    }
    pub(super) fn sample<R: Rng>(
        &self,
        plan: &CompiledNearCliffordExecutor,
        outputs: &mut [u64],
        shots: usize,
        observable: u32,
        sweep: &[bool],
        rng: &mut R,
    ) -> NearCliffordPostselectedCounts {
        let selected = self.detectors
            + self
                .observables
                .binary_search(&observable)
                .expect("observable preflight");
        let mut result = NearCliffordPostselectedCounts {
            attempted: shots,
            ..Default::default()
        };
        for first in (0..shots).step_by(64) {
            let lanes = (shots - first).min(64);
            let all = if lanes == 64 {
                u64::MAX
            } else {
                (1u64 << lanes) - 1
            };
            for (output, constant) in outputs.iter_mut().zip(&self.constants) {
                *output = if *constant { all } else { 0 };
            }
            let apply = |outputs: &mut [u64], variable: usize, bits: u64| {
                for &output in &self.edges[self.offsets[variable]..self.offsets[variable + 1]] {
                    outputs[output] ^= bits;
                }
            };
            for s in &self.sweeps {
                if sweep.get(s.index as usize).copied().unwrap_or(false) {
                    apply(outputs, s.variable, all);
                }
            }
            for lane in 0..lanes {
                let bit = 1u64 << lane;
                let mut random = RowRandom::live(&mut *rng);
                let mut emit = |event: usize, value: u64| {
                    let input = self.events[event];
                    if input.count == 0 {
                        return;
                    }
                    if input.binary {
                        let mut bits = value;
                        while bits != 0 {
                            let column = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            apply(outputs, input.first + column, bit);
                        }
                    } else {
                        apply(outputs, input.first + value as usize - 1, bit);
                    }
                };
                if let Some(runs) = &plan.random_runs {
                    runs.visit_selected(&mut random, &plan.random_kinds, &self.active_events, emit);
                } else {
                    for (event, &kind) in plan.random_kinds.iter().enumerate() {
                        let value = random.draw(kind);
                        if value != 0 {
                            emit(event, value);
                        }
                    }
                }
            }
            let rejected = outputs[..self.detectors].iter().fold(0, |bits, v| bits | v);
            let live = all & !rejected;
            result.accepted += live.count_ones() as usize;
            result.logical_errors += (live & outputs[selected]).count_ones() as usize;
        }
        result
    }
    pub(super) fn output_count(&self) -> usize {
        self.constants.len()
    }
}

fn adjoint(
    plan: &CompiledNearCliffordExecutor,
    start: usize,
    modes: &[bool],
    detectors: usize,
    observables: &[u32],
    columns: &mut [u64],
    x: &mut [u64],
    z: &mut [u64],
    records: &mut [u64],
) -> u64 {
    let seed = |output: usize| {
        if output >= start && output - start < 64 {
            1u64 << (output - start)
        } else {
            0
        }
    };
    let transpose = |gate: BasisGate, x: &mut [u64], z: &mut [u64]| match gate {
        BasisGate::H(q) => std::mem::swap(&mut x[q], &mut z[q]),
        BasisGate::S(q) => x[q] ^= z[q],
        BasisGate::CX(a, b) => {
            x[a] ^= x[b];
            z[b] ^= z[a];
        }
        BasisGate::CZ(a, b) => {
            x[b] ^= z[a];
            x[a] ^= z[b];
        }
    };
    let dot = |p: &PackedPauli, x: &[u64], z: &[u64]| {
        let mut v = 0;
        for_support(&p.x, |q| v ^= x[q]);
        for_support(&p.z, |q| v ^= z[q]);
        v
    };
    x.fill(0);
    z.fill(0);
    records.fill(0);
    columns.fill(0);
    let mut variable = columns.len();
    let mut detector = detectors;
    let mut affine = 0;
    for (position, op) in plan.operations.iter().enumerate().rev() {
        match op {
            PlanOp::Basis(gates) => {
                for &g in gates.iter().rev() {
                    transpose(g, x, z);
                }
            }
            PlanOp::Noise { choices, .. } => {
                let count = if modes[position] {
                    usize::BITS as usize - choices.len().leading_zeros() as usize
                } else {
                    choices.len()
                };
                variable -= count;
                for bit in 0..count {
                    let choice = &choices[if modes[position] {
                        (1usize << bit) - 1
                    } else {
                        bit
                    }];
                    columns[variable + bit] = dot(&choice.pauli, x, z)
                        ^ choice.record_flips.iter().fold(0, |v, r| v ^ records[*r]);
                }
            }
            PlanOp::Feedback { condition, pauli } => {
                let v = dot(pauli, x, z);
                match condition {
                    Condition::Record(r) => records[*r] ^= v,
                    Condition::Sweep(_) => {
                        variable -= 1;
                        columns[variable] = v;
                    }
                }
            }
            PlanOp::Measure(m) => {
                let r = m.record.map_or(0, |i| {
                    let v = records[i];
                    records[i] = 0;
                    v
                });
                if m.readout > 0. {
                    variable -= 1;
                    columns[variable] = r;
                }
                if m.inverted {
                    affine ^= r;
                }
                let physical = r ^ m.reset.as_ref().map_or(0, |p| dot(p, x, z));
                let branch = physical
                    ^ if let Projection::Independent { pivot } = m.projection {
                        x[pivot]
                    } else {
                        0
                    };
                for &g in m.basis.iter().rev() {
                    transpose(g, x, z);
                }
                // physical = branch XOR anticommutes(frame, measured Pauli).
                for_support(&m.pauli.physical.z, |q| x[q] ^= physical);
                for_support(&m.pauli.physical.x, |q| z[q] ^= physical);
                match m.projection {
                    Projection::Constant(true) => affine ^= branch,
                    Projection::Constant(false) => {}
                    Projection::Independent { .. } => {
                        variable -= 1;
                        columns[variable] = branch;
                    }
                    Projection::Active { .. } => unreachable!(),
                }
            }
            PlanOp::Annotation {
                offsets,
                observable,
            } => {
                let output = if let Some(index) = observable {
                    detectors + observables.binary_search(index).unwrap()
                } else {
                    detector -= 1;
                    detector
                };
                let v = seed(output);
                for &r in offsets {
                    records[r] ^= v;
                }
            }
            PlanOp::Rotate { .. } => unreachable!(),
        }
    }
    assert_eq!(variable, 0);
    assert_eq!(detector, 0);
    affine
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    #[test]
    fn optional_model_admission_and_lazy_sharing_keep_fallback_rng() {
        let plan = CompiledNearCliffordExecutor::compile_text(
            "H 0\nDEPOLARIZE1(0.003) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\n",
        )
        .unwrap();
        assert!(LinearCountsPlan::build(&plan, 0).is_none());
        assert!(LinearCountsPlan::build(&plan, size_of::<LinearCountsPlan>()).is_none());
        assert!(LinearCountsPlan::build(&plan, PLAN_BYTE_BUDGET).is_some());
        let shared = plan.clone();
        assert!(Arc::ptr_eq(&plan.linear_counts, &shared.linear_counts));
        let mut native = plan.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(81);
        let mut b = a.clone();
        assert!(native.sample_postselected_counts(65, 0, &mut a).is_err());
        native.sample_postselected_counts(0, 7, &mut a).unwrap();
        assert!(plan.linear_counts.get().is_none());
        assert_eq!(a.next_u64(), b.next_u64());
        native.sample_postselected_counts(65, 7, &mut a).unwrap();
        assert!(shared.linear_counts.get().unwrap().is_some());
        for runs in [false, true] {
            let mut fallback = plan.clone();
            fallback.linear_counts = Arc::new(OnceLock::new());
            fallback.counts_plan_budget = 0;
            if !runs {
                fallback.random_runs = None;
            }
            let mut reference = fallback.prepare_sampler().unwrap();
            let mut candidate = plan.prepare_sampler().unwrap();
            let mut a = StdRng::seed_from_u64(812);
            let mut b = a.clone();
            for shots in [1, 64, 65, 1024, 63] {
                assert_eq!(
                    candidate
                        .sample_postselected_counts(shots, 7, &mut a)
                        .unwrap(),
                    reference
                        .sample_postselected_counts(shots, 7, &mut b)
                        .unwrap()
                );
                for _ in 0..16 {
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
            assert!(fallback.linear_counts.get().unwrap().is_none());
        }
        let coherent = CompiledNearCliffordExecutor::compile_text(
            "H 0\nT 0\nMX 0\nOBSERVABLE_INCLUDE(7) rec[-1]\n",
        )
        .unwrap();
        assert!(LinearCountsPlan::build(&coherent, PLAN_BYTE_BUDGET).is_none());
    }

    #[test]
    fn wide_annotation_models_decline_before_sampling_without_changing_results() {
        let mut plan =
            CompiledNearCliffordExecutor::compile_text("M 0\nOBSERVABLE_INCLUDE(7) rec[-1]\n")
                .unwrap();
        plan.operations
            .extend((0..50_000).map(|_| PlanOp::Annotation {
                offsets: Vec::new(),
                observable: None,
            }));
        assert!(LinearCountsPlan::build(&plan, PLAN_BYTE_BUDGET).is_none());
        let mut sampler = plan.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(11);
        let mut b = a.clone();
        assert_eq!(
            sampler.sample_postselected_counts(1, 7, &mut a).unwrap(),
            NearCliffordPostselectedCounts {
                attempted: 1,
                accepted: 1,
                logical_errors: 0
            }
        );
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn generic_noise_categories_and_deferred_record_flips_match_physical_executor() {
        let mut plan=CompiledNearCliffordExecutor::compile_text("H 0 1\nDEPOLARIZE1(0.37) 0\nMPP !X0*Y1\nCX rec[-1] 0\nM 0 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\n").unwrap();
        assert!(plan.operations.iter().any(|op| matches!(op, PlanOp::Noise { choices, .. } if choices.iter().any(|c| !c.record_flips.is_empty()))));
        let noise = plan
            .operations
            .iter_mut()
            .find_map(|op| {
                if let PlanOp::Noise { choices, .. } = op {
                    Some(choices)
                } else {
                    None
                }
            })
            .unwrap();
        // Break category3 = category1 XOR category2 deliberately. This is a valid
        // compiled classical mixture; generic one-hot columns must handle it.
        noise[1] = noise[0].clone();
        assert!(!binary_channel(noise));
        let model = LinearCountsPlan::build(&plan, PLAN_BYTE_BUDGET).unwrap();
        assert!(model.events.iter().any(|i| !i.binary && i.count != 0));
        let mut fallback = plan.clone();
        fallback.linear_counts = Arc::new(OnceLock::new());
        fallback.counts_plan_budget = 0;
        let mut a = StdRng::seed_from_u64(199);
        let mut b = a.clone();
        let mut candidate = plan.prepare_sampler().unwrap();
        let mut reference = fallback.prepare_sampler().unwrap();
        for shots in [1, 31, 64, 65, 1024] {
            assert_eq!(
                candidate
                    .sample_postselected_counts(shots, 7, &mut a)
                    .unwrap(),
                reference
                    .sample_postselected_counts(shots, 7, &mut b)
                    .unwrap()
            );
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

#[cfg(test)]
#[test]
fn original_surface_models_are_admitted_and_match_complete_record_rng() {
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    for text in [
        include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/pure_surface_d7_r7_p1e-3.stim"
        ),
        include_str!(
            "../../../benchmarks/near_clifford/application_counts/fixtures/pure_surface_d9_r9_p1e-3.stim"
        ),
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        assert!(LinearCountsPlan::build(&plan, plan.counts_plan_budget).is_some());
        let mut counts = plan.prepare_sampler().unwrap();
        let mut reference = plan.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(739);
        let mut b = a.clone();
        for shots in [1, 64, 65, 1024] {
            let rows = reference.sample(shots, &mut a).unwrap();
            let accepted = rows
                .iter()
                .filter(|r| r.detectors.iter().all(|v| !*v))
                .collect::<Vec<_>>();
            let expected = NearCliffordPostselectedCounts {
                attempted: shots,
                accepted: accepted.len(),
                logical_errors: accepted
                    .iter()
                    .filter(|r| {
                        r.observables
                            .iter()
                            .filter(|(i, _)| *i == 0)
                            .fold(false, |v, (_, b)| v ^ *b)
                    })
                    .count(),
            };
            assert_eq!(
                counts.sample_postselected_counts(shots, 0, &mut b).unwrap(),
                expected
            );
            for _ in 0..16 {
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

#[cfg(test)]
#[test]
fn dense_feedback_declines_model_before_adjoint_work_exceeds_limit() {
    let text = "H 0\nM 0\nREPEAT 51000 {\nCY rec[-1] 0\n}\nM 0\nREPEAT 5000 {\nDETECTOR rec[-2]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    assert!(LinearCountsPlan::build(&plan, plan.counts_plan_budget).is_none());
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    for seed in [739, 1739] {
        let mut physical = plan.prepare_sampler().unwrap();
        let mut native = plan.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(seed);
        let mut b = a.clone();
        let rows = physical.sample(1, &mut a).unwrap();
        let expected = NearCliffordPostselectedCounts {
            attempted: 1,
            accepted: usize::from(rows[0].detectors.iter().all(|v| !*v)),
            logical_errors: usize::from(
                rows[0].detectors.iter().all(|v| !*v)
                    && rows[0]
                        .observables
                        .iter()
                        .filter(|(i, _)| *i == 0)
                        .fold(false, |v, (_, bit)| v ^ *bit),
            ),
        };
        assert_eq!(
            native.sample_postselected_counts(1, 0, &mut b).unwrap(),
            expected
        );
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
