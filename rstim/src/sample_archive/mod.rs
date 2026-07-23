pub mod format;
#[doc(hidden)]
pub mod syndrome;
pub mod telemetry;

mod dense;
mod integrity;
mod limits;
mod reader;
mod writer;
pub(crate) mod zstd_frame;

pub use limits::{ArchiveLimits, SampleArchiveOptions};
pub use reader::{ArchiveSummary, SampleArchiveReader};
pub use writer::SampleArchiveWriter;
