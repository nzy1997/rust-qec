# Issue 101 Exact CSS Distance Backend Design

Date: 2026-06-17
Status: Design approved in-session, written for review
Scope: GitHub issue #101, stable exact CSS distance backend for AutoQEC

## Summary

Issue #101 exposes a stable exact CSS distance backend from `qec-code` so
AutoQEC can enable its guarded `rstim-ilp-exact` distance registry entry for
the first bivariate-bicycle qLDPC campaign.

The chosen design adds a new exact method under the existing
`qec-code code css-distance` command family:

```text
qec-code code css-distance exact --code-id bb72 --json
qec-code code css-distance exact --code-id surface_rotated:d=5 --json
qec-code code css-distance exact --hx hx.json --hz hz.json --json
```

The command accepts either built-in CSS specs or external `sparse_rows` matrix
files, constructs a `CssCode`, runs the existing exact `compute_distance`
backend, and emits a completed JSON result with `method: "rstim-ilp-exact"` and
`bound_type: "exact"`.

This keeps randomized upper-bound search separate. The existing
`randomized-upper-bound` command continues to emit `bound_type: "upper"` and is
not reused by the exact backend.

## Goals

This milestone should:

1. Add a stable machine-readable exact CSS distance CLI command.
2. Support built-in CSS code specs, including `bb72` and
   `surface_rotated:d=<distance>`.
3. Support external `hx` and `hz` files using the existing `sparse_rows` JSON
   wrapper.
4. Reject malformed matrices and non-commuting CSS inputs before solving.
5. Return exact distances for rotated-surface fixtures `d = 3, 5, 7`.
6. Return exact distance `6` for the built-in bivariate-bicycle `bb72`
   `[[72,12,6]]` code when the ILP feature is enabled.
7. Emit enough method, options, witness, and provenance metadata for AutoQEC to
   record a reproducible exact distance payload.
8. Preserve the safety boundary between exact results and randomized upper
   bounds.

## Non-Goals

This milestone should not:

1. Add randomized or heuristic distance search.
2. Add certified lower bounds beyond exact distance.
3. Add non-CSS distance input to this CLI path.
4. Add direct support for AutoQEC's `dense_binary_matrix` artifact format inside
   `qec-code`.
5. Change AutoQEC's registry implementation.
6. Change the existing `code steane distance` human-readable command.

AutoQEC already has a dense-to-sparse conversion helper in its guarded
`rstim-ilp-exact` path. `qec-code` only needs to stabilize the sparse CSS exact
backend for this issue.

## Current State

The required pieces mostly exist, but they are not exposed as a stable exact
CSS distance command:

- `qec-code::css::CssCode::from_hx_hz(...)` validates dense CSS checks,
  rejects non-orthogonal X/Z checks, and accepts redundant orthogonal parity
  checks by selecting an independent stabilizer basis.
- `qec-code::css::SparseRowsMatrix` parses and serializes the existing
  `sparse_rows` JSON wrapper.
- `qec-code::distance::compute_distance(...)` is the exact distance API. With
  `distance-ilp-highs`, it uses the ILP lowering and `qec-ilp-core`; without
  that feature, it keeps the small exhaustive fallback and reports an explicit
  unsupported-configuration error for larger codes.
- `qec-code code css-distance randomized-upper-bound` already supports
  `--code-id` and `--hx/--hz` inputs, but emits an upper-bound result.
- Built-in CSS registry support for `bb72`, `surface_rotated:d=<distance>`,
  `toric:d=<distance>`, and repetition families is already present.
- PR #86 has landed, so rotated-surface CLI sparse-row fixtures are no longer
  an unmerged dependency.
- AutoQEC's `rstim-ilp-exact` registry entry currently converts its
  `dense_binary_matrix` `hx` and `hz` artifacts into `sparse_rows`, then fails
  clearly because no stable exact `qec-code` command exists yet.

The gap is a command and result contract that lets AutoQEC call the exact
backend instead of the guarded unavailable path.

## Alternatives Considered

### 1. Minimal stable exact CLI

Add `qec-code code css-distance exact` with the same input modes as
`randomized-upper-bound`: either `--code-id` or `--hx/--hz`, requiring `--json`.
Only `sparse_rows` matrix files are accepted.

Benefits:

- smallest implementation that satisfies issue #101
- matches AutoQEC's existing guarded registry shape
- reuses existing CSS parsing, built-in registry, and exact distance API
- avoids importing AutoQEC artifact schema into `qec-code`
- keeps exact and upper-bound result types separate

Costs:

- external dense matrix payloads must be converted before calling `qec-code`

This is the chosen approach.

### 2. Wider input compatibility

