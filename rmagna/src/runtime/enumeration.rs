//! Exact forward-causal replay of positive physical observation groups.
use super::*;
use crate::records::{RecordEnumerationOptions, enumerate_reported_conditioned_observed};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug)]
pub struct HistoryEnumerationOptions {
    /// Maximum total of retained complete histories and pending prefixes.
    pub max_branches: usize,
    /// Maximum independent structural bits in any unresolved observation group.
    pub max_record_bits: usize,
    /// Global physical marginal query budget, including incomplete replays.
    pub max_marginal_queries: usize,
    /// Global budget for complete or interrupted causal executions.
    pub max_replay_attempts: usize,
}
impl Default for HistoryEnumerationOptions {
    fn default() -> Self {
        Self {
            max_branches: 4096,
            max_record_bits: 64,
            max_marginal_queries: 1_000_000,
            max_replay_attempts: 1_000_000,
        }
    }
}
pub struct EnumeratedHistories {
    /// Positive histories conditional on the prescribed potential fault choices.
    pub histories: Vec<ResolvedHistory>,
    /// All actual physical marginal queries, including incomplete replays.
    pub marginal_queries: usize,
    /// All complete or interrupted causal executions.
    pub replay_attempts: usize,
    /// All post-reference cold compilations, including incomplete replays.
    pub network_compilations: usize,
}
fn limit(option: &'static str, bound: usize, required: u128) -> Result<(), RuntimeError> {
    if required > bound as u128 {
        return Err(RuntimeError::EnumerationLimit {
            option,
            limit: bound,
            required,
        });
    }
    Ok(())
}
type ObservedCallbacks = BTreeMap<(usize, Vec<bool>), Vec<bool>>;

