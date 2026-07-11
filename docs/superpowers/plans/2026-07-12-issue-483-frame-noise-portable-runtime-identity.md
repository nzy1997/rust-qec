# Issue 483 Frame-Noise Portable Runtime Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make instruction-wide frame-noise evidence validation portable by replacing default live `target/release/rstim` hashing with schema-v2 runtime identity checks and optional supplied-binary attestation.

**Architecture:** Keep bundle semantic checks in `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`, but source the publishing `tool://rstim` identity from the schema-v2 catalog added by #479. The frame bundle `environment.json` stores the same schema-v2 identity, while `--verify-runtime-binary <path>` is the only path that the checker hashes as a runtime binary.

**Tech Stack:** Python standard library (`argparse`, `hashlib`, `json`, `pathlib`, `subprocess`, `tempfile`, `unittest`), existing `benchmarks.rstim_vs_stim_simulator.portable_provenance`, TOML evidence catalog, existing Rust workspace verified by `cargo test`.

## Global Constraints

- Default checked-evidence validation must not open `target/release/rstim`.
- Optional `--verify-runtime-binary <path>` must hash only the supplied path and reject a different binary with `runtime binary SHA-256 does not match recorded identity`.
- Store the publishing binary as a schema-v2 runtime identity containing `role`, `version`, `basename`, and `sha256`.
- Cross-check the frame bundle's recorded runtime identity against the schema-v2 catalog identity for `frame-instruction-wide-release`.
- Preserve `summary.json`: `1b41f2b1f8ad1730ac61f62cd707225f547b42d6f65ac0752b22a6cfbc6cb422`.
- Preserve `report.md`: `fd1e291c7848599c9d06f48d3e139210c7daef25a51b1d1ba301ca3c37afbde4`.
- Preserve `correctness-summary.json`: `42d4e8ae02c8787292b4b16fc0e66f79c805fb04685d64630fd1778351b57484`.
- Preserve `fixture-load.json`: `c953d134c601568b9be1d73036ca652bb90438148adeb6c6942fca788435d27f`.
- Mutating correctness status to `failed` must still fail before artifact hashes.
- Do not rerun timing, claim a wall-clock speedup, or alter noise sampling behavior.
- Base branch is `master`; worker branch is `agent/issue-483-migrate-instruction-wide-frame-noise-evidence-to-run-2`.

---

## File Structure

- Modify `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`: add catalog-backed runtime identity validation, optional runtime binary attestation, and CLI argument plumbing.
- Modify `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`: update the synthetic valid bundle to use schema-v2 runtime identities and add regression tests for portability and optional attestation.
- Modify `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json`: replace live `rstim_binary` path fields with `runtime_identities`.
- Modify `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json`: update only the `environment.json` digest.
- Modify `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`: update only the frame bundle `environment.json` and `artifact-sha256.json` artifact digests if those file bytes changed.

### Task 1: Add Portable Runtime Identity Checker Behavior

**Files:**
- Modify: `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`

**Interfaces:**
- Consumes: `benchmarks.rstim_vs_stim_simulator.portable_provenance.load_catalog(path: Path) -> dict[str, Any]`.
- Produces:
  - `validate_bundle(results_dir: Path, verify_runtime_binary: Path | None = None) -> tuple[int, int, int]`
  - CLI option `--verify-runtime-binary <path>`
  - default runtime validation that cross-checks `environment.runtime_identities[0]` against the catalog `tool://rstim` identity without opening a live binary.

- [ ] **Step 1: Write the failing tests**

In `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`, update `run_checker()` so it accepts optional extra arguments:

```python
    def run_checker(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--dir", str(self.bundle), *extra_args],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
```

In `write_valid_bundle()`, replace:

```python
        "rstim_binary": str(rstim_binary),
        "rstim_binary_sha256": sha256_file(rstim_binary),
```

with:

```python
        "runtime_identities": [
            {
                "role": "tool://rstim",
                "version": "rstim 0.1.1",
                "basename": "rstim",
                "sha256": sha256_file(rstim_binary),
            }
        ],
```

Add these tests to `InstructionWideEvidenceCheckerTest`:

```python
    def test_default_validation_does_not_require_live_runtime_binary(self) -> None:
        (self.bundle / "rstim").unlink()

        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362\n",
        )

    def test_verify_runtime_binary_accepts_matching_supplied_binary(self) -> None:
        result = self.run_checker("--verify-runtime-binary", str(self.bundle / "rstim"))

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS instruction-wide frame-noise evidence", result.stdout)

    def test_verify_runtime_binary_rejects_different_supplied_binary(self) -> None:
        other_binary = self.bundle / "other-rstim"
        other_binary.write_bytes(b"different runtime binary\n")

        result = self.run_checker("--verify-runtime-binary", str(other_binary))

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("runtime binary SHA-256 does not match recorded identity", result.stderr)

    def test_rejects_legacy_runtime_binary_path_fields_without_hashing_them(self) -> None:
        rewrite_json(
            self.bundle / "environment.json",
            lambda payload: (
                payload.pop("runtime_identities"),
                payload.update(
                    {
                        "rstim_binary": str(self.bundle / "missing-rstim"),
                        "rstim_binary_sha256": "0" * 64,
                    }
                ),
            ),
        )
        rewrite_hashes(self.bundle)

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities must contain exactly one tool://rstim identity", result.stderr)
        self.assertNotIn("does not exist", result.stderr)
```

