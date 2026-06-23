# Issue 139 APM Sparse Matrix Verifier Design

Scope: GitHub issue #139, reusable APM sparse-row structural verifier for
`qec-code` tests.

## Context

Issues #132, #133, #134, and #138 added the APM Table A1 manifest, P=96
sparse-row fixtures, construction notes, and a crate-private native APM
matrix builder. `qec-code/tests/code.rs` currently has local APM structural
checks that validate fixture dimensions, degrees, orthogonality, and
rank-derived `k`. This issue turns that ad hoc logic into a reusable verifier
with a report shape that later P=96/P=192 tests can share.

The worker is already in an Agent Desk linked worktree on the requested
branch. No additional worktree is needed.

## Approaches Considered

1. Add a production `qec_code::apm_verifier` API.

   This would make the verifier easy to import anywhere, but the issue frames
   it as static test acceptance and explicitly recommends test-visible helpers
   unless production generation needs it.

2. Keep verifier logic only inside `qec-code/src/codes/apm.rs` tests.

   This can access the private native builder, but it would leave later
   integration tests without a reusable helper and would not remove the
   duplicate checks already in `qec-code/tests/code.rs`.

3. Add shared test support under `qec-code/tests/support` and include it from
   both integration tests and the private APM builder unit test.

   This is the selected approach. It keeps public API unchanged, provides a
   reusable helper for later tests, and still lets the required native P=96
   test run against the crate-private builder.

## Chosen Design

Add `qec-code/tests/support/apm_verifier.rs` with a pure sparse-row verifier
that works on `(num_cols, rows)` inputs:

- `ApmSparseMatrixView<'a>` names one sparse matrix view.
- `ApmSparseMatrixReport` records `num_cols`, `num_rows`, row-weight
  min/avg/max, column-weight min/avg/max, rank, and girth status.
- `ApmCssVerifierReport` records `num_cols`, `mx`, `mz`, the two matrix
  reports, `rank_x`, `rank_z`, `k`, and `orthogonal`.
- `ApmCssVerifierExpectations` stores optional expected values for shape,
  weights, `k`, orthogonality, and girth lower bounds.
- `verify_apm_css_matrices(hx, hz, expectations)` validates sparse rows first,
  computes all report fields, checks expectations, and returns either the
  report or a descriptive string error.

The verifier rejects invalid sparse rows before rank or girth work:

- `num_cols == 0`
- any support `>= num_cols`
- duplicate support entries within a row
- `Hx` and `Hz` with different widths

After validation, it computes:

- row and column weight stats
- `rank_x` and `rank_z` via `qec_code::binary::try_binary_rank`
- `k = n - rank_x - rank_z`
- orthogonality through sparse row overlap parity
- Tanner graph girth through deterministic BFS over the bipartite graph for
  each matrix

`GirthStatus` supports exact cycles, acyclic graphs, and "at least" lower
bounds. The APM manifest records `{"kind":"lower_bound","value":6}`, so the
P=96 assertion will require both X and Z girth status to satisfy lower bound
6 without requiring an exact value from the source contract.

## Test Contract

Refactor the existing APM fixture tests in `qec-code/tests/code.rs` to use the
shared support helper instead of local structural-stat logic.

Add the required private-builder test in `qec-code/src/codes/apm.rs`:

- `apm_p96_verifier_reports_paper_stats` builds the native P=96 matrices from
  the #138 crate-private builder and runs the shared verifier.
- It asserts `orthogonal == true`, `n=1152`, `mx=mz=288`, `k=580`, all row
  weights are exactly 12, all column weights are exactly 3, and both X/Z girth
  reports satisfy the manifest lower bound 6.
- It includes in-memory negative controls for duplicate support and
  out-of-range support. Both must be rejected before reporting rank/girth.

Focused verification:

```sh
cargo test -p qec-code apm_p96_verifier_reports_paper_stats -q
```

Full verification:

```sh
cargo test
```

## Out Of Scope

- No decoder benchmarking.
- No public built-in `apm_kasai` code id registration.
- No production verifier API unless later generator or CLI work needs one.
- No P=192 fixture generation in this issue.
