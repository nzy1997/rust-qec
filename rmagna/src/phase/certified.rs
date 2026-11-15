//! Owned provenance and resource receipts for raw path-tensor facades.
use super::*;
use crate::catalogue::{PhaseCatalogue, PhaseSelectionRoute};
use std::time::{Duration, Instant};

/// Caller data is retained verbatim and in order. Executed contraction stages
/// are appended by the library, never inferred from resource selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathCertificate<C> {
    Caller(C),
    GlobalRadicalContraction(RadicalAbsorptionStage),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertifiedPathTensor<C> {
    tensor: PhasePathTensor,
    certificates: Vec<PathCertificate<C>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathTermReport {
    StabilizerTerms(usize),
    AmplitudeTerms(usize),
}
#[derive(Clone, Debug)]
pub struct PathDecompositionReport {
    pub quotient_rank: usize,
    pub constructive_terms: usize,
    pub zero: bool,
    pub terms: PathTermReport,
    pub elapsed: Duration,
    /// Same portable admission charge as PhaseSelectionOptions. Zero bypasses
    /// selection entirely and therefore has charge zero.
    pub logical_table_and_coefficient_bytes: u128,
}
pub struct PathDecomposition<C> {
    pub tensor: CertifiedPathTensor<C>,
    pub resource: Option<PreparedPhaseResource>,
    pub report: PathDecompositionReport,
}
impl<C> CertifiedPathTensor<C> {
    pub fn new(tensor: PhasePathTensor) -> Self {
        Self {
            tensor,
            certificates: Vec::new(),
        }
    }
    pub fn with_caller_certificate(mut self, certificate: C) -> Self {
        self.certificates.push(PathCertificate::Caller(certificate));
        self
    }
    pub fn tensor(&self) -> &PhasePathTensor {
        &self.tensor
    }
    pub fn certificates(&self) -> &[PathCertificate<C>] {
        &self.certificates
    }
}
impl<C: Clone> CertifiedPathTensor<C> {
    pub fn absorb_boundaries(
        &self,
        options: RadicalAbsorptionOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PathTensorError> {
        let absorbed = self.tensor.absorb_clifford_radical(options, arithmetic)?;
        let mut certificates = self.certificates.clone();
        certificates.extend(
            absorbed
                .diagnostics()
                .stages
                .iter()
                .cloned()
                .map(PathCertificate::GlobalRadicalContraction),
        );
        Ok(Self {
            tensor: absorbed.into_tensor(),
            certificates,
        })
    }
    /// Absorption, when requested, runs before final resource limits. An explicit
    /// zero returns an empty resource before selection, including zero term,
    /// rank and table budgets. None selects Fourier; a supplied catalogue uses
    /// the established direct/product/Fourier selector without implicit data.
    pub fn decompose(
        &self,
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        absorb_first: Option<RadicalAbsorptionOptions>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PathDecomposition<C>, PathTensorError> {
        let start = Instant::now();
        let tensor = if let Some(absorption) = absorb_first {
            self.absorb_boundaries(absorption, arithmetic)?
        } else {
            self.clone()
        };
        let resource = tensor
            .tensor
            .prepare_native_resource(max_terms, options, catalogue, arithmetic)?;
        let report = if let Some(resource) = &resource {
            let native = resource
                .native_resource()
                .expect("native preparation returns a native resource");
            let receipt = native.receipt();
            PathDecompositionReport {
                quotient_rank: resource.quotient_rank(),
                constructive_terms: resource.num_terms(),
                zero: false,
                terms: if receipt.route == PhaseSelectionRoute::CertifiedCatalogue {
                    PathTermReport::StabilizerTerms(resource.num_terms())
                } else {
                    PathTermReport::AmplitudeTerms(resource.num_terms())
                },
                elapsed: start.elapsed(),
                logical_table_and_coefficient_bytes: receipt.logical_table_and_coefficient_bytes,
            }
        } else {
            PathDecompositionReport {
                quotient_rank: 0,
                constructive_terms: 0,
                zero: true,
                terms: PathTermReport::AmplitudeTerms(0),
                elapsed: start.elapsed(),
                logical_table_and_coefficient_bytes: 0,
            }
        };
        Ok(PathDecomposition {
            tensor,
            resource,
            report,
        })
    }
}
