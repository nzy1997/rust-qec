use std::collections::HashSet;

use qec_ilp_core::backend::build_binary_backend;
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelSolutionStatus, ModelVar,
};

use crate::error::EnvelopeDecodeError;
use crate::schema::{
    AtomLossCase, Effect, INPUT_SCHEMA_VERSION, InfeasibleResult, OptimalResult,
    RESULT_SCHEMA_VERSION, SelectedLossCandidate,
};

const BACKEND_NAME: &str = "highs";

#[derive(Debug, Clone, PartialEq)]
pub enum DecodeOutcome {
    Optimal(OptimalResult),
    Infeasible(InfeasibleResult),
}

#[derive(Debug, Clone, Copy)]
enum VariableSource {
    Independent(usize),
    LossCandidate {
        envelope_index: usize,
        candidate_index: usize,
    },
}

struct LoweredCase {
    model: BinaryIlpModel,
    sources: Vec<VariableSource>,
}

pub fn decode(case: &AtomLossCase) -> Result<DecodeOutcome, EnvelopeDecodeError> {
    validate(case)?;
    let lowered = lower(case);
    if lowered.sources.is_empty() {
        return if (0..case.num_detectors).any(|detector| parity(&case.observed_detectors, detector))
        {
            Ok(infeasible_outcome())
        } else {
            Ok(DecodeOutcome::Optimal(interpret_solution(case, &[], &[])))
        };
    }
    let mut backend = build_binary_backend(
        &lowered.model,
        &BinaryIlpConfig {
            backend: BackendConfig {
                kind: BackendKind::Highs,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )?;
    let solution = backend.solve()?;
    match solution.status {
        ModelSolutionStatus::Infeasible => Ok(infeasible_outcome()),
        ModelSolutionStatus::Optimal => {
            if solution.binary_values.len() != lowered.sources.len() {
                return Err(EnvelopeDecodeError::SolutionWidthMismatch {
                    expected: lowered.sources.len(),
                    actual: solution.binary_values.len(),
                });
            }
            Ok(DecodeOutcome::Optimal(interpret_solution(
                case,
                &lowered.sources,
                &solution.binary_values,
            )))
        }
        status => Err(EnvelopeDecodeError::UnexpectedSolveStatus(status)),
    }
}

fn infeasible_outcome() -> DecodeOutcome {
    DecodeOutcome::Infeasible(InfeasibleResult {
        schema_version: RESULT_SCHEMA_VERSION,
        status: "infeasible",
        backend: BACKEND_NAME,
    })
}

fn validate(case: &AtomLossCase) -> Result<(), EnvelopeDecodeError> {
    if case.schema_version != INPUT_SCHEMA_VERSION {
        return Err(EnvelopeDecodeError::UnsupportedSchema {
            expected: INPUT_SCHEMA_VERSION,
            actual: case.schema_version.clone(),
        });
    }
    validate_detector_indices(
        "observed_detectors",
        &case.observed_detectors,
        case.num_detectors,
    )?;

    let mut independent_ids = HashSet::new();
    for effect in &case.independent_effects {
        validate_id("independent effect", &effect.id, &mut independent_ids)?;
        validate_effect(effect, format!("independent effect {:?}", effect.id), case)?;
    }

    let mut loss_ids = HashSet::new();
    for envelope in &case.loss_envelopes {
        validate_id("loss", &envelope.loss_id, &mut loss_ids)?;
        if envelope.candidates.is_empty() {
            return Err(EnvelopeDecodeError::EmptyCandidates {
                loss_id: envelope.loss_id.clone(),
            });
        }
        let mut candidate_ids = HashSet::new();
        for candidate in &envelope.candidates {
            validate_id("loss candidate", &candidate.id, &mut candidate_ids)?;
            validate_effect(
                candidate,
                format!(
                    "candidate {:?} of loss {:?}",
                    candidate.id, envelope.loss_id
                ),
                case,
            )?;
        }
    }
    Ok(())
}

fn validate_id(
    kind: &'static str,
    id: &str,
    seen: &mut HashSet<String>,
) -> Result<(), EnvelopeDecodeError> {
    if id.is_empty() {
        return Err(EnvelopeDecodeError::EmptyId { kind });
    }
    if !seen.insert(id.to_string()) {
        return Err(EnvelopeDecodeError::DuplicateId {
            kind,
            id: id.to_string(),
        });
    }
    Ok(())
}

fn validate_effect(
    effect: &Effect,
    owner: String,
    case: &AtomLossCase,
) -> Result<(), EnvelopeDecodeError> {
    validate_detector_indices(&owner, &effect.detectors, case.num_detectors)?;
    for &index in &effect.observables {
        if index >= case.num_observables {
            return Err(EnvelopeDecodeError::ObservableOutOfRange {
                owner,
                index,
                num_observables: case.num_observables,
            });
        }
    }
    if !effect.weight.is_finite() || effect.weight < 0.0 {
        return Err(EnvelopeDecodeError::InvalidWeight {
            owner,
            weight: effect.weight,
        });
    }
    Ok(())
}

fn validate_detector_indices(
    owner: &str,
    indices: &[usize],
    num_detectors: usize,
) -> Result<(), EnvelopeDecodeError> {
    for &index in indices {
        if index >= num_detectors {
            return Err(EnvelopeDecodeError::DetectorOutOfRange {
                owner: owner.to_string(),
                index,
                num_detectors,
            });
        }
    }
    Ok(())
}

fn lower(case: &AtomLossCase) -> LoweredCase {
    let mut binary_vars = Vec::new();
    let mut sources = Vec::new();
    for (index, effect) in case.independent_effects.iter().enumerate() {
        push_binary_var(
            &mut binary_vars,
            &mut sources,
            format!("independent:{}", effect.id),
            effect.weight,
            VariableSource::Independent(index),
        );
    }
    let mut envelope_ranges = Vec::with_capacity(case.loss_envelopes.len());
    for (envelope_index, envelope) in case.loss_envelopes.iter().enumerate() {
        let start = binary_vars.len();
        for (candidate_index, candidate) in envelope.candidates.iter().enumerate() {
            push_binary_var(
                &mut binary_vars,
                &mut sources,
                format!("loss:{}:{}", envelope.loss_id, candidate.id),
                candidate.weight,
                VariableSource::LossCandidate {
                    envelope_index,
                    candidate_index,
                },
            );
        }
        envelope_ranges.push(start..binary_vars.len());
    }

    let mut integer_vars = Vec::with_capacity(case.num_detectors);
    let mut constraints = Vec::with_capacity(case.num_detectors + case.loss_envelopes.len());
    for detector in 0..case.num_detectors {
        let binary_terms: Vec<_> = sources
            .iter()
            .enumerate()
            .filter_map(|(variable, source)| {
                effect_for_source(case, *source)
                    .detectors
                    .iter()
                    .filter(|&&value| value == detector)
                    .count()
                    .rem_euclid(2)
                    .eq(&1)
                    .then_some((variable, 1.0))
            })
            .collect();
        let slack = integer_vars.len();
        integer_vars.push(ModelVar {
            name: format!("detector-parity:{detector}"),
            objective: 0.0,
            lower: 0.0,
            upper: (binary_terms.len() / 2) as f64,
        });
        constraints.push(LinearConstraint {
            name: format!("detector:{detector}"),
            sense: ConstraintSense::Eq,
            binary_terms,
            integer_terms: vec![(slack, -2.0)],
            rhs: parity(&case.observed_detectors, detector) as u8 as f64,
        });
    }
    for (envelope, variables) in case.loss_envelopes.iter().zip(envelope_ranges) {
        constraints.push(LinearConstraint {
            name: format!("loss-exactly-one:{}", envelope.loss_id),
            sense: ConstraintSense::Eq,
            binary_terms: variables.map(|variable| (variable, 1.0)).collect(),
            integer_terms: Vec::new(),
            rhs: 1.0,
        });
    }

    LoweredCase {
        model: BinaryIlpModel {
            solution_binary_prefix_len: binary_vars.len(),
            binary_vars,
            integer_vars,
            constraints,
        },
        sources,
    }
}

fn push_binary_var(
    binary_vars: &mut Vec<ModelVar>,
    sources: &mut Vec<VariableSource>,
    name: String,
    objective: f64,
    source: VariableSource,
) {
    binary_vars.push(ModelVar {
        name,
        objective,
        lower: 0.0,
        upper: 1.0,
    });
    sources.push(source);
}

fn effect_for_source(case: &AtomLossCase, source: VariableSource) -> &Effect {
    match source {
        VariableSource::Independent(index) => &case.independent_effects[index],
        VariableSource::LossCandidate {
            envelope_index,
            candidate_index,
        } => &case.loss_envelopes[envelope_index].candidates[candidate_index],
    }
}

fn parity(indices: &[usize], target: usize) -> bool {
    indices.iter().filter(|&&index| index == target).count() % 2 == 1
}

fn interpret_solution(
    case: &AtomLossCase,
    sources: &[VariableSource],
    selected: &[bool],
) -> OptimalResult {
    let mut selected_independent_effects = Vec::new();
    let mut selected_loss_candidates = Vec::new();
    let mut observable_bits = vec![false; case.num_observables];
    let mut objective = 0.0;

    for (&is_selected, &source) in selected.iter().zip(sources) {
        if !is_selected {
            continue;
        }
        let effect = effect_for_source(case, source);
        objective += effect.weight;
        for (observable, bit) in observable_bits.iter_mut().enumerate() {
            *bit ^= parity(&effect.observables, observable);
        }
        match source {
            VariableSource::Independent(index) => {
                selected_independent_effects.push(case.independent_effects[index].id.clone());
            }
            VariableSource::LossCandidate {
                envelope_index,
                candidate_index,
            } => {
                let envelope = &case.loss_envelopes[envelope_index];
                selected_loss_candidates.push(SelectedLossCandidate {
                    loss_id: envelope.loss_id.clone(),
                    candidate_id: envelope.candidates[candidate_index].id.clone(),
                });
            }
        }
    }

    OptimalResult {
        schema_version: RESULT_SCHEMA_VERSION,
        status: "optimal",
        backend: BACKEND_NAME,
        selected_independent_effects,
        selected_loss_candidates,
        predicted_observables: observable_bits
            .iter()
            .enumerate()
            .filter_map(|(index, &bit)| bit.then_some(index))
            .collect(),
        objective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::LossEnvelope;

    fn identity(id: &str) -> Effect {
        Effect {
            id: id.to_string(),
            detectors: vec![],
            observables: vec![],
            weight: 0.0,
        }
    }

    fn empty_case() -> AtomLossCase {
        AtomLossCase {
            schema_version: INPUT_SCHEMA_VERSION.to_string(),
            num_detectors: 1,
            num_observables: 1,
            observed_detectors: vec![],
            independent_effects: vec![],
            loss_envelopes: vec![],
        }
    }

    #[test]
    fn rejects_duplicate_ids_before_lowering() {
        let mut case = empty_case();
        case.independent_effects = vec![identity("same"), identity("same")];

        assert!(matches!(
            decode(&case),
            Err(EnvelopeDecodeError::DuplicateId {
                kind: "independent effect",
                id,
            }) if id == "same"
        ));
    }

    #[test]
    fn rejects_empty_candidate_lists_before_lowering() {
        let mut case = empty_case();
        case.loss_envelopes.push(LossEnvelope {
            loss_id: "loss-0".to_string(),
            candidates: vec![],
        });

        assert_eq!(
            decode(&case).unwrap_err(),
            EnvelopeDecodeError::EmptyCandidates {
                loss_id: "loss-0".to_string(),
            }
        );
    }

    #[test]
    fn rejects_out_of_range_indices_and_invalid_weights() {
        let mut case = empty_case();
        case.independent_effects.push(Effect {
            id: "bad-detector".to_string(),
            detectors: vec![1],
            observables: vec![],
            weight: 0.0,
        });
        assert!(matches!(
            decode(&case),
            Err(EnvelopeDecodeError::DetectorOutOfRange { index: 1, .. })
        ));

        case.independent_effects[0].detectors.clear();
        case.independent_effects[0].weight = f64::NAN;
        assert!(matches!(
            decode(&case),
            Err(EnvelopeDecodeError::InvalidWeight { .. })
        ));

        case.independent_effects[0].weight = 0.0;
        case.independent_effects[0].observables = vec![1];
        assert!(matches!(
            decode(&case),
            Err(EnvelopeDecodeError::ObservableOutOfRange { index: 1, .. })
        ));
    }

    #[test]
    fn rejects_unsupported_schema_and_empty_ids() {
        let mut case = empty_case();
        case.schema_version = "future-schema".to_string();
        assert!(matches!(
            decode(&case),
            Err(EnvelopeDecodeError::UnsupportedSchema { .. })
        ));

        case.schema_version = INPUT_SCHEMA_VERSION.to_string();
        case.independent_effects.push(identity(""));
        assert_eq!(
            decode(&case).unwrap_err(),
            EnvelopeDecodeError::EmptyId {
                kind: "independent effect"
            }
        );
    }

    #[test]
    fn repeated_indices_cancel_modulo_two() {
        let mut case = empty_case();
        case.observed_detectors = vec![0, 0];
        case.independent_effects.push(Effect {
            id: "double".to_string(),
            detectors: vec![0, 0],
            observables: vec![0, 0],
            weight: 1.0,
        });

        let DecodeOutcome::Optimal(result) = decode(&case).unwrap() else {
            panic!("case should be feasible");
        };
        assert!(result.selected_independent_effects.is_empty());
        assert!(result.predicted_observables.is_empty());
        assert_eq!(result.objective, 0.0);
    }

    #[test]
    fn valid_zero_size_case_returns_an_empty_optimal_result() {
        let case = AtomLossCase {
            schema_version: INPUT_SCHEMA_VERSION.to_string(),
            num_detectors: 0,
            num_observables: 0,
            observed_detectors: vec![],
            independent_effects: vec![],
            loss_envelopes: vec![],
        };

        let DecodeOutcome::Optimal(result) = decode(&case).unwrap() else {
            panic!("empty case should be optimal");
        };
        assert!(result.selected_independent_effects.is_empty());
        assert!(result.selected_loss_candidates.is_empty());
        assert!(result.predicted_observables.is_empty());
        assert_eq!(result.objective, 0.0);
    }

    #[test]
    fn no_effects_cannot_explain_an_observed_detector() {
        let mut case = empty_case();
        case.observed_detectors = vec![0];

        assert!(matches!(
            decode(&case).unwrap(),
            DecodeOutcome::Infeasible(_)
        ));
    }
}
