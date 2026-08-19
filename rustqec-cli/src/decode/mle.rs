use std::collections::HashSet;

use qec_ilp_core::backend::{BinaryBackend, build_binary_backend};
use qec_ilp_core::{
    BackendConfig, BackendKind, BinaryIlpConfig, BinaryIlpModel, ConstraintSense, LinearConstraint,
    ModelSolutionStatus, ModelVar,
};

use super::{CompiledCircuit, DecodeFailure, Effect, LossEnvelope, ShotFailure};

#[derive(Clone, Copy)]
enum VariableSource {
    Independent(usize),
    Candidate { envelope: usize, candidate: usize },
}

pub(super) struct CompiledMle {
    backend: Box<dyn BinaryBackend>,
    sources: Vec<VariableSource>,
    detector_rows: Vec<usize>,
    envelope_rows: Vec<usize>,
    independent: Vec<Effect>,
    envelopes: Vec<LossEnvelope>,
    num_observables: usize,
    force_timeout: bool,
}

impl CompiledMle {
    pub(super) fn new(
        circuit: &CompiledCircuit,
        timeout_ms: Option<u64>,
    ) -> Result<Self, DecodeFailure> {
        let mut binary_vars = Vec::new();
        let mut sources = Vec::new();
        for (index, effect) in circuit.independent_effects.iter().enumerate() {
            binary_vars.push(model_var(
                format!("independent:{}", effect.id),
                effect.weight,
            ));
            sources.push(VariableSource::Independent(index));
        }
        let mut envelope_ranges = Vec::new();
        for (envelope, value) in circuit.envelopes.iter().enumerate() {
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
        let mut detector_rows = Vec::new();
        for detector in 0..circuit.num_detectors {
            let binary_terms: Vec<_> = sources
                .iter()
                .enumerate()
                .filter_map(|(variable, source)| {
                    effect_for_source(&circuit.independent_effects, &circuit.envelopes, *source)
                        .detectors
                        .contains(&detector)
                        .then_some((variable, 1.0))
                })
                .collect();
            let slack = integer_vars.len();
            integer_vars.push(ModelVar {
                name: format!("parity:{detector}"),
                objective: 0.0,
                lower: 0.0,
                upper: (binary_terms.len() / 2) as f64,
            });
            detector_rows.push(constraints.len());
            constraints.push(LinearConstraint {
                name: format!("detector:{detector}"),
                sense: ConstraintSense::Eq,
                binary_terms,
                integer_terms: vec![(slack, -2.0)],
                rhs: 0.0,
            });
        }
        let mut envelope_rows = Vec::new();
        for (envelope, range) in circuit.envelopes.iter().zip(envelope_ranges) {
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
        Ok(Self {
            backend,
            sources,
            detector_rows,
            envelope_rows,
            independent: circuit.independent_effects.clone(),
            envelopes: circuit.envelopes.clone(),
            num_observables: circuit.num_observables,
            force_timeout: timeout_ms == Some(0),
        })
    }

    pub(super) fn decode(
        &mut self,
        syndrome: &[u8],
        losses: &[usize],
    ) -> Result<Vec<usize>, ShotFailure> {
        if self.force_timeout {
            return Err(ShotFailure::Timeout);
        }
        for (&row, &bit) in self.detector_rows.iter().zip(syndrome) {
            self.backend
                .set_rhs(row, f64::from(bit))
                .map_err(|error| ShotFailure::Other(error.to_string()))?;
        }
        let active: HashSet<usize> = losses.iter().copied().collect();
        for (envelope, &row) in self.envelope_rows.iter().enumerate() {
            self.backend
                .set_rhs(row, f64::from(active.contains(&envelope)))
                .map_err(|error| ShotFailure::Other(error.to_string()))?;
        }
        let solution = self.backend.solve().map_err(|error| match error {
            qec_ilp_core::BinaryIlpError::TimeLimitWithoutIncumbent { .. } => ShotFailure::Timeout,
            other => ShotFailure::Other(other.to_string()),
        })?;
        match solution.status {
            ModelSolutionStatus::Infeasible => Err(ShotFailure::Infeasible),
            ModelSolutionStatus::TimeLimit => Err(ShotFailure::Timeout),
            ModelSolutionStatus::Optimal => {
                let mut bits = vec![false; self.num_observables];
                for (&selected, &source) in solution.binary_values.iter().zip(&self.sources) {
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
