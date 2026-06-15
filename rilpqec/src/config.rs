pub use qec_ilp_core::{BackendConfig, BackendKind};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IlpDecoderConfig {
    pub backend: BackendConfig,
}
