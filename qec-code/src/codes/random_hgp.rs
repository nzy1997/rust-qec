use serde::Deserialize;

use crate::error::{QecError, Result};
use crate::family_contract::{CssClassicalCheckSpec, HypergraphProductSpec};
use crate::regular_classical::{RegularClassicalMatrixConfig, deterministic_regular_matrix};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RegularClassicalCodeSpec {
    pub column_count: usize,
    pub row_count: usize,
    pub column_weight: usize,
    pub row_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
    pub retry_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpSpec {
    pub left: RegularClassicalCodeSpec,
    pub right: RegularClassicalCodeSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpClassicalSample {
    pub spec: RegularClassicalCodeSpec,
    pub rows: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpClassicalSamples {
    pub left: RandomHgpClassicalSample,
    pub right: RandomHgpClassicalSample,
}

#[derive(Debug, Deserialize)]
struct RandomHgpSpecJson {
    left: RegularClassicalCodeSpecJson,
    right: RegularClassicalCodeSpecJson,
}

#[derive(Debug, Deserialize)]
struct RegularClassicalCodeSpecJson {
    column_count: usize,
    row_count: usize,
    column_weight: usize,
    row_weight: usize,
    seed: Option<u64>,
    algorithm_version: u32,
    retry_limit: usize,
}

impl RandomHgpSpec {
    pub fn new(left: RegularClassicalCodeSpec, right: RegularClassicalCodeSpec) -> Result<Self> {
        Ok(Self { left, right })
    }
}

pub fn random_hgp_spec_from_json_str(input: &str) -> Result<RandomHgpSpec> {
    let parsed: RandomHgpSpecJson = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidCssConstructionJson(error.to_string()))?;
    RandomHgpSpec::new(
        regular_spec_from_json(parsed.left)?,
        regular_spec_from_json(parsed.right)?,
    )
}

pub fn sample_random_hgp_classical_matrices(
    spec: &RandomHgpSpec,
) -> Result<RandomHgpClassicalSamples> {
    Ok(RandomHgpClassicalSamples {
        left: sample_classical(spec.left)?,
        right: sample_classical(spec.right)?,
    })
}

pub fn sampled_random_hgp_to_hgp_spec(
    samples: &RandomHgpClassicalSamples,
) -> HypergraphProductSpec {
    HypergraphProductSpec {
        left: CssClassicalCheckSpec {
            num_cols: samples.left.spec.column_count,
            rows: samples.left.rows.clone(),
        },
        right: CssClassicalCheckSpec {
            num_cols: samples.right.spec.column_count,
            rows: samples.right.rows.clone(),
        },
    }
}

fn regular_spec_from_json(
    parsed: RegularClassicalCodeSpecJson,
) -> Result<RegularClassicalCodeSpec> {
    let seed = parsed.seed.ok_or_else(|| QecError::InvalidRandomHgpSpec {
        option: "seed",
        reason: "must be provided".to_owned(),
    })?;
    Ok(RegularClassicalCodeSpec {
        column_count: parsed.column_count,
        row_count: parsed.row_count,
        column_weight: parsed.column_weight,
        row_weight: parsed.row_weight,
        seed,
        algorithm_version: parsed.algorithm_version,
        retry_limit: parsed.retry_limit,
    })
}

fn sample_classical(spec: RegularClassicalCodeSpec) -> Result<RandomHgpClassicalSample> {
    let rows = deterministic_regular_matrix(RegularClassicalMatrixConfig {
        column_count: spec.column_count,
        row_count: spec.row_count,
        column_weight: spec.column_weight,
        row_weight: spec.row_weight,
        seed: spec.seed,
        algorithm_version: spec.algorithm_version,
        retry_limit: spec.retry_limit,
    })?;
    Ok(RandomHgpClassicalSample { spec, rows })
}
