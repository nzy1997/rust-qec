//! Sample archive metadata and transforms; compressed I/O requires `archive-zstd`.

#[cfg(feature = "archive-zstd")]
pub mod corruption_corpus;
pub mod format;
#[doc(hidden)]
pub mod syndrome;
#[cfg(feature = "archive-zstd")]
pub mod telemetry;

mod dense;
#[cfg(feature = "archive-zstd")]
mod integrity;
mod limits;
#[cfg(feature = "archive-zstd")]
mod reader;
#[cfg(feature = "archive-zstd")]
mod writer;
#[cfg(feature = "archive-zstd")]
pub(crate) mod zstd_frame;

pub use limits::{ArchiveLimits, SampleArchiveOptions};
#[cfg(feature = "archive-zstd")]
pub use reader::{ArchiveSummary, SampleArchiveReader};
#[cfg(feature = "archive-zstd")]
pub use writer::SampleArchiveWriter;