Change `test_rejects_mismatched_fixture_manifest_binary_or_artifact_hash` to cover only fixture and manifest hash mismatches; binary mismatch is now covered by `--verify-runtime-binary`.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence -q
```

Expected: FAIL because the checker does not accept `--verify-runtime-binary`, still expects `rstim_binary`, and does not understand `runtime_identities`.

- [ ] **Step 3: Implement catalog-backed runtime identity validation**

In `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`, add the import:

```python
from benchmarks.rstim_vs_stim_simulator.portable_provenance import load_catalog
```

Add constants near the existing path constants:

```python
CATALOG_PATH = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
BUNDLE_ID = "frame-instruction-wide-release"
RUNTIME_ROLE = "tool://rstim"
RUNTIME_IDENTITY_FIELDS = frozenset({"role", "version", "basename", "sha256"})
```

Add helpers:

```python
def _normalize_runtime_identity(raw_identity: Any, label: str) -> dict[str, str]:
    if not isinstance(raw_identity, dict):
        raise ValueError(f"{label} must be an object")
    unsupported = sorted(set(raw_identity) - RUNTIME_IDENTITY_FIELDS)
    if unsupported:
        raise ValueError(f"{label} unsupported field(s): {', '.join(unsupported)}")
    missing = [field for field in ("role", "version", "basename", "sha256") if field not in raw_identity]
    if missing:
        raise ValueError(f"{label} missing required field(s): {', '.join(missing)}")
    role = raw_identity["role"]
    version = raw_identity["version"]
    basename = raw_identity["basename"]
    digest = _require_digest(raw_identity["sha256"], f"{label} sha256")
    if role != RUNTIME_ROLE:
        raise ValueError(f"{label} role must be {RUNTIME_ROLE}")
    if not isinstance(version, str) or not version:
        raise ValueError(f'{label} field "version" must be a nonempty string')
    if not isinstance(basename, str) or not basename:
        raise ValueError(f'{label} field "basename" must be a nonempty string')
    if "/" in basename or "\\" in basename:
        raise ValueError(f'{label} field "basename" must not contain path separators')
    return {"role": role, "version": version, "basename": basename, "sha256": digest}


def load_catalog_runtime_identity() -> dict[str, str]:
    catalog = load_catalog(CATALOG_PATH)
    bundles = catalog.get("bundles")
    if not isinstance(bundles, list):
        raise ValueError("evidence catalog bundles must be an array")
    for bundle in bundles:
        if isinstance(bundle, dict) and bundle.get("id") == BUNDLE_ID:
            identities = bundle.get("runtime_identities")
            if not isinstance(identities, list):
                raise ValueError(f'evidence catalog bundle "{BUNDLE_ID}" runtime_identities must be an array')
            matches = [
                _normalize_runtime_identity(identity, f'evidence catalog bundle "{BUNDLE_ID}" runtime identity')
                for identity in identities
                if isinstance(identity, dict) and identity.get("role") == RUNTIME_ROLE
            ]
            if len(matches) != 1:
                raise ValueError(f'evidence catalog bundle "{BUNDLE_ID}" must contain exactly one {RUNTIME_ROLE} identity')
            return matches[0]
    raise ValueError(f'evidence catalog missing bundle "{BUNDLE_ID}"')


def validate_environment_runtime_identity(environment: dict[str, Any]) -> dict[str, str]:
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must contain exactly one tool://rstim identity")
    matches = [
        _normalize_runtime_identity(identity, f"environment runtime_identities[{index}]")
        for index, identity in enumerate(identities)
        if isinstance(identity, dict) and identity.get("role") == RUNTIME_ROLE
    ]
    if len(matches) != 1:
        raise ValueError("environment runtime_identities must contain exactly one tool://rstim identity")
    identity = matches[0]
    catalog_identity = load_catalog_runtime_identity()
    if identity != catalog_identity:
        raise ValueError("environment runtime identity must match schema-v2 catalog identity")
    return identity


def validate_runtime_binary(runtime_binary: Path, identity: dict[str, str]) -> None:
    if not runtime_binary.is_file():
        raise ValueError(f"runtime binary path does not exist: {runtime_binary}")
    if sha256_file(runtime_binary) != identity["sha256"]:
        raise ValueError("runtime binary SHA-256 does not match recorded identity")
```

Change `validate_environment()` to accept `verify_runtime_binary: Path | None` and replace `_validate_path_hash(environment, "rstim_binary", "rstim_binary_sha256")` with:

```python
    runtime_identity = validate_environment_runtime_identity(environment)
    if verify_runtime_binary is not None:
        validate_runtime_binary(verify_runtime_binary, runtime_identity)
```

Change `validate_bundle()` to accept and forward `verify_runtime_binary`.

Change `build_parser()` to add:

```python
    parser.add_argument("--verify-runtime-binary", type=Path)
