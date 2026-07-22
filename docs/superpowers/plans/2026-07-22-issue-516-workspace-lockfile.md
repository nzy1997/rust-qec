# Issue 516 Workspace Lockfile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Commit the root workspace `Cargo.lock` and make normal developer and CI Cargo entry points fail when the lockfile is missing or stale.

**Architecture:** This is a repository configuration change. The root lockfile becomes a tracked workspace artifact, and existing Cargo commands in the top-level `Makefile` and GitHub Actions workflows opt into locked resolution.

**Tech Stack:** Cargo workspace, Makefile, GitHub Actions, Rust stable toolchain.

## Global Constraints

- Do not change dependency version requirements.
- Do not pin or change the Rust toolchain.
- Do not change decoder, simulator, benchmark, or release behavior beyond Cargo lockfile enforcement.
- Do not add per-crate lockfiles.
- Every executable Cargo command printed by `rg -n 'cargo (build|check|test|run|llvm-cov)' Makefile .github/workflows` must contain `--locked`, unless a wrapper is immediately preceded in the same job by `cargo metadata --locked`.

---

## File Structure

- Modify `.gitignore`: remove the root `Cargo.lock` ignore rule only.
- Create `Cargo.lock`: workspace dependency resolution generated from the current manifests.
- Modify `Makefile`: add `--locked` to Cargo build/check/test/run commands.
- Modify `.github/workflows/ci.yml`: add `--locked` to Cargo test, run, and llvm-cov commands.
- Modify `.github/workflows/rbposd-parity.yml`: add `--locked` to the rbposd parity Cargo test command.

### Task 1: Root Lockfile Tracking

**Files:**
- Modify: `.gitignore`
- Create: `Cargo.lock`

**Interfaces:**
- Consumes: current workspace manifests in `Cargo.toml` and member crate `Cargo.toml` files.
- Produces: a tracked root `Cargo.lock` that `cargo metadata --locked --format-version 1` can consume without rewriting.

- [ ] **Step 1: Verify the current negative state**

Run:

```bash
cargo metadata --locked --format-version 1 >/dev/null
```

Expected: FAIL with Cargo reporting that `Cargo.lock` needs to be generated or updated because `--locked` was passed.

- [ ] **Step 2: Stop ignoring the root lockfile**

Edit `.gitignore` by deleting this standalone line:

```gitignore
Cargo.lock
```

Expected: all other ignore rules stay unchanged.

- [ ] **Step 3: Generate the workspace lockfile**

Run:

```bash
cargo generate-lockfile
```

Expected: `Cargo.lock` exists at the repository root, and no `Cargo.toml` dependency requirements are modified.

- [ ] **Step 4: Verify locked metadata now succeeds**

Run:

```bash
test -f Cargo.lock
cargo metadata --locked --format-version 1 >/dev/null
git diff -- Cargo.toml qec-ilp-core/Cargo.toml qec-code/Cargo.toml rstim/Cargo.toml rsinter/Cargo.toml rbposd/Cargo.toml rmatching/Cargo.toml rilpqec/Cargo.toml benchmarks/surface_decoder_compare/rust_bridge/Cargo.toml
```

Expected: first two commands PASS; `git diff` prints nothing for workspace manifests.

### Task 2: Locked Developer and CI Entry Points

**Files:**
- Modify: `Makefile`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/rbposd-parity.yml`

**Interfaces:**
- Consumes: the tracked root `Cargo.lock` from Task 1.
- Produces: Makefile and GitHub Actions Cargo invocations that use `--locked`.

- [ ] **Step 1: Record the Cargo entry points to enforce**

Run:

```bash
rg -n 'cargo (build|check|test|run|llvm-cov)' Makefile .github/workflows
```

Expected before edits: output includes Cargo commands without `--locked` in the Makefile, `.github/workflows/ci.yml`, and `.github/workflows/rbposd-parity.yml`.

- [ ] **Step 2: Update `Makefile` Cargo commands**

Apply these replacements:

```diff
-	cargo test --workspace
+	cargo test --locked --workspace
-	cargo check --workspace
+	cargo check --locked --workspace
-	cargo run -p rsinter --bin rsinter --
+	cargo run --locked -p rsinter --bin rsinter --
-	cargo build --release -p rsinter
+	cargo build --locked --release -p rsinter
-	cargo build -p qec-code
+	cargo build --locked -p qec-code
-	cargo build --release -p qec-code
+	cargo build --locked --release -p qec-code
-	cargo check --workspace
+	cargo check --locked --workspace
```

Expected: every executable `cargo build`, `cargo check`, `cargo test`, and
`cargo run` line in the top-level `Makefile` contains `--locked`.

- [ ] **Step 3: Update GitHub Actions Cargo commands**

Apply these replacements:

```diff
-.github/workflows/ci.yml:      - run: cargo test --workspace
+.github/workflows/ci.yml:      - run: cargo test --locked --workspace
-.github/workflows/ci.yml:          cargo run -p rstim --bin rstim -- perf ci --out-dir perf-artifacts
+.github/workflows/ci.yml:          cargo run --locked -p rstim --bin rstim -- perf ci --out-dir perf-artifacts
-.github/workflows/ci.yml:        run: cargo llvm-cov --workspace --lcov --output-path lcov.info --ignore-filename-regex 'rbposd/dev/fixture_catalog\.rs$'
+.github/workflows/ci.yml:        run: cargo llvm-cov --locked --workspace --lcov --output-path lcov.info --ignore-filename-regex 'rbposd/dev/fixture_catalog\.rs$'
-.github/workflows/rbposd-parity.yml:        run: cargo test -p rbposd --test reference --test parity_cli --test tooling
+.github/workflows/rbposd-parity.yml:        run: cargo test --locked -p rbposd --test reference --test parity_cli --test tooling
```

Expected: every executable GitHub Actions Cargo command matched by the review
regex contains `--locked`.

- [ ] **Step 4: Verify entry-point enforcement**

Run:

```bash
rg -n 'cargo (build|check|test|run|llvm-cov)' Makefile .github/workflows
```

Expected: every executable Cargo command printed by this command contains
`--locked`.

- [ ] **Step 5: Run clean-checkout and repository verification**

Run:

```bash
test -f Cargo.lock
cargo metadata --locked --format-version 1 >/dev/null
cargo build --locked -p rsinter
cargo test --locked --workspace
cargo test
```

Expected: all commands PASS.

- [ ] **Step 6: Run the missing-lockfile negative control**

Run:

```bash
tmp_dir="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp_dir"
rm "$tmp_dir/Cargo.lock"
if (cd "$tmp_dir" && cargo metadata --locked --format-version 1); then
  echo "ERROR: locked metadata unexpectedly succeeded without Cargo.lock" >&2
  exit 1
fi
rm -r "$tmp_dir"
```

Expected: the inner `cargo metadata --locked --format-version 1` exits nonzero
and reports that the lockfile needs to be generated or updated.

## Self Review

- Task 1 covers the tracked root lockfile and avoids dependency requirement changes.
- Task 2 covers Makefile and CI Cargo entry points listed in the issue.
- Verification includes the issue commands, the required `cargo test`, the regex entry-point audit, and the missing-lockfile negative control.
- No placeholders or deferred implementation steps remain.
