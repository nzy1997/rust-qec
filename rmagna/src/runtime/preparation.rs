//! Static observation geometry; decisions affect values, never record rows.
use super::*;

pub(crate) fn sampling_layout_expression(expression: &BitExpr) -> BitExpr {
    resolve::expression(expression, &BTreeMap::new(), true)
        .expect("default decision values resolve every expression")
}
pub(crate) struct SamplingGeometry<'a> {
    pub reference: &'a CompiledInstrument,
    pub groups: Vec<Vec<BitExpr>>,
}
impl CausalProgram {
    /// Mirror observation-group order without resolving a history or callbacks.
    pub(crate) fn sampling_geometry(
        &self,
        requested: &[BitExpr],
        max_groups: usize,
        max_rows: usize,
    ) -> Result<SamplingGeometry<'_>, RuntimeError> {
        let reference = self
            .reference
            .as_ref()
            .ok_or(RuntimeError::PreparationUnavailable {
                strategy: "cold_prefix",
            })?;
        let mut groups = Vec::new();
        let mut rows = 0usize;
        let mut guards = std::collections::BTreeSet::new();
        let mut append = |inputs: &[BitExpr]| -> Result<(), RuntimeError> {
            if groups.len() >= max_groups {
                return Err(RuntimeError::ResourceLimit {
                    option: "preparation.max_requests",
                    limit: max_groups,
                });
            }
            if inputs.len() > max_rows.saturating_sub(rows) {
                return Err(RuntimeError::ResourceLimit {
                    option: "preparation.max_history_rows",
                    limit: max_rows,
                });
            }
            rows += inputs.len();
            groups.push(inputs.iter().map(sampling_layout_expression).collect());
            Ok(())
        };
        for located in self.circuit.operations() {
            match &located.operation {
                Operation::Control {
                    call: Some(_),
                    inputs,
                    ..
                } => append(inputs)?,
                Operation::Noise {
                    guard: Some(guard), ..
                } => {
                    let site =
                        *self
                            .source_sites
                            .get(&located.id)
                            .ok_or(RuntimeError::InternalSite {
                                operation: located.id,
                            })?;
                    if guards.insert(site) {
                        append(std::slice::from_ref(guard))?;
                    }
                }
                _ => {}
            }
        }
        if !requested.is_empty() {
            append(requested)?;
        }
        Ok(SamplingGeometry { reference, groups })
    }
}
