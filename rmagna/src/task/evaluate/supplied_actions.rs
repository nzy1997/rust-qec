use super::*;
use crate::circuit::{Basis, NoiseKind, PauliProduct};
use crate::noise::pauli_channels;
use crate::probability::Probability;
use std::collections::{BTreeMap, BTreeSet};

/// A Pauli on a physical wire targeted by an expanded noise instruction.
/// `anchor` is the parsed circuit's `LocatedOperation::id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuppliedPauliAction {
    pub anchor: usize,
    pub wire: usize,
    pub basis: Basis,
}

/// Caller-supplied noise for a physical circuit. Report flips name zero-based
/// physical measurement records and compose by XOR.
#[derive(Clone, Copy, Debug)]
pub struct SuppliedNoiseActions<'a> {
    pub paulis: &'a [SuppliedPauliAction],
    pub report_flips: &'a [usize],
    pub likelihood: &'a BigRational,
}

/// Bounds on raw lists before grouping or cancellation.
#[derive(Clone, Copy, Debug)]
pub struct SuppliedActionLimits {
    pub max_pauli_actions: usize,
    pub max_report_flips: usize,
}
impl Default for SuppliedActionLimits {
    fn default() -> Self {
        Self {
            max_pauli_actions: 1 << 20,
            max_report_flips: 1 << 20,
        }
    }
}

/// A prepared evaluator for one supplied action assignment and likelihood.
/// Each draw samples only the task's history.
pub struct SuppliedActionEvaluator {
    evaluator: TaskEvaluator,
    faults: Vec<FaultSelection>,
    likelihood: BigRational,
}
impl SuppliedActionEvaluator {
    /// Evaluate the assignment on a sampled history. Physical noise probability
    /// describes the exact active choices in the original categorical law. An
    /// active Pauli sequence outside that law has zero physical mass, even
    /// when its net operation equals a physical choice. The declared factor
    /// independently weights the contributions.
    pub fn sample(&self, random: &mut impl RandomSource) -> Result<TaskEvaluation, TaskError> {
        self.evaluator
            .sample_with_faults(&self.faults, self.likelihood.clone(), random)
    }
}

struct AnchoredPaulis {
    index: usize,
    gates: Vec<(usize, Basis)>,
}

fn matching_alternative(
    operation: &Operation,
    factors: &BTreeMap<usize, Basis>,
) -> Result<Option<usize>, TaskError> {
    let Operation::Noise {
        kind,
        probabilities,
        qubits,
        product,
        ..
    } = operation
    else {
        unreachable!("supplied actions only group noise operations");
    };
    let matches = |physical: &[(usize, Basis)]| {
        physical.len() == factors.len()
            && physical
                .iter()
                .all(|(wire, basis)| factors.get(wire) == Some(basis))
    };
    if let NoiseKind::Correlated { alternative, .. } = kind {
        let product = product.as_ref().expect("validated correlated product");
        return Ok(matches(&product.factors).then_some(*alternative));
    }
    Ok(pauli_channels(kind, probabilities, qubits)
        .map_err(|e| error("noise", e))?
        .iter()
        .position(|(_, physical)| matches(physical)))
}

