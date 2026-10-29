//! Original physical noise declarations, separate from coherent compiler sites.
use crate::circuit::{Basis, BitExpr, Circuit, NoiseKind, Operation, SourceLocation};
use crate::probability::{Probability, ProbabilityError};
use num_rational::BigRational;
use std::{collections::BTreeMap, fmt};

/// Bounds on the complete returned description, including zero-mass channels.
#[derive(Clone, Copy, Debug)]
pub struct OriginalNoiseOptions {
    pub max_categories: usize,
    pub max_channels: usize,
}
impl Default for OriginalNoiseOptions {
    fn default() -> Self {
        Self {
            max_categories: 1_000_000,
            max_channels: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalNoiseError {
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: usize,
    },
    Probability(ProbabilityError),
}
impl fmt::Display for OriginalNoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                option,
                limit,
                required,
            } => write!(f, "{option} requires {required}, limit is {limit}"),
            Self::Probability(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for OriginalNoiseError {}
impl From<ProbabilityError> for OriginalNoiseError {
    fn from(error: ProbabilityError) -> Self {
        Self::Probability(error)
    }
}

/// Expanded Rust operation identity and its captured source/repeat address.
/// This integer is not a native string operation ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalNoiseAnchor {
    pub operation: usize,
    pub location: SourceLocation,
}
/// Physical labels are retained even after a reset changes coherent wire labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalNoiseAction {
    ReportFlip { record: usize },
    Pauli { factors: Vec<(usize, Basis)> },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalNoiseChannel {
    pub source: OriginalNoiseAnchor,
    pub probability: Probability,
    pub action: OriginalNoiseAction,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalNoiseCategory {
    /// Existing internal noise-site index, including gaps from implicit readout.
    pub internal_site: usize,
    pub source_operations: Vec<OriginalNoiseAnchor>,
    pub guard: Option<BitExpr>,
    pub identity_probability: Probability,
    /// Ordered nonidentity alternatives; zero and one endpoints are retained.
    pub channels: Vec<OriginalNoiseChannel>,
}

fn add_bound(
    count: &mut usize,
    extra: usize,
    option: &'static str,
    limit: usize,
) -> Result<(), OriginalNoiseError> {
    let required = count
        .checked_add(extra)
        .ok_or(OriginalNoiseError::ResourceLimit {
            option,
            limit,
            required: usize::MAX,
        })?;
    if required > limit {
        return Err(OriginalNoiseError::ResourceLimit {
            option,
            limit,
            required,
        });
    }
    *count = required;
    Ok(())
}

/// Describe declarations without quantum compilation, callbacks or random draws.
/// Every measurement retains its existing internal site. Only measurements with
/// an explicit probability appear here. Correlated alternatives share one site.
pub fn original_noise_categories(
    circuit: &Circuit,
    options: &OriginalNoiseOptions,
) -> Result<Vec<OriginalNoiseCategory>, OriginalNoiseError> {
    // Admission is a separate pass: no category/channel/source description is
    // allocated until both total budgets have passed.
    let mut categories = 0;
    let mut channels = 0;
    for op in circuit.operations() {
        let counts = match &op.operation {
            Operation::Measure { .. } if circuit.readout_noise_declared(op.id) => (1, 1),
            Operation::Noise { kind, .. } => match kind {
                NoiseKind::Correlated { alternative, .. } => (usize::from(*alternative == 0), 1),
                NoiseKind::Depolarize1 | NoiseKind::Pauli1 => (1, 3),
                NoiseKind::Depolarize2 | NoiseKind::Pauli2 => (1, 15),
                _ => (1, 1),
            },
            _ => (0, 0),
        };
        add_bound(
            &mut categories,
            counts.0,
            "max_categories",
            options.max_categories,
        )?;
        add_bound(
            &mut channels,
            counts.1,
            "max_channels",
            options.max_channels,
        )?;
    }
    let mut result = Vec::<OriginalNoiseCategory>::with_capacity(categories);
    let mut chains = BTreeMap::<usize, usize>::new();
    let mut site = 0;
    for op in circuit.operations() {
        let anchor = || OriginalNoiseAnchor {
            operation: op.id,
            location: op.location.clone(),
        };
        match &op.operation {
            Operation::Measure {
                report_flip,
                record,
                ..
            } => {
                if circuit.readout_noise_declared(op.id) {
                    result.push(OriginalNoiseCategory {
                        internal_site: site,
                        source_operations: vec![anchor()],
                        guard: None,
                        identity_probability: report_flip.complement(),
                        channels: vec![OriginalNoiseChannel {
                            source: anchor(),
                            probability: report_flip.clone(),
                            action: OriginalNoiseAction::ReportFlip { record: *record },
                        }],
                    });
                }
                site += 1;
            }
            Operation::Noise {
                kind,
                probabilities,
                qubits,
                product,
                guard,
            } => {
                if let NoiseKind::Correlated { chain, alternative } = kind {
                    if *alternative == 0 {
                        chains.insert(*chain, result.len());
                        result.push(OriginalNoiseCategory {
                            internal_site: site,
                            source_operations: Vec::new(),
                            guard: guard.clone(),
                            identity_probability: Probability::one(),
                            channels: Vec::new(),
                        });
                        site += 1;
                    }
                    let category = &mut result[chains[chain]];
                    let (probability, remaining) =
                        correlated_channel(&category.identity_probability, &probabilities[0])?;
                    category.identity_probability = remaining;
                    category.source_operations.push(anchor());
                    category.channels.push(OriginalNoiseChannel {
                        source: anchor(),
                        probability,
                        action: OriginalNoiseAction::Pauli {
                            factors: product
                                .as_ref()
                                .expect("validated correlated product")
                                .factors
                                .clone(),
                        },
                    });
                } else {
                    let alternatives = pauli_channels(kind, probabilities, qubits)?;
                    let identity_probability =
                        Probability::checked_sum(alternatives.iter().map(|(p, _)| p.clone()))?
                            .complement();
                    result.push(OriginalNoiseCategory {
                        internal_site: site,
                        source_operations: vec![anchor()],
                        guard: guard.clone(),
                        identity_probability,
                        channels: alternatives
                            .into_iter()
                            .map(|(probability, factors)| OriginalNoiseChannel {
                                source: anchor(),
                                probability,
                                action: OriginalNoiseAction::Pauli { factors },
                            })
                            .collect(),
                    });
                    site += 1;
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

/// Conditional-to-exclusive survival law, independent of location actions.
pub(crate) fn correlated_channel(
    remaining: &Probability,
    conditional: &Probability,
) -> Result<(Probability, Probability), ProbabilityError> {
    Ok((
        Probability::from_rational(remaining.value() * conditional.value())?,
        Probability::from_rational(remaining.value() * conditional.complement().value())?,
    ))
}

/// Exact ordered Pauli channel law in the labels supplied by the caller.
/// Coherent lowering supplies its live wire labels; metadata supplies logical
/// physical labels. Sharing this law never shares actions between locations.
pub(crate) type PauliChannels = Vec<(Probability, Vec<(usize, Basis)>)>;

pub(crate) fn pauli_channels(
    kind: &NoiseKind,
    probabilities: &[Probability],
    qubits: &[usize],
) -> Result<PauliChannels, ProbabilityError> {
    let alternatives = match kind {
        NoiseKind::X => vec![(probabilities[0].clone(), vec![(qubits[0], Basis::X)])],
        NoiseKind::Y => vec![(probabilities[0].clone(), vec![(qubits[0], Basis::Y)])],
        NoiseKind::Z => vec![(probabilities[0].clone(), vec![(qubits[0], Basis::Z)])],
        NoiseKind::Depolarize1 | NoiseKind::Pauli1 => {
            let mut alternatives = Vec::with_capacity(3);
            for (j, basis) in [Basis::X, Basis::Y, Basis::Z].into_iter().enumerate() {
                let probability = if matches!(kind, NoiseKind::Depolarize1) {
                    Probability::from_rational(
                        probabilities[0].value() / BigRational::from_integer(3.into()),
                    )?
                } else {
                    probabilities[j].clone()
                };
                alternatives.push((probability, vec![(qubits[0], basis)]));
            }
            alternatives
        }
        NoiseKind::Depolarize2 | NoiseKind::Pauli2 => {
            let bases = [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)];
            let mut alternatives = Vec::with_capacity(15);
            for index in 1..16 {
                let probability = if matches!(kind, NoiseKind::Depolarize2) {
                    Probability::from_rational(
                        probabilities[0].value() / BigRational::from_integer(15.into()),
                    )?
                } else {
                    probabilities[index - 1].clone()
                };
                let mut factors = Vec::with_capacity(2);
                if let Some(basis) = bases[index / 4] {
                    factors.push((qubits[0], basis));
                }
                if let Some(basis) = bases[index % 4] {
                    factors.push((qubits[1], basis));
                }
                alternatives.push((probability, factors));
            }
            alternatives
        }
        NoiseKind::Correlated { .. } => unreachable!("correlated law requires survival state"),
    };
    Ok(alternatives)
}
