//! Explicit optional acceleration of an already selected native resource.
use super::PreparedPhaseResource;
use crate::catalogue::{CatalogueBoundaryOptions, CatalogueBoundaryUsage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeTransportMode {
    /// Keep the existing exact selected-resource evaluation path.
    #[default]
    Off,
    /// Prepare bounded shared geometry; an admission stop skips acceleration.
    Prepared,
}
impl NativeTransportMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Prepared => "prepared",
        }
    }
}
/// Bounds apply independently of decomposition selection and arithmetic limits.
/// Off is the default, including for existing explicit native callers.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeTransportOptions {
    pub mode: NativeTransportMode,
    pub limits: CatalogueBoundaryOptions,
}

/// Actual preparation outcome. Absence means no transport was constructed or
/// budget-skipped; selection alone does not produce a preparation receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTransportReceipt {
    Prepared(CatalogueBoundaryUsage),
    BudgetSkipped {
        option: &'static str,
        limit: u128,
        required: u128,
    },
}

/// Counts actual retained preparations, separate from decomposition routes and
/// per-query Gaussian replay/recomputation statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeTransportCounts {
    pub prepared: usize,
    pub budget_skipped: usize,
}
impl NativeTransportCounts {
    pub fn record(&mut self, resource: &PreparedPhaseResource) {
        match resource.native_transport_receipt() {
            Some(NativeTransportReceipt::Prepared(_)) => self.prepared += 1,
            Some(NativeTransportReceipt::BudgetSkipped { .. }) => self.budget_skipped += 1,
            None => {}
        }
    }
}
