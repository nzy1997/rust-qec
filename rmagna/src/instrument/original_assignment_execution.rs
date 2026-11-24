//! Explicit per-assignment global preparation with a closed fallback whitelist.
use super::*;

/// Preparation-only limits remain separate from the immutable source/literal
/// limits. Query-policy errors never select the more generous literal route.
#[derive(Clone, Copy, Debug)]
pub struct OriginalAssignmentExecutionOptions<'a> {
    /// Source-compiler geometry only; query preparation has `network.limits`.
    pub max_structure_geometry_words: usize,
    pub declared: OriginalDeclaredTraceLimits,
    pub network: OriginalNetworkOptions<'a>,
}
impl Default for OriginalAssignmentExecutionOptions<'_> {
    fn default() -> Self {
        Self {
            max_structure_geometry_words: usize::MAX,
            declared: Default::default(),
            network: OriginalNetworkOptions {
                resource: OriginalNetworkResource::Native(Default::default()),
                ..Default::default()
            },
        }
    }
}

/// Only these failures of the actual trace-preparation call permit fallback.
/// A boundary observer returning the same error still cannot request a retry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalAssignmentFallbackReason {
    /// The requested preparation-only cap refused geometry that still fits
    /// the captured source's immutable hard limit.
    PreparationGeometry { limit: usize, required: u128 },
    /// Authoritative affine lowering cannot represent a controlled odd phase.
    /// Other opaque/nonlinear/invalid declarations are not this refusal.
    ControlledOddPhase { operation: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalAssignmentExecutionRoute {
    Literal,
    PreparedGlobal,
    PreparationFallback,
}

/// Work of the most recent call, including failed attempts. Callback/literal
/// counts observe the actual production sites on this source session, including
/// alternate dispatches. Concurrent use of clones can contribute to those
/// observations; compare at quiescent boundaries. Saturation forbids a count
/// claim. No private Native object or dictionary identity is implied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalAssignmentExecutionStatistics {
    pub preparation_attempts: u64,
    pub prepared_sources: u64,
    pub prepared_queries: u64,
    pub prepared_evaluations: u64,
    pub control_evaluations: u64,
    pub control_completions: u64,
    pub literal_compilation_attempts: u64,
    pub literal_compilations: u64,
    pub boundary_observations: u64,
    pub saturated: bool,
}
enum Payload {
    Prepared {
        assignment: OriginalAssignment,
        decisions: Vec<OriginalDecision>,
        probability: ExactScalar,
    },
    Literal(OriginalAssignmentCertificate),
}
/// An owned result. A global probability needs no literal branch; an explicit
/// literal or fallback result additionally retains the complete certificate.
pub struct OriginalAssignmentExecution {
    payload: Payload,
    route: OriginalAssignmentExecutionRoute,
    reason: Option<OriginalAssignmentFallbackReason>,
    statistics: OriginalAssignmentExecutionStatistics,
}
impl OriginalAssignmentExecution {
    pub fn assignment(&self) -> &OriginalAssignment {
        match &self.payload {
            Payload::Prepared { assignment, .. } => assignment,
            Payload::Literal(c) => c.assignment(),
        }
    }
    pub fn probability(&self) -> &ExactScalar {
        match &self.payload {
            Payload::Prepared { probability, .. } => probability,
            Payload::Literal(c) => c.probability(),
        }
    }
    pub fn decisions(&self) -> &[OriginalDecision] {
        match &self.payload {
            Payload::Prepared { decisions, .. } => decisions,
            Payload::Literal(c) => c.branch().decisions(),
        }
    }
    pub fn literal_certificate(&self) -> Option<&OriginalAssignmentCertificate> {
        match &self.payload {
            Payload::Prepared { .. } => None,
            Payload::Literal(c) => Some(c),
        }
    }
    pub fn route(&self) -> OriginalAssignmentExecutionRoute {
        self.route
    }
    pub fn fallback_reason(&self) -> Option<&OriginalAssignmentFallbackReason> {
        self.reason.as_ref()
    }
    pub fn statistics(&self) -> OriginalAssignmentExecutionStatistics {
        self.statistics
    }
}

