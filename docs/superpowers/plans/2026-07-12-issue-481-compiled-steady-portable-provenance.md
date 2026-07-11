# Issue 481 Compiled Steady Portable Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the compiled steady-state checked evidence runner, checker, and bundle from host-absolute provenance to portable logical provenance.

**Architecture:** The runner writes repo-relative repository-input paths, logical `tool://` worker commands, and runtime identities without live paths. The checker validates repo inputs against the current checkout, validates runtime identities by role/version/basename/SHA-256 only, and keeps raw lifecycle, summary, report, and artifact hash validation ordering intact. The checked bundle is migrated in place without rerunning timing.

**Tech Stack:** Python 3 standard library (`argparse`, `hashlib`, `json`, `pathlib`, `re`, `shutil`, `unittest`), existing compiled-steady runner/checker, existing portable catalog, Cargo workspace tests.

## Global Constraints

- Preserve `summary.json` SHA-256 `2228e5460be43775d45f30861f28bc36c888557add981eeab8e47deadbfb8680`.
- Preserve `report.md` SHA-256 `84b730190bf7554f63dea3fe7629eb8e787db01cbe9ae387a242c2339605d6f4`.
- Preserve lifecycle `compile/reference/sample = 1/1/9`.
- Preserve measured record count `14`.
- Preserve every raw timing value in `raw.jsonl`.
- Do not rerun timing.
- Repository inputs and Python modules use repo-relative POSIX paths.
- Worker commands use logical executable roles.
- Runtime identities preserve versions and hashes without requiring the old worker or Python extension.
- The checker must fail an absolute `fair_manifest_path` negative control with `fair_manifest_path must be repository-relative`.
- Required success output is exactly `PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9`.

---

### Task 1: Red Tests For Portable Compiled-Steady Provenance

**Files:**
- Modify: `tools/test_check_rstim_vs_stim_compiled_steady_evidence.py`

**Interfaces:**
- Consumes: existing synthetic bundle helpers and committed bundle path.
- Produces: failing tests for committed-bundle acceptance, absolute provenance rejection, runtime live-path rejection, and host-absolute command rejection.

- [ ] **Step 1: Write failing tests**

Add tests equivalent to:

```python
COMMITTED_BUNDLE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release"

def rehash_bundle(bundle: Path) -> None:
    rewrite_artifact_hashes(bundle)

def test_accepts_committed_bundle(self) -> None:
    result = subprocess.run(
        ["python3", str(CHECKER), "--dir", str(COMMITTED_BUNDLE)],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertEqual(
        result.stdout,
        "PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9\n",
    )

def test_rejects_absolute_fair_manifest_path_with_required_message(self) -> None:
    def mutate(environment: dict[str, Any]) -> None:
        environment["fair_manifest_path"] = (
            "/Users/nzy/pycode/agent-desk/config/.agent-desk/worktrees/"
            "nzy1997-rstim/issue-454-run-1-agent-issue-454-publish-compiled-steady-state-sampling-evidence-run-1/"
            "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"
        )
    rewrite_json(self.bundle / "environment.json", mutate)
    rehash_bundle(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("fair_manifest_path must be repository-relative", result.stderr)

def test_rejects_runtime_identity_required_live_path(self) -> None:
    def mutate(environment: dict[str, Any]) -> None:
        environment["runtime_identities"][0]["required_live_path"] = True
    rewrite_json(self.bundle / "environment.json", mutate)
    rehash_bundle(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("checked evidence must not require a live runtime path", result.stderr)

def test_rejects_host_absolute_worker_argv(self) -> None:
    def mutate(environment: dict[str, Any]) -> None:
        environment["worker_argv"]["stim"][0] = "/usr/bin/python3"
        environment["canonical_worker_argv"]["stim"] = environment["worker_argv"]["stim"]
        environment["workers"][0]["command"] = environment["worker_argv"]["stim"]
    rewrite_json(self.bundle / "environment.json", mutate)
    rehash_bundle(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("worker argv contains host-absolute path", result.stderr)
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: the new committed-bundle acceptance test fails because the old checker rejects the absolute `fair_manifest_path`.

- [ ] **Step 3: Commit red tests**

```sh
git add tools/test_check_rstim_vs_stim_compiled_steady_evidence.py
git commit -m "test: require portable compiled steady provenance"
```

### Task 2: Implement Portable Runner And Checker Contract

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py`
- Modify: `tools/check_rstim_vs_stim_compiled_steady_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_compiled_steady_evidence.py`

**Interfaces:**
- Consumes: red tests from Task 1.
- Produces: runner/checker support for repo-relative paths, logical worker roles, and runtime identities without live paths.

- [ ] **Step 1: Add path and identity helpers**

In the checker, add helpers:

```python
def _repo_relative_path(raw: Any, field: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"{field} must be repository-relative")
    path = PurePosixPath(raw)
    if path.is_absolute() or "\\" in raw or any(part in {"", ".", ".."} for part in raw.split("/")):
        raise ValueError(f"{field} must be repository-relative")
    return REPO_ROOT / path

def _portable_worker_argv(role: str, input_path: str) -> list[str]:
    if role == "stim":
        return [
            "tool://python",
            "-m",
            "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
            "--input",
            input_path,
            "--seed",
            "0",
        ]
    return ["tool://rstim-worker", "--input", input_path, "--seed", "0"]
```

