# Issue 482 Reference-Build Portable Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make reference-build evidence validation portable by recording runtime identity separately from optional live binary attestation.

**Architecture:** The checked bundle records logical command roles and runtime identities. The reference-build checker validates artifact semantics and recorded identity by default, and only opens a supplied binary when `--verify-runtime-binary` is passed. The runner writes the same portable environment shape for future regenerations without rerunning the checked bundle.

**Tech Stack:** Python standard library (`argparse`, `hashlib`, `json`, `pathlib`, `subprocess`, `unittest`), existing benchmark runner/checker modules, Cargo for final Rust verification.

## Global Constraints

- Do not rerun reference construction or regenerate benchmark timing data.
- Do not change `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/raw.jsonl`.
- Preserve `summary.json` SHA-256 `614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5`.
- Preserve `report.md` SHA-256 `4a6a2dae36b546be472990651a27be20bfd11f1a3c15e9963a1e212bade1f6ef`.
- Default checker validation must not open `target/release/rstim_reference_build_worker`.
- Optional runtime verification must fail with `runtime binary SHA-256 does not match recorded identity` for a mismatched supplied binary.
- Use logical executable roles in recorded runner and worker argv.
- Keep repository inputs as repo-relative POSIX paths.

---

### Task 1: Checker Tests For Portable Runtime Identity

**Files:**
- Modify: `tools/test_check_rstim_vs_stim_reference_build_evidence.py`

**Interfaces:**
- Consumes: existing synthetic bundle helpers `write_valid_bundle`, `rewrite_json`, `rewrite_artifact_hashes`, and `run_checker`.
- Produces: failing tests for portable runtime identities and `--verify-runtime-binary` that Task 2 makes pass.

- [ ] **Step 1: Add logical role constants and remove live target setup**

Replace the `ensure_canonical_rstim_worker_binary` helper with role constants and a temp runtime helper:

```python
PYTHON_ROLE = "tool://python"
STIM_WORKER_ROLE = "tool://stim-reference-worker"
RSTIM_WORKER_ROLE = "tool://rstim-reference-worker"
STIM_WORKER_REL = "benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py"
RSTIM_WORKER_VERSION = "rstim 0.1.1"


def write_runtime_binary(path: Path, content: bytes = b"test rstim reference build worker\n") -> Path:
    path.write_bytes(content)
    path.chmod(0o755)
    return path
```

- [ ] **Step 2: Change `write_valid_bundle` to write portable environment JSON**

Use this environment shape inside `write_valid_bundle`; keep raw, summary, report, and artifact hash logic unchanged:

```python
    runner_python = Path(sys.executable).resolve()
    stim_worker_module = REPO_ROOT / STIM_WORKER_REL
    environment = {
        "profile": "release",
        "protocol": PROTOCOL,
        "timer_scope": TIMER_SCOPE,
        "seed_policy": "deterministic_no_seed_reference_builds",
        "fixture_path": FIXTURE_REL,
        "fixture_sha256": FIXTURE_DIGEST,
        "manifest_path": MANIFEST_REL,
        "manifest_sha256": MANIFEST_DIGEST,
        "stim_version": "1.15.0",
        "worker_argv": {
            STIM_VARIANT: [PYTHON_ROLE, "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build", "--protocol", PROTOCOL],
            RSTIM_VARIANT: [RSTIM_WORKER_ROLE, "--protocol", PROTOCOL],
        },
        "canonical_worker_argv": {
            STIM_VARIANT: [PYTHON_ROLE, "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build", "--protocol", PROTOCOL],
            RSTIM_VARIANT: [RSTIM_WORKER_ROLE, "--protocol", PROTOCOL],
        },
        "runner_argv": [
            PYTHON_ROLE,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark",
            "--fixture",
            FIXTURE_REL,
            "--manifest",
            MANIFEST_REL,
            "--stim-python",
            PYTHON_ROLE,
            "--rstim-worker",
            RSTIM_WORKER_ROLE,
            "--warmup-rounds",
            "2",
            "--measure-rounds",
            "7",
            "--out-dir",
            str(path),
        ],
        "runtime_identities": [
            {
                "role": PYTHON_ROLE,
                "version": sys.version.split()[0],
                "basename": runner_python.name,
                "sha256": sha256_file(runner_python),
            },
            {
                "role": STIM_WORKER_ROLE,
                "version": "1.15.0",
                "basename": "stim_reference_build.py",
                "sha256": sha256_file(stim_worker_module),
            },
            {
                "role": RSTIM_WORKER_ROLE,
                "version": RSTIM_WORKER_VERSION,
                "basename": "rstim_reference_build_worker",
                "sha256": sha256_file(rstim_worker),
            },
        ],
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "git_commit": command_stdout(["git", "rev-parse", "HEAD"]),
        "git_dirty": False,
        "os": "test-os",
        "cpu_model": "test-cpu",
        "rustc_version": "rustc test",
        "cargo_version": "cargo test",
        "python_version": sys.version.split()[0],
    }
```

