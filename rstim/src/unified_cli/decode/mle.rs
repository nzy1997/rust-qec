use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use qec_ilp_core::backend::{BinaryBackend, build_binary_backend};
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelSolutionStatus, ModelVar,
};
use rstim::m2d::LossAwareDetectorShot;

use super::{
    CompiledCircuit, DecodeFailure, Effect, LossEnvelope, MAX_CONDITIONED_DECODER_ITEMS,
    MAX_CONDITIONED_DECODER_WORK, ShotFailure, conditioned_cache_needs_eviction,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VariableSource {
    Independent(usize),
    Candidate { envelope: usize, candidate: usize },
}

pub(super) struct CompiledMle {
    cache: HashMap<Vec<usize>, ConditionedMleModel>,
    independent: Vec<Effect>,
    envelopes: Vec<LossEnvelope>,
    detector_count: usize,
    num_observables: usize,
    timeout_ms: Option<u64>,
    force_timeout: bool,
    cached_work: usize,
    cache_order: VecDeque<Vec<usize>>,
    model_builds: usize,
    cache_hits: usize,
}

struct ConditionedMleModel {
    backend: Box<dyn BinaryBackend>,
    sources: Vec<VariableSource>,
    detector_rows: Vec<usize>,
    detector_count: usize,
    work: usize,
}

struct ConditionedMlePlan {
    sources: Vec<VariableSource>,
    active_envelope_ranges: Vec<(usize, Range<usize>)>,
    detector_binary_variables: Vec<Vec<usize>>,
    work: usize,
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
            detector_count: circuit.loss_aware_m2d.layout().num_detectors(),
            num_observables: circuit.num_observables,
            timeout_ms,
            force_timeout: timeout_ms == Some(0),
            cached_work: 0,
            cache_order: VecDeque::new(),
            model_builds: 0,
            cache_hits: 0,
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
        if syndrome.canonical_detector_values.len() != self.detector_count {
            return Err(ShotFailure::Other(
                "loss pattern produced inconsistent canonical detector count".to_string(),
            ));
        }
        let key = losses.to_vec();
        if self.cache.contains_key(&key) {
            self.cache_hits += 1;
        } else {
            let plan = preflight_conditioned_mle(
                &self.independent,
                &self.envelopes,
                self.detector_count,
                losses,
            )
            .map_err(|error| ShotFailure::Other(error.message))?;
            let artifact_work = plan.work;
            self.evict_until_fits(artifact_work)?;
            let model = build_pattern_model(
                &self.independent,
                &self.envelopes,
                self.detector_count,
                self.timeout_ms,
                plan,
            )
            .map_err(|error| ShotFailure::Other(error.message))?;
            self.cache.insert(key.clone(), model);
            self.cache_order.push_back(key.clone());
            self.cached_work += artifact_work;
            self.model_builds += 1;
        }
        let model = self.cache.get_mut(&key).unwrap();
        if model.detector_count != syndrome.canonical_detector_values.len() {
            return Err(ShotFailure::Other(
                "loss pattern produced inconsistent canonical detector count".to_string(),
            ));
        }
        for (&row, &value) in model
            .detector_rows
            .iter()
            .zip(&syndrome.canonical_detector_values)
        {
            model
                .backend
                .set_rhs(row, f64::from(value))
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
        self.model_builds
    }

    pub(super) fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    pub(super) fn detector_rows(&self) -> usize {
        self.cache
            .values()
            .next()
            .map_or(0, |model| model.detector_rows.len())
    }

    fn evict_until_fits(&mut self, artifact_work: usize) -> Result<(), ShotFailure> {
        while conditioned_cache_needs_eviction(self.cache.len(), self.cached_work, artifact_work)
            .map_err(ShotFailure::Other)?
        {
            let oldest = self.cache_order.pop_front().ok_or_else(|| {
                ShotFailure::Other("conditioned MLE cache accounting drift".to_string())
            })?;
            let removed = self.cache.remove(&oldest).ok_or_else(|| {
                ShotFailure::Other("conditioned MLE cache accounting drift".to_string())
            })?;
            self.cached_work = self.cached_work.checked_sub(removed.work).ok_or_else(|| {
                ShotFailure::Other("conditioned MLE cache accounting drift".to_string())
            })?;
        }
        Ok(())
    }
}

fn build_pattern_model(
    independent: &[Effect],
    envelopes: &[LossEnvelope],
    detector_count: usize,
    timeout_ms: Option<u64>,
    plan: ConditionedMlePlan,
) -> Result<ConditionedMleModel, DecodeFailure> {
    let ConditionedMlePlan {
        sources,
        active_envelope_ranges,
        detector_binary_variables,
        work,
    } = plan;
    let binary_vars: Vec<_> = sources
        .iter()
        .map(|&source| match source {
            VariableSource::Independent(index) => {
                let effect = &independent[index];
                model_var(format!("independent:{}", effect.id), effect.weight)
            }
            VariableSource::Candidate {
                envelope,
                candidate,
            } => {
                let value = &envelopes[envelope];
                let effect = &value.candidates[candidate];
                model_var(format!("loss:{}:{}", value.id, effect.id), 0.0)
            }
        })
        .collect();
    let mut integer_vars = Vec::new();
    let mut constraints = Vec::new();
    let mut detector_rows = Vec::new();
    for (detector_index, variables) in detector_binary_variables.into_iter().enumerate() {
        let binary_terms: Vec<_> = variables
            .into_iter()
            .map(|variable| (variable, 1.0))
            .collect();
        let slack = integer_vars.len();
        integer_vars.push(ModelVar {
            name: format!("parity:{detector_index}"),
            objective: 0.0,
            lower: 0.0,
            upper: (binary_terms.len() / 2) as f64,
        });
        detector_rows.push(constraints.len());
        constraints.push(LinearConstraint {
            name: format!("detector:{detector_index}"),
            sense: ConstraintSense::Eq,
            binary_terms,
            integer_terms: vec![(slack, -2.0)],
            rhs: 0.0,
        });
    }
    for (envelope_index, range) in active_envelope_ranges {
        let envelope = &envelopes[envelope_index];
        constraints.push(LinearConstraint {
            name: format!("loss-active:{}", envelope.id),
            sense: ConstraintSense::Eq,
            binary_terms: range.map(|variable| (variable, 1.0)).collect(),
            integer_terms: Vec::new(),
            rhs: 1.0,
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
        detector_rows,
        detector_count,
        work,
    })
}

fn preflight_conditioned_mle(
    independent: &[Effect],
    envelopes: &[LossEnvelope],
    detector_count: usize,
    losses: &[usize],
) -> Result<ConditionedMlePlan, DecodeFailure> {
    preflight_conditioned_mle_with_work_limit(
        independent,
        envelopes,
        detector_count,
        losses,
        MAX_CONDITIONED_DECODER_WORK,
    )
}

fn preflight_conditioned_mle_with_work_limit(
    independent: &[Effect],
    envelopes: &[LossEnvelope],
    detector_count: usize,
    losses: &[usize],
    max_work: usize,
) -> Result<ConditionedMlePlan, DecodeFailure> {
    if independent.len() > MAX_CONDITIONED_DECODER_ITEMS
        || detector_count > MAX_CONDITIONED_DECODER_ITEMS
        || losses.len() > MAX_CONDITIONED_DECODER_ITEMS
    {
        return Err(conditioned_mle_limit_error());
    }
    if losses.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DecodeFailure::new(
            "decode_error",
            "conditioned MLE loss pattern must contain strictly increasing envelope indices",
        ));
    }

    let mut source_count = independent.len();
    let mut effect_terms = independent
        .iter()
        .try_fold(0usize, |total, effect| {
            total.checked_add(effect.detectors.len())
        })
        .ok_or_else(conditioned_mle_limit_error)?;
    let mut active_candidate_count = 0usize;
    for &envelope_index in losses {
        let envelope = envelopes.get(envelope_index).ok_or_else(|| {
            DecodeFailure::new(
                "decode_error",
                format!("loss pattern references unknown envelope {envelope_index}"),
            )
        })?;
        source_count = source_count
            .checked_add(envelope.candidates.len())
            .ok_or_else(conditioned_mle_limit_error)?;
        active_candidate_count = active_candidate_count
            .checked_add(envelope.candidates.len())
            .ok_or_else(conditioned_mle_limit_error)?;
        effect_terms = envelope
            .candidates
            .iter()
            .try_fold(effect_terms, |total, effect| {
                total.checked_add(effect.detectors.len())
            })
            .ok_or_else(conditioned_mle_limit_error)?;
    }
    if source_count > MAX_CONDITIONED_DECODER_ITEMS {
        return Err(conditioned_mle_limit_error());
    }
    let mut work = 0usize;
    // Charge both the detector-row storage and its implicit singleton basis
    // incidence, matching the previous accounting for no-loss checks without
    // materializing a detector-to-check map.
    for count in [
        source_count,
        detector_count,
        effect_terms,
        detector_count,
        active_candidate_count,
    ] {
        charge_conditioned_mle_work(&mut work, count, max_work)?;
    }

    let mut sources = Vec::with_capacity(source_count);
    sources.extend((0..independent.len()).map(VariableSource::Independent));
    let mut active_envelope_ranges = Vec::with_capacity(losses.len());
    for &envelope in losses {
        let start = sources.len();
        sources.extend((0..envelopes[envelope].candidates.len()).map(|candidate| {
            VariableSource::Candidate {
                envelope,
                candidate,
            }
        }));
        active_envelope_ranges.push((envelope, start..sources.len()));
    }

    let mut detector_binary_variables = vec![Vec::new(); detector_count];
    let mut parity = vec![false; detector_count];
    let mut touched = vec![false; detector_count];
    let mut touched_rows = Vec::new();
    for (variable, &source) in sources.iter().enumerate() {
        let effect = effect_for_source(independent, envelopes, source);
        for &detector in &effect.detectors {
            if detector >= detector_count {
                return Err(DecodeFailure::new(
                    "decode_error",
                    format!(
                        "conditioned MLE effect references detector {detector}, but the canonical detector count is {detector_count}"
                    ),
                ));
            }
            charge_conditioned_mle_work(&mut work, 1, max_work)?;
            if !touched[detector] {
                touched[detector] = true;
                touched_rows.push(detector);
            }
            parity[detector] ^= true;
        }
        for row in touched_rows.drain(..) {
            if parity[row] {
                charge_conditioned_mle_work(&mut work, 1, max_work)?;
                detector_binary_variables[row].push(variable);
            }
            parity[row] = false;
            touched[row] = false;
        }
    }

    Ok(ConditionedMlePlan {
        sources,
        active_envelope_ranges,
        detector_binary_variables,
        work,
    })
}

