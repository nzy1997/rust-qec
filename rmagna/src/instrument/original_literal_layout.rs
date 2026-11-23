// SPDX-License-Identifier: MPL-2.0
// Adapted from itpyi/magna@56847478, native/compile/program.hpp.
//! Explicit normalization of literal IDs and anonymous measurement records.
use super::*;

/// Normalize an explicitly chosen literal sequence before constructing an
/// instrument. Original IDs are replaced by deterministic literal IDs. An empty
/// measurement record requests an inline name that avoids every supplied name.
/// Other original-instrument entry points keep their strict identity checks.
pub struct OriginalLiteralProgramLayout {
    operations: Vec<OriginalOperation>,
    truth: Vec<(String, bool)>,
}
impl OriginalLiteralProgramLayout {
    pub fn new(
        mut operations: Vec<OriginalOperation>,
        outcomes: &[i32],
        limits: OriginalLimits,
    ) -> Result<Self, OriginalError> {
        if operations.len() != outcomes.len() {
            return Err(invalid("literal operation outcome shape mismatch", ""));
        }
        admit(
            "max_operations",
            limits.max_operations,
            operations.len() as u128,
        )?;
        let mut bytes = operations
            .iter()
            .map(|op| 96 + op.id.len() as u128 + op.gate.bytes())
            .sum::<u128>();
        admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
        let mut used = operations
            .iter()
            .filter_map(|op| op.gate.record())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut truth = Vec::new();
        for (j, op) in operations.iter_mut().enumerate() {
            op.gate.validate()?;
            let id = format!("__literal_operation_{j}");
            bytes += id.len() as u128;
            admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
            op.id = id;
            let record = match &mut op.gate {
                OriginalGate::Measure { record, .. } | OriginalGate::Destructive { record, .. } => {
                    record
                }
                _ => continue,
            };
            if !(0..=1).contains(&outcomes[j]) {
                return Err(invalid(
                    "literal record outcome must be zero or one",
                    &op.id,
                ));
            }
            if record.is_empty() {
                let mut name = format!("__inline_{j}");
                while used.contains(&name) {
                    name.push('_');
                }
                bytes += 2 * name.len() as u128;
                admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
                used.insert(name.clone());
                *record = name;
            }
            truth.push((record.clone(), outcomes[j] != 0));
        }
        Ok(Self { operations, truth })
    }
    pub fn operations(&self) -> &[OriginalOperation] {
        &self.operations
    }
    pub fn truth(&self) -> &[(String, bool)] {
        &self.truth
    }
    pub fn into_parts(self) -> (Vec<OriginalOperation>, Vec<(String, bool)>) {
        (self.operations, self.truth)
    }
}
