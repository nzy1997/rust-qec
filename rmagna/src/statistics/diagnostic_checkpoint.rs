//! Versioned policy plus draw evidence, distinct from legacy exact-only totals.
use super::*;
use crate::numeric::NumericOptions;
use checkpoint::{count, error, integer_text};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyWire {
    ratio_semantics: String,
    min_ratio_frexp_exponent: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceWire {
    draws: String,
    checked_ratios: String,
    out_of_domain: String,
    missing_errors: String,
    // A required string avoids serde's implicit missing-Option acceptance.
    minimum_ratio_frexp_exponent: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    kind: String,
    #[serde(deserialize_with = "checkpoint::object")]
    policy: PolicyWire,
    #[serde(deserialize_with = "checkpoint::object")]
    evidence: EvidenceWire,
    exact_checkpoint: String,
}
fn exponent(text: &str, field: &str) -> Result<i64, CheckpointError> {
    if text.len() > 20 || !integer_text(text, true) {
        return Err(error(field, "expected a canonical i64 decimal string"));
    }
    text.parse().map_err(|e| error(field, e))
}
impl ScientificEfficiency {
    /// Persist raw exact totals and declared per-draw evidence, not a sampling
    /// task or proof that the recorded observations actually occurred.
    pub fn checkpoint_json(&self, options: CheckpointOptions) -> Result<String, CheckpointError> {
        let e = self.evidence;
        checkpoint::encode(
            &Wire {
                schema_version: 2,
                kind: "history_efficiency_scientific_domain".into(),
                policy: PolicyWire {
                    ratio_semantics: self.domain.ratio_semantics().into(),
                    min_ratio_frexp_exponent: self.domain.min_ratio_frexp_exponent().to_string(),
                },
                evidence: EvidenceWire {
                    draws: e.draws.to_string(),
                    checked_ratios: e.checked_ratios.to_string(),
                    out_of_domain: e.out_of_domain.to_string(),
                    missing_errors: e.missing_errors.to_string(),
                    minimum_ratio_frexp_exponent: e
                        .minimum_ratio_frexp_exponent
                        .map_or_else(|| "none".into(), |x| x.to_string()),
                },
                exact_checkpoint: self.exact.checkpoint_json(options)?,
            },
            options,
        )
    }
    /// Restore under caller-owned budgets and the expected semantic policy.
    /// Legacy exact aggregates lack draw provenance and are rejected. Schema,
    /// encoding and basic consistency are checked, not observation authenticity.
    pub fn from_checkpoint_json(
        text: &str,
        arithmetic: FieldArithmetic,
        domain: ScientificDiagnosticDomain,
        numeric: NumericOptions,
        options: CheckpointOptions,
    ) -> Result<Self, CheckpointError> {
        let wire: Wire = checkpoint::decode(text, options)?;
        if wire.schema_version != 2 {
            return Err(error("schema_version", "expected schema version 2"));
        }
        if wire.kind != "history_efficiency_scientific_domain" {
            return Err(error(
                "kind",
                "expected history_efficiency_scientific_domain",
            ));
        }
        if wire.policy.ratio_semantics != domain.ratio_semantics()
            || exponent(
                &wire.policy.min_ratio_frexp_exponent,
                "policy.min_ratio_frexp_exponent",
            )? != domain.min_ratio_frexp_exponent()
        {
            return Err(error(
                "policy",
                "checkpoint and caller diagnostic policies differ",
            ));
        }
        let exact =
            HistoryEfficiency::from_checkpoint_json(&wire.exact_checkpoint, arithmetic, options)?;
        let e = wire.evidence;
        let evidence = ScientificDomainEvidence {
            draws: count(&e.draws, "evidence.draws")?,
            checked_ratios: count(&e.checked_ratios, "evidence.checked_ratios")?,
            out_of_domain: count(&e.out_of_domain, "evidence.out_of_domain")?,
            missing_errors: count(&e.missing_errors, "evidence.missing_errors")?,
            minimum_ratio_frexp_exponent: if e.minimum_ratio_frexp_exponent == "none" {
                None
            } else {
                Some(exponent(
                    &e.minimum_ratio_frexp_exponent,
                    "evidence.minimum_ratio_frexp_exponent",
                )?)
            },
        };
        let e = evidence;
        if e.draws != exact.count()
            || e.missing_errors > e.draws
            || e.checked_ratios > e.draws - e.missing_errors
            || e.out_of_domain > e.checked_ratios
            || (e.missing_errors != 0) != exact.missing_error
            || (e.checked_ratios == 0) != e.minimum_ratio_frexp_exponent.is_none()
            || (e.checked_ratios == 0) != exact.statistics.sums[1].is_zero()
            || e.minimum_ratio_frexp_exponent.is_some_and(|m| {
                m > 1 || (m < domain.min_ratio_frexp_exponent()) != (e.out_of_domain != 0)
            })
        {
            return Err(error(
                "evidence",
                "draw counts, minimum ratio exponent, policy and exact totals are inconsistent",
            ));
        }
        Ok(Self {
            exact,
            domain,
            numeric,
            evidence,
        })
    }
}
