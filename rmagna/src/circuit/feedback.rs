//! Fixed reverse incidence of external decisions on ordered Pauli offsets.
use super::*;
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct FeedbackTemplateOptions {
    pub minimum_width: usize,
    pub max_wires: usize,
    pub max_updates: usize,
    pub max_incidences: usize,
}
impl Default for FeedbackTemplateOptions {
    fn default() -> Self {
        Self {
            minimum_width: 0,
            max_wires: 65536,
            max_updates: 1 << 20,
            max_incidences: 4 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedbackTemplateError {
    Limit {
        option: &'static str,
        required: u128,
        limit: usize,
    },
    MissingWire(usize),
    UnsupportedGate(Gate),
}
impl fmt::Display for FeedbackTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit {
                option,
                required,
                limit,
            } => write!(f, "{option}={limit} is insufficient; need {required}"),
            Self::MissingWire(w) => write!(f, "missing feedback wire remapping for {w}"),
            Self::UnsupportedGate(g) => {
                write!(f, "{} has no fixed external-decision template", g.name())
            }
        }
    }
}
impl std::error::Error for FeedbackTemplateError {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackUpdate {
    /// Stable expanded Rust operation ID. It is not a native string anchor.
    pub operation: usize,
    pub wire: usize,
    pub pauli: Basis,
    /// The record-dependent part stays in the fixed quantum template.
    pub guard: BitExpr,
    pub decisions: BTreeSet<BitAtom>,
    /// Final selection offsets include the expression constant; ordinary gates
    /// keep their constant in `guard` instead of adding it to Pauli offsets.
    pub selection_offset: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackVirtualControl {
    pub operation: usize,
    pub wire: usize,
    /// Target gate (S, H, CX, CZ, ...) controlled by the virtual wire.
    pub gate: Gate,
    pub targets: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackFault {
    pub after: String,
    pub wire: usize,
    pub pauli: Basis,
}
/// Immutable template, built without invoking controllers. Parsed register
/// versions have already been resolved to unique `(call, bit)` decision atoms.
/// Missing decisions mean zero in this *offset* query, not resolved callbacks.
/// This incidence service does not execute or approximate a quantum circuit.
#[derive(Clone, Debug)]
pub struct FeedbackTemplate {
    wire_count: usize,
    updates: Vec<FeedbackUpdate>,
    virtual_controls: Vec<FeedbackVirtualControl>,
    consumers: BTreeMap<BitAtom, Vec<usize>>,
    constant_updates: BTreeSet<usize>,
}
impl FeedbackTemplate {
    pub fn new(
        circuit: &Circuit,
        options: FeedbackTemplateOptions,
    ) -> Result<Self, FeedbackTemplateError> {
        let admit = |option, required: u128, limit| {
            if required > limit as u128 {
                Err(FeedbackTemplateError::Limit {
                    option,
                    required,
                    limit,
                })
            } else {
                Ok(())
            }
        };
        let mut result = Self {
            wire_count: circuit.num_qubits.max(options.minimum_width),
            updates: vec![],
            virtual_controls: vec![],
            consumers: BTreeMap::new(),
            constant_updates: BTreeSet::new(),
        };
        admit(
            "feedback.max_wires",
            result.wire_count as u128,
            options.max_wires,
        )?;
        let mut incidences = 0u128;
        for located in circuit.operations() {
            let (expression, gate, targets, selection) = match &located.operation {
                Operation::Unitary {
                    gate,
                    qubits,
                    control: Some(expression),
                } => (expression, Some(*gate), qubits.as_slice(), false),
                Operation::Postselect { expression } => (expression, None, &[][..], true),
                _ => continue,
            };
            let decisions = expression
                .terms
                .iter()
                .filter(|t| matches!(t, BitAtom::Decision { .. }))
                .cloned()
                .collect::<BTreeSet<_>>();
            if decisions.is_empty() {
                continue;
            }
            admit(
                "feedback.max_updates",
                result.updates.len() as u128 + 1,
                options.max_updates,
            )?;
            incidences += decisions.len() as u128;
            admit(
                "feedback.max_incidences",
                incidences,
                options.max_incidences,
            )?;
            let pauli = match gate {
                Some(Gate::X) => Some(Basis::X),
                Some(Gate::Y) => Some(Basis::Y),
                Some(Gate::Z) => Some(Basis::Z),
                _ => None,
            };
            let wire = if pauli.is_some() {
                targets[0]
            } else {
                admit(
                    "feedback.max_wires",
                    result.wire_count as u128 + 1,
                    options.max_wires,
                )?;
                let wire = result.wire_count;
                result.wire_count += 1;
                if let Some(gate) = gate {
                    if gate.coherently_controlled().is_none() {
                        return Err(FeedbackTemplateError::UnsupportedGate(gate));
                    }
                    result.virtual_controls.push(FeedbackVirtualControl {
                        operation: located.id,
                        wire,
                        gate,
                        targets: targets.to_vec(),
                    });
                }
                wire
            };
            let mut guard = BitExpr {
                constant: expression.constant,
                terms: expression
                    .terms
                    .iter()
                    .filter(|t| matches!(t, BitAtom::Record(_)))
                    .cloned()
                    .collect(),
            };
            if selection {
                guard.constant = false;
            }
            let index = result.updates.len();
            if selection && expression.constant {
                result.constant_updates.insert(index);
            }
            for atom in &decisions {
                result
                    .consumers
                    .entry(atom.clone())
                    .or_default()
                    .push(index);
            }
            result.updates.push(FeedbackUpdate {
                operation: located.id,
                wire,
                pauli: pauli.unwrap_or(Basis::X),
                guard,
                decisions,
                selection_offset: selection && expression.constant,
            });
        }
        Ok(result)
    }
    pub fn wire_count(&self) -> usize {
        self.wire_count
    }
    pub fn updates(&self) -> &[FeedbackUpdate] {
        &self.updates
    }
    pub fn virtual_controls(&self) -> &[FeedbackVirtualControl] {
        &self.virtual_controls
    }
    pub fn faults(
        &self,
        values: &BTreeMap<BitAtom, bool>,
        prefix: &str,
        wires: Option<&BTreeMap<usize, usize>>,
    ) -> Result<Vec<FeedbackFault>, FeedbackTemplateError> {
        let mut active = self.constant_updates.clone();
        for (atom, bit) in values {
            if *bit && let Some(rows) = self.consumers.get(atom) {
                for row in rows {
                    if !active.remove(row) {
                        active.insert(*row);
                    }
                }
            }
        }
        active
            .into_iter()
            .map(|row| {
                let update = &self.updates[row];
                let wire = match wires {
                    Some(map) => *map
                        .get(&update.wire)
                        .ok_or(FeedbackTemplateError::MissingWire(update.wire))?,
                    None => update.wire,
                };
                Ok(FeedbackFault {
                    after: format!("{prefix}op{}", update.operation),
                    wire,
                    pauli: update.pauli,
                })
            })
            .collect()
    }
}