/// One explicit policy over a captured source. Preparation is attempted afresh
/// per call, with no structure/result cache or performance-parity claim. Capture
/// and chronological rule collection precede preparation and any observer.
pub struct OriginalAssignmentExecutor<'a> {
    plan: OriginalAssignmentPlan,
    policy: OriginalAssignmentExecutionPolicy,
    options: OriginalAssignmentExecutionOptions<'a>,
    statistics: OriginalAssignmentExecutionStatistics,
    reason: Option<OriginalAssignmentFallbackReason>,
}
impl OriginalAssignmentPlan {
    /// Opt into preparation fallback without changing this plan's default
    /// literal API, input ownership token, or captured source limits. Options
    /// apply only to `PrepareOrLiteral`; literal compilation uses source limits.
    pub fn executor<'a>(
        &self,
        policy: OriginalAssignmentExecutionPolicy,
        options: OriginalAssignmentExecutionOptions<'a>,
    ) -> OriginalAssignmentExecutor<'a> {
        OriginalAssignmentExecutor {
            plan: self.clone(),
            policy,
            options,
            statistics: Default::default(),
            reason: None,
        }
    }
}
impl OriginalAssignmentExecutor<'_> {
    pub fn execution_policy(&self) -> OriginalAssignmentExecutionPolicy {
        self.policy
    }
    /// Reset at each new call; still available when that call returns an error.
    pub fn statistics(&self) -> OriginalAssignmentExecutionStatistics {
        self.statistics
    }
    pub fn last_fallback_reason(&self) -> Option<&OriginalAssignmentFallbackReason> {
        self.reason.as_ref()
    }
    pub fn compile(
        &mut self,
        input: impl Into<OriginalAssignmentInput>,
    ) -> Result<OriginalAssignmentExecution, OriginalError> {
        self.execute(
            input.into(),
            None::<
                fn(
                    &mut OriginalAssignment,
                    Option<&OriginalAssignmentFallbackReason>,
                ) -> Result<(), OriginalError>,
            >,
        )
    }
    /// Observe the completed preparation attempt before dispatching a fallback.
    /// This backend-comparison seam receives detached mutable scratch, not the
    /// retained input or lowering. Its error propagates once, even if its shape
    /// matches a whitelisted preparation error. Original rules never run after
    /// this boundary. The optional reason describes the actual prior refusal.
    pub fn compile_with_boundary(
        &mut self,
        input: impl Into<OriginalAssignmentInput>,
        observer: impl FnOnce(
            &mut OriginalAssignment,
            Option<&OriginalAssignmentFallbackReason>,
        ) -> Result<(), OriginalError>,
    ) -> Result<OriginalAssignmentExecution, OriginalError> {
        self.execute(input.into(), Some(observer))
    }
    fn execute<F>(
        &mut self,
        input: OriginalAssignmentInput,
        observer: Option<F>,
    ) -> Result<OriginalAssignmentExecution, OriginalError>
    where
        F: FnOnce(
            &mut OriginalAssignment,
            Option<&OriginalAssignmentFallbackReason>,
        ) -> Result<(), OriginalError>,
    {
        self.statistics = Default::default();
        self.reason = None;
        let before = self.plan.0.instrument.work.snapshot();
        let result = self.dispatch(input, observer);
        let after = self.plan.0.instrument.work.snapshot();
        let mut delta = [0; 4];
        self.statistics.saturated = before.1 || after.1;
        for (j, value) in delta.iter_mut().enumerate() {
            match after.0[j].checked_sub(before.0[j]) {
                Some(n) => *value = n,
                None => self.statistics.saturated = true,
            }
        }
        [
            self.statistics.control_evaluations,
            self.statistics.control_completions,
            self.statistics.literal_compilation_attempts,
            self.statistics.literal_compilations,
        ] = delta;
        result.map(|(payload, route)| OriginalAssignmentExecution {
            payload,
            route,
            reason: self.reason.clone(),
            statistics: self.statistics,
        })
    }
    fn dispatch<F>(
        &mut self,
        input: OriginalAssignmentInput,
        observer: Option<F>,
    ) -> Result<(Payload, OriginalAssignmentExecutionRoute), OriginalError>
    where
        F: FnOnce(
            &mut OriginalAssignment,
            Option<&OriginalAssignmentFallbackReason>,
        ) -> Result<(), OriginalError>,
    {
        let assignment = self
            .plan
            .capture_input(input, &mut None, &mut Default::default())?;
        let source = &self.plan.0;
        let collected = source.instrument.collect_assignment(&assignment)?;
        let prepared = if self.policy == OriginalAssignmentExecutionPolicy::PrepareOrLiteral {
            match prepare(source, self.options, &assignment, &mut self.statistics)? {
                Ok(value) => Some(value),
                Err(reason) => {
                    self.reason = Some(reason);
                    None
                }
            }
        } else {
            None
        };
        if let Some(observer) = observer {
            let mut scratch = assignment.clone();
            self.statistics.boundary_observations += 1;
            observer(&mut scratch, self.reason.as_ref())?;
        }
        if let Some(probability) = prepared {
            return Ok((
                Payload::Prepared {
                    assignment,
                    decisions: collected.decisions,
                    probability,
                },
                OriginalAssignmentExecutionRoute::PreparedGlobal,
            ));
        }
        let branch = collected.compile(&source.instrument, source.strategy, &source.arithmetic)?;
        let probability = branch.probability(&source.arithmetic)?;
        let certificate = self
            .plan
            .assignment_certificate(assignment, branch, probability);
        Ok((
            Payload::Literal(certificate),
            if self.reason.is_some() {
                OriginalAssignmentExecutionRoute::PreparationFallback
            } else {
                OriginalAssignmentExecutionRoute::Literal
            },
        ))
    }
}