- [ ] **Step 3: Update setup and checker launcher**

Use a temp recorded binary and allow extra checker arguments:

```python
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.bundle = Path(self.temp_dir.name) / "bundle"
        self.rstim_worker = write_runtime_binary(Path(self.temp_dir.name) / "rstim_reference_build_worker")
        write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

    def run_checker(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--dir", str(self.bundle), *extra_args],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
```

- [ ] **Step 4: Add runtime verification tests**

Add these tests near `test_accepts_valid_bundle`:

```python
    def test_accepts_matching_runtime_binary_when_supplied(self) -> None:
        result = self.run_checker("--verify-runtime-binary", str(self.rstim_worker))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS packed reference-build evidence\n")

    def test_rejects_supplied_runtime_binary_with_different_sha(self) -> None:
        other = write_runtime_binary(Path(self.temp_dir.name) / "different-worker", b"different worker\n")
        result = self.run_checker("--verify-runtime-binary", str(other))
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("runtime binary SHA-256 does not match recorded identity", result.stderr)
```

- [ ] **Step 5: Replace legacy executable-path negative controls**

Replace the old runner-Python-path and executable-hash tests with runtime identity tests:

```python
    def test_rejects_rehashed_environment_bad_runtime_identity_sha_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runtime_identities"][0]["sha256"] = "not-a-sha"

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities tool://python sha256", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_missing_runtime_identity_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runtime_identities"] = [
                identity
                for identity in environment["runtime_identities"]
                if identity["role"] != RSTIM_WORKER_ROLE
            ]

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities must contain exactly", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)
```

- [ ] **Step 6: Run tests to verify failure before implementation**

Run: `python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence -q`

Expected: FAIL because the checker still requires legacy executable paths and does not accept `--verify-runtime-binary`.

### Task 2: Implement Portable Checker Behavior

**Files:**
- Modify: `tools/check_rstim_vs_stim_reference_build_evidence.py`

**Interfaces:**
- Consumes: portable `environment.json` with `runtime_identities`.
- Produces: `validate_bundle(results_dir: Path, verify_runtime_binary: Path | None = None) -> None` and CLI `--verify-runtime-binary`.

- [ ] **Step 1: Add role and identity constants**

Add after the existing path/hash constants:

```python
PYTHON_ROLE = "tool://python"
STIM_WORKER_ROLE = "tool://stim-reference-worker"
RSTIM_WORKER_ROLE = "tool://rstim-reference-worker"
EXPECTED_RUNTIME_ROLES = frozenset({PYTHON_ROLE, STIM_WORKER_ROLE, RSTIM_WORKER_ROLE})
EXPECTED_STIM_WORKER = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py"
EXPECTED_RSTIM_WORKER_VERSION = "rstim 0.1.1"
RUNTIME_IDENTITY_FIELDS = frozenset({"role", "version", "basename", "sha256"})
LEGACY_RUNTIME_PATH_FIELDS = frozenset(
    {
        "python_executable",
        "python_executable_sha256",
        "runner_python_executable",
        "runner_python_executable_sha256",
        "rstim_worker_binary_path",
        "rstim_worker_binary_sha256",
    }
)
```

- [ ] **Step 2: Add runtime identity parsing and validation helpers**

Add these helpers before `_validate_worker_argv`:

