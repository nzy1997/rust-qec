pub mod driver;
pub mod flooder;
pub mod interop;
pub mod matcher;
pub mod search;
pub mod types;
pub mod util;

pub use driver::decoding::Matching;

#[cfg(test)]
pub mod test_alloc;
