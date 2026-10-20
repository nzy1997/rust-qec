use super::RuntimeError;
use std::{collections::BTreeMap, sync::Arc};

/// A deterministic, stateless decoder over declared reported observations.
/// Mutable cross-draw state and hidden random choices violate this contract.
pub trait Controller: Send + Sync {
    fn evaluate(&self, inputs: &[bool]) -> Result<Vec<bool>, String>;
}
impl<F> Controller for F
where
    F: Fn(&[bool]) -> Result<Vec<bool>, String> + Send + Sync,
{
    fn evaluate(&self, inputs: &[bool]) -> Result<Vec<bool>, String> {
        self(inputs)
    }
}
#[derive(Clone)]
pub struct ControllerBinding {
    version: String,
    controller: Arc<dyn Controller>,
}
#[derive(Clone, Default)]
pub struct ControllerRegistry {
    bindings: BTreeMap<String, ControllerBinding>,
}
impl ControllerRegistry {
    pub fn register(
        &mut self,
        name: impl Into<String>,
        version: impl Into<String>,
        controller: impl Controller + 'static,
    ) -> Result<(), RuntimeError> {
        let name = name.into();
        let version = version.into();
        if name.is_empty() || version.is_empty() || matches!(name.as_str(), "XOR" | "NOT") {
            return Err(RuntimeError::InvalidBinding { name });
        }
        if self.bindings.contains_key(&name) {
            return Err(RuntimeError::DuplicateBinding { name });
        }
        self.bindings.insert(
            name,
            ControllerBinding {
                version,
                controller: Arc::new(controller),
            },
        );
        Ok(())
    }
    pub fn names(&self) -> std::collections::BTreeSet<String> {
        self.bindings.keys().cloned().collect()
    }
    pub fn versions(&self) -> BTreeMap<String, String> {
        self.bindings
            .iter()
            .map(|(name, binding)| (name.clone(), binding.version.clone()))
            .collect()
    }
    pub(super) fn evaluate(
        &self,
        name: &str,
        call: usize,
        inputs: &[bool],
        outputs: usize,
    ) -> Result<Vec<bool>, RuntimeError> {
        let binding = self
            .bindings
            .get(name)
            .ok_or_else(|| RuntimeError::MissingBinding {
                name: name.to_owned(),
            })?;
        let values =
            binding
                .controller
                .evaluate(inputs)
                .map_err(|message| RuntimeError::Controller {
                    name: name.to_owned(),
                    call,
                    message,
                })?;
        if values.len() != outputs {
            return Err(RuntimeError::OutputShape {
                name: name.to_owned(),
                call,
                expected: outputs,
                actual: values.len(),
            });
        }
        Ok(values)
    }
}