```python
def _validate_runtime_identities(environment: dict[str, Any]) -> dict[str, dict[str, str]]:
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must be an array")
    by_role: dict[str, dict[str, str]] = {}
    for index, identity in enumerate(identities):
        if not isinstance(identity, dict):
            raise ValueError(f"environment runtime_identities[{index}] must be a JSON object")
        unsupported = sorted(set(identity) - RUNTIME_IDENTITY_FIELDS)
        if unsupported:
            raise ValueError(
                f"environment runtime_identities[{index}] unsupported field(s): {', '.join(unsupported)}"
            )
        role = identity.get("role")
        if not isinstance(role, str) or role not in EXPECTED_RUNTIME_ROLES:
            raise ValueError(f"environment runtime_identities[{index}] role must be an expected tool:// role")
        if role in by_role:
            raise ValueError(f"environment runtime_identities duplicate role: {role}")
        version = identity.get("version")
        basename = identity.get("basename")
        digest = identity.get("sha256")
        if not isinstance(version, str) or not version:
            raise ValueError(f"environment runtime_identities {role} version must be nonempty")
        if not isinstance(basename, str) or not basename or "/" in basename or "\\" in basename:
            raise ValueError(f"environment runtime_identities {role} basename must be a filename")
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError(f"environment runtime_identities {role} sha256 must be a lowercase SHA-256 digest")
        by_role[role] = {
            "role": role,
            "version": version,
            "basename": basename,
            "sha256": digest,
        }

    if set(by_role) != EXPECTED_RUNTIME_ROLES:
        roles = ", ".join(sorted(EXPECTED_RUNTIME_ROLES))
        raise ValueError(f"environment runtime_identities must contain exactly: {roles}")
    if by_role[PYTHON_ROLE]["version"] != environment.get("python_version"):
        raise ValueError("environment runtime_identities tool://python version must match python_version")
    stim_identity = by_role[STIM_WORKER_ROLE]
    if stim_identity["version"] != environment.get("stim_version"):
        raise ValueError("environment runtime_identities tool://stim-reference-worker version must match stim_version")
    if stim_identity["basename"] != "stim_reference_build.py":
        raise ValueError("environment runtime_identities tool://stim-reference-worker basename must be stim_reference_build.py")
    if stim_identity["sha256"] != sha256_file(EXPECTED_STIM_WORKER):
        raise ValueError("environment runtime_identities tool://stim-reference-worker sha256 must match canonical Stim worker")
    rstim_identity = by_role[RSTIM_WORKER_ROLE]
    if rstim_identity["version"] != EXPECTED_RSTIM_WORKER_VERSION:
        raise ValueError(f"environment runtime_identities tool://rstim-reference-worker version must be {EXPECTED_RSTIM_WORKER_VERSION}")
    if rstim_identity["basename"] != "rstim_reference_build_worker":
        raise ValueError("environment runtime_identities tool://rstim-reference-worker basename must be rstim_reference_build_worker")
    return by_role


def _validate_no_legacy_runtime_paths(environment: dict[str, Any]) -> None:
    present = sorted(field for field in LEGACY_RUNTIME_PATH_FIELDS if field in environment)
    if present:
        raise ValueError(f"environment legacy runtime path field is not portable: {present[0]}")


def _verify_runtime_binary(path: Path, identity: dict[str, str]) -> None:
    if not path.is_file():
        raise ValueError(f"runtime binary does not exist: {path}")
    if sha256_file(path) != identity["sha256"]:
        raise ValueError("runtime binary SHA-256 does not match recorded identity")
```

- [ ] **Step 3: Rewrite worker argv validation for logical roles**

Replace `_validate_worker_argv` body so it no longer resolves executable paths:

```python
    expected_canonical = {
        runner.STIM_VARIANT: runner.default_stim_worker_argv(PYTHON_ROLE),
        runner.RSTIM_VARIANT: runner.default_rstim_worker_argv(RSTIM_WORKER_ROLE),
    }
    if canonical_worker_argv != expected_canonical:
        raise ValueError("environment canonical_worker_argv must match release reference-build commands")

    stim_argv = _validate_string_list(worker_argv[runner.STIM_VARIANT], f"environment worker_argv {runner.STIM_VARIANT}")
    rstim_argv = _validate_string_list(worker_argv[runner.RSTIM_VARIANT], f"environment worker_argv {runner.RSTIM_VARIANT}")
    if stim_argv != expected_canonical[runner.STIM_VARIANT]:
        raise ValueError(f"environment worker_argv {runner.STIM_VARIANT} must run the canonical Stim worker")
    if rstim_argv != expected_canonical[runner.RSTIM_VARIANT]:
        raise ValueError(f"environment worker_argv {runner.RSTIM_VARIANT} must run the canonical rstim worker")
```

