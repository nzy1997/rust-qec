use std::collections::{HashMap, HashSet};

use qec_ilp_core::backend::{BinaryBackend, build_binary_backend};
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelSolutionStatus, ModelVar,
};
use rstim::m2d::{LossAwareDetectorCheck, LossAwareDetectorShot};

use super::{
    CompiledCircuit, DecodeFailure, Effect, LossEnvelope, MAX_CONDITIONED_DECODER_ITEMS,
    MAX_CONDITIONED_DECODER_WORK, ShotFailure, conditioned_cache_total,
};

#[derive(Clone, Copy)]
enum VariableSource {
    Independent(usize),
    Candidate { envelope: usize, candidate: usize },
}

pub(super) struct CompiledMle {
    cache: HashMap<Vec<usize>, ConditionedMleModel>,
    independent: Vec<Effect>,
    envelopes: Vec<LossEnvelope>,
    num_observables: usize,
    timeout_ms: Option<u64>,
    force_timeout: bool,
    cached_work: usize,
}

struct ConditionedMleModel {
    backend: Box<dyn BinaryBackend>,
    sources: Vec<VariableSource>,
    check_rows: Vec<usize>,
    envelope_rows: Vec<usize>,
    check_sources: Vec<Vec<usize>>,
}

impl CompiledMle {
    pub(super) fn new(
        circuit: &CompiledCircuit,
        timeout_ms: Option<u64>,
    ) -> Result<Self, DecodeFailure> {
        Ok(Self {
            cache: HashMap::new(),
            independent: circuit.independent_effects.clone(),
            envelopes: circuit.envelopes.clone(),
            num_observables: circuit.num_observables,
            timeout_ms,
            force_timeout: timeout_ms == Some(0),
            cached_work: 0,
        })
    }

    pub(super) fn decode(
        &mut self,
        syndrome: &LossAwareDetectorShot,
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        if self.force_timeout {
            return Err(ShotFailure::Timeout);
        }
        let key = losses.to_vec();
        if !self.cache.contains_key(&key) {
            let (source_count, artifact_work) = preflight_conditioned_mle(
                &self.independent,
                &self.envelopes,
                &syndrome.checks,
            )
            .map_err(|error| ShotFailure::Other(error.message))?;
            let cached_work = conditioned_cache_total(
                "MLE",
                self.cache.len(),
                self.cached_work,
                artifact_work,
            )
            .map_err(ShotFailure::Other)?;
            let model = build_pattern_model(
                &self.independent,
                &self.envelopes,
                &syndrome.checks,
                self.timeout_ms,
                source_count,
            )
            .map_err(|error| ShotFailure::Other(error.message))?;
            self.cache.insert(key.clone(), model);
            self.cached_work = cached_work;
        }
        let model = self.cache.get_mut(&key).unwrap();
        if model.check_sources.len() != syndrome.checks.len()
            || model
                .check_sources
                .iter()
                .zip(&syndrome.checks)
                .any(|(expected, actual)| expected != &actual.source_detectors)
        {
            return Err(ShotFailure::Other(
                "loss pattern produced inconsistent detector-check basis".to_string(),
            ));
        }
        for (&row, check) in model.check_rows.iter().zip(&syndrome.checks) {
            model
                .backend
                .set_rhs(row, f64::from(check.value))
                .map_err(|error| ShotFailure::Other(error.to_string()))?;
        }
        let active: HashSet<usize> = losses.iter().copied().collect();
        for (envelope, &row) in model.envelope_rows.iter().enumerate() {
            model
                .backend
                .set_rhs(row, f64::from(active.contains(&envelope)))
                .map_err(|error| ShotFailure::Other(error.to_string()))?;
        }
        let solution = model.backend.solve().map_err(|error| match error {
            qec_ilp_core::BinaryIlpError::TimeLimitWithoutIncumbent { .. } => ShotFailure::Timeout,
            other => ShotFailure::Other(other.to_string()),
        })?;
        match solution.status {
            ModelSolutionStatus::Infeasible => Err(ShotFailure::Infeasible),
            ModelSolutionStatus::TimeLimit => Err(ShotFailure::Timeout),
            ModelSolutionStatus::Optimal => {
                let mut bits = vec![false; self.num_observables];
                for (&selected, &source) in solution.binary_values.iter().zip(&model.sources) {
                    if selected {
                        for &observable in
                            &effect_for_source(&self.independent, &self.envelopes, source)
                                .observables
                        {
                            bits[observable] ^= true;
                        }
                    }
                }
                Ok(bits
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &bit)| bit.then_some(index))
                    .collect())
            }
            status => Err(ShotFailure::Other(format!(
                "unexpected MLE solve status {status:?}"
            ))),
        }
    }

    pub(super) fn model_builds(&self) -> usize {
        self.cache.len()
    }
}

