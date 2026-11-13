//! Structural affine classification; no callback or report truth table is used.
use super::*;

#[derive(Clone, Debug)]
pub(in crate::instrument::original) struct AffineGuard {
    pub(in crate::instrument::original) names: BTreeSet<String>,
    pub(in crate::instrument::original) inverted: bool,
}
#[derive(Clone, Debug)]
pub(in crate::instrument::original) struct AffineGate {
    pub(in crate::instrument::original) kind: ControlGateKind,
    pub(in crate::instrument::original) wires: Vec<Wire>,
    pub(in crate::instrument::original) exponent: Option<u8>,
    pub(in crate::instrument::original) guard: AffineGuard,
}
#[derive(Clone, Debug)]
pub(in crate::instrument::original) struct AffineRulePlan {
    pub(in crate::instrument::original) gates: Vec<AffineGate>,
    pub(in crate::instrument::original) records: BTreeSet<String>,
}
fn unsupported(reason: &'static str) -> ControlRuleError {
    ControlRuleError::UnsupportedFeedback(reason)
}
fn guard(expression: &ControlExpression) -> Result<AffineGuard, ControlRuleError> {
    match expression.node.as_ref() {
        Node::Constant(value) if *value <= 1 => Ok(AffineGuard {
            names: BTreeSet::new(),
            inverted: *value != 0,
        }),
        Node::Record(name) => Ok(AffineGuard {
            names: BTreeSet::from([name.clone()]),
            inverted: false,
        }),
        Node::Apply(ControlOperator::Not, children) => {
            let mut result = guard(&children[0])?;
            result.inverted ^= true;
            Ok(result)
        }
        Node::Apply(ControlOperator::Xor, children) => {
            let mut a = guard(&children[0])?;
            let b = guard(&children[1])?;
            a.inverted ^= b.inverted;
            for name in b.names {
                if !a.names.remove(&name) {
                    a.names.insert(name);
                }
            }
            Ok(a)
        }
        _ => Err(unsupported(
            "only bit constants, records, NOT and XOR form affine guards",
        )),
    }
}
#[derive(Default)]
struct PhaseSum {
    constant: u8,
    terms: BTreeMap<BTreeSet<String>, u8>,
}
fn phase(expression: &ControlExpression) -> Result<PhaseSum, ControlRuleError> {
    match expression.node.as_ref() {
        Node::Constant(value) => Ok(PhaseSum {
            constant: *value,
            ..Default::default()
        }),
        Node::Apply(ControlOperator::Add, children) => {
            let mut a = phase(&children[0])?;
            let b = phase(&children[1])?;
            a.constant = (a.constant + b.constant) & 7;
            for (names, value) in b.terms {
                let coefficient = (a.terms.get(&names).copied().unwrap_or(0) + value) & 7;
                if coefficient == 0 {
                    a.terms.remove(&names);
                } else {
                    a.terms.insert(names, coefficient);
                }
            }
            Ok(a)
        }
        Node::Apply(ControlOperator::Multiply, children) => {
            let (constant, other) = match (children[0].node.as_ref(), children[1].node.as_ref()) {
                (Node::Constant(c), _) => (*c, &children[1]),
                (_, Node::Constant(c)) => (*c, &children[0]),
                _ => {
                    return Err(unsupported(
                        "phase multiplication requires an immediate constant operand",
                    ));
                }
            };
            let mut result = phase(other)?;
            result.constant = (result.constant * constant) & 7;
            for coefficient in result.terms.values_mut() {
                *coefficient = (*coefficient * constant) & 7;
            }
            result.terms.retain(|_, coefficient| *coefficient != 0);
            Ok(result)
        }
        _ => {
            let g = guard(expression)?;
            let mut result = PhaseSum {
                constant: u8::from(g.inverted),
                ..Default::default()
            };
            if !g.names.is_empty() {
                result.terms.insert(g.names, if g.inverted { 7 } else { 1 });
            }
            Ok(result)
        }
    }
}
fn multiply(a: usize, b: usize) -> Result<usize, ControlRuleError> {
    a.checked_mul(b).ok_or(ControlRuleError::Invalid(
        "affine preparation size overflow",
    ))
}
impl ControlRule {
    pub(in crate::instrument::original) fn affine_target_occurrences(
        &self,
        max_gate_targets: usize,
    ) -> Result<usize, ControlRuleError> {
        self.gates.iter().try_fold(0, |n, gate| {
            admit(
                "original.max_gate_targets",
                gate.wires.len(),
                max_gate_targets,
            )?;
            count(n, gate.wires.len())
        })
    }
    /// Conservative bound before any canonical name/phase-map cloning. Every
    /// phase term is rooted in an expression node; its axis and record strings
    /// fit the original gate's union. The factor also covers recursive maps.
    pub(in crate::instrument::original) fn affine_preparation_words(
        &self,
    ) -> Result<usize, ControlRuleError> {
        self.gates.iter().try_fold(0, |total, gate| {
            let expressions = std::iter::once(&gate.when).chain(gate.exponent.iter());
            let mut nodes = 1;
            let mut words = count(32, multiply(4, gate.wires.len())?)?;
            for expression in expressions {
                nodes = count(nodes, expression.nodes)?;
                let bytes = expression
                    .records
                    .iter()
                    .try_fold(0, |n, name| count(n, name.len()))?;
                words = count(
                    words,
                    count(bytes.div_ceil(8), multiply(16, expression.records.len())?)?,
                )?;
            }
            count(total, multiply(nodes, words)?)
        })
    }
    pub(in crate::instrument::original) fn prepare_affine(
        &self,
        max_words: usize,
    ) -> Result<AffineRulePlan, ControlRuleError> {
        admit(
            "original_trace.max_preparation_words",
            self.affine_preparation_words()?,
            max_words,
        )?;
        let mut gates = Vec::new();
        for gate in &self.gates {
            let constant = match gate.exponent.as_ref().map(|e| e.node.as_ref()) {
                Some(Node::Constant(value)) => Some(*value),
                None => gate
                    .kind
                    .is_phase()
                    .then_some(if gate.kind == ControlGateKind::S {
                        2
                    } else {
                        1
                    }),
                _ => None,
            };
            if gate.exponent.is_none() || constant.is_some() {
                gates.push(AffineGate {
                    kind: gate.kind,
                    wires: gate.wires.clone(),
                    exponent: constant,
                    guard: guard(&gate.when)?,
                });
                continue;
            }
            let g = guard(&gate.when)?;
            if !g.names.is_empty() {
                return Err(unsupported(
                    "record-dependent exponent has a variable guard",
                ));
            }
            if !g.inverted {
                continue;
            }
            let sum = phase(gate.exponent.as_ref().expect("nonconstant exponent"))?;
            if sum.constant != 0 {
                gates.push(AffineGate {
                    kind: gate.kind,
                    wires: gate.wires.clone(),
                    exponent: Some(sum.constant),
                    guard: AffineGuard {
                        names: BTreeSet::new(),
                        inverted: true,
                    },
                });
            }
            for (names, coefficient) in sum.terms {
                if coefficient != 0 {
                    gates.push(AffineGate {
                        kind: gate.kind,
                        wires: gate.wires.clone(),
                        exponent: Some(coefficient),
                        guard: AffineGuard {
                            names,
                            inverted: false,
                        },
                    });
                }
            }
        }
        for gate in &gates {
            if !gate.guard.names.is_empty() && gate.exponent.is_some_and(|c| c & 1 != 0) {
                return Err(unsupported(
                    "controlled odd phase requires complex feedback",
                ));
            }
        }
        let records = gates
            .iter()
            .flat_map(|g| g.guard.names.iter().cloned())
            .collect();
        Ok(AffineRulePlan { gates, records })
    }
}
