//! Owned output/environment labels around the exact raw tensor facade.
use super::*;
use crate::catalogue::PhaseCatalogue;

#[derive(Debug)]
pub enum NamedPathTensorError {
    Boundary {
        physical: usize,
        output_count: usize,
        environment: usize,
    },
    JsonLimit {
        bytes: usize,
        limit: usize,
    },
    Json(serde_json::Error),
    Tensor(PathTensorError),
}
impl fmt::Display for NamedPathTensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boundary {
                physical,
                output_count,
                environment,
            } => write!(
                f,
                "{output_count} outputs and {environment} environment labels do not match {physical} physical boundary rows"
            ),
            Self::JsonLimit { bytes, limit } => {
                write!(f, "environment JSON has {bytes} bytes; limit is {limit}")
            }
            Self::Json(e) => write!(f, "invalid environment label JSON: {e}"),
            Self::Tensor(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for NamedPathTensorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            Self::Tensor(e) => Some(e),
            _ => None,
        }
    }
}
impl From<PathTensorError> for NamedPathTensorError {
    fn from(e: PathTensorError) -> Self {
        Self::Tensor(e)
    }
}
/// Labels are descriptive owned UTF-8 strings, in physical boundary order after
/// the declared outputs. Embedded NUL and non-BMP characters are preserved.
/// Paths can shrink during absorption; physical row order and labels cannot.
#[derive(Clone)]
pub struct NamedPhasePathTensor<C> {
    tensor: CertifiedPathTensor<C>,
    output_count: usize,
    environment: Vec<String>,
}
pub struct NamedPathDecomposition<C> {
    pub decomposition: PathDecomposition<C>,
    pub output_count: usize,
    pub environment: Vec<String>,
}
impl<C> NamedPhasePathTensor<C> {
    pub fn new(
        tensor: CertifiedPathTensor<C>,
        output_count: usize,
        environment: Vec<String>,
    ) -> Result<Self, NamedPathTensorError> {
        let physical = tensor.tensor().boundary().width();
        if output_count.checked_add(environment.len()) != Some(physical) {
            return Err(NamedPathTensorError::Boundary {
                physical,
                output_count,
                environment: environment.len(),
            });
        }
        Ok(Self {
            tensor,
            output_count,
            environment,
        })
    }
    /// A JSON array of strings is validated once before either tensor service
    /// can run. Invalid UTF-8 and unpaired Unicode surrogates return Json errors;
    /// Rust String has no corresponding invalid-surrogate value. The byte cap
    /// is for input JSON, not allocator/process memory, and precedes parsing.
    pub fn from_json_environment(
        tensor: CertifiedPathTensor<C>,
        output_count: usize,
        json: &[u8],
        max_json_bytes: usize,
    ) -> Result<Self, NamedPathTensorError> {
        if json.len() > max_json_bytes {
            return Err(NamedPathTensorError::JsonLimit {
                bytes: json.len(),
                limit: max_json_bytes,
            });
        }
        let environment = serde_json::from_slice(json).map_err(NamedPathTensorError::Json)?;
        Self::new(tensor, output_count, environment)
    }
    pub fn tensor(&self) -> &CertifiedPathTensor<C> {
        &self.tensor
    }
    pub fn output_count(&self) -> usize {
        self.output_count
    }
    pub fn environment(&self) -> &[String] {
        &self.environment
    }
}
impl<C: Clone> NamedPhasePathTensor<C> {
    pub fn absorb_boundaries(
        &self,
        options: RadicalAbsorptionOptions,
        a: &ScalarArithmetic,
    ) -> Result<Self, NamedPathTensorError> {
        Self::new(
            self.tensor.absorb_boundaries(options, a)?,
            self.output_count,
            self.environment.clone(),
        )
    }
    pub fn decompose(
        &self,
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        absorb_first: Option<RadicalAbsorptionOptions>,
        a: &ScalarArithmetic,
    ) -> Result<NamedPathDecomposition<C>, NamedPathTensorError> {
        let decomposition =
            self.tensor
                .decompose(max_terms, options, catalogue, absorb_first, a)?;
        // The raw service's physical boundary contract is checked again before
        // publishing its labelled result, including the zero-resource case.
        if decomposition.tensor.tensor().boundary().width()
            != self.output_count + self.environment.len()
        {
            return Err(NamedPathTensorError::Boundary {
                physical: decomposition.tensor.tensor().boundary().width(),
                output_count: self.output_count,
                environment: self.environment.len(),
            });
        }
        Ok(NamedPathDecomposition {
            decomposition,
            output_count: self.output_count,
            environment: self.environment.clone(),
        })
    }
}
