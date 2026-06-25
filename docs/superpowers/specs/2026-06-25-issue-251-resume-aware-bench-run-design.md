# Issue 251 Resume-Aware Bench Run Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter bench run` artifact-directory output

## Summary

`rsinter bench run` currently removes every matching runner's `test-run`
artifact directory before running. That is simple, but it makes interrupted
long runs expensive because users must manually trim benchmark specs or repeat
completed rows.

This change adds an opt-in `--resume` flag. In resume mode, the runner reads
existing per-runner `test-run/results.jsonl` files before any cleanup, matches
rows by the stable row identity from issue #250, skips identities that already
have a complete `ok` row, reruns missing or incomplete identities, and writes a
merged final `results.jsonl` through the existing temporary staging directory.

## Goals

- Add `--resume` to `rsinter bench run` while preserving the current `--out`
  artifact-directory model.
- Load existing `<out>/<runner>/test-run/results.jsonl` before scheduling work
  when resume is enabled.
- Use `BenchmarkResultRow::identity()` to match existing rows to planned rows.
- Preserve completed rows without duplicating them.
- Treat partially completed rows deterministically: keep them staged, rerun the
  planned row, and merge with issue #250 merge semantics.
- Avoid deleting existing `test-run` artifacts before resume data has been read
  and staged.
- Continue writing through `test-run.tmp` and rename it into `test-run` only
  after a valid manifest and merged JSONL have been produced.
- Fail clearly on corrupted existing JSONL without overwriting the original
  artifact directory.
- Document the behavior in CLI help and the benchmark README.

## Non-Goals

- Do not add a separate single-file `--output` mode.
- Do not add distributed scheduling, cross-machine locking, or automatic point
  selection.
- Do not continue counters inside an in-progress sampler. The existing runner
  API returns whole rows, so incomplete existing rows are rerun and merged.
- Do not change benchmark plot, merge, or runner semantics outside the resume
  path.

## Current State

`run_rust_benchmark` validates the spec, creates the output root, clears each
matching runner's `test-run` and `test-run.tmp` directories, plans all runner
points, executes every point, writes `run_manifest.json` and `results.jsonl`
into `test-run.tmp`, then renames `test-run.tmp` to `test-run`.

`BenchmarkResultRow::identity()` is now available from issue #250. It provides a
stable task identity derived from benchmark, runner, language, params, and
stable case summary fields. `merge_result_rows` can merge compatible rows with
the same identity and fail on incompatible duplicates.

## Design

### CLI

Add a boolean `resume` field to the `BenchCommands::Run` clap variant:

```rust
#[arg(long, help = "Resume from existing per-runner test-run/results.jsonl rows under --out")]
resume: bool,
```

Pass the flag to a new `run_rust_benchmark_with_options` entry point. Keep the
existing `run_rust_benchmark` function as a wrapper that uses
`BenchRunOptions { resume: false }` so existing tests and callers continue to
compile without churn.

### Planning and Completed Identity Detection

Resume needs the identity before running a point. Add a helper that builds a
preview row using the runner implementation and a `BenchRunContext`, then calls
`identity()` on that preview row. This is acceptable for the current runners
because point execution is deterministic for a fixed point and seed; it avoids
creating a second identity model that could drift from actual row generation.

A row is considered complete when `status == "ok"`. Existing `error` rows and
same-identity partial rows are not skipped. They are carried into the staging
set, rerun, and passed to `merge_result_rows` with the fresh row. If merge
detects incompatible state, the run fails before replacing the original
artifact directory.

### Artifact Safety

In non-resume mode, keep the existing cleanup behavior.

In resume mode:

1. Read and parse every existing runner `test-run/results.jsonl` before
   removing any artifact directory.
2. If any existing JSONL is malformed, return an error that names the path and
   leave both `test-run` and any stale `test-run.tmp` untouched.
3. Remove stale `test-run.tmp` after resume data has been loaded.
4. Execute only planned points whose identities are not completed.
5. Merge existing staged rows and fresh rows with `merge_result_rows`.
6. Write manifest and merged JSONL into `test-run.tmp`.
7. Rename the old `test-run` away only after staging succeeds, then rename
   `test-run.tmp` to `test-run`.

This preserves append safety through the existing write-to-temp-and-rename
model and avoids corrupting existing results on read or merge failures.

### Documentation

Update `benchmarks/surface_decoder_compare/README.md` to show `--resume` with
the artifact-directory flow and state the skip/rerun behavior.

### Testing

Add the requested regression test to `rsinter/tests/bench_run.rs`. The test will
use a two-point `predict-zero` benchmark so it runs quickly and deterministically.
It will:

- Create fixture output by running both points once.
- Keep only one completed row in the existing `results.jsonl`.
- Run again with resume and verify the kept row is not duplicated while the
  missing row is added.
- Run the same resume command a second time and verify row count and identities
  stay stable.
- Change a decoder parameter and verify the changed identity runs as a new row.
- Write malformed JSONL and verify resume fails while preserving the corrupt
  file.

## Alternatives Considered

### 1. Continue partially completed counters in place

Rejected for this issue. The current runner API returns a whole
`BenchmarkResultRow` per point and does not expose sampler continuation state.
Rerun-and-merge is deterministic, much smaller, and uses issue #250 semantics.

### 2. Append missing rows to the existing JSONL

Rejected because an interrupted append can leave a partial line and because it
does not naturally deduplicate existing rows. Writing a full staged JSONL and
renaming it preserves the existing safety model.

### 3. Delete artifacts first, then load resume data from a copied backup

Rejected because it adds unnecessary data movement and failure modes. Reading
the existing JSONL before cleanup is simpler and directly satisfies the issue.

## Verification

Run the focused requested regression test:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_resumes_partial_results
```

Then run broader verification:

```bash
cargo test
```