fn build_pattern_model(
    independent: &[Effect],
    envelopes: &[LossEnvelope],
    checks: &[LossAwareDetectorCheck],
    timeout_ms: Option<u64>,
    source_count: usize,
) -> Result<ConditionedMleModel, DecodeFailure> {
    let mut binary_vars = Vec::with_capacity(source_count);
    let mut sources = Vec::with_capacity(source_count);
    for (index, effect) in independent.iter().enumerate() {
        binary_vars.push(model_var(
            format!("independent:{}", effect.id),
            effect.weight,
        ));
        sources.push(VariableSource::Independent(index));
    }
    let mut envelope_ranges = Vec::new();
    for (envelope, value) in envelopes.iter().enumerate() {
        let start = binary_vars.len();
        for (candidate, effect) in value.candidates.iter().enumerate() {
            binary_vars.push(model_var(format!("loss:{}:{}", value.id, effect.id), 0.0));
            sources.push(VariableSource::Candidate {
                envelope,
                candidate,
            });
        }
        envelope_ranges.push(start..binary_vars.len());
    }
    let mut integer_vars = Vec::new();
    let mut constraints = Vec::new();
    let mut check_rows = Vec::new();
    for (check_index, check) in checks.iter().enumerate() {
        let binary_terms: Vec<_> = sources
            .iter()
            .enumerate()
            .filter_map(|(variable, source)| {
                let effect = effect_for_source(independent, envelopes, *source);
                odd_sorted_intersection(&check.source_detectors, &effect.detectors)
                    .then_some((variable, 1.0))
            })
            .collect();
        let slack = integer_vars.len();
        integer_vars.push(ModelVar {
            name: format!("parity:{check_index}"),
            objective: 0.0,
            lower: 0.0,
            upper: (binary_terms.len() / 2) as f64,
        });
        check_rows.push(constraints.len());
        constraints.push(LinearConstraint {
            name: format!("check:{check_index}"),
            sense: ConstraintSense::Eq,
            binary_terms,
            integer_terms: vec![(slack, -2.0)],
            rhs: 0.0,
        });
    }
    let mut envelope_rows = Vec::new();
    for (envelope, range) in envelopes.iter().zip(envelope_ranges) {
        envelope_rows.push(constraints.len());
        constraints.push(LinearConstraint {
            name: format!("loss-active:{}", envelope.id),
            sense: ConstraintSense::Eq,
            binary_terms: range.map(|variable| (variable, 1.0)).collect(),
            integer_terms: Vec::new(),
            rhs: 0.0,
        });
    }
    let model = BinaryIlpModel {
        solution_binary_prefix_len: binary_vars.len(),
        binary_vars,
        integer_vars,
        constraints,
    };
    let backend = build_binary_backend(
        &model,
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: timeout_ms.map(|value| value as f64 / 1000.0),
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .map_err(|error| DecodeFailure::new("decode_error", error.to_string()))?;
    Ok(ConditionedMleModel {
        backend,
        sources,
        check_rows,
        envelope_rows,
        check_sources: checks
            .iter()
            .map(|check| check.source_detectors.clone())
            .collect(),
    })
}

fn preflight_conditioned_mle(
    independent: &[Effect],
    envelopes: &[LossEnvelope],
    checks: &[LossAwareDetectorCheck],
) -> Result<(usize, usize), DecodeFailure> {
    let source_count = envelopes
        .iter()
        .try_fold(independent.len(), |total, envelope| {
            total.checked_add(envelope.candidates.len())
        })
        .ok_or_else(conditioned_mle_limit_error)?;
    let effect_terms = independent
        .iter()
        .chain(envelopes.iter().flat_map(|envelope| &envelope.candidates))
        .try_fold(0usize, |total, effect| {
            total.checked_add(effect.detectors.len())
        })
        .ok_or_else(conditioned_mle_limit_error)?;
    let check_terms = checks
        .iter()
        .try_fold(0usize, |total, check| {
            total.checked_add(check.source_detectors.len())
        })
        .ok_or_else(conditioned_mle_limit_error)?;
    let work = conditioned_mle_work_from_counts(
        source_count,
        checks.len(),
        effect_terms,
        check_terms,
    )?;
    Ok((source_count, work))
}

fn conditioned_mle_work_from_counts(
    source_count: usize,
    check_count: usize,
    effect_terms: usize,
    check_terms: usize,
) -> Result<usize, DecodeFailure> {
    if source_count > MAX_CONDITIONED_DECODER_ITEMS
        || check_count > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_mle_limit_error());
    }
    let work = source_count
        .checked_add(check_count)
        .and_then(|value| value.checked_add(effect_terms))
        .and_then(|value| value.checked_add(check_terms))
        .and_then(|value| value.checked_add(source_count.checked_mul(check_count)?))
        .and_then(|value| value.checked_add(source_count.checked_mul(check_terms)?))
        .and_then(|value| value.checked_add(check_count.checked_mul(effect_terms)?))
        .ok_or_else(conditioned_mle_limit_error)?;
    if work > MAX_CONDITIONED_DECODER_WORK {
        return Err(conditioned_mle_limit_error());
    }
    Ok(work)
}

