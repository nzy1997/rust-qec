use super::*;
use crate::circuit::{Basis, NoiseKind, PauliProduct};
use crate::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
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
        let mut noise_indices = BTreeMap::new();
        let mut next_chain = 0;
        for (index, op) in physical.operations().iter().enumerate() {
            match &op.operation {
                Operation::Noise { kind, .. } => {
                    noise_indices.insert(op.id, index);
                    if let NoiseKind::Correlated { chain, .. } = kind {
                        next_chain = next_chain.max(chain.saturating_add(1));
                    }
                }
                Operation::Measure { record, .. } if flips.contains(record) => {
                    selected.insert(op.id, 0);
                }
                _ => {}
            }
        }
        for (&anchor, gates) in &actions {
            let index = noise_indices
                .get(&anchor)
                .ok_or_else(|| error("noise", format!("unknown supplied noise anchor {anchor}")))?;
            let Operation::Noise {
                qubits, product, ..
            } = &physical.operations()[*index].operation
            else {
                unreachable!("noise indices only contain noise operations");
            };
            let allowed: BTreeSet<_> = qubits
                .iter()
                .copied()
                .chain(
                    product
                        .iter()
                        .flat_map(|p| p.factors.iter().map(|&(q, _)| q)),
                )
                .collect();
            for &(wire, _) in gates {
                if !allowed.contains(&wire) {
                    return Err(error(
                        "noise",
                        format!("supplied anchor {anchor} does not target wire {wire}"),
                    ));
                }
            }
        }

        let mut shadow = None;
        if !actions.is_empty() {
            let categories = original_noise_categories(
                physical,
                &OriginalNoiseOptions {
                    max_categories: options.runtime.compile.max_noise_sites,
                    max_channels: physical.operations().len().saturating_mul(15),
                },
            )
            .map_err(|e| error("noise", e))?;
            for category in categories {
                let mut combined = BTreeMap::<usize, u8>::new();
                let mut count = 0;
                let mut anchors = 0;
                let mut action_anchor = 0;
                for source in &category.source_operations {
                    let Some(gates) = actions.remove(&source.operation) else {
                        continue;
                    };
                    anchors += 1;
                    action_anchor = source.operation;
                    count += gates.len();
                    for (wire, basis) in gates {
                        *combined.entry(wire).or_default() ^= match basis {
                            Basis::X => 1,
                            Basis::Z => 2,
                            Basis::Y => 3,
                        };
                    }
                }
                if count == 0 {
                    continue;
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
                let first = category.source_operations[0].operation;
                if anchors == 1 && count == factors.len()
                    && let Some(alternative) = category.channels.iter().position(|channel| {
                        channel.source.operation == action_anchor
                            && matches!(&channel.action, OriginalNoiseAction::Pauli { factors: physical }
                                if physical.len() == factors.len()
                                    && physical.iter().all(|(wire, basis)| factors.get(wire) == Some(basis)))
                    })
                {
                    selected.insert(first, alternative);
                    continue;
                }

                // A private prepared circuit replaces this category by a
                // zero-mass selectable insertion. Its only supplied choice
                // has zero mass under the original law. When inactive, the
                // same guard discards it entirely, as for the original site.
                let operations = shadow.get_or_insert_with(|| physical.operations().to_vec());
                let chain = next_chain;
                next_chain = next_chain
                    .checked_add(1)
                    .ok_or_else(|| error("noise", "supplied noise chain limit"))?;
                operations[noise_indices[&first]].operation = Operation::Noise {
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
                    guard: category.guard,
                };
                for source in &category.source_operations[1..] {
                    // Keep all expanded operation IDs and record identities.
                    operations[noise_indices[&source.operation]].operation =
                        Operation::Annotation {
                            name: "TICK".into(),
                            coordinates: vec![],
                            qubits: vec![],
                        };
                }
                selected.insert(first, 0);
            }
            debug_assert!(actions.is_empty());
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
                    .map(|(site, category)| (category.source_operations()[0], site))
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
