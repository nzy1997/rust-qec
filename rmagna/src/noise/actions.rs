//! Owned mixed action tables with one stored block per fixed Pauli channel.
use super::SparseNoiseAction;
use crate::circuit::Basis;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseActionCapacityError;
impl fmt::Display for NoiseActionCapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("noise-action table exceeds platform capacity or cannot allocate another block")
    }
}
impl std::error::Error for NoiseActionCapacityError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Explicit(SparseNoiseAction),
    One(String, usize),
    Two(String, usize, usize),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Block {
    start: usize,
    action: Action,
}
/// Entries retain chronological action order. Fixed one- and two-wire sites
/// store their anchor and wires once; lookup materializes an owned action.
/// Explicit products preserve repeated wires and empty products without sorting.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactNoiseActions {
    blocks: Vec<Block>,
    count: usize,
    templates: usize,
}
impl CompactNoiseActions {
    pub const fn new() -> Self {
        Self {
            blocks: Vec::new(),
            count: 0,
            templates: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Actual fixed-channel blocks, excluding explicit actions and their factors.
    /// This is a retained-site count, not a process-memory measurement.
    pub fn template_sites(&self) -> usize {
        self.templates
    }

    fn append(&mut self, action: Action, count: usize) -> Result<(), NoiseActionCapacityError> {
        let next = self
            .count
            .checked_add(count)
            .ok_or(NoiseActionCapacityError)?;
        self.blocks
            .try_reserve(1)
            .map_err(|_| NoiseActionCapacityError)?;
        let template = !matches!(action, Action::Explicit(_));
        self.blocks.push(Block {
            start: self.count,
            action,
        });
        self.count = next;
        self.templates += usize::from(template);
        Ok(())
    }
    pub fn push(&mut self, action: SparseNoiseAction) -> Result<(), NoiseActionCapacityError> {
        self.append(Action::Explicit(action), 1)
    }
    /// Append X, Y, Z, in that order.
    pub fn add_one(
        &mut self,
        operation: String,
        wire: usize,
    ) -> Result<(), NoiseActionCapacityError> {
        self.append(Action::One(operation, wire), 3)
    }
    /// Append the lexicographic IXYZ x IXYZ alternatives, omitting II.
    /// A nonidentity factor on `left` precedes a nonidentity factor on `right`.
    pub fn add_two(
        &mut self,
        operation: String,
        left: usize,
        right: usize,
    ) -> Result<(), NoiseActionCapacityError> {
        self.append(Action::Two(operation, left, right), 15)
    }
    pub fn get(&self, index: usize) -> Option<SparseNoiseAction> {
        if index >= self.count {
            return None;
        }
        let block = &self.blocks[self.blocks.partition_point(|b| b.start <= index) - 1];
        let local = index - block.start;
        let basis = [Basis::X, Basis::Y, Basis::Z];
        Some(match &block.action {
            Action::Explicit(action) => action.clone(),
            Action::One(operation, wire) => SparseNoiseAction::Pauli {
                operation: operation.clone(),
                factors: vec![(*wire, basis[local])],
            },
            Action::Two(operation, left, right) => {
                let word = local + 1;
                let mut factors = Vec::with_capacity(2);
                for (wire, letter) in [(*left, word / 4), (*right, word % 4)] {
                    if letter != 0 {
                        factors.push((wire, basis[letter - 1]));
                    }
                }
                SparseNoiseAction::Pauli {
                    operation: operation.clone(),
                    factors,
                }
            }
        })
    }
}