fn conditioned_mle_limit_error() -> DecodeFailure {
    DecodeFailure::new("decode_error", "conditioned MLE work limit exceeded")
}

fn odd_sorted_intersection(left: &[usize], right: &[usize]) -> bool {
    let (mut a, mut b, mut parity) = (0, 0, false);
    while a < left.len() && b < right.len() {
        match left[a].cmp(&right[b]) {
            std::cmp::Ordering::Less => a += 1,
            std::cmp::Ordering::Greater => b += 1,
            std::cmp::Ordering::Equal => {
                parity ^= true;
                a += 1;
                b += 1;
            }
        }
    }
    parity
}

fn model_var(name: String, objective: f64) -> ModelVar {
    ModelVar {
        name,
        objective,
        lower: 0.0,
        upper: 1.0,
    }
}

fn effect_for_source<'a>(
    independent: &'a [Effect],
    envelopes: &'a [LossEnvelope],
    source: VariableSource,
) -> &'a Effect {
    match source {
        VariableSource::Independent(index) => &independent[index],
        VariableSource::Candidate {
            envelope,
            candidate,
        } => &envelopes[envelope].candidates[candidate],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditioned_mle_preflights_incidence_work() {
        assert!(conditioned_mle_work_from_counts(1_000, 1_000, 0, 0).is_ok());
        let error = conditioned_mle_work_from_counts(10_001, 1_000, 0, 0).unwrap_err();
        assert_eq!(error.code, "decode_error");
        assert!(error.message.contains("work limit"));
        assert!(conditioned_mle_work_from_counts(usize::MAX, 0, 0, 0).is_err());
        assert!(conditioned_mle_work_from_counts(1, 1, 0, usize::MAX).is_err());
    }

    #[test]
    fn conditioned_mle_caps_many_cached_patterns() {
        assert!(conditioned_cache_total("MLE", 0, 0, 1).is_ok());
        assert!(conditioned_cache_total(
            "MLE",
            super::super::MAX_CONDITIONED_DECODER_ARTIFACTS,
            0,
            1,
        )
        .is_err());
        assert!(conditioned_cache_total("MLE", 1, MAX_CONDITIONED_DECODER_WORK, 1).is_err());
    }
}
