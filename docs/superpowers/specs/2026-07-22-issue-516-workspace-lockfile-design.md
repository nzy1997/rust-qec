# Issue 516 Workspace Lockfile Design

Issue: #516

## Context

The root Cargo workspace currently ignores `Cargo.lock`, so a clean checkout can
resolve newer compatible dependency versions before building executable crates
and benchmark runners. The workspace includes reusable library crates plus
applications such as `rsinter`; tracking the workspace lockfile fixes this
repository's build graph without changing dependency requirements for downstream
library users.

No repository-specific `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` file was present
in this worktree. Issue #516 is open with no comments, and no pull request for
the worker branch existed before implementation.

## Approaches Considered

1. Commit the root workspace lockfile, stop ignoring it, and add `--locked` to
   every top-level Makefile and CI Cargo build/check/test/run/llvm-cov command.
2. Commit the lockfile and enforce `cargo metadata --locked` only in CI before
   unlocked commands. This would catch many manifest/lockfile mismatches, but
   developer and benchmark entry points could still run with commands that
   rewrite the lockfile.
3. Commit only the lockfile and rely on reviewers to notice rewrites. This is
   the smallest patch, but it does not make normal build paths fail when the
   manifest changes without the lockfile.

The selected approach is option 1. It is the smallest change that makes both
local Makefile entry points and CI fail instead of silently resolving a new
dependency graph.

## Behavior

Generate a root `Cargo.lock` from the current workspace manifests without
changing any dependency requirements. Remove only the root `Cargo.lock` ignore
rule from `.gitignore`, leaving nested or generated lockfile behavior
unchanged.

Update the top-level `Makefile` so all real Cargo commands covered by issue
#516 use locked resolution:

- workspace `test` and `check`;
- benchmark `cargo run` calls for `rsinter`;
- benchmark release/debug `cargo build` calls for `rsinter` and `qec-code`;
- release-path workspace `cargo check`.

Update GitHub Actions workflows so pull requests and scheduled checks use
locked resolution for Cargo invocations:

- CI workspace tests;
- perf-gate `cargo run`;
- coverage `cargo llvm-cov`;
- rbposd parity tests.

Workflows and Make targets that do not invoke Cargo directly stay unchanged.

## Testing

Run the issue's clean-checkout verification:

```sh
test -f Cargo.lock
cargo metadata --locked --format-version 1 >/dev/null
cargo build --locked -p rsinter
cargo test --locked --workspace
git status --short
```

Run the requested repository gate:

```sh
cargo test
```

Review executable Cargo entry points:

```sh
rg -n 'cargo (build|check|test|run|llvm-cov)' Makefile .github/workflows
```

Run the negative control by archiving `HEAD`, removing only that copy's
`Cargo.lock`, and confirming `cargo metadata --locked --format-version 1` fails.

## Scope Limits

- Do not change dependency version requirements.
- Do not pin or change the Rust toolchain.
- Do not change decoder, simulator, benchmark, or release behavior beyond Cargo
  lockfile enforcement.
- Do not add per-crate lockfiles.

## Self Review

- The design directly satisfies the requested root lockfile tracking and locked
  build enforcement.
- The selected approach covers normal developer paths and CI Cargo entry points.
- Downstream library consumers remain unaffected because no dependency
  requirements change.
- Negative-control verification proves locked resolution cannot recreate a
  missing workspace lockfile.
