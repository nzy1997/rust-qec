pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod ir;
pub mod recorder;
pub mod parser;
pub mod executor;
pub mod sim;
pub mod coords;
