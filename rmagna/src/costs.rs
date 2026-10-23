//! Explicit call-local logical operation counters and inclusive wall times.
use std::{
    cell::RefCell,
    fmt,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Compilation,
    Query,
    Decoder,
    Support,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OperationCost {
    pub attempted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub seconds: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExecutionCosts {
    pub compilation: OperationCost,
    pub query: OperationCost,
    pub decoder: OperationCost,
    pub support: OperationCost,
}
impl ExecutionCosts {
    fn get_mut(&mut self, kind: OperationKind) -> &mut OperationCost {
        match kind {
            OperationKind::Compilation => &mut self.compilation,
            OperationKind::Query => &mut self.query,
            OperationKind::Decoder => &mut self.decoder,
            OperationKind::Support => &mut self.support,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostError {
    CounterOverflow { operation: OperationKind },
    QueryLimit { limit: u64 },
    TimeLimit { limit: Duration },
}
impl fmt::Display for CostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CounterOverflow { operation } => write!(
                f,
                "execution {operation:?} counter exhausted u64 before operation"
            ),
            Self::QueryLimit { limit } => {
                write!(f, "run exhausted max_queries = {limit} before next query")
            }
            Self::TimeLimit { limit } => write!(
                f,
                "run exhausted max_seconds = {} between operations",
                limit.as_secs_f64()
            ),
        }
    }
}
impl std::error::Error for CostError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_queries: u64,
    pub max_duration: Option<Duration>,
}
impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_queries: u64::MAX,
            max_duration: None,
        }
    }
}
impl ExecutionLimits {
    /// Preserve the exact duration instead of rounding it through binary64.
    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({"max_queries":self.max_queries,"max_duration":self.max_duration.map(|d|serde_json::json!({"seconds":d.as_secs(),"nanoseconds":d.subsec_nanos()}))})
    }
}

