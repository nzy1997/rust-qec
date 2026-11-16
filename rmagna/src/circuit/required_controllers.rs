//! Conservative backward liveness of already expanded decision expressions.
use super::*;

impl Circuit {
    /// Task-only opt-in optimization. Parsed expressions already inline every
    /// built-in register write, so those metadata writes are not live roots.
    /// Keep stable operation/call IDs and all quantum/noise/selection operations.
    pub(crate) fn retaining_required_controllers(&self) -> Self {
        fn decisions(expression: &BitExpr, live: &mut BTreeSet<usize>) {
            live.extend(expression.terms.iter().filter_map(|atom| match atom {
                BitAtom::Decision { call, .. } => Some(*call),
                BitAtom::Record(_) => None,
            }));
        }
        let mut live = BTreeSet::new();
        for located in &self.operations {
            match &located.operation {
                Operation::Unitary {
                    control: Some(expression),
                    ..
                }
                | Operation::Noise {
                    guard: Some(expression),
                    ..
                }
                | Operation::Postselect { expression }
                | Operation::Detector { expression, .. }
                | Operation::Observable { expression, .. } => decisions(expression, &mut live),
                _ => {}
            }
        }
        // Causality guarantees every dependency belongs to an earlier call.
        // Any live output retains the whole invocation and its ordered inputs.
        for located in self.operations.iter().rev() {
            if let Operation::Control {
                call: Some(call),
                inputs,
                ..
            } = &located.operation
                && live.contains(call)
            {
                for input in inputs {
                    decisions(input, &mut live);
                }
            }
        }
        let operations = self.operations.iter().filter(|located| {
            !matches!(&located.operation, Operation::Control { call: Some(call), .. } if !live.contains(call))
        }).cloned().collect();
        self.runtime_view(operations)
    }
}
