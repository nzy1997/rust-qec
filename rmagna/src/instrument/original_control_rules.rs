//! Immutable declared control arithmetic and ordered gate emission.
use super::{OriginalGate, OriginalPauli, OriginalWire as Wire};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlRuleError {
    MissingRecord(String),
    Invalid(&'static str),
    UnsupportedFeedback(&'static str),
    ResourceLimit {
        option: &'static str,
        required: usize,
        limit: usize,
    },
}
impl fmt::Display for ControlRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRecord(name) => write!(f, "declared control record {name:?} is absent"),
            Self::UnsupportedFeedback(reason) => write!(f, "unsupported affine feedback: {reason}"),
            Self::Invalid(reason) => write!(f, "invalid declared control: {reason}"),
            Self::ResourceLimit {
                option,
                required,
                limit,
            } => write!(
                f,
                "declared control needs {option} >= {required}; configured limit is {limit}"
            ),
        }
    }
}
impl std::error::Error for ControlRuleError {}
#[derive(Clone, Copy, Debug)]
pub struct ControlRuleLimits {
    pub max_expression_nodes: usize,
    /// Constructor depth is also capped at 256 for safe destruction/equality.
    pub max_expression_depth: usize,
    pub max_record_bytes: usize,
    pub max_gates: usize,
    pub max_gate_targets: usize,
    pub max_rule_targets: usize,
    pub max_rule_expression_nodes: usize,
}
impl Default for ControlRuleLimits {
    fn default() -> Self {
        Self {
            max_expression_nodes: 4096,
            max_expression_depth: 128,
            max_record_bytes: 1 << 20,
            max_gates: 4096,
            max_gate_targets: 65536,
            max_rule_targets: 1_000_000,
            max_rule_expression_nodes: 1_000_000,
        }
    }
}
fn admit(option: &'static str, required: usize, limit: usize) -> Result<(), ControlRuleError> {
    if required > limit {
        Err(ControlRuleError::ResourceLimit {
            option,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}
fn count(a: usize, b: usize) -> Result<usize, ControlRuleError> {
    a.checked_add(b)
        .ok_or(ControlRuleError::Invalid("size overflow"))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlOperator {
    Not,
    Xor,
    And,
    Or,
    Add,
    Multiply,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Constant(u8),
    Record(String),
    Apply(ControlOperator, Vec<ControlExpression>),
}
/// Arithmetic constants/intermediates lie in Z/8Z. Record values are explicit
/// i32 values; Boolean Rust reports convert to zero or one. NOT uses zero truth;
/// XOR/AND/OR are bitwise operations, matching the declared expression contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlExpression {
    node: Arc<Node>,
    records: Arc<BTreeSet<String>>,
    nodes: usize,
    depth: usize,
}
impl ControlExpression {
    pub fn constant(value: impl Into<BigInt>) -> Self {
        let value = ((value.into() % 8i32 + 8i32) % 8i32)
            .to_u8()
            .expect("modulo eight");
        Self {
            node: Arc::new(Node::Constant(value)),
            records: Arc::new(BTreeSet::new()),
            nodes: 1,
            depth: 1,
        }
    }
    pub fn record(name: String, limits: ControlRuleLimits) -> Result<Self, ControlRuleError> {
        admit("max_record_bytes", name.len(), limits.max_record_bytes)?;
        admit("max_expression_nodes", 1, limits.max_expression_nodes)?;
        admit(
            "max_expression_depth",
            1,
            limits.max_expression_depth.min(256),
        )?;
        Ok(Self {
            node: Arc::new(Node::Record(name.clone())),
            records: Arc::new(BTreeSet::from([name])),
            nodes: 1,
            depth: 1,
        })
    }
    pub fn compose(
        operator: ControlOperator,
        operands: Vec<Self>,
        limits: ControlRuleLimits,
    ) -> Result<Self, ControlRuleError> {
        if operands.len()
            != if operator == ControlOperator::Not {
                1
            } else {
                2
            }
        {
            return Err(ControlRuleError::Invalid("expression arity"));
        }
        let nodes = operands.iter().try_fold(1, |n, e| count(n, e.nodes))?;
        let depth = count(1, operands.iter().map(|e| e.depth).max().unwrap_or(0))?;
        admit("max_expression_nodes", nodes, limits.max_expression_nodes)?;
        admit(
            "max_expression_depth",
            depth,
            limits.max_expression_depth.min(256),
        )?;
        let references: BTreeSet<_> = operands.iter().flat_map(|e| e.records.iter()).collect();
        let bytes = references
            .iter()
            .try_fold(0, |n, name| count(n, name.len()))?;
        admit("max_record_bytes", bytes, limits.max_record_bytes)?;
        let records = references.into_iter().cloned().collect();
        Ok(Self {
            node: Arc::new(Node::Apply(operator, operands)),
            records: Arc::new(records),
            nodes,
            depth,
        })
    }
    pub fn records(&self) -> &BTreeSet<String> {
        &self.records
    }
    pub fn evaluate(&self, reports: &BTreeMap<String, i32>) -> Result<i32, ControlRuleError> {
        self.evaluate_with(&|name| reports.get(name).copied())
    }
    fn evaluate_with(
        &self,
        reports: &impl Fn(&str) -> Option<i32>,
    ) -> Result<i32, ControlRuleError> {
        let mut stack = vec![(self, false)];
        let mut values = Vec::new();
        while let Some((e, ready)) = stack.pop() {
            match e.node.as_ref() {
                Node::Constant(value) => values.push(i32::from(*value)),
                Node::Record(name) => values.push(
                    reports(name).ok_or_else(|| ControlRuleError::MissingRecord(name.clone()))?,
                ),
                Node::Apply(_, operands) if !ready => {
                    stack.push((e, true));
                    for child in operands.iter().rev() {
                        stack.push((child, false));
                    }
                }
                Node::Apply(op, _) => {
                    let b = values.pop().expect("validated expression");
                    let value = if *op == ControlOperator::Not {
                        i32::from(b == 0)
                    } else {
                        let a = values.pop().expect("validated expression");
                        match op {
                            ControlOperator::Xor => a ^ b,
                            ControlOperator::And => a & b,
                            ControlOperator::Or => a | b,
                            ControlOperator::Add => {
                                (i64::from(a) + i64::from(b)).rem_euclid(8) as i32
                            }
                            ControlOperator::Multiply => {
                                (i64::from(a) * i64::from(b)).rem_euclid(8) as i32
                            }
                            ControlOperator::Not => unreachable!(),
                        }
                    };
                    values.push(value);
                }
            }
        }
        Ok(values.pop().expect("nonempty validated expression"))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlGateKind {
    I,
    X,
    Y,
    Z,
    H,
    Cx,
    Cz,
    T,
    S,
    Phase,
}
impl ControlGateKind {
    fn is_phase(self) -> bool {
        matches!(self, Self::T | Self::S | Self::Phase)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlGate {
    kind: ControlGateKind,
    wires: Vec<Wire>,
    exponent: Option<ControlExpression>,
    when: ControlExpression,
}
/// Actual emitted payload, preserving order and an explicit versus absent phase
/// exponent before conversion to the owned original gate representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlEmission {
    pub kind: ControlGateKind,
    pub wires: Vec<Wire>,
    pub exponent: Option<i32>,
}
impl ControlEmission {
    pub(super) fn into_original_gate(self) -> OriginalGate {
        if self.kind.is_phase() {
            return OriginalGate::Phase {
                wires: self.wires,
                turns: i64::from(self.exponent.unwrap_or(if self.kind == ControlGateKind::S {
                    2
                } else {
                    1
                })),
            };
        }
        match self.kind {
            // Declared I is action-free even when its metadata names a nonlive wire.
            ControlGateKind::I => OriginalGate::Identity(vec![]),
            ControlGateKind::X => OriginalGate::Pauli {
                wire: self.wires[0],
                pauli: OriginalPauli::X,
            },
            ControlGateKind::Y => OriginalGate::Pauli {
                wire: self.wires[0],
                pauli: OriginalPauli::Y,
            },
            ControlGateKind::Z => OriginalGate::Pauli {
                wire: self.wires[0],
                pauli: OriginalPauli::Z,
            },
            ControlGateKind::H => OriginalGate::H(self.wires[0]),
            ControlGateKind::Cx => OriginalGate::Cx(self.wires[0], self.wires[1]),
            ControlGateKind::Cz => OriginalGate::Cz(self.wires[0], self.wires[1]),
            _ => unreachable!("phase payload handled above"),
        }
    }
}

impl ControlGate {
    pub fn new(
        kind: ControlGateKind,
        wires: Vec<Wire>,
        exponent: Option<ControlExpression>,
        when: ControlExpression,
        limits: ControlRuleLimits,
    ) -> Result<Self, ControlRuleError> {
        admit("max_gate_targets", wires.len(), limits.max_gate_targets)?;
        if wires.iter().copied().collect::<BTreeSet<_>>().len() != wires.len() {
            return Err(ControlRuleError::Invalid("duplicate gate wire"));
        }
        let valid = match kind {
            ControlGateKind::I => true,
            ControlGateKind::Cx | ControlGateKind::Cz => wires.len() == 2,
            ControlGateKind::T | ControlGateKind::S | ControlGateKind::Phase => !wires.is_empty(),
            _ => wires.len() == 1,
        };
        if !valid {
            return Err(ControlRuleError::Invalid(
                "gate target count does not match its kind",
            ));
        }
        if exponent.is_some() && !kind.is_phase() {
            return Err(ControlRuleError::Invalid(
                "only phase gates accept exponent expressions",
            ));
        }
        Ok(Self {
            kind,
            wires,
            exponent,
            when,
        })
    }
    pub fn records(&self) -> BTreeSet<String> {
        self.when
            .records
            .iter()
            .chain(self.exponent.iter().flat_map(|e| e.records.iter()))
            .cloned()
            .collect()
    }
    pub fn evaluate(
        &self,
        reports: &BTreeMap<String, i32>,
    ) -> Result<Option<ControlEmission>, ControlRuleError> {
        self.evaluate_with(&|name| reports.get(name).copied())
    }
    fn evaluate_with(
        &self,
        reports: &impl Fn(&str) -> Option<i32>,
    ) -> Result<Option<ControlEmission>, ControlRuleError> {
        if self.when.evaluate_with(reports)? == 0 {
            return Ok(None);
        }
        let exponent = self
            .exponent
            .as_ref()
            .map(|e| e.evaluate_with(reports))
            .transpose()?;
        Ok(Some(ControlEmission {
            kind: self.kind,
            wires: self.wires.clone(),
            exponent,
        }))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlRule {
    gates: Vec<ControlGate>,
}
impl ControlRule {
    pub fn new(
        gates: Vec<ControlGate>,
        limits: ControlRuleLimits,
    ) -> Result<Self, ControlRuleError> {
        admit("max_gates", gates.len(), limits.max_gates)?;
        let targets = gates.iter().try_fold(0, |n, g| count(n, g.wires.len()))?;
        let nodes = gates.iter().try_fold(0, |n, g| {
            count(
                count(n, g.when.nodes)?,
                g.exponent.as_ref().map_or(0, |e| e.nodes),
            )
        })?;
        admit("max_rule_targets", targets, limits.max_rule_targets)?;
        admit(
            "max_rule_expression_nodes",
            nodes,
            limits.max_rule_expression_nodes,
        )?;
        Ok(Self { gates })
    }
    pub fn gates(&self) -> &[ControlGate] {
        &self.gates
    }
    pub fn records(&self) -> BTreeSet<String> {
        self.gates.iter().flat_map(ControlGate::records).collect()
    }
    pub fn evaluate(
        &self,
        reports: &BTreeMap<String, i32>,
    ) -> Result<Vec<ControlEmission>, ControlRuleError> {
        self.evaluate_with(&|name| reports.get(name).copied())
    }
    fn evaluate_with(
        &self,
        reports: &impl Fn(&str) -> Option<i32>,
    ) -> Result<Vec<ControlEmission>, ControlRuleError> {
        self.gates
            .iter()
            .filter_map(|gate| gate.evaluate_with(reports).transpose())
            .collect()
    }
}
/// Owned rule-backed original callback declaration. Its rule is authoritative;
/// an opaque callback is never invoked to infer an equivalent declaration.
#[derive(Clone, Debug)]
pub struct OriginalDeclaredControl {
    pub(super) operation: String,
    pub(super) parents: Vec<String>,
    pub(super) version: String,
    pub(super) rule: Arc<ControlRule>,
}
impl OriginalDeclaredControl {
    pub fn new(
        operation: String,
        parents: Vec<String>,
        version: String,
        rule: ControlRule,
    ) -> Result<Self, ControlRuleError> {
        let names: BTreeSet<_> = parents.iter().map(String::as_str).collect();
        if !rule
            .records()
            .iter()
            .all(|name| names.contains(name.as_str()))
        {
            return Err(ControlRuleError::Invalid(
                "rule record is not a declared parent",
            ));
        }
        if operation.is_empty() || version.is_empty() {
            return Err(ControlRuleError::Invalid(
                "control identity and version must be nonempty",
            ));
        }
        Ok(Self {
            operation,
            parents,
            version,
            rule: Arc::new(rule),
        })
    }
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn parents(&self) -> &[String] {
        &self.parents
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn rule(&self) -> &ControlRule {
        &self.rule
    }
    pub(super) fn callback(&self) -> super::OriginalControl {
        let rule = self.rule.clone();
        super::OriginalControl {
            operation: self.operation.clone(),
            parents: self.parents.clone(),
            version: self.version.clone(),
            callback: Arc::new(move |inputs| {
                rule.evaluate_with(&|name| inputs.get(name).ok().map(i32::from))
                    .map(|gates| {
                        gates
                            .into_iter()
                            .map(ControlEmission::into_original_gate)
                            .collect()
                    })
                    .map_err(|error| super::OriginalCallbackError::Failed(error.to_string()))
            }),
        }
    }
}

#[path = "original_control_affine.rs"]
mod affine;
pub(super) use affine::{AffineGate, AffineRulePlan};