/// One caller owns each collector. It is neither global nor shared across
/// threads; concurrent runs construct their own collectors. Nested operations
/// are supported, and elapsed times are inclusive, not additive partitions.
pub struct ExecutionMeter {
    costs: RefCell<ExecutionCosts>,
    limits: ExecutionLimits,
    started: Instant,
}
impl Default for ExecutionMeter {
    fn default() -> Self {
        Self::new(ExecutionLimits::default())
    }
}
impl ExecutionMeter {
    pub fn new(limits: ExecutionLimits) -> Self {
        Self {
            costs: RefCell::new(ExecutionCosts::default()),
            limits,
            started: Instant::now(),
        }
    }
    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }
    fn check_elapsed(&self, elapsed: Duration) -> Result<(), CostError> {
        if let Some(limit) = self.limits.max_duration
            && elapsed >= limit
        {
            return Err(CostError::TimeLimit { limit });
        }
        Ok(())
    }
    /// Check only the deadline, for example before drawing more entropy.
    pub fn check_time(&self) -> Result<(), CostError> {
        self.check_elapsed(self.started.elapsed())
    }
    pub fn snapshot(&self) -> ExecutionCosts {
        *self.costs.borrow()
    }
    /// Counter exhaustion rejects before invoking action. Returned errors and
    /// unwinding panics both count as failed operations. Physics is not retried.
    pub fn measure<T, E: From<CostError>>(
        &self,
        kind: OperationKind,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.check_time()?;
        {
            let mut costs = self.costs.borrow_mut();
            if kind == OperationKind::Query && costs.query.attempted >= self.limits.max_queries {
                return Err(CostError::QueryLimit {
                    limit: self.limits.max_queries,
                }
                .into());
            }
            let cost = costs.get_mut(kind);
            cost.attempted = cost
                .attempted
                .checked_add(1)
                .ok_or(CostError::CounterOverflow { operation: kind })?;
        }
        let mut guard = Measurement {
            meter: self,
            kind,
            started: Instant::now(),
            succeeded: false,
        };
        let result = action();
        guard.succeeded = result.is_ok();
        result
    }
}
struct Measurement<'a> {
    meter: &'a ExecutionMeter,
    kind: OperationKind,
    started: Instant,
    succeeded: bool,
}
impl Drop for Measurement<'_> {
    fn drop(&mut self) {
        let seconds = self.started.elapsed().as_secs_f64();
        let mut costs = self.meter.costs.borrow_mut();
        let cost = costs.get_mut(self.kind);
        // A finish count cannot exceed its already reserved attempted count.
        if self.succeeded {
            cost.succeeded += 1;
        } else {
            cost.failed += 1;
        }
        cost.seconds += seconds;
    }
}
pub(crate) fn observe<T, E: From<CostError>>(
    meter: Option<&ExecutionMeter>,
    kind: OperationKind,
    action: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    match meter {
        Some(meter) => meter.measure(kind, action),
        None => action(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_deadlines_and_nested_quotas_reject_before_starting_more_work() {
        let deadline = Duration::from_nanos(7);
        let meter = ExecutionMeter::new(ExecutionLimits {
            max_queries: 1,
            max_duration: Some(deadline),
        });
        assert_eq!(meter.check_elapsed(Duration::from_nanos(6)), Ok(()));
        for elapsed in [deadline, Duration::from_secs(100)] {
            assert_eq!(
                meter.check_elapsed(elapsed),
                Err(CostError::TimeLimit { limit: deadline })
            );
        }
        let meter = ExecutionMeter::new(ExecutionLimits {
            max_queries: 1,
            max_duration: None,
        });
        let result: Result<(), CostError> = meter.measure(OperationKind::Query, || {
            meter.measure(OperationKind::Query, || {
                panic!("nested query must be blocked")
            })
        });
        assert_eq!(result, Err(CostError::QueryLimit { limit: 1 }));
        let cost = meter.snapshot().query;
        assert_eq!((cost.attempted, cost.succeeded, cost.failed), (1, 0, 1));
        let result: Result<(), CostError> = meter.measure(OperationKind::Compilation, || Ok(()));
        assert_eq!(result, Ok(()));
        for kind in [
            OperationKind::Compilation,
            OperationKind::Query,
            OperationKind::Decoder,
            OperationKind::Support,
        ] {
            let meter = ExecutionMeter::new(ExecutionLimits {
                max_queries: u64::MAX,
                max_duration: Some(Duration::ZERO),
            });
            let result: Result<(), CostError> =
                meter.measure(kind, || panic!("expired operation must not start"));
            assert_eq!(
                result,
                Err(CostError::TimeLimit {
                    limit: Duration::ZERO
                })
            );
            assert_eq!(meter.snapshot(), ExecutionCosts::default());
        }
    }
    #[test]
    fn nested_errors_unwinds_and_counter_exhaustion_keep_exact_counts() {
        let meter = ExecutionMeter::default();
        let result: Result<(), CostError> = meter.measure(OperationKind::Query, || {
            assert_eq!(meter.snapshot().query.attempted, 1);
            assert_eq!(meter.snapshot().query.succeeded, 0);
            meter.measure(OperationKind::Query, || Ok(()))?;
            Err(CostError::CounterOverflow {
                operation: OperationKind::Decoder,
            })
        });
        assert!(result.is_err());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), CostError> =
                    meter.measure(OperationKind::Query, || panic!("operation panic"));
            }))
            .is_err()
        );
        let snapshot = meter.snapshot();
        assert_eq!(
            (
                snapshot.query.attempted,
                snapshot.query.succeeded,
                snapshot.query.failed
            ),
            (3, 1, 2)
        );
        assert!(snapshot.query.seconds.is_finite() && snapshot.query.seconds >= 0.0);
        meter.costs.borrow_mut().decoder.attempted = u64::MAX;
        let result: Result<(), CostError> = meter.measure(OperationKind::Decoder, || {
            panic!("must reject before action")
        });
        assert_eq!(
            result,
            Err(CostError::CounterOverflow {
                operation: OperationKind::Decoder
            })
        );
        assert_eq!(meter.snapshot().decoder.attempted, u64::MAX);
        assert_eq!(meter.snapshot().decoder.failed, 0);
    }
}