- [ ] **Step 4: Rewrite runner argv validation for logical roles**

Change `_validate_runner_argv` signature to remove `runner_python_path`, then require `argv[0] == PYTHON_ROLE` and the logical role tail:

```python
def _validate_runner_argv(environment: dict[str, Any], results_dir: Path) -> None:
    argv = _validate_string_list(environment.get("runner_argv"), "environment runner_argv")
    if len(argv) != 17:
        raise ValueError("environment runner_argv must match the full canonical runner command")
    if argv[0] != PYTHON_ROLE:
        raise ValueError("environment runner_argv executable must be tool://python")
    if argv[1:4] != ["-m", runner.MODULE_NAME, "--fixture"] or argv[5] != "--manifest":
        raise ValueError("environment runner_argv must invoke the canonical runner module")
    if _resolve_recorded_path(argv[4], "runner_argv fixture") != _resolve_recorded_path(environment.get("fixture_path"), "fixture_path"):
        raise ValueError("environment runner_argv fixture must match fixture_path")
    if _resolve_recorded_path(argv[6], "runner_argv manifest") != _resolve_recorded_path(environment.get("manifest_path"), "manifest_path"):
        raise ValueError("environment runner_argv manifest must match manifest_path")
    expected_tail = [
        "--stim-python",
        PYTHON_ROLE,
        "--rstim-worker",
        RSTIM_WORKER_ROLE,
        "--warmup-rounds",
        "2",
        "--measure-rounds",
        "7",
        "--out-dir",
    ]
    if argv[7:16] != expected_tail or not argv[16]:
        raise ValueError("environment runner_argv must match the full canonical runner command")
    if _resolve_recorded_path(argv[16], "runner_argv --out-dir") != results_dir.resolve():
        raise ValueError("environment runner_argv --out-dir must match checked bundle directory")
```

- [ ] **Step 5: Thread runtime verification through environment and bundle validation**

Update signatures and calls:

```python
def validate_environment(
    environment: dict[str, Any],
    derived: dict[str, Any],
    records: list[dict[str, Any]],
    results_dir: Path,
    verify_runtime_binary: Path | None = None,
) -> None:
    _validate_no_legacy_runtime_paths(environment)
    runtime_identities = _validate_runtime_identities(environment)
    _validate_worker_argv(environment)
    _validate_runner_argv(environment, results_dir)
    if verify_runtime_binary is not None:
        _verify_runtime_binary(verify_runtime_binary, runtime_identities[RSTIM_WORKER_ROLE])
```

Then update:

```python
def validate_bundle(results_dir: Path, verify_runtime_binary: Path | None = None) -> None:
    validate_required_files(results_dir)
    records = load_raw_records(results_dir / "raw.jsonl")
    derived = validate_raw_semantics(records)
    summary = derive_summary(records)
    if load_json_object(results_dir / "summary.json", "summary.json") != summary:
        raise ValueError("summary.json does not match summary derived from raw.jsonl")
    if (results_dir / "report.md").read_text(encoding="utf-8") != render_report(summary):
        raise ValueError("report.md does not match summary.json")
    environment = load_json_object(results_dir / "environment.json", "environment.json")
    validate_environment(environment, derived, records, results_dir, verify_runtime_binary)
    validate_artifact_hashes(results_dir)
```

And add CLI parsing:

```python
    parser.add_argument("--verify-runtime-binary", type=Path)
    try:
        validate_bundle(args.results_dir, args.verify_runtime_binary)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
```

- [ ] **Step 6: Run focused checker tests**

Run: `python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence -q`

Expected: PASS.

### Task 3: Runner Portable Environment Output

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`

**Interfaces:**
- Consumes: the same logical role constants as the checker.
- Produces: future runner-generated `environment.json` with logical argv and `runtime_identities`.

- [ ] **Step 1: Add role constants and portable argv helpers to the runner**

Add constants next to `STIM_WORKER_MODULE`:

```python
PYTHON_ROLE = "tool://python"
STIM_WORKER_ROLE = "tool://stim-reference-worker"
RSTIM_WORKER_ROLE = "tool://rstim-reference-worker"
RSTIM_WORKER_VERSION = "rstim 0.1.1"
STIM_WORKER_PATH = PACKAGE_DIR / "workers/stim_reference_build.py"
```

Add:

```python
def logical_stim_worker_argv() -> list[str]:
    return default_stim_worker_argv(PYTHON_ROLE)


