//! Exact unconditional task response over a bounded physical noise sector.
use super::*;
use crate::enumeration::{EnumerationOptions, enumerate_noise};
use crate::instrument::FaultSelection;
use crate::probability::Probability;
use crate::runtime::{HistoryEnumerationOptions, RuntimeError};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub struct TaskEnumerationOptions {
    pub noise: EnumerationOptions,
    /// Global complete-history, query and replay budgets across all patterns.
    /// The independent-bit limit remains local to each unresolved group.
    pub histories: HistoryEnumerationOptions,
    pub include_patterns: bool,
    /// Total scalar response fields across retained patterns, excluding choices
    /// already bounded by noise.max_fault_entries. Checked before traversal.
    pub max_retained_response_values: usize,
}
impl Default for TaskEnumerationOptions {
    fn default() -> Self {
        Self {
            noise: EnumerationOptions::default(),
            histories: HistoryEnumerationOptions::default(),
            include_patterns: false,
            max_retained_response_values: 1_000_000,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskSectorMasses {
    pub accepted: ExactField,
    pub error: ExactField,
    pub target_success: Option<ExactField>,
    pub moments: Vec<ExactField>,
}
impl TaskSectorMasses {
    fn zero(evaluator: &TaskEvaluator) -> Self {
        Self {
            accepted: ExactField::zero(),
            error: ExactField::zero(),
            target_success: evaluator.target.as_ref().map(|_| ExactField::zero()),
            moments: vec![ExactField::zero(); evaluator.task.observables.len()],
        }
    }
    fn weighted(
        row: &TaskEvaluation,
        probability: &BigRational,
        f: &FieldArithmetic,
    ) -> Result<Self, TaskError> {
        let scale = |v: &ExactField| {
            f.scale(v, probability)
                .map_err(|e| error("enumeration.response", e))
        };
        Ok(Self {
            accepted: scale(&row.accepted)?,
            error: scale(&row.error)?,
            target_success: row.target_success.as_ref().map(scale).transpose()?,
            moments: row.moments.iter().map(scale).collect::<Result<_, _>>()?,
        })
    }
    fn add(&mut self, other: &Self, f: &FieldArithmetic) -> Result<(), TaskError> {
        let add = |x: &ExactField, y: &ExactField| {
            f.add(x, y).map_err(|e| error("enumeration.response", e))
        };
        self.accepted = add(&self.accepted, &other.accepted)?;
        self.error = add(&self.error, &other.error)?;
        if let (Some(x), Some(y)) = (&mut self.target_success, &other.target_success) {
            *x = add(x, y)?;
        }
        for (x, y) in self.moments.iter_mut().zip(&other.moments) {
            *x = add(x, y)?;
        }
        Ok(())
    }
}
/// One original categorical assignment, with quantum histories integrated.
/// Response masses exclude this pattern's physical noise probability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskPatternResponse {
    pub choices: Vec<FaultSelection>,
    pub probability: Probability,
    pub histories: usize,
    pub response: TaskSectorMasses,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskWeightSector {
    pub weight: usize,
    pub probability: Probability,
    pub patterns: usize,
    pub histories: usize,
    pub masses: TaskSectorMasses,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactInterval {
    pub lower: ExactField,
    pub upper: ExactField,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskEnumeration {
    pub masses: TaskSectorMasses,
    /// Captured unnormalized projected-target norm; masses already divide by it.
    pub target_normalization: Option<ExactScalar>,
    pub by_weight: Vec<TaskWeightSector>,
    pub covered_probability: Probability,
    pub omitted_probability: Probability,
    pub patterns: usize,
    pub pattern_responses: Option<Vec<TaskPatternResponse>>,
    pub histories: usize,
    pub marginal_queries: usize,
    pub replay_attempts: usize,
    /// All actual post-reference constructions, including interrupted prefixes.
    pub network_compilations: usize,
    pub full_acceptance: ExactInterval,
    pub full_error: ExactInterval,
    /// Covered error divided by covered acceptance, without omitted noise mass.
    /// Absent for zero covered acceptance or an undefined error metric.
    pub conditional_error_within_enumerated_sector: Option<ExactField>,
    /// Undefined only when covered acceptance plus omitted mass is zero.
    pub conditional_error: Option<ExactInterval>,
}

fn limit(name: &str, configured: usize, required: u128) -> Result<(), TaskError> {
    if required > configured as u128 {
        return Err(TaskError::new(
            format!("enumeration.{name}"),
            format!("needs {name} >= {required}; configured limit is {configured}"),
        ));
    }
    Ok(())
}
fn history_error(
    e: RuntimeError,
    options: &HistoryEnumerationOptions,
    used_histories: usize,
    queries: usize,
    replays: usize,
) -> TaskError {
    match e {
        RuntimeError::EnumerationLimit {
            option,
            required,
            limit: original_limit,
        } => {
            let (configured, used) = match option {
                "max_branches" => (options.max_branches, used_histories),
                "max_marginal_queries" => (options.max_marginal_queries, queries),
                "max_replay_attempts" => (options.max_replay_attempts, replays),
                _ => (original_limit, 0),
            };
            TaskError::new(
                format!("enumeration.{option}"),
                format!(
                    "needs {option} >= {}; configured limit is {configured}",
                    used as u128 + required
                ),
            )
        }
        e => error("enumeration.runtime", e),
    }
}
fn bounds(
    masses: &TaskSectorMasses,
    covered: &Probability,
    omitted: &Probability,
    f: &FieldArithmetic,
) -> Result<(ExactInterval, ExactInterval, Option<ExactInterval>), TaskError> {
    let field_error = |e| error("enumeration.bounds", e);
    let c = f
        .from_rational(covered.value().clone())
        .map_err(field_error)?;
    let u = f
        .from_rational(omitted.value().clone())
        .map_err(field_error)?;
    for value in [
        masses.error.clone(),
        f.subtract(&masses.accepted, &masses.error)
            .map_err(field_error)?,
        f.subtract(&c, &masses.accepted).map_err(field_error)?,
    ] {
        if value.real_sign().map_err(field_error)? == Ordering::Less {
            return Err(TaskError::new(
                "enumeration.bounds",
                "response violates 0 <= error <= acceptance <= covered physical probability",
            ));
        }
    }
    let acceptance_upper = f.add(&masses.accepted, &u).map_err(field_error)?;
    let error_upper = f.add(&masses.error, &u).map_err(field_error)?;
    let conditional = if acceptance_upper.is_zero() {
        None
    } else {
        Some(ExactInterval {
            lower: f
                .divide(&masses.error, &acceptance_upper)
                .map_err(field_error)?,
            upper: f
                .divide(&error_upper, &acceptance_upper)
                .map_err(field_error)?,
        })
    };
    Ok((
        ExactInterval {
            lower: masses.accepted.clone(),
            upper: acceptance_upper,
        },
        ExactInterval {
            lower: masses.error.clone(),
            upper: error_upper,
        },
        conditional,
    ))
}

impl TaskEvaluator {
    /// Integrate exact unconditional responses, including all positive causal
    /// histories, against each potential physical noise pattern once.
    /// Sampling proposals and requested sampled records are rejected.
    pub fn enumerate_physical(
        &self,
        options: &TaskEnumerationOptions,
    ) -> Result<TaskEnumeration, TaskError> {
        self.enumerate_physical_observed(options, None)
    }
    /// Enumerate with call-local operation accounting and cooperative limits.
    /// The caller's meter retains successful and failed work even on error;
    /// no partial-sector estimates are returned. Preparation is separate.
    pub fn enumerate_physical_with_costs(
        &self,
        options: &TaskEnumerationOptions,
        meter: &ExecutionMeter,
    ) -> Result<TaskEnumeration, TaskError> {
        self.enumerate_physical_observed(options, Some(meter))
    }
    fn enumerate_physical_observed(
        &self,
        options: &TaskEnumerationOptions,
        meter: Option<&ExecutionMeter>,
    ) -> Result<TaskEnumeration, TaskError> {
        if let Some(meter) = meter {
            meter.check_time()?;
        }
        if self.scaled_proposal_summary().is_some() {
            return Err(TaskError::new(
                "enumeration.sampling",
                "exact physical enumeration requires physical sampling configuration; importance proposals, including unit scale, are unsupported",
            ));
        }
        if !self.requested.is_empty() {
            return Err(TaskError::new(
                "enumeration.sample_records",
                "exact task enumeration requires an empty sample_records list",
            ));
        }
        limit("max_branches", options.histories.max_branches, 1)?;
        let model = match &self.execution {
            Execution::Linear { model, .. } => model,
            Execution::Causal(program) => program.potential_noise_model(),
        };
        let sector =
            enumerate_noise(model, &options.noise).map_err(|e| error("enumeration.noise", e))?;
        if options.include_patterns {
            let fields =
                2_u128 + u128::from(self.target.is_some()) + self.task.observables.len() as u128;
            limit(
                "max_retained_response_values",
                options.max_retained_response_values,
                sector.patterns().len() as u128 * fields,
            )?;
        }
        let mut pattern_responses = options
            .include_patterns
            .then(|| Vec::with_capacity(sector.patterns().len()));
        let mut by_weight = sector
            .by_weight()
            .iter()
            .enumerate()
            .map(|(weight, p)| TaskWeightSector {
                weight,
                probability: p.clone(),
                patterns: 0,
                histories: 0,
                masses: TaskSectorMasses::zero(self),
            })
            .collect::<Vec<_>>();
        let mut masses = TaskSectorMasses::zero(self);
        let (mut histories, mut queries, mut replays, mut compilations) =
            (0_usize, 0_usize, 0_usize, 0_usize);
        let one = ExactScalar::one();
        let likelihood = BigRational::one();
        for pattern in sector.patterns() {
            if let Some(meter) = meter {
                meter.check_time()?;
            }
            let weight = &mut by_weight[pattern.weight()];
            weight.patterns += 1;
            let start_histories = weight.histories;
            let mut response = options
                .include_patterns
                .then(|| TaskSectorMasses::zero(self));
            let mut add = |row: TaskEvaluation| -> Result<(), TaskError> {
                let weighted = TaskSectorMasses::weighted(
                    &row,
                    pattern.probability.value(),
                    &self.options.fields,
                )?;
                weight.masses.add(&weighted, &self.options.fields)?;
                masses.add(&weighted, &self.options.fields)?;
                weight.histories += 1;
                if let Some(response) = &mut response {
                    response.add(
                        &TaskSectorMasses {
                            accepted: row.accepted,
                            error: row.error,
                            target_success: row.target_success,
                            moments: row.moments,
                        },
                        &self.options.fields,
                    )?;
                }
                Ok(())
            };
            match &self.execution {
                Execution::Linear { instrument, .. } => {
                    limit(
                        "max_branches",
                        options.histories.max_branches,
                        histories as u128 + 1,
                    )?;
                    add(self.evaluate_observed(
                        EvaluationInput {
                            instrument,
                            faults: &pattern.faults,
                            history_mass: &one,
                            noise_probability: pattern.probability.value(),
                            likelihood: &likelihood,
                            history: None,
                        },
                        meter,
                    )?)?;
                    histories += 1;
                }
                Execution::Causal(program) => {
                    let remaining = HistoryEnumerationOptions {
                        max_branches: options.histories.max_branches - histories,
                        max_marginal_queries: options.histories.max_marginal_queries - queries,
                        max_replay_attempts: options.histories.max_replay_attempts - replays,
                        max_record_bits: options.histories.max_record_bits,
                    };
                    let result = program
                        .enumerate_prescribed_histories_observed(
                            &[],
                            &pattern.faults,
                            remaining,
                            &self.options.scalars,
                            meter,
                        )
                        .map_err(|e| {
                            history_error(e, &options.histories, histories, queries, replays)
                        })?;
                    let mut total_mass = ExactScalar::zero();
                    for history in &result.histories {
                        total_mass = self
                            .options
                            .scalars
                            .add(&total_mass, history.history_mass())
                            .map_err(|e| error("enumeration.history_mass", e))?;
                        let mut input = EvaluationInput::history(history);
                        // Exact integration needs the joint quantum response;
                        // do not divide by H and subsequently multiply H back.
                        input.history_mass = &one;
                        add(self.evaluate_observed(input, meter)?)?;
                    }
                    if total_mass != one {
                        return Err(TaskError::new(
                            "enumeration.history_mass",
                            "positive conditional history masses do not sum to one",
                        ));
                    }
                    histories += result.histories.len();
                    queries += result.marginal_queries;
                    replays += result.replay_attempts;
                    compilations = compilations
                        .checked_add(result.network_compilations)
                        .ok_or_else(|| {
                            TaskError::new(
                                "enumeration.network_compilations",
                                "circuit construction counter overflow",
                            )
                        })?;
                }
            }
            if let Some(records) = &mut pattern_responses {
                records.push(TaskPatternResponse {
                    choices: pattern.faults.clone(),
                    probability: pattern.probability.clone(),
                    histories: weight.histories - start_histories,
                    response: response.expect("enabled pattern response"),
                });
            }
        }
        let (full_acceptance, full_error, conditional_error) = bounds(
            &masses,
            sector.covered_probability(),
            sector.omitted_probability(),
            &self.options.fields,
        )?;
        let conditional_error_within_enumerated_sector = if (self.target.is_some()
            || self.error_parities.is_some())
            && !masses.accepted.is_zero()
        {
            Some(
                self.options
                    .fields
                    .divide(&masses.error, &masses.accepted)
                    .map_err(|e| {
                        error("enumeration.conditional_error_within_enumerated_sector", e)
                    })?,
            )
        } else {
            None
        };
        if let Some(meter) = meter {
            meter.check_time()?;
        }
        Ok(TaskEnumeration {
            masses,
            target_normalization: self.target.as_ref().map(|target| target.norm.clone()),
            by_weight,
            covered_probability: sector.covered_probability().clone(),
            omitted_probability: sector.omitted_probability().clone(),
            patterns: sector.patterns().len(),
            pattern_responses,
            histories,
            marginal_queries: queries,
            replay_attempts: replays,
            network_compilations: compilations,
            full_acceptance,
            full_error,
            conditional_error_within_enumerated_sector,
            conditional_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;

    fn rational(n: i64, d: i64) -> BigRational {
        BigRational::new(n.into(), d.into())
    }
    fn real(x: BigRational) -> ExactField {
        FieldArithmetic::default().from_rational(x).unwrap()
    }
    fn masses(accepted: BigRational, error: BigRational) -> TaskSectorMasses {
        TaskSectorMasses {
            accepted: real(accepted),
            error: real(error),
            target_success: None,
            moments: vec![],
        }
    }
    fn contains(interval: &ExactInterval, value: &ExactField, f: &FieldArithmetic) -> bool {
        f.subtract(value, &interval.lower)
            .unwrap()
            .real_sign()
            .unwrap()
            != Ordering::Less
            && f.subtract(&interval.upper, value)
                .unwrap()
                .real_sign()
                .unwrap()
                != Ordering::Less
    }

    #[test]
    fn omitted_response_vertices_and_interior_points_obey_tight_conditional_bounds() {
        let f = FieldArithmetic::default();
        for quarter in 0..=4 {
            let covered = rational(quarter, 4);
            let omitted = BigRational::one() - &covered;
            for acceptance_fraction in 0..=4 {
                let accepted = &covered * rational(acceptance_fraction, 4);
                for error_fraction in 0..=4 {
                    let error = &accepted * rational(error_fraction, 4);
                    let (ai, ei, ci) = bounds(
                        &masses(accepted.clone(), error.clone()),
                        &Probability::from_rational(covered.clone()).unwrap(),
                        &Probability::from_rational(omitted.clone()).unwrap(),
                        &f,
                    )
                    .unwrap();
                    for missing_acceptance_fraction in 0..=4 {
                        let missing_a = &omitted * rational(missing_acceptance_fraction, 4);
                        for missing_error_fraction in 0..=4 {
                            let missing_e = &missing_a * rational(missing_error_fraction, 4);
                            let full_a = &accepted + &missing_a;
                            let full_e = &error + &missing_e;
                            assert!(contains(&ai, &real(full_a.clone()), &f));
                            assert!(contains(&ei, &real(full_e.clone()), &f));
                            if !full_a.is_zero() {
                                assert!(contains(ci.as_ref().unwrap(), &real(full_e / full_a), &f));
                            }
                        }
                    }
                    if let Some(ci) = ci {
                        // Both endpoints are attainable by admissible omitted sectors.
                        assert_eq!(ci.lower, real(&error / (&accepted + &omitted)));
                        assert_eq!(ci.upper, real((&error + &omitted) / (&accepted + &omitted)));
                    } else {
                        assert!(accepted.is_zero() && omitted.is_zero());
                    }
                }
            }
        }
        let (_, _, ci) = bounds(
            &masses(rational(1, 4), rational(1, 8)),
            &"1/2".parse().unwrap(),
            &"1/2".parse().unwrap(),
            &f,
        )
        .unwrap();
        assert_eq!(
            ci,
            Some(ExactInterval {
                lower: real(rational(1, 6)),
                upper: real(rational(5, 6))
            })
        );
    }

    #[test]
    fn invalid_covered_probability_responses_are_rejected() {
        let f = FieldArithmetic::default();
        for (accepted, error) in [
            (rational(-1, 4), rational(0, 1)),
            (rational(1, 4), rational(-1, 8)),
            (rational(1, 4), rational(1, 2)),
            (rational(3, 4), rational(0, 1)),
        ] {
            assert!(
                bounds(
                    &masses(accepted, error),
                    &"1/2".parse().unwrap(),
                    &"1/2".parse().unwrap(),
                    &f
                )
                .is_err()
            );
        }
    }
}
