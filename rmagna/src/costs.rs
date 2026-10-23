//! Explicit call-local logical operation counters and inclusive wall times.
use std::{cell::RefCell, fmt, time::Instant};

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
pub struct CostError {
    pub operation: OperationKind,
}
impl fmt::Display for CostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "execution {:?} counter exhausted u64 before operation",
            self.operation
        )
    }
}
impl std::error::Error for CostError {}

/// One caller owns each collector. It is neither global nor shared across
/// threads; concurrent runs construct their own collectors. Nested operations
/// are supported, and elapsed times are inclusive, not additive partitions.
#[derive(Default)]
pub struct ExecutionMeter {
    costs: RefCell<ExecutionCosts>,
}
impl ExecutionMeter {
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
        {
            let mut costs = self.costs.borrow_mut();
            let cost = costs.get_mut(kind);
            cost.attempted = cost
                .attempted
                .checked_add(1)
                .ok_or(CostError { operation: kind })?;
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
    fn nested_errors_unwinds_and_counter_exhaustion_keep_exact_counts() {
        let meter = ExecutionMeter::default();
        let result: Result<(), CostError> = meter.measure(OperationKind::Query, || {
            assert_eq!(meter.snapshot().query.attempted, 1);
            assert_eq!(meter.snapshot().query.succeeded, 0);
            meter.measure(OperationKind::Query, || Ok(()))?;
            Err(CostError {
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
            Err(CostError {
                operation: OperationKind::Decoder
            })
        );
        assert_eq!(meter.snapshot().decoder.attempted, u64::MAX);
        assert_eq!(meter.snapshot().decoder.failed, 0);
    }
}