def logical_rstim_worker_argv() -> list[str]:
    return default_rstim_worker_argv(RSTIM_WORKER_ROLE)
```

- [ ] **Step 2: Make recorded runner argv portable**

Replace `_runner_argv` with:

```python
def _runner_argv(args: argparse.Namespace) -> list[str]:
    return [
        PYTHON_ROLE,
        "-m",
        MODULE_NAME,
        "--fixture",
        _repo_relative_or_abs(args.fixture),
        "--manifest",
        _repo_relative_or_abs(args.manifest),
        "--stim-python",
        PYTHON_ROLE,
        "--rstim-worker",
        RSTIM_WORKER_ROLE,
        "--warmup-rounds",
        str(args.warmup_rounds),
        "--measure-rounds",
        str(args.measure_rounds),
        "--out-dir",
        _repo_relative_or_abs(args.out_dir),
    ]
```

- [ ] **Step 3: Record runtime identities instead of legacy path fields**

In `collect_environment`, keep resolving the actual binaries to hash them, but replace the legacy argv/path entries in the returned dict with this portable block:

```python
"worker_argv": {
    STIM_VARIANT: logical_stim_worker_argv(),
    RSTIM_VARIANT: logical_rstim_worker_argv(),
},
"canonical_worker_argv": {
    STIM_VARIANT: logical_stim_worker_argv(),
    RSTIM_VARIANT: logical_rstim_worker_argv(),
},
"runner_argv": _runner_argv(args),
"runtime_identities": [
    {
        "role": PYTHON_ROLE,
        "version": platform.python_version(),
        "basename": runner_python_path.name,
        "sha256": sha256_file(runner_python_path),
    },
    {
        "role": STIM_WORKER_ROLE,
        "version": stim_version,
        "basename": STIM_WORKER_PATH.name,
        "sha256": sha256_file(STIM_WORKER_PATH),
    },
    {
        "role": RSTIM_WORKER_ROLE,
        "version": RSTIM_WORKER_VERSION,
        "basename": "rstim_reference_build_worker",
        "sha256": sha256_file(rstim_worker_path),
    },
],
```

Remove these returned keys: `runner_python_executable`, `runner_python_executable_sha256`, `python_executable`, `python_executable_sha256`, `rstim_worker_binary_path`, `rstim_worker_binary_sha256`.

- [ ] **Step 4: Update runner environment assertions**

In `test_fake_workers_emit_required_artifacts_and_hash_manifest`, replace the expected key list with `runtime_identities` and remove legacy path keys. Expected argv should be:

```python
            expected_runner_argv = [
                "tool://python",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark",
                "--fixture",
                FIXTURE_REL,
                "--manifest",
                MANIFEST_REL,
                "--stim-python",
                "tool://python",
                "--rstim-worker",
                "tool://rstim-reference-worker",
                "--warmup-rounds",
                "2",
                "--measure-rounds",
                "7",
                "--out-dir",
                str(out_dir),
            ]
```

Expected worker argv should use `tool://python` and `tool://rstim-reference-worker`.

Assert runtime identities with:

```python
            runtime_identities = {identity["role"]: identity for identity in environment["runtime_identities"]}
            self.assertEqual(set(runtime_identities), {"tool://python", "tool://stim-reference-worker", "tool://rstim-reference-worker"})
            expected_runner_python = Path(sys.executable).resolve()
            self.assertEqual(runtime_identities["tool://python"]["version"], platform.python_version())
            self.assertEqual(runtime_identities["tool://python"]["basename"], expected_runner_python.name)
            self.assertEqual(runtime_identities["tool://python"]["sha256"], sha256_file(expected_runner_python))
            self.assertEqual(runtime_identities["tool://stim-reference-worker"]["version"], "1.15.0")
            self.assertEqual(runtime_identities["tool://stim-reference-worker"]["basename"], "stim_reference_build.py")
            self.assertEqual(
                runtime_identities["tool://stim-reference-worker"]["sha256"],
                sha256_file(ROOT / "benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py"),
            )
            self.assertEqual(runtime_identities["tool://rstim-reference-worker"]["version"], "rstim 0.1.1")
            self.assertEqual(runtime_identities["tool://rstim-reference-worker"]["basename"], "rstim_reference_build_worker")
            self.assertEqual(runtime_identities["tool://rstim-reference-worker"]["sha256"], sha256_file(rstim_worker))
```

