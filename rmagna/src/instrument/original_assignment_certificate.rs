//! Owned captured inputs and results, with independent JSON exports.
use super::*;
use serde_json::{Value, json};

pub(super) fn input_json(input: &OriginalAssignment) -> Value {
    json!({"true_values":input.true_values,"report_flips":input.report_flips,
        "faults":input.faults.iter().map(|f|json!({"after":f.after,"wire":f.wire,
            "pauli":match f.pauli {OriginalPauli::I=>"I",OriginalPauli::X=>"X",OriginalPauli::Y=>"Y",OriginalPauli::Z=>"Z"}})).collect::<Vec<_>>()})
}
pub(super) fn input_key_json(input: &OriginalAssignment) -> Value {
    // Arrays make the digest independent of serde_json object-order features.
    json!([
        input.true_values,
        input.report_flips,
        input
            .faults
            .iter()
            .map(|f| json!([
                f.after,
                f.wire,
                match f.pauli {
                    OriginalPauli::I => "I",
                    OriginalPauli::X => "X",
                    OriginalPauli::Y => "Y",
                    OriginalPauli::Z => "Z",
                }
            ]))
            .collect::<Vec<_>>()
    ])
}
/// The input is captured before branch callbacks or the evaluation kernel.
/// A certificate retains no callback handles and does not refer to a caller's
/// mutable assignment. The fingerprint is diagnostic; probability evaluation
/// does not use it or retain any probability cache.
pub struct OriginalAssignmentCertificate {
    assignment: OriginalAssignment,
    branch: OriginalBranch,
    probability: ExactScalar,
    input_fingerprint: String,
    source_identity: Arc<()>,
}
impl OriginalAssignmentCertificate {
    pub(super) fn new(
        assignment: OriginalAssignment,
        branch: OriginalBranch,
        probability: ExactScalar,
        input_fingerprint: String,
        source_identity: Arc<()>,
    ) -> Self {
        Self {
            assignment,
            branch,
            probability,
            input_fingerprint,
            source_identity,
        }
    }
    /// Session identity is independent of the input digest and survives after
    /// dropping the source, without retaining its controller callbacks.
    pub fn shares_source_snapshot_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
    }
    pub fn assignment(&self) -> &OriginalAssignment {
        &self.assignment
    }
    pub fn branch(&self) -> &OriginalBranch {
        &self.branch
    }
    pub fn probability(&self) -> &ExactScalar {
        &self.probability
    }
    pub fn input_fingerprint(&self) -> &str {
        &self.input_fingerprint
    }
    /// Rust snapshot schema, not the private Python branch serialization. Each
    /// call owns its strings/arrays; editing this value cannot edit a certificate.
    pub fn to_json(&self) -> Value {
        json!({"schema":"rmagna.original-assignment-certificate.v1",
            "input":input_json(&self.assignment),"input_fingerprint":self.input_fingerprint,
            "outputs":self.branch.outputs(),
            "records":self.branch.records().iter().map(|r|json!({"name":r.name,"true":r.true_value,"reported":r.reported_value})).collect::<Vec<_>>(),
            "decisions":self.branch.decisions().iter().map(|d|json!({"operation":d.operation,"version":d.version,"inputs":d.inputs.entries,"emitted_operations":d.emitted_operations})).collect::<Vec<_>>(),
            "probability":{"coefficients":self.probability.coefficients().iter().map(ToString::to_string).collect::<Vec<_>>(),"power":self.probability.power()}})
    }
}
