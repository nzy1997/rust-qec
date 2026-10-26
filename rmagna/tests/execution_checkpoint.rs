use rmagna::costs::{
    CostError, ExecutionCosts, ExecutionLimits, ExecutionMeter, ExecutionMeterCheckpoint,
    OperationCost, OperationKind,
};
use std::time::Duration;
fn checkpoint() -> ExecutionMeterCheckpoint {
    ExecutionMeterCheckpoint {
        limits: ExecutionLimits::default(),
        costs: ExecutionCosts::default(),
        elapsed: Duration::ZERO,
    }
}
fn set(checkpoint: &mut ExecutionMeterCheckpoint, kind: OperationKind, cost: OperationCost) {
    match kind {
        OperationKind::Compilation => checkpoint.costs.compilation = cost,
        OperationKind::Query => checkpoint.costs.query = cost,
        OperationKind::Decoder => checkpoint.costs.decoder = cost,
        OperationKind::Support => checkpoint.costs.support = cost,
    }
}
fn counts(costs: ExecutionCosts) -> [(u64, u64, u64); 4] {
    [costs.compilation, costs.query, costs.decoder, costs.support]
        .map(|c| (c.attempted, c.succeeded, c.failed))
}
#[test]
fn restored_meters_preserve_completed_nested_failed_and_unwound_operations() {
    let meter = ExecutionMeter::new(ExecutionLimits {
        max_queries: 10,
        max_duration: None,
    });
    let _: Result<(), CostError> = meter.measure(OperationKind::Compilation, || {
        meter.measure(OperationKind::Query, || Ok(()))
    });
    let result: Result<(), CostError> = meter.measure(OperationKind::Decoder, || {
        Err(CostError::CounterOverflow {
            operation: OperationKind::Support,
        })
    });
    assert!(result.is_err());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), CostError> = meter.measure(OperationKind::Support, || {
                panic!("deliberate checkpoint unwind")
            });
        }))
        .is_err()
    );
    let saved = meter.checkpoint().unwrap();
    assert_eq!(
        counts(saved.costs),
        [(1, 1, 0), (1, 1, 0), (1, 0, 1), (1, 0, 1)]
    );
    let restored = ExecutionMeter::from_checkpoint(saved).unwrap();
    assert_eq!(restored.snapshot(), saved.costs);
    assert_eq!(restored.limits(), saved.limits);
    assert!(restored.elapsed() >= saved.elapsed);
    let result: Result<(), CostError> = restored.measure(OperationKind::Query, || {
        restored.measure(OperationKind::Support, || Ok(()))
    });
    assert_eq!(result, Ok(()));
    let after = restored.checkpoint().unwrap();
    assert_eq!(
        counts(after.costs),
        [(1, 1, 0), (2, 2, 0), (1, 0, 1), (2, 1, 1)]
    );
    assert!(after.elapsed >= saved.elapsed);
    for (old, new) in [
        saved.costs.compilation,
        saved.costs.query,
        saved.costs.decoder,
        saved.costs.support,
    ]
    .into_iter()
    .zip([
        after.costs.compilation,
        after.costs.query,
        after.costs.decoder,
        after.costs.support,
    ]) {
        assert!(new.seconds >= old.seconds);
    }
    assert_eq!(meter.snapshot(), saved.costs);
}
#[test]
fn query_and_exact_deadline_budgets_remain_spent_after_restoration() {
    let meter = ExecutionMeter::new(ExecutionLimits {
        max_queries: 3,
        max_duration: None,
    });
    meter
        .measure::<_, CostError>(OperationKind::Query, || Ok(()))
        .unwrap();
    assert!(
        meter
            .measure::<(), CostError>(OperationKind::Query, || Err(CostError::CounterOverflow {
                operation: OperationKind::Decoder
            }))
            .is_err()
    );
    let restored = ExecutionMeter::from_checkpoint(meter.checkpoint().unwrap()).unwrap();
    restored
        .measure::<_, CostError>(OperationKind::Query, || Ok(()))
        .unwrap();
    let before = restored.snapshot();
    let mut calls = 0;
    let result: Result<(), CostError> = restored.measure(OperationKind::Query, || {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err(CostError::QueryLimit { limit: 3 }));
    assert_eq!(calls, 0);
    assert_eq!(restored.snapshot(), before);
    for limit in [Duration::ZERO, Duration::from_nanos(7)] {
        let saved = ExecutionMeterCheckpoint {
            limits: ExecutionLimits {
                max_queries: 19,
                max_duration: Some(limit),
            },
            elapsed: limit,
            ..checkpoint()
        };
        let resumed = ExecutionMeter::from_checkpoint(saved).unwrap();
        assert_eq!(resumed.check_time(), Err(CostError::TimeLimit { limit }));
        let result: Result<(), CostError> = resumed.measure(OperationKind::Support, || {
            calls += 1;
            Ok(())
        });
        assert_eq!(result, Err(CostError::TimeLimit { limit }));
        assert_eq!(calls, 0);
        assert_eq!(resumed.snapshot(), saved.costs);
    }
    let exact = Duration::new(123, 987654321);
    let resumed = ExecutionMeter::from_checkpoint(ExecutionMeterCheckpoint {
        elapsed: exact,
        ..checkpoint()
    })
    .unwrap();
    assert!(resumed.elapsed() >= exact);
    assert!(resumed.checkpoint().unwrap().elapsed >= exact);
}
#[test]
fn active_measurements_and_invalid_counter_or_time_states_cannot_be_restored() {
    let meter = ExecutionMeter::default();
    meter
        .measure::<_, CostError>(OperationKind::Query, || {
            assert!(matches!(
                meter.checkpoint(),
                Err(CostError::InvalidCheckpoint {
                    operation: OperationKind::Query,
                    ..
                })
            ));
            meter.measure::<_, CostError>(OperationKind::Support, || Ok(()))?;
            assert!(meter.checkpoint().is_err());
            Ok(())
        })
        .unwrap();
    assert!(meter.checkpoint().is_ok());
    for kind in [
        OperationKind::Compilation,
        OperationKind::Query,
        OperationKind::Decoder,
        OperationKind::Support,
    ] {
        for bad in [
            OperationCost {
                attempted: 1,
                ..Default::default()
            },
            OperationCost {
                succeeded: 1,
                ..Default::default()
            },
            OperationCost {
                attempted: u64::MAX,
                succeeded: u64::MAX,
                failed: 1,
                seconds: 0.0,
            },
            OperationCost {
                attempted: 1,
                succeeded: 1,
                seconds: -0.1,
                ..Default::default()
            },
            OperationCost {
                attempted: 1,
                succeeded: 1,
                seconds: f64::INFINITY,
                ..Default::default()
            },
            OperationCost {
                attempted: 1,
                succeeded: 1,
                seconds: f64::NAN,
                ..Default::default()
            },
            OperationCost {
                seconds: 0.1,
                ..Default::default()
            },
        ] {
            let mut saved = checkpoint();
            set(&mut saved, kind, bad);
            let error = ExecutionMeter::from_checkpoint(saved).err().unwrap();
            assert!(matches!(error,CostError::InvalidCheckpoint{operation,..} if operation==kind));
            assert!(error.to_string().contains("checkpoint"));
        }
    }
    let mut over = checkpoint();
    over.limits.max_queries = 1;
    over.costs.query = OperationCost {
        attempted: 2,
        succeeded: 2,
        ..Default::default()
    };
    assert!(matches!(
        ExecutionMeter::from_checkpoint(over),
        Err(CostError::InvalidCheckpoint {
            operation: OperationKind::Query,
            ..
        })
    ));
}
#[test]
fn restored_counter_exhaustion_and_extreme_elapsed_offsets_do_not_start_actions() {
    let mut saved = checkpoint();
    saved.costs.support = OperationCost {
        attempted: u64::MAX,
        succeeded: u64::MAX,
        ..Default::default()
    };
    let restored = ExecutionMeter::from_checkpoint(saved).unwrap();
    let result: Result<(), CostError> = restored.measure(OperationKind::Support, || {
        panic!("counter overflow must reject before action")
    });
    assert_eq!(
        result,
        Err(CostError::CounterOverflow {
            operation: OperationKind::Support
        })
    );
    assert_eq!(restored.snapshot(), saved.costs);
    let saved = ExecutionMeterCheckpoint {
        limits: ExecutionLimits {
            max_queries: u64::MAX,
            max_duration: Some(Duration::MAX),
        },
        elapsed: Duration::MAX,
        ..checkpoint()
    };
    let meter = ExecutionMeter::from_checkpoint(saved).unwrap();
    assert_eq!(meter.elapsed(), Duration::MAX);
    assert_eq!(
        meter.check_time(),
        Err(CostError::TimeLimit {
            limit: Duration::MAX
        })
    );
    assert_eq!(meter.checkpoint().unwrap().elapsed, Duration::MAX);
}
