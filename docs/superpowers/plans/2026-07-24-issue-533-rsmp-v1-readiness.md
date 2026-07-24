# Issue 533 RSMP v1 Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the `rsmp v1` operational documentation, deterministic readiness gate, Make target, CI job, and negative controls for issue #533.

**Architecture:** Add a Python readiness aggregator that runs existing locked RSMP checks, validates current structured inputs, validates semantic documentation plus normalized live CLI help, writes `benchmarks/out/rsmp-v1/readiness.json`, and owns the final readiness PASS line. Keep existing Rust checkers as the evidence source and use the Python layer only for orchestration, normalization, and artifact construction.

**Tech Stack:** Rust/Cargo locked integration tests, Python 3 standard library, existing `tools/check_rsmp_fixture_catalog.py`, existing `tools/check_rsmp_v1_compression_evidence.py`, GitHub Actions.

## Global Constraints

- Final successful stdout line must be exactly `PASS rsmp v1 readiness valid_cases=7 corruption_cases>=12 compatibility=1 compression=pass`.
- `valid_cases=7` counts the seven required semantic roles, even if the catalog has additional cases.
- `corruption_cases>=12` counts distinct named recipes, not generated truncations or bit flips.
- Readiness validates committed compression evidence only; it must not run a timing benchmark or add wall-clock performance gates.
- Readiness must use locked Cargo commands and support offline execution after dependencies are fetched.
- Failure must exit nonzero, omit the readiness PASS line, and write `status = "fail"` plus named failed checks whenever output creation is possible.
- Documentation checks must inspect designated semantic sections and normalized CLI help; repository-wide substring searches and raw Clap whitespace snapshots are insufficient.
- CI `rsmp-v1-readiness` must be always-on for the main workflow's push and pull_request triggers, with no label-gated `if:`.

---

### Task 1: Negative Controls And Readiness Test Harness

**Files:**
- Create: `tools/test_check_rsmp_v1_readiness.py`

**Interfaces:**
- Consumes: planned `tools/check_rsmp_v1_readiness.py` CLI and validation helpers.
- Produces: `RsmpV1ReadinessNegativeControls` with exactly the four issue-required tests.

- [ ] **Step 1: Write failing negative-control tests**

Use temporary repository roots created by copying only required inputs:

```python
class RsmpV1ReadinessNegativeControls(unittest.TestCase):
    def test_rejects_missing_compression_input_hash(self) -> None:
        self.expect_mutation_failure(
            mutate=lambda root: self.remove_first_artifact_hash(root),
            expected="not ready: compression repository input hash is missing",
        )

    def test_rejects_failed_compression_gate(self) -> None:
        self.expect_mutation_failure(
            mutate=lambda root: self.increase_benchmark_archive_bytes(root),
            expected="not ready: compression acceptance gate failed",
        )

    def test_rejects_missing_sweep_unsupported_normative_statement(self) -> None:
        self.expect_mutation_failure(
            mutate=lambda root: self.remove_sweep_support_boundary(root),
            expected="not ready: normative documentation does not mark sweep-bit circuits unsupported",
        )

    def test_rejects_documented_cli_surface_drift(self) -> None:
        self.expect_mutation_failure(
            mutate=lambda root: self.rename_documented_verify_only_option(root),
            expected="not ready: documented CLI surface differs from rstim help",
        )
```

Each test runs the checker through `subprocess.run([sys.executable, str(CHECKER), "--repo-root", str(temp_root), "--out-dir", str(artifact_dir), "--skip-commands"])`, asserts nonzero exit, asserts the required diagnostic, asserts the final PASS line is absent, and asserts `readiness.json.status == "fail"`.

- [ ] **Step 2: Verify RED**

Run:

```bash
python3 -m unittest tools.test_check_rsmp_v1_readiness.RsmpV1ReadinessNegativeControls
```

Expected: import or command failure because `tools/check_rsmp_v1_readiness.py` does not exist yet.

- [ ] **Step 3: Keep tests staged for the checker GREEN task**

Do not commit this task until the checker implementation makes the four tests pass.

### Task 2: Readiness Aggregator

**Files:**
- Create: `tools/check_rsmp_v1_readiness.py`
- Modify: `tools/test_check_rsmp_v1_readiness.py`

**Interfaces:**
- Consumes: catalog JSON, compatibility TOML, compression evidence bundle, normative doc, CLI doc, live `rstim` help, focused Cargo checks, corruption-corpus example.
- Produces: `readiness.json`, logs, normalized help hash, failed-check diagnostics, and the final PASS line.

- [ ] **Step 1: Implement pure validation helpers**

Implement:

```python
def build_readiness_report(repo_root: Path, out_dir: Path, *, run_commands: bool) -> dict[str, Any]
def validate_catalog(repo_root: Path, report: dict[str, Any]) -> None
def validate_corruption_summary(repo_root: Path, out_dir: Path, report: dict[str, Any], *, run_commands: bool) -> None
def validate_compatibility(repo_root: Path, report: dict[str, Any]) -> None
def validate_compression(repo_root: Path, report: dict[str, Any]) -> None
def validate_documentation(repo_root: Path, report: dict[str, Any], *, run_commands: bool) -> None
def normalized_help_model(pack_help: str, unpack_help: str) -> dict[str, Any]
```

