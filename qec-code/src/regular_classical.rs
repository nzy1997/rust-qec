#![doc = include_str!("../doc/regular_classical.md")]

use crate::error::{QecError, Result};

pub const REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegularClassicalMatrixConfig {
    pub column_count: usize,
    pub row_count: usize,
    pub column_weight: usize,
    pub row_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
    pub retry_limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64V1 {
    state: u64,
}

impl SplitMix64V1 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^ (value >> 31)
    }
}

pub fn bounded_index_v1(stream: &mut SplitMix64V1, upper_bound: u64) -> Option<u64> {
    if upper_bound == 0 {
        return None;
    }

    let threshold = 0u64.wrapping_sub(upper_bound) % upper_bound;
    loop {
        let value = stream.next_u64();
        if value >= threshold {
            return Some(value % upper_bound);
        }
    }
}

pub fn deterministic_regular_matrix(
    config: RegularClassicalMatrixConfig,
) -> Result<Vec<Vec<usize>>> {
    validate_config(config)?;

    let mut stream = SplitMix64V1::new(config.seed);
    for attempt in 1..=config.retry_limit {
        if let Some(rows) = try_regular_matrix_attempt(config, &mut stream)? {
            return Ok(rows);
        }
        if attempt == config.retry_limit {
            return Err(QecError::RegularClassicalMatrixGenerationExhausted {
                retry_limit: config.retry_limit,
                attempts: attempt,
                algorithm_version: config.algorithm_version,
                seed: config.seed,
            });
        }
    }

    unreachable!("regular matrix retry_limit is validated to be nonzero")
}

fn validate_config(config: RegularClassicalMatrixConfig) -> Result<()> {
    if config.algorithm_version != REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1 {
        return Err(QecError::UnsupportedRegularClassicalMatrixAlgorithm {
            algorithm_version: config.algorithm_version,
        });
    }

    validate_nonzero(config.column_count, "column_count")?;
    validate_nonzero(config.row_count, "row_count")?;
    validate_nonzero(config.column_weight, "column_weight")?;
    validate_nonzero(config.row_weight, "row_weight")?;
    validate_nonzero(config.retry_limit, "retry_limit")?;

    if config.column_weight > config.row_count {
        return Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "column_weight",
            reason: "must be at most row_count".to_owned(),
        });
    }
    if config.row_weight > config.column_count {
        return Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "row_weight",
            reason: "must be at most column_count".to_owned(),
        });
    }

    let column_stubs = config
        .column_count
        .checked_mul(config.column_weight)
        .ok_or(QecError::RegularClassicalMatrixStubCountOverflow { side: "column" })?;
    let row_stubs = config
        .row_count
        .checked_mul(config.row_weight)
        .ok_or(QecError::RegularClassicalMatrixStubCountOverflow { side: "row" })?;

    if column_stubs != row_stubs {
        return Err(QecError::RegularClassicalMatrixStubCountMismatch {
            column_stubs,
            row_stubs,
        });
    }

    Ok(())
}

fn validate_nonzero(value: usize, option: &'static str) -> Result<()> {
    if value == 0 {
        return Err(QecError::InvalidRegularClassicalMatrixConfig {
            option,
            reason: "must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn try_regular_matrix_attempt(
    config: RegularClassicalMatrixConfig,
    stream: &mut SplitMix64V1,
) -> Result<Option<Vec<Vec<usize>>>> {
    let row_stub_count = config
        .row_count
        .checked_mul(config.row_weight)
        .expect("regular matrix config was validated before sampling");
    let mut row_slots = Vec::with_capacity(row_stub_count);
    for row in 0..config.row_count {
        row_slots.extend(std::iter::repeat_n(row, config.row_weight));
    }

    let mut rows = vec![Vec::with_capacity(config.row_weight); config.row_count];
    for column in 0..config.column_count {
        let mut selected_rows = Vec::with_capacity(config.column_weight);
        for _ in 0..config.column_weight {
            let valid_slot_count = count_non_duplicate_slots(&row_slots, &selected_rows);
            if valid_slot_count == 0 {
                return Ok(None);
            }

            let selected_rank = bounded_index_v1(stream, valid_slot_count as u64)
                .expect("valid_slot_count is checked to be nonzero");
            let slot_index = slot_index_for_rank(&row_slots, &selected_rows, selected_rank)
                .expect("selected rank should identify one remaining slot");
            let row = row_slots.remove(slot_index);
            selected_rows.push(row);
            rows[row].push(column);
        }
    }

    for row in &mut rows {
        row.sort_unstable();
    }
    rows.sort_unstable();
    Ok(Some(rows))
}

fn count_non_duplicate_slots(row_slots: &[usize], selected_rows: &[usize]) -> usize {
    row_slots
        .iter()
        .filter(|row| !selected_rows.contains(row))
        .count()
}

fn slot_index_for_rank(
    row_slots: &[usize],
    selected_rows: &[usize],
    selected_rank: u64,
) -> Option<usize> {
    let mut valid_rank = 0u64;
    for (slot_index, row) in row_slots.iter().enumerate() {
        if selected_rows.contains(row) {
            continue;
        }
        if valid_rank == selected_rank {
            return Some(slot_index);
        }
        valid_rank += 1;
    }
    None
}