```

Change `main()` to call:

```python
        builds, attempts, legacy_setups = validate_bundle(args.dir, args.verify_runtime_binary)
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence -q
```

Expected: OK.

- [ ] **Step 5: Commit**

Run:

```bash
git add tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py
git commit -m "fix: validate frame evidence runtime identity portably"
```

### Task 2: Migrate The Committed Frame Evidence Provenance

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: Task 1 checker contract and the existing catalog runtime identity for `tool://rstim`.
- Produces: committed frame evidence that validates from a clean archive without `target/release/rstim`.

- [ ] **Step 1: Write a failing default validation against the committed bundle**

Run:

```bash
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release
```

Expected: FAIL with `environment runtime_identities must contain exactly one tool://rstim identity` because the committed `environment.json` still contains `rstim_binary` and `rstim_binary_sha256`.

- [ ] **Step 2: Migrate `environment.json`**

In `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json`, remove:

```json
  "rstim_binary": "target/release/rstim",
  "rstim_binary_sha256": "336ab36864ba884314507d39378628aa653f16f9c51693512da510cbf3982568",
```

Add:

```json
  "runtime_identities": [
    {
      "basename": "rstim",
      "role": "tool://rstim",
      "sha256": "336ab36864ba884314507d39378628aa653f16f9c51693512da510cbf3982568",
      "version": "rstim 0.1.1"
    }
  ],
```

Do not change `summary.json`, `report.md`, `correctness-summary.json`, `fixture-load.json`, or `raw.jsonl`.

- [ ] **Step 3: Recompute changed-file artifact hashes**

Run:

```bash
sha256sum benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json
```

Update `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json` so `environment.json` has the new digest.

Run:

```bash
sha256sum benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json
```

Update the `frame-instruction-wide-release` artifact entries in `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml` so only `environment.json` and `artifact-sha256.json` use the new digests.

- [ ] **Step 4: Run focused validation**

Run:

```bash
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release
```

Expected:

```text
PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362
```

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS portable evidence catalog bundles=4 schema=2
```

- [ ] **Step 5: Run the clean-archive verification**

Run:

```bash
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && test ! -e target/release/rstim && \
  python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release)
```

Expected:

```text
PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362
```

- [ ] **Step 6: Run digest preservation check**

Run:

```bash
sha256sum \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/summary.json \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/report.md \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/correctness-summary.json \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/fixture-load.json
```

Expected digests:

```text
1b41f2b1f8ad1730ac61f62cd707225f547b42d6f65ac0752b22a6cfbc6cb422
fd1e291c7848599c9d06f48d3e139210c7daef25a51b1d1ba301ca3c37afbde4
42d4e8ae02c8787292b4b16fc0e66f79c805fb04685d64630fd1778351b57484
c953d134c601568b9be1d73036ca652bb90438148adeb6c6942fca788435d27f
```

- [ ] **Step 7: Commit**

Run:

```bash
git add \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json \
  benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/artifact-sha256.json \
  benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
git commit -m "data: migrate frame evidence runtime identity"
```

### Task 3: Final Verification

**Files:**
- No source changes expected. Verify the completed branch before PR creation.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: final verification evidence for the pull request.

- [ ] **Step 1: Run checker unit tests**

Run:

```bash
python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence -q
```

Expected: OK.

- [ ] **Step 2: Run portable catalog tests**

Run:

```bash
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: OK.

- [ ] **Step 3: Run issue verification command**

Run:

```bash
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && test ! -e target/release/rstim && \
  python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release)
```

Expected both checker invocations print:

```text
PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362
```

- [ ] **Step 4: Run negative controls**

Run:

```bash
bad_binary="$(mktemp)"
printf 'different runtime binary\n' > "$bad_binary"
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release \
  --verify-runtime-binary "$bad_binary"
```

Expected: nonzero exit and stderr contains:

```text
runtime binary SHA-256 does not match recorded identity
```

Run a temporary mutation of `correctness-summary.json` in a copied bundle and keep `artifact-sha256.json` stale:

```bash
tmp="$(mktemp -d)"
cp -R benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release "$tmp/bundle"
python3 - "$tmp/bundle/correctness-summary.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
payload = json.loads(path.read_text(encoding="utf-8"))
payload["status"] = "failed"
path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
python3 tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py --dir "$tmp/bundle"
```

Expected: nonzero exit, stderr contains `correctness-summary status must be pass`, and stderr does not mention `artifact`.

- [ ] **Step 5: Run repository-required Rust verification**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Inspect status and commit any final fixes**

Run:

```bash
git status --short
git log --oneline origin/master..HEAD
```

Expected: no uncommitted changes, with commits for the design, plan, checker, and data migration.

## Plan Self-Review

- Spec coverage: Task 1 covers default portability and optional runtime binary attestation; Task 2 covers committed provenance and preserved artifact hashes; Task 3 covers issue verification, negative controls, and `cargo test`.
- Placeholder scan: no TBD/TODO placeholders remain.
- Type consistency: the checker API uses `Path | None` for optional runtime attestation and `dict[str, str]` for normalized schema-v2 runtime identities throughout.