Use structured parsing of JSON/TOML and section extraction by Markdown heading.

- [ ] **Step 2: Implement command runner**

Run each child command with `cwd=repo_root`, capture stdout/stderr to
`benchmarks/out/rsmp-v1/logs/<check>.log`, preserve each command and exit code
in `checked_commands`, and fail on nonzero exit without scraping PASS text for
success.

- [ ] **Step 3: Implement CLI**

Support:

```bash
python3 tools/check_rsmp_v1_readiness.py --repo-root . --out-dir benchmarks/out/rsmp-v1
python3 tools/check_rsmp_v1_readiness.py --repo-root <temp> --out-dir <temp-out> --skip-commands
```

On success, print only the final required readiness PASS line. On failure,
print `not ready: <diagnostic>` lines to stderr and return `1`.

- [ ] **Step 4: Verify GREEN for negative controls**

Run:

```bash
python3 -m unittest tools.test_check_rsmp_v1_readiness.RsmpV1ReadinessNegativeControls
```

Expected: four tests pass and the harness ends with `OK`.

### Task 3: Normative And Operational Documentation

**Files:**
- Modify: `rstim/doc/rsmp-v1.md`
- Create: `rstim/doc/rsmp-cli.md`

**Interfaces:**
- Consumes: existing format contract, CLI help, compatibility manifest, compression evidence.
- Produces: semantic sections consumed by `validate_documentation()`.

- [ ] **Step 1: Expand normative document sections**

Add designated headings for:

```text
## Circuit-Derived Lossless Transform
## Binary Fields and Canonical Encoding
## Support Boundaries
## Integrity, Authentication, and Access Model
## Resource Limits and Validation Precedence
## Stable Error Taxonomy
## Compatibility Fixture Policy
## Compression Evidence and Claim Limits
```

- [ ] **Step 2: Add operational CLI guide**

Document `pack_samples`, `unpack_samples`, `unpack_samples --verify_only`,
all supported result formats, original-circuit requirement, DEM-only and
sweep-bit unsupported boundaries, publication atomicity limits, stdout
rollback limits, compression-evidence reproduction command, and
`--verify_only` as the recommended nondeveloper route.

- [ ] **Step 3: Run documentation validation**

Run:

```bash
python3 tools/check_rsmp_v1_readiness.py --repo-root . --out-dir benchmarks/out/rsmp-v1 --skip-commands
```

Expected: no documentation diagnostic after all committed input validations pass.

### Task 4: Make Target And CI Job

**Files:**
- Modify: `Makefile`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `tools/check_rsmp_v1_readiness.py`.
- Produces: `make rsmp-v1-readiness` and an always-on CI job.

- [ ] **Step 1: Add Make target**

Add `rsmp-v1-readiness` to `.PHONY` and `make help`, then implement:

```make
rsmp-v1-readiness:
	python3 tools/check_rsmp_v1_readiness.py --repo-root . --out-dir benchmarks/out/rsmp-v1
```

- [ ] **Step 2: Add CI job**

Add an `rsmp-v1-readiness` job with checkout, system deps, Stim install, Rust
toolchain, rust-cache, `cargo fetch --locked`, captured readiness execution,
`actions/upload-artifact@v4` with `if: always()`, concise failure summary, and
explicit failure after artifact upload.

- [ ] **Step 3: Verify Make target**

Run:

```bash
make rsmp-v1-readiness
```

Expected final non-empty stdout line:

```text
PASS rsmp v1 readiness valid_cases=7 corruption_cases>=12 compatibility=1 compression=pass
```

### Task 5: Final Verification, Review, And PR

**Files:**
- No new files; uses full branch diff.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: committed branch, pushed worker branch, PR URL.

- [ ] **Step 1: Run required verification**

Run:

```bash
python3 -m unittest tools.test_check_rsmp_v1_readiness.RsmpV1ReadinessNegativeControls
make rsmp-v1-readiness
cargo test
```

- [ ] **Step 2: Inspect readiness artifact**

Confirm:

```text
status = "pass"
valid_catalog.required_role_count = 7
corruption.named_recipe_count >= 12
compatibility.fixture_count = 1
compatibility.block_count = 2
compatibility.codecs = ["sparse", "dense"]
compression.status = "pass"
failed_checks = []
```

- [ ] **Step 3: Request whole-branch review**

Use `superpowers:requesting-code-review` on the branch diff. Fix Critical and
Important findings before finishing.

- [ ] **Step 4: Commit, push, and create PR**

Commit scoped changes, push
`agent/issue-533-publish-rsmp-v1-operational-documentation-and-ci-run-1`, and
create a PR against `master` with issue #533 in the body and verification
commands listed.
