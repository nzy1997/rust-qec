mod decode;
mod error;
mod matching;
mod schema;

pub use decode::{DecodeOutcome, decode};
pub use error::EnvelopeDecodeError;
pub use matching::{
    EdgeKind, EnvelopeMatchingCase, EnvelopeMatchingEdge, EnvelopeMatchingError,
    EnvelopeMatchingResult, EnvelopeMatchingShot, LossEdgeMap, decode_matching,
};
pub use schema::{
    AtomLossCase, Effect, InfeasibleResult, LossEnvelope, OptimalResult, SelectedLossCandidate,
};
