use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{QecError, Result};
use crate::family_contract::verify_css_orthogonality;
use crate::finite_group::{
    FiniteGroupSpec, GroupAlgebraElement, left_regular_lift, right_regular_lift,
};
use crate::regular_classical::{SplitMix64V1, bounded_index_v1};

pub const RANDOM_TWO_BLOCK_ALGORITHM_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockSpec {
    pub group: FiniteGroupSpec,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockCssChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub support_a: Vec<usize>,
    pub support_b: Vec<usize>,
    pub metadata: RandomTwoBlockMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockMetadata {
    pub group_digest: String,
    pub seed: u64,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub algorithm_version: u32,
}

#[derive(Debug, Deserialize)]
struct RandomTwoBlockSpecJson {
    group: ExplicitRandomTwoBlockGroupJson,
    support_a_weight: usize,
    support_b_weight: usize,
    seed: Option<u64>,
    algorithm_version: u32,
}

#[derive(Debug, Deserialize)]
struct ExplicitRandomTwoBlockGroupJson {
    name: Option<String>,
    element_order: Option<String>,
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
}

impl RandomTwoBlockSpec {
    pub fn new(
        group: FiniteGroupSpec,
        support_a_weight: usize,
        support_b_weight: usize,
        seed: u64,
        algorithm_version: u32,
    ) -> Result<Self> {
        let spec = Self {
            group,
            support_a_weight,
            support_b_weight,
            seed,
            algorithm_version,
        };
        verify_random_two_block_spec(&spec)?;
        Ok(spec)
    }
}

pub fn random_two_block_spec_from_json_str(input: &str) -> Result<RandomTwoBlockSpec> {
    let parsed: RandomTwoBlockSpecJson = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidCssConstructionJson(error.to_string()))?;
    let seed = parsed
        .seed
        .ok_or_else(|| QecError::InvalidRandomTwoBlockSpec {
            option: "seed",
            reason: "must be provided".to_owned(),
        })?;
    let ExplicitRandomTwoBlockGroupJson {
        name,
        element_order,
        order,
        identity,
        multiplication_table,
    } = parsed.group;
    let _ = (name, element_order);
    let group = FiniteGroupSpec::new(order, identity, multiplication_table)?;
    RandomTwoBlockSpec::new(
        group,
        parsed.support_a_weight,
        parsed.support_b_weight,
        seed,
        parsed.algorithm_version,
    )
}

pub fn random_two_block_css_checks(spec: &RandomTwoBlockSpec) -> Result<RandomTwoBlockCssChecks> {
    verify_random_two_block_spec(spec)?;

    let mut stream = SplitMix64V1::new(spec.seed);
    let support_a = sample_support_v1(&mut stream, spec.group.order(), spec.support_a_weight);
    let support_b = sample_support_v1(&mut stream, spec.group.order(), spec.support_b_weight);
    let a = GroupAlgebraElement::new(&spec.group, support_a.clone())?;
    let b = GroupAlgebraElement::new(&spec.group, support_b.clone())?;
    let left_a = left_regular_lift(&spec.group, &[vec![a]])?;
    let right_b = right_regular_lift(&spec.group, &[vec![b]])?;
    let h_x = left_a.hconcat(&right_b)?;
    let h_z = right_b.transpose()?.hconcat(&left_a.transpose()?)?;

    verify_css_orthogonality(h_x.num_cols(), h_x.rows(), h_z.rows())?;

    Ok(RandomTwoBlockCssChecks {
        num_cols: h_x.num_cols(),
        h_x: h_x.rows().to_vec(),
        h_z: h_z.rows().to_vec(),
        support_a,
        support_b,
        metadata: RandomTwoBlockMetadata {
            group_digest: group_digest(&spec.group),
            seed: spec.seed,
            support_a_weight: spec.support_a_weight,
            support_b_weight: spec.support_b_weight,
            algorithm_version: spec.algorithm_version,
        },
    })
}

fn sample_support_v1(stream: &mut SplitMix64V1, order: usize, weight: usize) -> Vec<usize> {
    let mut pool = (0..order).collect::<Vec<_>>();
    for i in 0..weight {
        let offset = bounded_index_v1(stream, (order - i) as u64)
            .expect("sampling support from nonempty candidates");
        let j = i + offset as usize;
        pool.swap(i, j);
    }
    pool[..weight].sort_unstable();
    pool.truncate(weight);
    pool
}

fn verify_random_two_block_spec(spec: &RandomTwoBlockSpec) -> Result<()> {
    if spec.algorithm_version != RANDOM_TWO_BLOCK_ALGORITHM_V1 {
        return Err(QecError::UnsupportedRandomTwoBlockAlgorithm {
            algorithm_version: spec.algorithm_version,
        });
    }
    verify_support_weight(
        spec.support_a_weight,
        spec.group.order(),
        "support_a_weight",
    )?;
    verify_support_weight(
        spec.support_b_weight,
        spec.group.order(),
        "support_b_weight",
    )
}

fn verify_support_weight(weight: usize, order: usize, option: &'static str) -> Result<()> {
    if weight == 0 {
        return Err(QecError::InvalidRandomTwoBlockSpec {
            option,
            reason: "must be greater than zero".to_owned(),
        });
    }
    if weight > order {
        return Err(QecError::InvalidRandomTwoBlockSpec {
            option,
            reason: "must be at most the group order".to_owned(),
        });
    }
    Ok(())
}

fn group_digest(group: &FiniteGroupSpec) -> String {
    format!(
        "sha256:{}",
        lower_hex(Sha256::digest(group.to_json_string()))
    )
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