fn charge_conditioned_mle_work(
    work: &mut usize,
    amount: usize,
    max_work: usize,
) -> Result<(), DecodeFailure> {
    *work = work
        .checked_add(amount)
        .ok_or_else(conditioned_mle_limit_error)?;
    if *work > max_work {
        return Err(conditioned_mle_limit_error());
    }
    Ok(())
}

fn conditioned_mle_limit_error() -> DecodeFailure {
    DecodeFailure::new("decode_error", "conditioned MLE work limit exceeded")
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

    fn empty_decoder() -> CompiledMle {
        CompiledMle {
            cache: HashMap::new(),
            independent: Vec::new(),
            envelopes: Vec::new(),
            detector_count: 0,
            num_observables: 0,
            timeout_ms: None,
            force_timeout: false,
            cached_work: 0,
            cache_order: VecDeque::new(),
            model_builds: 0,
            cache_hits: 0,
        }
    }

    fn cached_model(work: usize) -> ConditionedMleModel {
        let independent = [Effect {
            id: "cache-test".to_string(),
            detectors: Vec::new(),
            observables: Vec::new(),
            weight: 1.0,
        }];
        let mut plan = preflight_conditioned_mle(&independent, &[], 0, &[]).unwrap();
        plan.work = work;
        build_pattern_model(&independent, &[], 0, None, plan).unwrap()
    }

    #[test]
    fn conditioned_mle_preflights_incidence_work() {
        let mut work = 0;
        charge_conditioned_mle_work(
            &mut work,
            MAX_CONDITIONED_DECODER_WORK,
            MAX_CONDITIONED_DECODER_WORK,
        )
        .unwrap();
        let error =
            charge_conditioned_mle_work(&mut work, 1, MAX_CONDITIONED_DECODER_WORK).unwrap_err();
        assert_eq!(error.code, "decode_error");
        assert!(error.message.contains("work limit"));
        let mut overflow = 1;
        assert!(
            charge_conditioned_mle_work(&mut overflow, usize::MAX, MAX_CONDITIONED_DECODER_WORK,)
                .is_err()
        );
    }

    #[test]
    fn conditioned_mle_preflight_charges_sparse_incidence_and_materialized_terms() {
        let effect = |detectors| Effect {
            id: "effect".to_string(),
            detectors,
            observables: Vec::new(),
            weight: 1.0,
        };
        let cancelling = [effect(vec![0, 0])];
        assert!(preflight_conditioned_mle_with_work_limit(&cancelling, &[], 1, &[], 7).is_ok());
        assert!(
            preflight_conditioned_mle_with_work_limit(&cancelling, &[], 1, &[], 6).is_err(),
            "two sparse incidence visits must be charged even when their parity cancels"
        );

        let materialized = [effect(vec![0])];
        assert!(preflight_conditioned_mle_with_work_limit(&materialized, &[], 1, &[], 6).is_ok());
        assert!(
            preflight_conditioned_mle_with_work_limit(&materialized, &[], 1, &[], 5).is_err(),
            "the detector-row binary term must be charged after its incidence visit"
        );
    }

    #[test]
    fn conditioned_mle_plan_keeps_only_active_envelopes_in_canonical_detector_space() {
        let independent = [Effect {
            id: "independent".to_string(),
            detectors: vec![0],
            observables: Vec::new(),
            weight: 1.0,
        }];
        let envelopes = [
            LossEnvelope {
                id: "inactive".to_string(),
                candidates: vec![Effect {
                    id: "inactive-candidate".to_string(),
                    detectors: vec![1],
                    observables: Vec::new(),
                    weight: 0.0,
                }],
            },
            LossEnvelope {
                id: "active".to_string(),
                candidates: vec![
                    Effect {
                        id: "active-candidate".to_string(),
                        detectors: vec![0, 1],
                        observables: Vec::new(),
                        weight: 0.0,
                    },
                    Effect {
                        id: "identity".to_string(),
                        detectors: Vec::new(),
                        observables: Vec::new(),
                        weight: 0.0,
                    },
                ],
            },
        ];
        let plan = preflight_conditioned_mle(&independent, &envelopes, 2, &[1]).unwrap();
        assert_eq!(
            plan.sources,
            [
                VariableSource::Independent(0),
                VariableSource::Candidate {
                    envelope: 1,
                    candidate: 0,
                },
                VariableSource::Candidate {
                    envelope: 1,
                    candidate: 1,
                },
            ]
        );
        assert_eq!(plan.active_envelope_ranges, [(1, 1..3)]);
        assert_eq!(plan.detector_binary_variables, [vec![0, 1], vec![1]]);

        let duplicate = preflight_conditioned_mle(&independent, &envelopes, 2, &[1, 1])
            .err()
            .unwrap();
        assert!(duplicate.message.contains("strictly increasing"));
        let unknown = preflight_conditioned_mle(&independent, &envelopes, 2, &[2])
            .err()
            .unwrap();
        assert!(unknown.message.contains("unknown envelope"));

        let out_of_range = preflight_conditioned_mle(&independent, &envelopes, 1, &[1])
            .err()
            .unwrap();
        assert!(out_of_range.message.contains("references detector 1"));
    }

    #[test]
    fn conditioned_mle_evicts_fifo_and_reports_accounting_drift() {
        let mut decoder = empty_decoder();
        decoder
            .cache
            .insert(vec![0], cached_model(MAX_CONDITIONED_DECODER_WORK));
        decoder.cache_order.push_back(vec![0]);
        decoder.cached_work = MAX_CONDITIONED_DECODER_WORK;
        decoder.evict_until_fits(1).unwrap();
        assert!(decoder.cache.is_empty());
        assert_eq!(decoder.cached_work, 0);

        let mut missing_order = empty_decoder();
        missing_order.cached_work = MAX_CONDITIONED_DECODER_WORK;
        assert!(matches!(
            missing_order.evict_until_fits(1),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));

        let mut missing_entry = empty_decoder();
        missing_entry.cached_work = MAX_CONDITIONED_DECODER_WORK;
        missing_entry.cache_order.push_back(vec![0]);
        assert!(matches!(
            missing_entry.evict_until_fits(1),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));

        let mut underflow = empty_decoder();
        underflow.cache.insert(vec![0], cached_model(2));
        underflow.cache_order.push_back(vec![0]);
        underflow.cached_work = 1;
        assert!(matches!(
            underflow.evict_until_fits(MAX_CONDITIONED_DECODER_WORK),
            Err(ShotFailure::Other(message)) if message.contains("accounting drift")
        ));
    }
}
