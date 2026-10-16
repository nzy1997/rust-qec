pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod circuit_gen;
pub mod cli;
pub mod codegen;
pub mod compiled;
pub mod coords;
pub mod data_path;
pub mod dem;
pub mod dem_provenance;
pub mod error_analyzer;
pub mod executor;
pub mod explain_errors;
pub mod ir;
pub mod m2d;
pub mod measurement_transform;
pub mod output;
pub mod parser;
pub mod perf;
pub mod qp101;
pub mod qp101_svg;
pub mod recorder;
pub mod result_stream;
pub mod sample_archive;
pub mod sample_trace;
pub mod sampler;
pub mod showcase;
pub mod sim;
pub mod stats;
pub mod transforms;

pub use sampler::{CompiledMeasurementSampler, CompiledMeasurementSamplerDiagnostics};

#[doc(hidden)]
pub mod rare_error_iterator;
pub mod reference_sample_tree;
