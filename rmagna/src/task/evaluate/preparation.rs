//! Explicit preparation of the configured reusable sampling query families.
use super::*;
use crate::instrument::{InstrumentQueryLayout, QueryPreparationOutcome};
use crate::runtime::{HistorySampler, sampling_layout_expression};
use crate::unitary::DensityLayout;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplingPreparationOptions {
    /// Planned requests, including keys shared by multiple semantic requests.
    pub max_requests: usize,
    /// Total control-input, unique guarded-category and requested-record bits.
    pub max_history_rows: usize,
    /// Sum of logical row counts across retained request layouts. This bounds
    /// prefix duplication, independently of compiled physical-matrix budgets.
    pub max_layout_rows: usize,
}
impl Default for SamplingPreparationOptions {
    fn default() -> Self {
        Self {
            max_requests: 4096,
            max_history_rows: 4096,
            max_layout_rows: 1_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FamilyPreparationCounts {
    pub planned: usize,
    pub attempted: usize,
    pub created: usize,
    pub reused: usize,
    pub cold: usize,
    /// Construction failures; deadline/preflight failures have no fake attempt.
    pub failed: usize,
}
/// An owned receipt, including a failed stage's successfully retained families.
#[derive(Clone, Debug, PartialEq)]
pub struct SamplingPreparationReport {
    /// This scope excludes cold-prefix programs and exact-enumeration layouts.
    pub scope: &'static str,
    pub execution_route: &'static str,
    pub options: SamplingPreparationOptions,
    pub result: Result<(), TaskError>,
    pub counts: FamilyPreparationCounts,
    pub by_kind: BTreeMap<&'static str, FamilyPreparationCounts>,
    pub history_rows: usize,
    pub layout_rows: usize,
    pub elapsed_seconds: f64,
}
impl SamplingPreparationReport {
    /// All planned requests completed and none was disabled by a cold policy.
    /// Trace plans can still defer phase-resource construction until evaluation.
    pub fn all_families_retained(&self) -> bool {
        self.result.is_ok() && self.counts.cold == 0
    }
}
enum Request {
    Reported(Vec<BitExpr>),
    Support,
    Acceptance,
    Error(Vec<BitExpr>),
    Target,
    Moment(usize),
}
impl Request {
    fn kind(&self) -> &'static str {
        match self {
            Self::Reported(_) => "reported_probability",
            Self::Support => "reported_support",
            Self::Acceptance => "acceptance",
            Self::Error(_) => "accepted_probability",
            Self::Target => "target_density",
            Self::Moment(_) => "pauli_moment",
        }
    }
    fn uses_final_instrument(&self) -> bool {
        !matches!(self, Self::Reported(_) | Self::Support)
    }
}
struct Plan<'a> {
    reference: &'a CompiledInstrument,
    final_instrument: CompiledInstrument,
    requests: Vec<Request>,
}
fn limit(option: &str, configured: usize, required: usize) -> TaskError {
    TaskError::new(
        format!("sampling_preparation.{option}"),
        format!("requires {required}; configured limit is {configured}"),
    )
}
struct PlanBuilder<'a> {
    options: SamplingPreparationOptions,
    report: &'a mut SamplingPreparationReport,
    requests: Vec<Request>,
}
impl PlanBuilder<'_> {
    fn push(&mut self, rows: usize, make: impl FnOnce() -> Request) -> Result<(), TaskError> {
        if self.requests.len() >= self.options.max_requests {
            return Err(limit(
                "max_requests",
                self.options.max_requests,
                self.requests.len().saturating_add(1),
            ));
        }
        if rows
            > self
                .options
                .max_layout_rows
                .saturating_sub(self.report.layout_rows)
        {
            return Err(limit(
                "max_layout_rows",
                self.options.max_layout_rows,
                self.report.layout_rows.saturating_add(rows),
            ));
        }
        let request = make();
        self.report.layout_rows += rows;
        self.report.counts.planned += 1;
        self.report
            .by_kind
            .entry(request.kind())
            .or_default()
            .planned += 1;
        self.requests.push(request);
        Ok(())
    }
}
impl TaskEvaluator {
    /// Prepare static sampling layouts without scalar evaluations, entropy,
    /// controller calls, or execution-counter increments. The caller's deadline
    /// applies between constructions and once more after the final construction.
    /// Previously retained families remain valid after a later stage failure.
    pub fn prepare_sampling_queries(
        &self,
        options: SamplingPreparationOptions,
        meter: &ExecutionMeter,
    ) -> SamplingPreparationReport {
        let start = Instant::now();
        let mut report = SamplingPreparationReport {
            scope: "configured_reusable_sampling_families",
            execution_route: self.execution_route(),
            options,
            result: Ok(()),
            counts: Default::default(),
            by_kind: BTreeMap::new(),
            history_rows: 0,
            layout_rows: 0,
            elapsed_seconds: 0.0,
        };
        report.result = self.prepare_sampling_stage(options, meter, &mut report);
        report.elapsed_seconds = start.elapsed().as_secs_f64();
        report
    }
    fn prepare_sampling_stage(
        &self,
        options: SamplingPreparationOptions,
        meter: &ExecutionMeter,
        report: &mut SamplingPreparationReport,
    ) -> Result<(), TaskError> {
        meter.check_time()?;
        let plan = self.sampling_plan(options, report)?;
        for (index, request) in plan.requests.iter().enumerate() {
            meter.check_time().map_err(|e| {
                error(
                    &format!("sampling_preparation.requests[{index}].{}", request.kind()),
                    e,
                )
            })?;
            let instrument = if request.uses_final_instrument() {
                &plan.final_instrument
            } else {
                plan.reference
            };
            let outcome = self.prepare_sampling_request(instrument, request);
            let kind = report
                .by_kind
                .get_mut(request.kind())
                .expect("planned kind");
            report.counts.attempted += 1;
            kind.attempted += 1;
            match outcome {
                Ok(QueryPreparationOutcome::Created) => {
                    report.counts.created += 1;
                    kind.created += 1;
                }
                Ok(QueryPreparationOutcome::Reused) => {
                    report.counts.reused += 1;
                    kind.reused += 1;
                }
                Ok(QueryPreparationOutcome::Cold) => {
                    report.counts.cold += 1;
                    kind.cold += 1;
                }
                Err(e) => {
                    report.counts.failed += 1;
                    kind.failed += 1;
                    return Err(error(
                        &format!("sampling_preparation.requests[{index}].{}", request.kind()),
                        e,
                    ));
                }
            }
        }
        meter.check_time()?;
        Ok(())
    }
    fn prepare_sampling_request(
        &self,
        instrument: &CompiledInstrument,
        request: &Request,
    ) -> Result<QueryPreparationOutcome, crate::instrument::InstrumentError> {
        let layout = match request {
            Request::Reported(parities) => InstrumentQueryLayout::ReportedProbability { parities },
            Request::Support => InstrumentQueryLayout::ReportedSupport,
            Request::Acceptance => InstrumentQueryLayout::Acceptance,
            Request::Error(parities) => InstrumentQueryLayout::AcceptedProbability { parities },
            Request::Target => {
                let target = self.target.as_ref().expect("planned target");
                InstrumentQueryLayout::TargetDensity {
                    outputs: &self.task.outputs,
                    target: &target.unitary,
                    target_layout: DensityLayout {
                        outputs: &target.outputs,
                        event_rows: &target.rows,
                    },
                }
            }
            Request::Moment(index) => InstrumentQueryLayout::PauliMoment {
                outputs: &self.task.outputs,
                word: &self.task.observables[*index],
            },
        };
        instrument.prepare_query(
            layout,
            self.options.runtime.sampling.max_terms,
            &self.options.scalars,
        )
    }
    fn sampling_plan<'a>(
        &'a self,
        options: SamplingPreparationOptions,
        report: &mut SamplingPreparationReport,
    ) -> Result<Plan<'a>, TaskError> {
        let (reference, groups) = match &self.execution {
            Execution::Linear { instrument, .. } => (instrument.as_ref(), Vec::new()),
            Execution::Causal(program) => {
                let geometry = program
                    .sampling_geometry(
                        &self.requested,
                        options
                            .max_requests
                            .min(self.options.runtime.max_history_queries),
                        options
                            .max_history_rows
                            .min(self.options.runtime.max_history_inputs),
                    )
                    .map_err(|e| error("sampling_preparation.history", e))?;
                (geometry.reference, geometry.groups)
            }
        };
        let mut builder = PlanBuilder {
            options,
            report,
            requests: Vec::new(),
        };
        let mut conditions = Vec::new();
        if !groups.is_empty()
            && self.options.runtime.history_sampler == HistorySampler::AffineSupport
        {
            builder.push(0, || Request::Support)?;
        }
        for group in groups {
            if self.options.runtime.history_sampler == HistorySampler::ExactMarginal {
                builder.push(conditions.len(), || Request::Reported(conditions.clone()))?;
                for parity in group {
                    conditions.push(parity);
                    builder.push(conditions.len(), || Request::Reported(conditions.clone()))?;
                }
            } else {
                conditions.extend(group);
            }
        }
        builder.report.history_rows = conditions.len();
        if matches!(self.execution, Execution::Causal(_)) {
            builder.push(conditions.len(), || Request::Reported(conditions.clone()))?;
        }
        builder.push(conditions.len(), || Request::Acceptance)?;
        if self.target.is_some() {
            builder.push(conditions.len(), || Request::Target)?;
        } else if let Some(parities) = &self.error_parities {
            let rows = conditions.len().saturating_add(parities.len());
            builder.push(rows, || {
                Request::Error(parities.iter().map(sampling_layout_expression).collect())
            })?;
        }
        for index in 0..self.task.observables.len() {
            builder.push(conditions.len(), || Request::Moment(index))?;
        }
        let conditions = conditions
            .into_iter()
            .map(|p| (p, false))
            .collect::<Vec<_>>();
        let final_instrument = reference
            .conditioned(&conditions)
            .map_err(|e| error("sampling_preparation.final_layout", e))?;
        Ok(Plan {
            reference,
            final_instrument,
            requests: builder.requests,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{ParseOptions, parse_circuit};
    use crate::gf2::BitVector;
    use crate::instrument::QueryPreparation;
    use crate::noise::SplitMix64;
    use crate::pauli::PauliWord;
    use crate::runtime::ControllerRegistry;
    use crate::unitary::CompilerStrategy;

    #[test]
    fn typed_complex_words_prepare_all_phase_keys_without_scalar_work() {
        let physical = parse_circuit("H 0\nS 0", &ParseOptions::default()).unwrap();
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for policy in [
                QueryPreparation::Cold,
                QueryPreparation::Trace,
                QueryPreparation::FixedFibers(Default::default()),
            ] {
                for phase in 0..8 {
                    let mut modules = parse_task(
                        r#"{"outputs":[0],"observables":[[1,1]]}"#,
                        Path::new("."),
                        &TaskOptions::default(),
                    )
                    .unwrap()
                    .load_modules(&physical, &ParseOptions::default())
                    .unwrap();
                    // JSON task masks denote Hermitian Y. Exercise the internal typed
                    // word without broadening that public schema to arbitrary phases.
                    modules.task.observables[0] = PauliWord::new(
                        BitVector::from_indices(1, [0]).unwrap(),
                        BitVector::from_indices(1, [0]).unwrap(),
                        phase,
                    )
                    .unwrap();
                    let mut options = EvaluationOptions::default();
                    options.runtime.compile.compiler_strategy = compiler;
                    options.runtime.compile.probability_preparation = policy;
                    options.runtime.compile.moment_preparation = policy;
                    let e = TaskEvaluator::new(
                        &physical,
                        &modules,
                        &ControllerRegistry::default(),
                        options,
                    )
                    .unwrap();
                    let meter = ExecutionMeter::new(crate::costs::ExecutionLimits {
                        max_queries: 0,
                        ..Default::default()
                    });
                    let report = e.prepare_sampling_queries(Default::default(), &meter);
                    assert_eq!(report.result, Ok(()));
                    assert_eq!(report.counts.planned, 2);
                    assert_eq!(meter.snapshot(), crate::costs::ExecutionCosts::default());
                    let before = e.prepared_moment_statistics().unwrap();
                    let value = e.sample(&mut SplitMix64::new(2)).unwrap();
                    // <+i| omega^phase XZ |+i> = omega^(phase+6).
                    assert_eq!(
                        value.raw_moments,
                        vec![
                            ScalarArithmetic::default()
                                .omega(i64::from(phase) + 6, 0)
                                .unwrap()
                        ]
                    );
                    assert_eq!(e.prepared_moment_statistics().unwrap(), before);
                }
            }
        }
    }
}
