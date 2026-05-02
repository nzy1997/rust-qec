pub mod types;
pub mod util;
pub mod interop;
pub mod flooder;
pub mod matcher;
pub mod search;
pub mod driver;

pub use driver::decoding::Matching;

#[cfg(feature = "rsinter")]
pub mod decoder;

#[cfg(test)]
pub mod test_alloc;