Teach `qec-code` to parse both `sparse_rows` and AutoQEC's
`dense_binary_matrix` wrapper in the exact CLI.

Benefits:

- slightly simpler AutoQEC-side command invocation
- potentially friendlier standalone CLI behavior

Costs:

- leaks AutoQEC-specific artifact schema into `qec-code`
- requires more parser and error-surface tests
- broadens issue #101 beyond the needed backend contract

This can be added later if a direct dense input consumer appears.

### 3. Public library API first

Add a public `compute_exact_css_distance(...)` library API and make the CLI a
thin wrapper around it.

Benefits:

- clean long-term API boundary for library consumers
- reusable result construction outside the CLI

Costs:

- commits to a public Rust API before there is a non-CLI caller
- larger design surface than AutoQEC needs for this milestone

The implementation should still keep result construction factored and testable,
but it does not need to promise a broad public API in v1.

## Decision

Add a new `Exact` subcommand to `CssDistanceCommands`:

```rust
pub enum CssDistanceCommands {
    Exact(ExactCssDistanceCli),
    RandomizedUpperBound(RandomizedUpperBoundCli),
}
```

`ExactCssDistanceCli` should mirror the existing randomized input shape where
that makes sense:

```rust
pub struct ExactCssDistanceCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
```

The exact command should require `--json` and support exactly one input mode:

- `--code-id <built-in-css-spec>`
- `--hx <path> --hz <path>`

The file input mode accepts only the existing sparse-row JSON wrapper:

```json
{
  "format": "sparse_rows",
  "num_cols": 7,
  "rows": [[0, 3, 5, 6]]
}
```

AutoQEC can keep converting `dense_binary_matrix` artifacts to this wrapper
before invoking `qec-code`.

## Result Contract

Successful exact runs write one JSON object to stdout and nothing to stderr.
The binary appends its normal trailing newline.

The JSON shape should be:

```json
{
  "status": "completed",
  "distance": 6,
  "method": "rstim-ilp-exact",
  "bound_type": "exact",
  "logical_class": "x_like",
  "witness": {
    "x": [1, 1, 1, 1, 1, 1],
    "z": [0, 0, 0, 0, 0, 0],
    "weight": 6
  },
  "options": {
    "input": "code_id",
    "code_id": "bb72"
  },
  "provenance": {
    "tool": "qec-code",
    "tool_version": "0.1.0",
    "method_revision": 1
  }
}
```

For `--hx/--hz` input, `options` should record:

```json
{
  "input": "files",
  "hx": "...",
  "hz": "..."
}
```

The path strings can be the CLI-provided display strings. They are provenance,
not canonical identity.

Recommended Rust types:

```rust
pub enum ExactCssDistanceMethod {
    RstimIlpExact,
}

pub enum ExactDistanceBoundType {
    Exact,
}

pub enum ExactCssDistanceStatus {
    Completed,
}

pub enum ExactCssDistanceInput {
    CodeId { code_id: String },
    Files { hx: String, hz: String },
}

pub struct ExactCssDistanceOptions {
    pub input: ExactCssDistanceInput,
}

pub struct ExactCssDistanceProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

pub struct ExactCssDistanceResult {
    pub status: ExactCssDistanceStatus,
    pub distance: usize,
    pub method: ExactCssDistanceMethod,
    pub bound_type: ExactDistanceBoundType,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: ExactCssDistanceOptions,
    pub provenance: ExactCssDistanceProvenance,
}
```

These names can be adjusted to match local style. The important contract is the
serialized JSON semantics.

## Data Flow

Built-in input:

```text
qec-code code css-distance exact --code-id bb72 --json
  -> validate --json and input mode
  -> built_in_css_checks("bb72")
  -> SparseRowsMatrix::new(...).to_dense_rows()
  -> CssCode::from_hx_hz(...)
  -> compute_distance(css.code())
  -> ExactCssDistanceResult::completed(...)
  -> serde_json::to_string(...)
```

File input:

```text
qec-code code css-distance exact --hx hx.json --hz hz.json --json
  -> validate --json and input mode
  -> read_css_sparse_rows_matrix(hx)
  -> read_css_sparse_rows_matrix(hz)
  -> check width match
  -> CssCode::from_hx_hz(...)
  -> compute_distance(css.code())
  -> ExactCssDistanceResult::completed(...)
  -> serde_json::to_string(...)
```

`compute_distance` remains the exact solver boundary. If the binary is built
with `distance-ilp-highs`, the command can solve larger CSS codes through ILP.
If the binary is built without that feature, the command still works for small
codes that the exhaustive fallback can handle and fails clearly for unsupported
larger codes.

## Error Handling

The exact command should follow the existing CLI convention: on errors, stdout
is empty, stderr contains a clear message, and the process exits nonzero.