- [ ] **Step 5: Run runner unit tests**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark -q`

Expected: PASS.

### Task 4: Migrate Checked Bundle And Catalog Hashes

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: Task 2 checker and Task 3 portable environment shape.
- Produces: checked-in bundle that validates without live runtime binary.

- [ ] **Step 1: Replace checked `environment.json` runtime fields**

Use logical argv and this `runtime_identities` array:

```json
[
  {
    "basename": "python3.14",
    "role": "tool://python",
    "sha256": "cbf84109626aa1013bbe408fbb9590bd0f1c1548f038b2221c6b8b87de26ca43",
    "version": "3.14.3"
  },
  {
    "basename": "stim_reference_build.py",
    "role": "tool://stim-reference-worker",
    "sha256": "04de9822e624e5b08094ae3bd213b118aaf3ad296cb0aab2e0b9492c9aa32ec9",
    "version": "1.15.0"
  },
  {
    "basename": "rstim_reference_build_worker",
    "role": "tool://rstim-reference-worker",
    "sha256": "82d395176ebe76d6890bb9e747771fac46a019867287f69b0da8d6d5075e1265",
    "version": "rstim 0.1.1"
  }
]
```

Remove `python_executable`, `python_executable_sha256`, `runner_python_executable`, `runner_python_executable_sha256`, `rstim_worker_binary_path`, and `rstim_worker_binary_sha256`.

- [ ] **Step 2: Recompute only changed hashes**

Run:

```sh
shasum -a 256 benchmarks/rstim_vs_stim_simulator/results/reference-build-release/environment.json
shasum -a 256 benchmarks/rstim_vs_stim_simulator/results/reference-build-release/artifact-sha256.json
shasum -a 256 benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json
shasum -a 256 benchmarks/rstim_vs_stim_simulator/results/reference-build-release/report.md
```

Expected: `summary.json` and `report.md` remain the required issue hashes. Update `artifact-sha256.json` with the new environment digest, then update `evidence_bundles.toml` with the new `environment.json` and `artifact-sha256.json` artifact digests for `reference-build-release`.

- [ ] **Step 3: Run bundle checker and catalog checker**

Run:

```sh
python3 tools/check_rstim_vs_stim_reference_build_evidence.py --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS packed reference-build evidence
PASS portable evidence catalog bundles=4 schema=2
```

### Task 5: Archive Verification, Full Tests, Commit, PR

**Files:**
- Modify: implementation files from Tasks 1-4
- Create/modify: no additional source files

**Interfaces:**
- Consumes: all prior tasks.
- Produces: committed and pushed worker branch with a pull request against `master`.

- [ ] **Step 1: Run issue verification exactly**

Run:

```sh
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && test ! -e target/release/rstim_reference_build_worker && \
  python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release)
```

Expected output from both checker calls:

```text
PASS packed reference-build evidence
```

- [ ] **Step 2: Run focused Python tests**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence -q
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark -q
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: each command exits 0 with `OK`.

- [ ] **Step 3: Run required broad verification**

Run: `cargo test`

Expected: all Rust tests pass.

- [ ] **Step 4: Inspect final diff and commit implementation**

Run:

```sh
git status --short
git diff --check
git add benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py \
  benchmarks/rstim_vs_stim_simulator/results/reference-build-release/environment.json \
  benchmarks/rstim_vs_stim_simulator/results/reference-build-release/artifact-sha256.json \
  benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  tools/check_rstim_vs_stim_reference_build_evidence.py \
  tools/test_check_rstim_vs_stim_reference_build_evidence.py \
  docs/superpowers/plans/2026-07-12-issue-482-reference-build-portable-provenance.md
git commit -m "fix: migrate reference-build provenance"
```

- [ ] **Step 5: Push and create PR**

Use `superpowers:finishing-a-development-branch`, choose "Push and create a Pull Request", then run:

```sh
git push -u origin agent/issue-482-migrate-reference-build-evidence-to-portable-pro-run-1
gh pr create \
  --repo nzy1997/rstim \
  --base master \
  --head agent/issue-482-migrate-reference-build-evidence-to-portable-pro-run-1 \
  --title "Migrate reference-build evidence to portable provenance" \
  --body-file /tmp/issue-482-pr-body.md
```

The PR body must include summary, verification commands with outcomes, and `Closes #482`.