impl TaskEvaluator {
    /// Prepare caller-supplied actions before drawing entropy or calling a
    /// controller. Pauli lists follow circuit order across anchors and input
    /// order within each anchor. Repeated actions compose on the same wire.
    /// Their global phase cancels from the branch's masses and moments.
    ///
    /// Each action must belong to an actual physical noise instruction and
    /// target one of its wires. Guarded actions only apply on active histories.
    /// Report flips name physical records, including records with a zero flip
    /// probability. The same exact factor rules as `sample_supplied_noise`
    /// apply. The caller's assignment and factor specify no proposal law.
    pub fn prepare_supplied_actions(
        physical: &Circuit,
        modules: &LoadedModules,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        supplied: SuppliedNoiseActions<'_>,
        limits: SuppliedActionLimits,
    ) -> Result<SuppliedActionEvaluator, TaskError> {
        let limit = options
            .scalars
            .max_coefficient_bits()
            .min(options.fields.max_coefficient_bits());
        let likelihood = validate_supplied_likelihood(supplied.likelihood, limit)
            .map_err(|e| error("noise", e))?;
        for (name, count, limit) in [
            ("paulis", supplied.paulis.len(), limits.max_pauli_actions),
            (
                "report_flips",
                supplied.report_flips.len(),
                limits.max_report_flips,
            ),
        ] {
            if count > limit {
                return Err(error(
                    "noise",
                    format!("supplied {name} require {count}, limit is {limit}"),
                ));
            }
        }

        let mut actions = BTreeMap::<usize, Vec<(usize, Basis)>>::new();
        for action in supplied.paulis {
            actions
                .entry(action.anchor)
                .or_default()
                .push((action.wire, action.basis));
        }
        let mut flips = BTreeSet::new();
        for &record in supplied.report_flips {
            if record >= physical.num_records() {
                return Err(error(
                    "noise",
                    format!("unknown supplied report record {record}"),
                ));
            }
            if !flips.insert(record) {
                flips.remove(&record);
            }
        }

        let mut selected = BTreeMap::new();
        let mut groups = BTreeMap::<usize, Vec<AnchoredPaulis>>::new();
        let mut correlated_start = 0;
        let mut next_chain = 0;
        let physical_operations = physical.operations();
        for (index, op) in physical_operations.iter().enumerate() {
            match &op.operation {
                Operation::Noise {
                    kind,
                    qubits,
                    product,
                    ..
                } => {
                    // Correlated alternatives are contiguous and share a guard.
                    let first = match kind {
                        NoiseKind::Correlated {
                            chain,
                            alternative: 0,
                        } => {
                            correlated_start = index;
                            next_chain = next_chain.max(chain.saturating_add(1));
                            index
                        }
                        NoiseKind::Correlated { .. } => correlated_start,
                        _ => index,
                    };
                    if let Some(gates) = actions.remove(&op.id) {
                        for &(wire, _) in &gates {
                            if !qubits.contains(&wire)
                                && !product.as_ref().is_some_and(|p| {
                                    // Parsed correlated products are sorted by physical wire.
                                    p.factors.binary_search_by_key(&wire, |&(q, _)| q).is_ok()
                                })
                            {
                                return Err(error(
                                    "noise",
                                    format!(
                                        "supplied anchor {} does not target wire {wire}",
                                        op.id
                                    ),
                                ));
                            }
                        }
                        groups
                            .entry(first)
                            .or_default()
                            .push(AnchoredPaulis { index, gates });
                    }
                }
                Operation::Measure { record, .. } if flips.contains(record) => {
                    selected.insert(op.id, 0);
                }
                _ => {}
            }
        }
        if let Some((&anchor, _)) = actions.first_key_value() {
            return Err(error(
                "noise",
                format!("unknown supplied noise anchor {anchor}"),
            ));
        }

        let mut shadow = None;
        for (first, group) in groups {
            let mut combined = BTreeMap::<usize, u8>::new();
            let mut count = 0;
            for action in &group {
                count += action.gates.len();
                for &(wire, basis) in &action.gates {
                    *combined.entry(wire).or_default() ^= match basis {
                        Basis::X => 1,
                        Basis::Z => 2,
                        Basis::Y => 3,
                    };
                }
            }
            let factors: BTreeMap<_, _> = combined
                .into_iter()
                .filter_map(|(wire, bits)| {
                    let basis = match bits {
                        0 => return None,
                        1 => Basis::X,
                        2 => Basis::Z,
                        _ => Basis::Y,
                    };
                    Some((wire, basis))
                })
                .collect();
            let source = &physical_operations[first];
            if group.len() == 1
                && count == factors.len()
                && let Some(alternative) =
                    matching_alternative(&physical_operations[group[0].index].operation, &factors)?
            {
                selected.insert(source.id, alternative);
                continue;
            }

            // This selectable insertion has zero physical mass when active.
            // Its original guard discards it on inactive histories.
            let Operation::Noise { kind, guard, .. } = &source.operation else {
                unreachable!("supplied groups only start at noise operations");
            };
            let operations = shadow.get_or_insert_with(|| physical_operations.to_vec());
            if let NoiseKind::Correlated { chain, .. } = kind {
                for member in operations[first + 1..].iter_mut().take_while(|op| {
                    matches!(op.operation, Operation::Noise {
                        kind: NoiseKind::Correlated { chain: other, .. }, ..
                    } if other == *chain)
                }) {
                    // Keep expanded operation and record identities stable.
                    member.operation = Operation::Annotation {
                        name: "TICK".into(),
                        coordinates: vec![],
                        qubits: vec![],
                    };
                }
            }
            let chain = next_chain;
            next_chain = next_chain
                .checked_add(1)
                .ok_or_else(|| error("noise", "supplied noise chain limit"))?;
            operations[first].operation = Operation::Noise {
                kind: NoiseKind::Correlated {
                    chain,
                    alternative: 0,
                },
                probabilities: vec![Probability::zero()],
                qubits: vec![],
                product: Some(PauliProduct {
                    factors: factors.into_iter().collect(),
                    inverted: false,
                }),
                guard: guard.clone(),
            };
            selected.insert(source.id, 0);
        }
        let shadow = shadow.map(|ops| physical.runtime_view(ops));
        let evaluator = Self::new(
            shadow.as_ref().unwrap_or(physical),
            modules,
            registry,
            options,
        )?;
        let linear_sites;
        let sites = match &evaluator.execution {
            Execution::Causal(program) => program.noise_source_sites(),
            Execution::Linear { instrument, .. } => {
                linear_sites = instrument
                    .noise_sites()
                    .iter()
                    .enumerate()
                    .filter_map(|(site, category)| {
                        let anchor = category.source_operations()[0];
                        selected.contains_key(&anchor).then_some((anchor, site))
                    })
                    .collect();
                &linear_sites
            }
        };
        let faults = selected
            .into_iter()
            .map(|(anchor, alternative)| {
                let &site = sites.get(&anchor).ok_or_else(|| {
                    error("noise", format!("missing prepared noise anchor {anchor}"))
                })?;
                Ok(FaultSelection { site, alternative })
            })
            .collect::<Result<_, TaskError>>()?;
        Ok(SuppliedActionEvaluator {
            evaluator,
            faults,
            likelihood,
        })
    }
}
