//! Call-local progress observations of already committed draws.
use super::*;
use crate::runtime::HistorySampler;
use crate::statistics::StatisticsError;
use std::num::NonZeroU64;

#[derive(Clone, Debug, PartialEq)]
pub struct BatchProgress {
    pub completed: u64,
    pub requested: u64,
    pub history_sampler: HistorySampler,
    /// An unavailable diagnostic is represented by its status; arithmetic
    /// failure remains an error value and does not itself change the batch.
    pub efficiency: Result<EfficiencySummary, StatisticsError>,
    pub execution_costs: ExecutionCosts,
    /// Committed sampling work so far, excluding diagnostics and observers.
    pub sampling_seconds: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressReceipt {
    pub every: NonZeroU64,
    pub attempted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub diagnostic_failures: u64,
    pub last_completed: u64,
    /// Prefix efficiency diagnostics, notifications and their owned cleanup.
    pub seconds: f64,
}
impl ProgressReceipt {
    fn new(every: NonZeroU64) -> Self {
        Self {
            every,
            attempted: 0,
            succeeded: 0,
            failed: 0,
            diagnostic_failures: 0,
            last_completed: 0,
            seconds: 0.0,
        }
    }
}
/// One borrowed observer and its most recent run receipt. No event list or RNG
/// is retained. Reusing the observer resets its receipt before run validation.
pub struct ProgressObserver<'a> {
    callback: &'a mut dyn FnMut(&BatchProgress) -> Result<(), TaskError>,
    receipt: ProgressReceipt,
}
impl<'a> ProgressObserver<'a> {
    pub fn new(
        every: NonZeroU64,
        callback: &'a mut dyn FnMut(&BatchProgress) -> Result<(), TaskError>,
    ) -> Self {
        Self {
            callback,
            receipt: ProgressReceipt::new(every),
        }
    }
    pub fn receipt(&self) -> ProgressReceipt {
        self.receipt
    }
    pub(super) fn reset(&mut self) {
        self.receipt = ProgressReceipt::new(self.receipt.every);
    }
    pub(super) fn due(&self, completed: u64, requested: u64) -> bool {
        completed.is_multiple_of(self.receipt.every.get()) || completed == requested
    }
    pub(super) fn add_seconds(&mut self, seconds: f64) {
        self.receipt.seconds += seconds;
    }
    pub(super) fn notify(
        &mut self,
        accumulator: &Accumulator,
        options: &TaskRunOptions,
        sampler: HistorySampler,
        meter: &ExecutionMeter,
        sampling_seconds: f64,
    ) -> Result<(), TaskError> {
        let progress = BatchProgress {
            completed: accumulator.statistics.count(),
            requested: options.shots,
            history_sampler: sampler,
            efficiency: accumulator.efficiency.summarize(),
            execution_costs: meter.snapshot(),
            sampling_seconds,
        };
        self.receipt.attempted += 1;
        self.receipt.last_completed = progress.completed;
        if progress.efficiency.is_err() {
            self.receipt.diagnostic_failures += 1;
        }
        let outcome = (self.callback)(&progress);
        if outcome.is_ok() {
            self.receipt.succeeded += 1;
        } else {
            self.receipt.failed += 1;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_prefix_diagnostic_error_is_delivered_without_poisoning_zero_weight_accumulation() {
        let options = StatisticsOptions {
            arithmetic: FieldArithmetic::new(1),
            ..Default::default()
        };
        let mut accumulator = Accumulator::new(2, true, options).unwrap();
        let zero = ExactField::zero();
        let one = ExactField::one();
        for _ in 0..2 {
            accumulator
                .statistics
                .push(&[zero.clone(), zero.clone()], &zero)
                .unwrap();
            accumulator
                .efficiency
                .push(&zero, Some(&zero), &zero, &one)
                .unwrap();
        }
        assert!(accumulator.efficiency.summarize().is_err());
        let mut delivered = 0;
        let mut callback = |progress: &BatchProgress| {
            delivered += 1;
            assert_eq!(progress.completed, 2);
            assert!(progress.efficiency.is_err());
            assert_eq!(progress.execution_costs, ExecutionCosts::default());
            Ok(())
        };
        let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
        observer
            .notify(
                &accumulator,
                &TaskRunOptions {
                    shots: 3,
                    ..Default::default()
                },
                HistorySampler::AffineSupport,
                &ExecutionMeter::default(),
                0.0,
            )
            .unwrap();
        let receipt = observer.receipt();
        assert_eq!(receipt.attempted, 1);
        assert_eq!(receipt.succeeded, 1);
        assert_eq!(receipt.failed, 0);
        assert_eq!(receipt.diagnostic_failures, 1);
        assert_eq!(accumulator.statistics.count(), 2);
        assert_eq!(accumulator.efficiency.count(), 2);
        accumulator
            .statistics
            .push(&[zero.clone(), zero.clone()], &zero)
            .unwrap();
        accumulator
            .efficiency
            .push(&zero, Some(&zero), &zero, &one)
            .unwrap();
        assert_eq!(accumulator.statistics.count(), 3);
        assert_eq!(accumulator.efficiency.count(), 3);
        assert_eq!(delivered, 1);
    }
}