Expected failures:

- missing `--json`: `JSON output is required for code css-distance exact`
- no input source: `provide --code-id or both --hx and --hz`
- both input modes: `use either --code-id or --hx/--hz, not both`
- one matrix missing: `--hx and --hz must be provided together`
- failed file read: existing `CssMatrixReadFailed`
- malformed JSON: existing `InvalidCssMatrixJson`
- missing `format`: existing `MissingCssMatrixFormat`
- unsupported matrix format: existing `UnsupportedCssMatrixFormat`
- sparse support out of range or duplicated: existing sparse-row validation
- `hx`/`hz` width mismatch: existing `InvalidCssDistanceInput` style
- non-orthogonal CSS checks: existing `InvalidCssOrthogonality`
- zero-logical-qubit code: existing `DistanceWitnessNotFound`
- exact distance unsupported without ILP: existing
  `DistanceComputationUnsupported`
- ILP backend unavailable or failed: existing ILP errors

No failure path should emit a completed exact JSON payload.

## Testing

Add focused tests at three layers.

### Result contract tests

In a new exact-distance result test or an existing distance-related test file:

1. Completed exact result serializes with:
   - `status: "completed"`
   - `method: "rstim-ilp-exact"`
   - `bound_type: "exact"`
   - positive `distance`
   - `distance == witness.weight`
   - `provenance.tool == "qec-code"`
   - `provenance.method_revision == 1`
2. File-input options serialize as `input: "files"` and preserve `hx`/`hz`.
3. Code-id options serialize as `input: "code_id"` and preserve `code_id`.

### CLI tests

Add binary and in-process coverage in `qec-code/tests/cli.rs`:

1. `code css-distance exact --code-id steane --json` returns distance `3`.
2. `code css-distance exact --hx steane_hx.json --hz steane_hz.json --json`
   returns distance `3`.
3. `--json` is required.
4. `--code-id` cannot be combined with `--hx/--hz`.
5. `--hx` and `--hz` must be provided together.
6. mismatched matrix widths fail with empty stdout.
7. non-commuting CSS matrices fail before solving with empty stdout.
8. `randomized-upper-bound` CLI output remains unchanged and still uses
   `bound_type: "upper"`.

### ILP feature acceptance tests

Under `#[cfg(feature = "distance-ilp-highs")]`, add exact backend coverage for
known fixtures:

1. `surface_rotated:d=3` returns distance `3`.
2. `surface_rotated:d=5` returns distance `5`.
3. `surface_rotated:d=7` returns distance `7`.
4. `bb72` returns distance `6`.

The `bb72` exact test may be heavier than normal CLI smoke tests. Keep it
feature-gated and focused so default `cargo test -p qec-code` remains
reasonable.

Recommended verification commands:

```bash
cargo test -p qec-code --test cli code_css_distance_exact_
cargo test -p qec-code --features distance-ilp-highs --test logical_distance
cargo test -p qec-code --features distance-ilp-highs --test cli code_css_distance_exact_
cargo fmt --check --package qec-code
```

The final implementation can widen to `cargo test -p qec-code` and, if runtime
is acceptable, `cargo test --workspace`.

## AutoQEC Integration Boundary

This issue only changes `rstim` / `qec-code`.

After this command exists, AutoQEC can update its guarded `rstim-ilp-exact`
entry to:

1. convert `dense_binary_matrix` artifacts to temporary `sparse_rows` files
2. run:

   ```text
   qec-code code css-distance exact --hx <hx.sparse.json> --hz <hz.sparse.json> --json
   ```

3. accept only `bound_type: "exact"` and `method: "rstim-ilp-exact"`
4. reject any upper-bound or heuristic result from this method path

No AutoQEC-side code changes are required in the `rstim` implementation branch,
but this design intentionally matches AutoQEC's current guarded registry so the
follow-up is small.

## Open Implementation Notes

- The exact result type can reuse `DistanceBoundWitness` to avoid duplicating
  witness JSON shape. That reuse is acceptable because the result type itself
  and `bound_type` are exact-specific.
- The existing helper that reads sparse-row files is currently named around the
  randomized command. It can be generalized for both exact and randomized
  paths.
- If `compute_distance` returns a valid exact result through the small
  exhaustive fallback, the method label should still be `rstim-ilp-exact` for
  AutoQEC compatibility only when the exact backend command is intended to
  represent that registry entry. If this is uncomfortable during
  implementation, add a provenance field such as `solver_feature:
  "distance-ilp-highs"` or `solver_path: "exhaustive"` while keeping
  `bound_type: "exact"`. The AutoQEC acceptance criteria care about exactness
  and reproducibility; the method string should stay stable.
