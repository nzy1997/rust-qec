//! Explicit bounded literal-prefix execution for declared complex feedback.
use super::*;
use crate::{
    noise::RandomSource,
    records::{SamplingError, bernoulli_ratio},
};

/// Fixed odd phase signature for an opaque monomial callback. Single-wire axes
/// are implicit and even unless explicitly listed in `odd_axes`. Extra axes
/// must be declared exactly once across the two collections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalPhaseFeedback {
    wires: BTreeSet<OriginalWire>,
    axes: BTreeMap<Vec<OriginalWire>, bool>,
}
impl OriginalPhaseFeedback {
    pub fn new(
        wires: &[OriginalWire],
        odd_axes: &[Vec<OriginalWire>],
        even_axes: &[Vec<OriginalWire>],
    ) -> Result<Self, OriginalError> {
        let bytes = 32 * wires.len() as u128
            + odd_axes
                .iter()
                .chain(even_axes)
                .map(|a| 64 + 16 * a.len() as u128)
                .sum::<u128>();
        admit(
            "max_feedback_family_metadata_bytes",
            OriginalBoundaryLimits::default().max_metadata_bytes,
            bytes,
        )?;
        if !distinct(wires) {
            return Err(invalid("duplicate feedback wire", ""));
        }
        let wires: BTreeSet<_> = wires.iter().copied().collect();
        let mut axes: BTreeMap<_, _> = wires.iter().map(|&w| (vec![w], false)).collect();
        let mut declared = BTreeSet::new();
        for (items, odd) in [(odd_axes, true), (even_axes, false)] {
            for item in items {
                let mut key = item.clone();
                key.sort_unstable();
                if key.is_empty()
                    || !distinct(&key)
                    || !key.iter().all(|w| wires.contains(w))
                    || !declared.insert(key.clone())
                {
                    return Err(invalid("invalid or duplicate feedback axis", ""));
                }
                axes.insert(key, odd);
            }
        }
        Ok(Self { wires, axes })
    }
    fn validate(
        &self,
        operation: &str,
        gates: &[OriginalGate],
    ) -> Result<(), OriginalComplexBoundaryError> {
        let mismatch = |reason| OriginalComplexBoundaryError::FeedbackMismatch {
            operation: operation.into(),
            reason,
        };
        let mut phases: BTreeMap<Vec<OriginalWire>, bool> = BTreeMap::new();
        for gate in gates {
            gate.validate()?;
            if !gate.wires().iter().all(|w| self.wires.contains(w)) {
                return Err(mismatch("wire outside declared family"));
            }
            match gate {
                OriginalGate::Identity(_) | OriginalGate::Pauli { .. } => {}
                OriginalGate::Phase { wires, turns } => {
                    let mut key = wires.clone();
                    key.sort_unstable();
                    if !self.axes.contains_key(&key) {
                        return Err(mismatch("undeclared parity phase axis"));
                    }
                    let odd = phases.entry(key).or_default();
                    *odd ^= turns.rem_euclid(2) != 0;
                }
                _ => {
                    return Err(mismatch(
                        "emission is outside the declared Pauli/phase family",
                    ));
                }
            }
        }
        // Pauli-frame conjugation changes phase signs, not their parity. Global
        // scalar phases cancel from every mass and signed output moment.
        if self
            .axes
            .iter()
            .any(|(axis, &odd)| phases.get(axis).copied().unwrap_or(false) != odd)
        {
            return Err(mismatch(
                "emission changes the declared odd phase signature",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OriginalFeedbackBoundaryOptions {
    pub compiler: CompilerStrategy,
    pub boundary: OriginalBoundaryLimits,
    pub max_literal_records: usize,
    /// Aggregate prefix/final branch compilations, admitted before callbacks.
    pub max_branch_compilations: usize,
    pub max_scalar_queries: usize,
    /// Conservative source/gate visits across admitted branch queries. Scalar
    /// kernel geometry and integer limbs have their existing separate limits.
    pub max_work: usize,
    pub max_entropy_words: usize,
}
impl Default for OriginalFeedbackBoundaryOptions {
    fn default() -> Self {
        Self {
            compiler: CompilerStrategy::ParityCopies,
            boundary: Default::default(),
            max_literal_records: 16,
            max_branch_compilations: 1 << 20,
            max_scalar_queries: 1 << 24,
            max_work: 1 << 28,
            max_entropy_words: 1 << 20,
        }
    }
}
#[derive(Debug)]
pub enum OriginalComplexBoundaryError {
    Original(OriginalError),
    Sampling(SamplingError),
    UndeclaredFamily {
        operation: String,
    },
    FeedbackMismatch {
        operation: String,
        reason: &'static str,
    },
}
impl From<OriginalError> for OriginalComplexBoundaryError {
    fn from(e: OriginalError) -> Self {
        Self::Original(e)
    }
}
impl From<ScalarError> for OriginalComplexBoundaryError {
    fn from(e: ScalarError) -> Self {
        Self::Original(e.into())
    }
}
impl From<Gf2Error> for OriginalComplexBoundaryError {
    fn from(e: Gf2Error) -> Self {
        Self::Original(e.into())
    }
}
impl fmt::Display for OriginalComplexBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Original(e) => e.fmt(f),
            Self::Sampling(e) => e.fmt(f),
            Self::UndeclaredFamily { operation } => write!(
                f,
                "complex feedback {operation:?} requires a declared unitary family"
            ),
            Self::FeedbackMismatch { operation, reason } => write!(
                f,
                "feedback {operation:?} does not match its declaration: {reason}"
            ),
        }
    }
}
impl std::error::Error for OriginalComplexBoundaryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalFeedbackBoundaryRoute {
    Direct,
    ComplexFeedback,
}
#[derive(Clone, Debug, Default)]
pub struct OriginalFeedbackBoundaryStatistics {
    /// Actual source callbacks, including authoritative typed rules; excludes
    /// pure frozen-emission extractors used by literal kernel compilation.
    pub controls_executed: usize,
    pub emitted_operations: usize,
    pub branch_compilations: usize,
    pub scalar_queries: usize,
    pub entropy_words: usize,
}
/// Owned scalar results. Complex tasks expose only reports needed by controls;
/// unrequested final records remain integrated out, even in postselection.
#[derive(Clone, Debug)]
pub struct OriginalFeedbackBoundaryDraw {
    pub mass: ExactScalar,
    pub acceptance: ExactScalar,
    pub moments: Vec<ExactScalar>,
    pub reports: BTreeMap<String, bool>,
    pub true_values: BTreeMap<String, bool>,
    pub statistics: OriginalFeedbackBoundaryStatistics,
}

/// Owns one immutable source and, for structurally affine rules, the original
/// direct trace. Complex execution is explicitly literal/global-prefix; it is
/// not a retry of a failed resource policy and retains no prefix-plan cache.
pub struct OriginalFeedbackBoundaryProgram {
    plan: OriginalAssignmentPlan,
    direct: Option<OriginalStaticTrace>,
    families: BTreeMap<String, OriginalPhaseFeedback>,
    required: Vec<String>,
    prefixes: BTreeMap<usize, Vec<OriginalWire>>,
    options: OriginalFeedbackBoundaryOptions,
    arithmetic: ScalarArithmetic,
}
impl OriginalFeedbackBoundaryProgram {
    pub fn new(
        source: OriginalInstrument,
        families: &BTreeMap<String, OriginalPhaseFeedback>,
        options: OriginalFeedbackBoundaryOptions,
        arithmetic: ScalarArithmetic,
    ) -> Result<Self, OriginalComplexBoundaryError> {
        let bytes = families
            .iter()
            .map(|(name, f)| {
                64 + name.len() as u128
                    + 16 * f.wires.len() as u128
                    + f.axes
                        .keys()
                        .map(|v| 32 + 16 * v.len() as u128)
                        .sum::<u128>()
            })
            .sum();
        admit(
            "max_boundary_metadata_bytes",
            options.boundary.max_metadata_bytes,
            bytes,
        )?;
        for name in families.keys() {
            if !source.controls.contains_key(name) || source.declarations.contains_key(name) {
                return Err(invalid("phase families bind opaque controls only", name).into());
            }
        }
        let mut complex = false;
        for (name, _) in &source.controls {
            if let Some(rule) = source.control_rule(name) {
                match rule
                    .prepare_affine(OriginalDeclaredTraceLimits::default().max_preparation_words)
                {
                    Ok(_) => {}
                    Err(ControlRuleError::UnsupportedFeedback(_)) => complex = true,
                    Err(error) => {
                        return Err(OriginalError::DeclaredControl {
                            operation: name.clone(),
                            error,
                        }
                        .into());
                    }
                }
            } else {
                if !families.contains_key(name) {
                    return Err(OriginalComplexBoundaryError::UndeclaredFamily {
                        operation: name.clone(),
                    });
                }
                complex = true;
            }
        }
        let direct = if complex {
            None
        } else {
            Some(source.compile_trace(options.compiler, &arithmetic)?)
        };
        if complex {
            admit(
                "max_literal_records",
                options.max_literal_records,
                source.records.len() as u128,
            )?;
            if source.records.len() >= usize::BITS as usize {
                return Err(invalid("literal record word exceeds usize", "").into());
            }
            let worst =
                (1u128 << source.records.len()).saturating_mul(source.controls.len() as u128 + 1);
            admit(
                "max_branch_compilations",
                options.max_branch_compilations,
                worst,
            )?;
            admit(
                "max_boundary_work",
                options.max_work,
                worst.saturating_mul(
                    128 + source.operations.len() as u128 + source.inputs.len() as u128,
                ),
            )?;
            // Conservative peak headers for a full prefix table, owned record
            // maps, family copies and all captured live-output lists. Exact
            // integer limbs remain subject to ScalarArithmetic's separate cap.
            let record_bytes = source
                .records
                .iter()
                .map(|r| 64 + r.len() as u128)
                .sum::<u128>();
            let live_bound = source.inputs.len() as u128
                + source
                    .operations
                    .iter()
                    .filter(|op| matches!(op.gate, OriginalGate::Prepare { .. }))
                    .count() as u128;
            let payload = bytes
                .saturating_add((1u128 << source.records.len()).saturating_mul(128 + record_bytes))
                .saturating_add(source.controls.len() as u128 * (64 + 16 * live_bound));
            admit(
                "max_boundary_metadata_bytes",
                options.boundary.max_metadata_bytes,
                payload,
            )?;
            admit(
                "max_boundary_task_words",
                options.boundary.max_task_words,
                payload.div_ceil(8),
            )?;
        }
        let prefixes = if complex {
            Self::walk(&source, families, None, true)?
        } else {
            BTreeMap::new()
        };
        let names: BTreeSet<_> = source
            .controls
            .values()
            .flat_map(|c| c.parents.iter().cloned())
            .collect();
        let required = if complex {
            source
                .records
                .iter()
                .filter(|name| names.contains(*name))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self {
            plan: OriginalAssignmentPlan::from_owned(source, options.compiler, arithmetic),
            direct,
            families: families.clone(),
            required,
            prefixes,
            options,
            arithmetic,
        })
    }
    // Reuse the literal compiler's lifetime semantics. Control families are
    // unitary, so they cannot change live wires. Faults are checked at the exact
    // after-operation point; identity faults preserve their action-free policy.
    fn walk(
        source: &OriginalInstrument,
        families: &BTreeMap<String, OriginalPhaseFeedback>,
        faults: Option<&[OriginalFault]>,
        keep_prefixes: bool,
    ) -> Result<BTreeMap<usize, Vec<OriginalWire>>, OriginalComplexBoundaryError> {
        let mut lower = BranchLowering::new(&source.inputs, source.limits);
        let mut prefixes = BTreeMap::new();
        let mut indexed: BTreeMap<&str, Vec<&OriginalFault>> = BTreeMap::new();
        for fault in faults.into_iter().flatten() {
            indexed.entry(&fault.after).or_default().push(fault);
        }
        for (end, op) in source.operations.iter().enumerate() {
            if op.gate == OriginalGate::Control {
                if keep_prefixes {
                    prefixes.insert(end, lower.live.keys().copied().collect());
                }
                if let Some(f) = families.get(&op.id) {
                    for &wire in &f.wires {
                        lower.live(wire)?;
                    }
                }
            } else {
                lower.gate(&op.gate, &OriginalOrigin::Operation(op.id.clone()), None)?;
            }
            for fault in indexed.get(op.id.as_str()).into_iter().flatten() {
                if fault.pauli != OriginalPauli::I {
                    lower.live(fault.wire)?;
                }
            }
        }
        if lower.live.len() != source.outputs.len()
            || source.outputs.iter().any(|w| !lower.live.contains_key(w))
        {
            return Err(OriginalError::OutputMismatch.into());
        }
        Ok(prefixes)
    }
    pub fn route(&self) -> OriginalFeedbackBoundaryRoute {
        if self.direct.is_some() {
            OriginalFeedbackBoundaryRoute::Direct
        } else {
            OriginalFeedbackBoundaryRoute::ComplexFeedback
        }
    }
    pub fn required_reports(&self) -> &[String] {
        &self.required
    }
    pub fn prepare_task<'a>(
        &'a self,
        task: &OriginalBoundaryTask,
    ) -> Result<OriginalFeedbackBoundarySampler<'a>, OriginalComplexBoundaryError> {
        let a = &self.arithmetic;
        let limits = self.options.boundary;
        let source = self.plan.instrument();
        let layout = OriginalRecordLayout::new(&source.records, &task.postselection, limits)?;
        let effect_bytes = task.observables.len() as u128
            * (64 + 16 * (source.outputs.len() as u128).div_ceil(64));
        admit(
            "max_boundary_metadata_bytes",
            limits.max_metadata_bytes,
            effect_bytes,
        )?;
        admit(
            "max_boundary_task_words",
            limits.max_task_words,
            effect_bytes.div_ceil(8),
        )?;
        admit(
            "max_boundary_observables",
            limits.max_observables,
            task.observables.len() as u128,
        )?;
        admit(
            "max_boundary_queries",
            limits.max_queries,
            1 + u128::from(!task.postselection.is_empty()) + task.observables.len() as u128,
        )?;
        for word in &task.observables {
            if word.width() != source.outputs.len() {
                return Err(OriginalError::DimensionMismatch.into());
            }
            if word.phase() & 3 != 2 * u8::from(word.x().dot(word.z())?) {
                return Err(
                    OriginalError::Completed(CompletedRecordError::NonHermitianPauli).into(),
                );
            }
        }
        let worst = (1u128 << source.records.len().min(127))
            .saturating_mul(source.controls.len() as u128 + 1);
        if self.direct.is_none() {
            admit(
                "max_scalar_queries",
                self.options.max_scalar_queries,
                worst.saturating_mul(1 + task.observables.len() as u128),
            )?;
            admit(
                "max_boundary_work",
                self.options.max_work,
                worst.saturating_mul(
                    128 + source.operations.len() as u128
                        + source.inputs.len() as u128
                        + task.observables.len() as u128,
                ),
            )?;
        }
        let direct = self
            .direct
            .as_ref()
            .map(|trace| {
                trace.prepare_boundary_task(
                    task,
                    &OriginalBoundaryOptions {
                        limits,
                        ..Default::default()
                    },
                    a,
                )
            })
            .transpose()?;
        Ok(OriginalFeedbackBoundarySampler {
            program: self,
            task: task.clone(),
            direct,
            compatible: layout.compatible(),
        })
    }
}
pub struct OriginalFeedbackBoundarySampler<'a> {
    program: &'a OriginalFeedbackBoundaryProgram,
    task: OriginalBoundaryTask,
    direct: Option<OriginalBoundaryService<'a>>,
    compatible: bool,
}
impl OriginalFeedbackBoundarySampler<'_> {
    pub fn sample(
        &self,
        input: &OriginalTraceAssignment,
        random: &mut impl RandomSource,
    ) -> Result<OriginalFeedbackBoundaryDraw, OriginalComplexBoundaryError> {
        let program = self.program;
        let a = &program.arithmetic;
        let source = program.plan.instrument();
        program
            .plan
            .admit_group(&input.faults, &input.report_flips)?;
        if let Some(service) = &self.direct {
            let trace = program.direct.as_ref().expect("direct trace");
            let view = trace.with_assignment(input, a)?;
            let draw = service.sample(&view, random, a)?;
            let statistics = OriginalFeedbackBoundaryStatistics {
                scalar_queries: draw.final_resources.len(),
                ..Default::default()
            };
            return Ok(OriginalFeedbackBoundaryDraw {
                mass: draw.mass,
                acceptance: draw.acceptance,
                moments: draw.moments,
                reports: draw.reports,
                true_values: BTreeMap::new(),
                statistics,
            });
        }
        OriginalFeedbackBoundaryProgram::walk(
            source,
            &program.families,
            Some(&input.faults),
            false,
        )?;
        let group = program
            .plan
            .shared_faults(input.faults.clone(), input.report_flips.clone())?;
        let mut statistics = OriginalFeedbackBoundaryStatistics::default();
        let mut reports = BTreeMap::<String, bool>::new();
        let mut frozen = BTreeMap::<String, Arc<Vec<OriginalGate>>>::new();
        let mut emitted_bytes = 0u128;
        for (end, op) in source.operations.iter().enumerate() {
            if op.gate != OriginalGate::Control {
                continue;
            }
            let control = &source.controls[&op.id];
            let prefix = self.prefix(end, &frozen)?;
            let rows = self.table(&prefix, &group, &reports, &mut statistics)?;
            for name in &control.parents {
                if reports.contains_key(name) {
                    continue;
                }
                let mut yes = ExactScalar::zero();
                let mut total = ExactScalar::zero();
                for (values, mass) in &rows {
                    if reports.iter().any(|(r, v)| values.get(r) != Some(v)) {
                        continue;
                    }
                    total = a.add(&total, mass)?;
                    if values[name] {
                        yes = a.add(&yes, mass)?;
                    }
                }
                let draw = bernoulli_ratio(
                    &yes,
                    &total,
                    random,
                    program.options.max_entropy_words - statistics.entropy_words,
                    a,
                )
                .map_err(OriginalComplexBoundaryError::Sampling)?;
                statistics.entropy_words += draw.entropy_words;
                reports.insert(name.clone(), draw.selected);
            }
            let inputs = OriginalControlInputs {
                entries: control
                    .parents
                    .iter()
                    .map(|p| (p.clone(), reports[p]))
                    .collect(),
            };
            let gates = (control.callback)(&inputs).map_err(|error| OriginalError::Callback {
                operation: op.id.clone(),
                error,
            })?;
            statistics.controls_executed += 1;
            statistics.emitted_operations = statistics
                .emitted_operations
                .checked_add(gates.len())
                .ok_or(OriginalError::DimensionMismatch)?;
            admit(
                "max_emitted_operations",
                source.limits.max_emitted_operations,
                statistics.emitted_operations as u128,
            )?;
            emitted_bytes = emitted_bytes.saturating_add(
                gates
                    .iter()
                    .map(|g| g.bytes() + op.id.len() as u128 + 64)
                    .sum::<u128>(),
            );
            admit(
                "max_boundary_emission_bytes",
                source
                    .limits
                    .max_metadata_bytes
                    .min(program.options.boundary.max_metadata_bytes),
                emitted_bytes,
            )?;
            let worst =
                (1u128 << source.records.len()).saturating_mul(source.controls.len() as u128 + 1);
            admit(
                "max_boundary_work",
                program.options.max_work,
                worst.saturating_mul(
                    128 + source.operations.len() as u128
                        + source.inputs.len() as u128
                        + self.task.observables.len() as u128
                        + statistics.emitted_operations as u128,
                ),
            )?;
            if let Some(family) = program.families.get(&op.id) {
                family.validate(&op.id, &gates)?;
            }
            frozen.insert(op.id.clone(), Arc::new(gates));
        }
        let resolved = self.prefix(source.operations.len(), &frozen)?;
        let mut mass = ExactScalar::zero();
        let mut acceptance = ExactScalar::zero();
        let mut moments = vec![ExactScalar::zero(); self.task.observables.len()];
        self.branches(
            &resolved,
            &group,
            &reports,
            &mut statistics,
            |values, branch, stats| {
                let p = branch.probability(a)?;
                stats.scalar_queries += 1;
                mass = a.add(&mass, &p)?;
                let accepted = self.compatible
                    && self.task.postselection.iter().all(|event| {
                        event
                            .parity
                            .records
                            .iter()
                            .fold(event.parity.inverted, |bit, r| bit ^ values[r])
                            == event.value
                    });
                if accepted {
                    acceptance = a.add(&acceptance, &p)?;
                    for (value, word) in moments.iter_mut().zip(&self.task.observables) {
                        *value = a.add(value, &branch.output_moment(word, a)?)?;
                        stats.scalar_queries += 1;
                    }
                }
                Ok(())
            },
        )?;
        let true_values = reports
            .iter()
            .map(|(r, &b)| (r.clone(), b ^ group.report_flips().contains(r)))
            .collect();
        Ok(OriginalFeedbackBoundaryDraw {
            mass,
            acceptance,
            moments,
            reports,
            true_values,
            statistics,
        })
    }
    fn prefix(
        &self,
        end: usize,
        frozen: &BTreeMap<String, Arc<Vec<OriginalGate>>>,
    ) -> Result<OriginalInstrument, OriginalComplexBoundaryError> {
        let source = self.program.plan.instrument();
        let operations = source.operations[..end].to_vec();
        let controls = operations
            .iter()
            .filter_map(|op| source.controls.get(&op.id))
            .map(|c| {
                let gates = Arc::clone(&frozen[&c.operation]);
                OriginalControl {
                    operation: c.operation.clone(),
                    parents: c.parents.clone(),
                    version: c.version.clone(),
                    callback: Arc::new(move |_| Ok((*gates).clone())),
                }
            })
            .collect();
        let outputs = if end == source.operations.len() {
            source.outputs.clone()
        } else {
            self.program.prefixes[&end].clone()
        };
        Ok(OriginalInstrument::new(
            source.inputs.clone(),
            outputs,
            source.initial.clone(),
            operations,
            controls,
            source.limits,
            &self.program.arithmetic,
        )?)
    }
    fn table(
        &self,
        source: &OriginalInstrument,
        group: &OriginalSharedFaults,
        reports: &BTreeMap<String, bool>,
        stats: &mut OriginalFeedbackBoundaryStatistics,
    ) -> Result<Vec<(BTreeMap<String, bool>, ExactScalar)>, OriginalComplexBoundaryError> {
        let mut result = Vec::new();
        self.branches(source, group, reports, stats, |values, branch, stats| {
            let p = branch.probability(&self.program.arithmetic)?;
            stats.scalar_queries += 1;
            result.push((values.clone(), p));
            Ok(())
        })?;
        Ok(result)
    }
    fn branches(
        &self,
        source: &OriginalInstrument,
        group: &OriginalSharedFaults,
        reports: &BTreeMap<String, bool>,
        stats: &mut OriginalFeedbackBoundaryStatistics,
        mut collect: impl FnMut(
            &BTreeMap<String, bool>,
            &OriginalBranch,
            &mut OriginalFeedbackBoundaryStatistics,
        ) -> Result<(), OriginalComplexBoundaryError>,
    ) -> Result<(), OriginalComplexBoundaryError> {
        let names: BTreeSet<_> = source.records.iter().cloned().collect();
        let ids: BTreeSet<_> = source.operations.iter().map(|op| op.id.as_str()).collect();
        let faults: Vec<_> = group
            .faults()
            .iter()
            .filter(|f| ids.contains(f.after.as_str()))
            .cloned()
            .collect();
        let flips: Vec<_> = group
            .report_flips()
            .iter()
            .filter(|r| names.contains(*r))
            .cloned()
            .collect();
        for word in 0..(1usize << source.records.len()) {
            let true_values: Vec<_> = source
                .records
                .iter()
                .enumerate()
                .map(|(j, r)| (r.clone(), word & (1 << j) != 0))
                .collect();
            let values: BTreeMap<_, _> = true_values
                .iter()
                .map(|(r, b)| (r.clone(), *b ^ flips.contains(r)))
                .collect();
            if reports
                .iter()
                .any(|(r, b)| values.get(r).is_some_and(|v| v != b))
            {
                continue;
            }
            let branch = source.compile(
                &OriginalAssignment {
                    true_values,
                    faults: faults.clone(),
                    report_flips: flips.clone(),
                },
                self.program.options.compiler,
                &self.program.arithmetic,
            )?;
            stats.branch_compilations += 1;
            collect(&values, &branch, stats)?;
        }
        Ok(())
    }
}