pub(super) struct Budget {
    options: HistoryEnumerationOptions,
    queries: Cell<usize>,
    attempts: Cell<usize>,
    compilations: Cell<usize>,
    callbacks: RefCell<ObservedCallbacks>,
}
impl Budget {
    fn query(&self) -> Result<(), RuntimeError> {
        limit(
            "max_marginal_queries",
            self.options.max_marginal_queries,
            self.queries.get() as u128 + 1,
        )?;
        self.queries.set(self.queries.get() + 1);
        Ok(())
    }
    fn attempt(&self) -> Result<(), RuntimeError> {
        limit(
            "max_replay_attempts",
            self.options.max_replay_attempts,
            self.attempts.get() as u128 + 1,
        )?;
        self.attempts.set(self.attempts.get() + 1);
        Ok(())
    }
    fn observe(&self, call: &ControllerCall) -> Result<(), RuntimeError> {
        let mut callbacks = self.callbacks.borrow_mut();
        let key = (call.call, call.inputs.clone());
        if let Some(previous) = callbacks.get(&key) {
            if previous != &call.outputs {
                return Err(RuntimeError::Controller {name:call.name.clone(),call:call.call,message:"repeated inputs produced different outputs during exact enumeration; controllers must be deterministic and stateless".to_owned()});
            }
        } else {
            callbacks.insert(key, call.outputs.clone());
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(super) enum HistoryMode<'a> {
    Sample,
    Prescribed(&'a [Vec<bool>]),
    Enumerated {
        groups: &'a [Vec<bool>],
        budget: &'a Budget,
        available_branches: usize,
    },
}
impl<'a> HistoryMode<'a> {
    pub(super) fn groups(self) -> Option<&'a [Vec<bool>]> {
        match self {
            Self::Sample => None,
            Self::Prescribed(groups) | Self::Enumerated { groups, .. } => Some(groups),
        }
    }
    pub(super) fn query(self) -> Result<(), RuntimeError> {
        if let Self::Enumerated { budget, .. } = self {
            budget.query()?;
        }
        Ok(())
    }
    pub(super) fn compilation(self) -> Result<(), RuntimeError> {
        if let Self::Enumerated { budget, .. } = self {
            limit(
                "network_compilations",
                usize::MAX,
                budget.compilations.get() as u128 + 1,
            )?;
            budget.compilations.set(budget.compilations.get() + 1);
        }
        Ok(())
    }
    pub(super) fn observe(self, call: &ControllerCall) -> Result<(), RuntimeError> {
        if let Self::Enumerated { budget, .. } = self {
            budget.observe(call)?;
        }
        Ok(())
    }
    pub(super) fn unresolved(
        self,
        instrument: &CompiledInstrument,
        query: &RecordQuery<'_>,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
        meter: Option<&ExecutionMeter>,
    ) -> Result<Option<Vec<Vec<bool>>>, RuntimeError> {
        let Self::Enumerated {
            budget,
            available_branches,
            ..
        } = self
        else {
            return Ok(None);
        };
        let options = RecordEnumerationOptions {
            max_branches: available_branches,
            max_record_bits: budget.options.max_record_bits,
            max_marginal_queries: budget.options.max_marginal_queries - budget.queries.get(),
            max_terms,
        };
        let result = enumerate_reported_conditioned_observed(
            instrument, query, faults, &options, arithmetic, meter,
        )
        .map_err(|error| match error {
            SamplingError::EnumerationLimit {
                option: "max_marginal_queries",
                required,
                ..
            } => RuntimeError::EnumerationLimit {
                option: "max_marginal_queries",
                limit: budget.options.max_marginal_queries,
                required: budget.queries.get() as u128 + required,
            },
            SamplingError::EnumerationLimit {
                option: "max_branches",
                required,
                ..
            } => RuntimeError::EnumerationLimit {
                option: "max_branches",
                limit: budget.options.max_branches,
                required: (budget.options.max_branches - available_branches) as u128 + required,
            },
            SamplingError::EnumerationLimit {
                option: "max_record_bits",
                required,
                ..
            } => RuntimeError::EnumerationLimit {
                option: "max_record_bits",
                limit: budget.options.max_record_bits,
                required,
            },
            error => RuntimeError::Sampling(error),
        })?;
        budget
            .queries
            .set(budget.queries.get() + result.marginal_queries);
        Ok(Some(
            result
                .branches
                .into_iter()
                .map(|branch| branch.values)
                .collect(),
        ))
    }
}
pub(super) enum ExecutionError {
    Runtime(RuntimeError),
    Branch {
        group: usize,
        outcomes: Vec<Vec<bool>>,
    },
}
impl ExecutionError {
    pub(super) fn runtime(self) -> RuntimeError {
        match self {
            Self::Runtime(error) => error,
            Self::Branch { group, .. } => RuntimeError::InvalidHistory {
                group,
                message: "unexpected unresolved history in ordinary execution".to_owned(),
            },
        }
    }
}
impl From<RuntimeError> for ExecutionError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}
impl From<InstrumentError> for ExecutionError {
    fn from(error: InstrumentError) -> Self {
        Self::Runtime(error.into())
    }
}
impl From<SamplingError> for ExecutionError {
    fn from(error: SamplingError) -> Self {
        Self::Runtime(error.into())
    }
}

impl CausalProgram {
    /// Sum-ready positive histories conditional on one potential noise assignment.
    /// Callbacks must obey their deterministic/stateless contract; repeated
    /// observations are audited. No proposal or noise weight is applied here.
    pub fn enumerate_prescribed_histories(
        &self,
        requested: &[BitExpr],
        faults: &[FaultSelection],
        options: HistoryEnumerationOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<EnumeratedHistories, RuntimeError> {
        self.enumerate_prescribed_histories_observed(requested, faults, options, arithmetic, None)
    }
    pub(crate) fn enumerate_prescribed_histories_observed(
        &self,
        requested: &[BitExpr],
        faults: &[FaultSelection],
        options: HistoryEnumerationOptions,
        arithmetic: &ScalarArithmetic,
        meter: Option<&ExecutionMeter>,
    ) -> Result<EnumeratedHistories, RuntimeError> {
        if let Some(meter) = meter {
            meter.check_time()?;
        }
        self.validate_requested(requested)?;
        limit("max_branches", options.max_branches, 1)?;
        let probability = self.model.probability(faults)?;
        let budget = Budget {
            options,
            queries: Cell::new(0),
            attempts: Cell::new(0),
            compilations: Cell::new(0),
            callbacks: RefCell::new(BTreeMap::new()),
        };
        let mut pending = vec![Vec::<Vec<bool>>::new()];
        let mut histories = Vec::new();
        while let Some(prefix) = pending.pop() {
            if let Some(meter) = meter {
                meter.check_time()?;
            }
            budget.attempt()?;
            let available_branches = options.max_branches - histories.len() - pending.len();
            let draw = NoiseDraw {
                faults: faults.to_vec(),
                physical_probability: probability.clone(),
                proposal_probability: probability.clone(),
                likelihood_ratio: BigRational::one(),
                entropy_words: 0,
            };
            match self.execute(
                requested,
                draw,
                &mut NoEntropy,
                arithmetic,
                HistoryMode::Enumerated {
                    groups: &prefix,
                    budget: &budget,
                    available_branches,
                },
                meter,
            ) {
                Ok(history) => {
                    if history
                        .history_mass()
                        .real_sign()
                        .map_err(InstrumentError::from)?
                        != std::cmp::Ordering::Greater
                    {
                        return Err(RuntimeError::InvalidHistory {
                            group: prefix.len(),
                            message: "complete enumerated history has nonpositive physical mass"
                                .to_owned(),
                        });
                    }
                    histories.push(history);
                }
                Err(ExecutionError::Runtime(error)) => return Err(error),
                Err(ExecutionError::Branch { outcomes, .. }) => {
                    limit(
                        "max_branches",
                        options.max_branches,
                        histories.len() as u128 + pending.len() as u128 + outcomes.len() as u128,
                    )?;
                    for outcome in outcomes.into_iter().rev() {
                        let mut groups = prefix.clone();
                        groups.push(outcome);
                        pending.push(groups);
                    }
                }
            }
        }
        Ok(EnumeratedHistories {
            histories,
            marginal_queries: budget.queries.get(),
            replay_attempts: budget.attempts.get(),
            network_compilations: budget.compilations.get(),
        })
    }
}
struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("exact history traversal attempted to draw entropy")
    }
}