Add recursive host-path detection for strings in argv lists and reject with
`worker argv contains host-absolute path`.

- [ ] **Step 2: Validate runtime identities without live paths**

Require `environment["runtime_identities"]` to contain exactly these roles:

```python
("tool://python", "tool://stim-extension", "tool://stim-worker", "tool://rstim-worker")
```

Each identity must contain only `role`, `version`, `basename`, and `sha256`.
Reject `required_live_path = true` with
`checked evidence must not require a live runtime path`. Verify:

- `tool://stim-extension` version equals `1.15.0`;
- `tool://stim-worker` version equals `1.15.0`;
- `tool://rstim-worker` version equals `environment["rstim_version"]`;
- `tool://stim-worker` SHA-256 equals the repo `stim_worker_module_path` hash;
- every SHA-256 is lowercase hex;
- basenames are nonempty and contain no path separators.

- [ ] **Step 3: Update environment validation**

Replace live path checks for `python_executable`, `loaded_stim_extension_path`,
and `rstim_worker_binary_path` with runtime identity validation. Keep
repo-relative file hash checks for `fair_manifest_path`, `source_manifest_path`,
`fixture_path`, and `stim_worker_module_path`.

Validate worker commands against:

```python
{
    "stim": ["tool://python", "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady", "--input", fixture_path, "--seed", "0"],
    "rstim": ["tool://rstim-worker", "--input", fixture_path, "--seed", "0"],
}
```

Validate preflight argv against the same logical roles and the logical input
`fixture://compiled-steady-known-answer`.

- [ ] **Step 4: Update runner output**

In `_collect_environment()`, emit repo-relative paths for repository inputs and
the Stim worker module. Emit logical worker argv and runtime identities derived
from the resolved generation-time executables/modules, including the same
versions and SHA-256 digests previously written in path-bearing fields. Remove
live runtime path fields from future output.

- [ ] **Step 5: Run tests to verify GREEN for synthetic bundles**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: tests still fail only until the committed bundle is migrated in Task
3; synthetic portable tests pass.

- [ ] **Step 6: Commit runner/checker changes**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py \
  tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  tools/test_check_rstim_vs_stim_compiled_steady_evidence.py
git commit -m "feat: validate portable compiled steady provenance"
```

### Task 3: Migrate The Checked Bundle And Catalog Digests

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: checker contract from Task 2.
- Produces: portable committed compiled-steady bundle and catalog entry.

- [ ] **Step 1: Rewrite environment provenance without rerunning timing**

Transform the existing `environment.json` only:

- repo-relative paths for repository inputs and `stim_worker_module_path`;
- logical worker argv for `worker_argv`, `canonical_worker_argv`, `workers`, and preflight argv;
- `runtime_identities` entries for Python, Stim extension, Stim worker module, and rstim worker binary;
- `stim_python_probe` without host paths.

Do not edit `raw.jsonl`, `summary.json`, or `report.md`.

- [ ] **Step 2: Rehash the bundle**

Update `artifact-sha256.json` for:

```text
raw.jsonl
summary.json
report.md
environment.json
```

The `summary.json` and `report.md` digests must remain the issue #481 values.

- [ ] **Step 3: Update portable catalog digests**

Update only the compiled-steady bundle's catalog digests affected by this PR:

- `repository_inputs` digest for `run_compiled_steady.py`;
- artifact digest for `environment.json`;
- artifact digest for `artifact-sha256.json`.

- [ ] **Step 4: Run targeted verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
```

Expected: all pass, with the checker success line exactly as required.

- [ ] **Step 5: Commit bundle/catalog migration**

```sh
git add benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/environment.json \
  benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/artifact-sha256.json \
  benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
git commit -m "data: migrate compiled steady provenance"
```

### Task 4: Final Verification And Pull Request

**Files:**
- Modify only if verification exposes a scoped issue in Task 1-3 files.

**Interfaces:**
- Consumes: committed implementation and migrated bundle.
- Produces: pushed worker branch and pull request.

- [ ] **Step 1: Run issue-required local checker**

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
```

Expected:

```text
PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9
```

- [ ] **Step 2: Run issue-required archive checker**

```sh
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release)
```

Expected:

```text
PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9
```

- [ ] **Step 3: Run regression and catalog tests**

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected: unit tests pass and catalog validator prints
`PASS portable evidence catalog bundles=4 schema=2`.

- [ ] **Step 4: Run Cargo verification**

```sh
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 5: Push and create PR**

```sh
git push -u origin agent/issue-481-migrate-compiled-steady-state-evidence-to-portab-run-1
gh pr create --base master --head agent/issue-481-migrate-compiled-steady-state-evidence-to-portab-run-1 --title "Migrate compiled steady evidence to portable provenance" --body-file <generated-body>
```

Expected: PR URL is printed.

## Plan Self-Review

- Every issue #481 preservation requirement maps to a task and a final check.
- The negative-control error string is explicit and tested.
- No task reruns timing or edits raw/summary/report measured data.
- The plan uses the recommended subagent-driven execution approach under the Standing Answer Policy.
