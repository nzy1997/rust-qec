//! Owned control-expression snapshots and provenance-safe circuit inspection.

use super::{BitExpr, Circuit, LocatedOperation, Operation};
use std::{collections::BTreeMap, fmt, sync::Arc};

/// One write version of a classical control register, including overwritten writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterVersion {
    pub register: usize,
    pub version: usize,
}

impl RegisterVersion {
    pub const fn new(register: usize, version: usize) -> Self {
        Self { register, version }
    }
}

/// An owned snapshot of resolved expressions for every control-register write.
///
/// Expressions contain reported-record and external-decision atoms; register
/// reads have already been resolved by the parser. Editing this inspection data
/// does not change the parsed circuit or its compiler behavior.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlAnalysis {
    expressions: BTreeMap<RegisterVersion, BitExpr>,
}

impl ControlAnalysis {
    /// Own an explicitly supplied expression map for inspection.
    pub fn from_expressions(expressions: BTreeMap<RegisterVersion, BitExpr>) -> Self {
        Self { expressions }
    }

    /// Return the exact expression, or `None` for an absent register/version.
    /// An explicitly supplied zero expression is present and never falls back
    /// to a parsed expression.
    pub fn expression(&self, target: RegisterVersion) -> Option<&BitExpr> {
        self.expressions.get(&target)
    }

    pub fn expressions(&self) -> &BTreeMap<RegisterVersion, BitExpr> {
        &self.expressions
    }

    /// Replace the complete map with owned values, preserving the old snapshot
    /// when this method is called on a clone.
    pub fn with_expressions(mut self, expressions: BTreeMap<RegisterVersion, BitExpr>) -> Self {
        self.expressions = expressions;
        self
    }
}

impl From<&Circuit> for ControlAnalysis {
    fn from(circuit: &Circuit) -> Self {
        let mut expressions = BTreeMap::new();
        for operation in circuit.operations() {
            if let Operation::Control { outputs, .. } = &operation.operation {
                for write in outputs {
                    expressions.insert(
                        RegisterVersion::new(write.register, write.version),
                        write.expression.clone(),
                    );
                }
            }
        }
        Self { expressions }
    }
}

/// A view has no parsed form after any explicit operation or analysis edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitViewError {
    NoParsedForm,
}

impl fmt::Display for CircuitViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "this CircuitView has no parsed form; compile or export an unchanged view, or parse the intended source text again",
        )
    }
}

impl std::error::Error for CircuitViewError {}

/// An owned circuit inspection view with explicit parsed provenance.
///
/// An unchanged view grants access to its validated parsed circuit. Every
/// explicit replacement invalidates that access, including replacements with
/// identical values. Edited operations and analysis remain inspectable, and the
/// original source is retained as a snapshot. They cannot become a compiler
/// input through this view.
///
/// ```
/// use rmagna::circuit::{CircuitViewError, ParseOptions, parse_circuit};
/// let text = "M 0\nCONTROL[NOT] rec[-1] -> ctrl[0]";
/// let circuit = parse_circuit(text, &ParseOptions::default())?;
/// let view = circuit.view();
/// assert_eq!(view.to_text()?, text);
/// let edited = view.clone().with_operations(view.operations().to_vec());
/// assert_eq!(edited.format_version(), Err(CircuitViewError::NoParsedForm));
/// assert_eq!(edited.source_snapshot(), text);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct CircuitView {
    original: Arc<Circuit>,
    edited_operations: Option<Vec<LocatedOperation>>,
    analysis: ControlAnalysis,
    has_parsed_form: bool,
}

impl From<Circuit> for CircuitView {
    fn from(circuit: Circuit) -> Self {
        let analysis = circuit.control_analysis();
        Self {
            original: Arc::new(circuit),
            edited_operations: None,
            analysis,
            has_parsed_form: true,
        }
    }
}

impl From<&Circuit> for CircuitView {
    fn from(circuit: &Circuit) -> Self {
        Self::from(circuit.clone())
    }
}

impl CircuitView {
    pub fn operations(&self) -> &[LocatedOperation] {
        self.edited_operations
            .as_deref()
            .unwrap_or(self.original.operations())
    }

    pub fn analysis(&self) -> &ControlAnalysis {
        &self.analysis
    }

    /// Return the original source text even after edits. This is a snapshot,
    /// not serialization of the edited operations or analysis.
    pub fn source_snapshot(&self) -> &str {
        self.original.to_text()
    }

    pub fn has_parsed_form(&self) -> bool {
        self.has_parsed_form
    }

    pub fn with_operations(mut self, operations: Vec<LocatedOperation>) -> Self {
        self.edited_operations = Some(operations);
        self.has_parsed_form = false;
        self
    }

    pub fn with_analysis(mut self, analysis: ControlAnalysis) -> Self {
        self.analysis = analysis;
        self.has_parsed_form = false;
        self
    }

    /// Obtain validated input for the existing circuit/compiler/export APIs.
    pub fn parsed_circuit(&self) -> Result<&Circuit, CircuitViewError> {
        if self.has_parsed_form {
            Ok(&self.original)
        } else {
            Err(CircuitViewError::NoParsedForm)
        }
    }

    pub fn format_version(&self) -> Result<&'static str, CircuitViewError> {
        Ok(self.parsed_circuit()?.format_version())
    }

    /// Export source only while the view still has validated parsed provenance.
    pub fn to_text(&self) -> Result<&str, CircuitViewError> {
        Ok(self.parsed_circuit()?.to_text())
    }
}

impl Circuit {
    /// Snapshot every versioned control write into owned resolved expressions.
    pub fn control_analysis(&self) -> ControlAnalysis {
        ControlAnalysis::from(self)
    }

    /// Create an owned inspection view. Use `CircuitView::from(circuit)` to
    /// consume a circuit without cloning its parsed operations/source.
    pub fn view(&self) -> CircuitView {
        CircuitView::from(self)
    }
}