fn prepare(
    source: &Snapshot,
    options: OriginalAssignmentExecutionOptions<'_>,
    assignment: &OriginalAssignment,
    statistics: &mut OriginalAssignmentExecutionStatistics,
) -> Result<Result<ExactScalar, OriginalAssignmentFallbackReason>, OriginalError> {
    let instrument = &source.instrument;
    let hard_limit = instrument.limits.expansion.max_geometry_words;
    let cap = hard_limit.min(options.max_structure_geometry_words);
    statistics.preparation_attempts += 1;
    let trace = match instrument.compile_trace_with_preparation_limit(
        source.strategy,
        options.declared,
        options.max_structure_geometry_words,
        &source.arithmetic,
    ) {
        Ok(trace) => trace,
        Err(OriginalError::ResourceLimit {
            option: "max_geometry_words",
            limit,
            required,
        }) if limit == cap && cap < hard_limit && required <= hard_limit as u128 => {
            return Ok(Err(OriginalAssignmentFallbackReason::PreparationGeometry {
                limit,
                required,
            }));
        }
        Err(OriginalError::DeclaredControl {
            operation,
            error:
                ControlRuleError::UnsupportedFeedback("controlled odd phase requires complex feedback"),
        }) => {
            return Ok(Err(OriginalAssignmentFallbackReason::ControlledOddPhase {
                operation,
            }));
        }
        Err(error) => return Err(error),
    };
    statistics.prepared_sources += 1;
    let events = source
        .instrument
        .records
        .iter()
        .map(|name| OriginalRecordParity {
            records: vec![name.clone()],
            inverted: false,
        })
        .collect::<Vec<_>>();
    let query = trace.prepare_network_probability(&events, &options.network, &source.arithmetic)?;
    statistics.prepared_queries += 1;
    let view = trace.with_assignment(
        &OriginalTraceAssignment {
            faults: assignment.faults.clone(),
            report_flips: assignment.report_flips.clone(),
        },
        &source.arithmetic,
    )?;
    // Controls consume reported values. The retained original input remains in
    // true-value coordinates; explicit XOR report flips convert the event word.
    let mut values = source.instrument.assignment_true_values(assignment)?;
    for name in &assignment.report_flips {
        let value = values
            .get_mut(name.as_str())
            .expect("collected report admission");
        *value = !*value;
    }
    let outcomes = BitVector::from_indices(
        events.len(),
        source
            .instrument
            .records
            .iter()
            .enumerate()
            .filter_map(|(j, name)| values[name.as_str()].then_some(j)),
    )?;
    statistics.prepared_evaluations += 1;
    Ok(Ok(query
        .evaluate(&view, &outcomes, &source.arithmetic)?
        .value))
}
