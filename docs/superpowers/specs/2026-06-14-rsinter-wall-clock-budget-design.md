# rsinter Wall-Clock Budget Design

## Context

Issue 48 asks `rsinter` collection and benchmark runs to support a wall-clock stopping rule. Today `CollectionOptions`, `CollectOptions`, and bench runner parameters can stop by shots and logical errors only. `TaskStats.seconds` already records elapsed time, but it is not usable as a stop condition.

AutoQEC needs this so candidates can be compared under a fixed time budget, independent of how many shots each candidate can process.

## Goals

- Add `max_wall_seconds: Option<f64>` to per-task and global collection options.
- Add optional `max_wall_seconds` to bench runner params while keeping `max_shots` and `max_errors` required.
- Stop collection and bench runner loops when shots, errors, or wall-clock time reaches its configured limit.
- Make `TaskStats.seconds` reflect actual wall-clock batch time, including sampling, packing, decoding, and error counting.
- Reject non-positive or non-finite wall-clock budgets with clear errors.
- Preserve behavior for callers that do not configure `max_wall_seconds`, except for the intentional `seconds` timing-scope correction.

## Non-Goals

- Do not change decoder traits or add interruption support inside a decoder call.
- Do not make `max_shots` optional in bench specs.
- Do not change the Python surface-decoder-compare harness in this issue.
- Do not redesign stopping rules into a new abstraction unless needed by implementation details.

## Architecture

Use the current option structs and bench point model.

`rsinter/src/task.rs`:

- Add `max_wall_seconds: Option<f64>` to `CollectionOptions`.

`rsinter/src/collect.rs`:

- Add `max_wall_seconds: Option<f64>` to `CollectOptions`.
- Resolve the effective per-task budget with task options taking precedence over global options.
- Validate configured wall-clock limits before starting collection.
- Use one loop condition helper or small local predicate so shots, errors, and time are checked consistently.

`rsinter/src/bench/registry.rs`:

- Add `max_wall_seconds: Option<f64>` to `BenchCasePoint`.
- Accept optional `max_wall_seconds` in generic runner params.
- Keep `max_shots`, `max_errors`, and `batch_size` required.
- Validate `max_wall_seconds` if present.

`rsinter/src/bench/runners/mod.rs`:

- Stop `run_decoder_point` when `point.max_wall_seconds` is reached.
- Measure batch elapsed wall-clock time across sampling, packing, decoding, and error counting.
- Add `wall_seconds` to benchmark result metrics so consumers can see the elapsed wall-clock time for each point.

No public decoder APIs need to change.

## Data Flow And Semantics

Effective collect budgets are:

```text
effective_max_shots = task.max_shots.or(global.max_shots).unwrap_or(u64::MAX)
effective_max_errors = task.max_errors.or(global.max_errors).unwrap_or(u64::MAX)
effective_max_wall_seconds = task.max_wall_seconds.or(global.max_wall_seconds)
```

For `collect`, `TaskStats.seconds` starts at resumed CSV seconds when resume data exists. That resumed value counts as already-spent time. If a resumed task is already at or beyond `max_wall_seconds`, `collect` returns the existing stats without sampling more shots.

For both collect and bench runner loops:

- Check whether any stop condition is already satisfied before starting a batch.
- Run one whole batch.
- Measure elapsed wall-clock time for the batch.
- Update shots, errors, and elapsed seconds.
- Stop before the next batch if any limit is reached.

The implementation will not interrupt a decoder in the middle of a batch because the existing decoder API accepts and returns whole batches. Users who need tighter time-budget adherence should choose smaller `start_batch_size`, `max_batch_size`, or bench `batch_size`.

## Error Handling

Any configured `max_wall_seconds` must be finite and greater than zero.

Invalid examples:

- `max_wall_seconds = 0`
- `max_wall_seconds < 0`
- `NaN`
- positive or negative infinity

Errors should name the invalid field clearly, for example `max_wall_seconds must be positive`.

Missing `max_wall_seconds` means no wall-clock limit. Missing `max_shots` in bench specs remains an error.

## Compatibility

Existing collect callers must add `max_wall_seconds: None` where they construct `CollectionOptions` or `CollectOptions` directly. Existing bench specs continue to parse and run unchanged.

The only intended behavior change without a configured wall-clock budget is that `collect` seconds will include the full batch wall-clock work instead of only the sampler call. This is required for a truthful wall-clock budget.

## Testing

Add focused tests in the owning crate.

`rsinter/tests/collect.rs`:

- `collect_respects_wall_clock`: use a slow test decoder, configure `max_wall_seconds`, keep the shot cap high, and assert elapsed seconds is inside a tolerant range, `shots > 0`, and shots stay well below the cap.
- `collect_rejects_non_positive_wall_clock`: assert zero or negative limits fail with a clear error.

`rsinter/tests/bench_registry.rs`:

- Confirm optional `max_wall_seconds` parses into `BenchCasePoint`.
- Confirm zero or negative `max_wall_seconds` is rejected.
- Confirm `max_shots` stays required.

`rsinter/src/bench/runners/mod.rs`:

- Add a focused runner-loop test showing a wall-clock budget stops before the shot cap when a slow decoder is used, and assert the result includes `wall_seconds`.

Run narrow checks first:

```text
cargo test -p rsinter collect_respects_wall_clock
cargo test -p rsinter bench_registry
```

Then run:

```text
cargo test -p rsinter
```
