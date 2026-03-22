pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod ir;
pub mod recorder;
pub mod parser;
pub mod executor;
pub mod data_path;
pub mod sim;
pub mod coords;
pub mod sampler;
pub mod dem;
pub mod error_analyzer;
pub mod showcase;
pub mod output;
pub mod cli;
pub mod stats;
pub mod transforms;
pub mod circuit_gen;
pub mod codegen;
pub mod m2d;
pub mod explain_errors;
pub mod qp101;
