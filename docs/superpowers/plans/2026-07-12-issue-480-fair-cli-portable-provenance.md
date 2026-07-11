# Issue 480 Fair CLI Portable Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate fair CLI checked evidence to schema-v2 logical provenance without rerunning the benchmark or changing summary/report bytes.

**Architecture:** Keep execution-time local paths inside the runner, but record logical `tool://` argv and repo-relative fixture paths in raw/environment artifacts. Update the checker to validate that portable shape, runtime identities, raw-derived summary/report bytes, historical #406 separation, and artifact hashes in the same semantic-before-hash order. Rewrite only fair bundle provenance files and catalog digests affected by those provenance changes.

**Tech Stack:** Python standard library (`argparse`, `hashlib`, `json`, `pathlib`, `re`, `subprocess`, `tempfile`, `unittest`), existing `benchmarks.rstim_vs_stim_simulator.fair_cli_contract`, `run_fair_cli`, and schema-v2 `evidence_bundles.toml`.

## Global Constraints

- Do not rerun the fair CLI benchmark.
- Do not change any recorded `elapsed_ns`, `actual_output_bytes`, `stdout_sha256`, summary bytes, or report bytes.
- `summary.json` SHA-256 must remain `131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07`.
- `report.md` SHA-256 must remain `1b28385ccf1523fac930feb4dc11542751884bdf99416e98815e0591d1960e51`.
- Raw `argv` must use `tool://stim` or `tool://rstim` and the repo-relative fixture path.
- `environment.json` must record runtime identities without original absolute executable paths.
- The checker must derive `summary.json` and `report.md` from raw records.
- The checker must preserve the historical #406 digest guard for `results/full/speed-summary.json`.
- Host-absolute argv paths must fail semantically before artifact hashes.
- Do not update the site or overwrite historical #406 evidence.
- Required final verification includes the working-checkout checker command, relocated archive checker command, negative control, focused unittests, portable catalog validation, and `cargo test`.

---

## File Structure

- Modify `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`: keep real subprocess argv for execution, add logical recorded argv/runtime identity helpers, and write portable environment provenance.
- Modify `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`: assert recorded raw/environment provenance is logical while subprocess invocation logs still prove real binaries were executed.
- Modify `tools/check_rstim_vs_stim_fair_cli_evidence.py`: validate logical argv, runtime identities, no host paths, repo-relative input hashes, raw-derived summary/report, historical digest, and artifact hashes.
- Modify `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`: update temporary bundle helper to schema-v2 provenance and add #480 negative controls.
- Modify `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/raw.jsonl`: replace only recorded argv executable and fixture path fields.
- Modify `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/environment.json`: remove live binary paths, add runtime identities, and rewrite recorded argv fields.
- Modify `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/artifact-sha256.json`: refresh digests for changed fair bundle artifacts only.
- Modify `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`: refresh fair bundle artifact digests for changed files only.

### Task 1: Add Portable Provenance Tests

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`
- Modify: `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`

**Interfaces:**
- Consumes: existing `run_fair_cli.run_fair_cli(args, repo_root=ROOT)`.
- Produces: failing tests that define `logical_argv(variant: str, seed: int, *, shots: int = 1024, input_token: str = FIXTURE_REPO_PATH) -> list[str]` expectations and checker negative controls.

- [ ] **Step 1: Add logical argv constants to runner tests**

In `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`, add these constants near the existing case constants:

```python
FIXTURE_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}
```

Replace `expected_argv(binary: Path, seed: int)` with:

```python
def expected_execution_argv(binary: Path, seed: int) -> list[str]:
    return [
        str(binary.resolve()),
        "sample",
        "--shots",
        str(SHOTS),
        "--seed",
        str(seed),
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        str(FIXTURE),
    ]


def expected_recorded_argv(variant: str, seed: int) -> list[str]:
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(SHOTS),
        "--seed",
        str(seed),
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        FIXTURE_REPO_PATH,
    ]


def expected_preflight_argv(variant: str) -> list[str]:
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        "1",
        "--seed",
        "0",
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        KNOWN_ANSWER_INPUT_TOKEN,
    ]
```

- [ ] **Step 2: Update runner artifact assertions**

In `RunFairCliTest.assert_artifacts`, change the argv assertion loop to:

```python
for variant in ("stim-cli-b8", "rstim-cli-b8"):
    variant_records = [record for record in records if record["variant"] == variant]
    self.assertEqual([record["seed"] for record in variant_records], list(range(9)))
    self.assertEqual(
        [(record["phase"], record["round_index"]) for record in variant_records],
        [("warmup", 0), ("warmup", 1)] + [("measured", index) for index in range(7)],
    )
    for record in variant_records:
        self.assertEqual(record["argv"], expected_recorded_argv(variant, record["seed"]))
```

Change environment binary assertions to runtime identities:

```python
self.assertNotIn("stim_binary", environment)
self.assertNotIn("rstim_binary", environment)
self.assertNotIn("stim_binary_sha256", environment)
self.assertNotIn("rstim_binary_sha256", environment)
self.assertEqual(
    environment["runtime_identities"],
    [
        {
            "role": "tool://stim",
            "version": "1.15.0",
            "basename": "stim",
            "sha256": hashlib.sha256(stim.read_bytes()).hexdigest(),
        },
        {
            "role": "tool://rstim",
            "version": "rstim 0.0.0-test",
            "basename": "rstim",
            "sha256": hashlib.sha256(rstim.read_bytes()).hexdigest(),
        },
    ],
)
```

After validating `known_answer_preflight`, add:

```python
details_by_variant = {
    detail["variant"]: detail
    for detail in environment["known_answer_preflight_details"]
}
self.assertEqual(set(details_by_variant), {"stim-cli-b8", "rstim-cli-b8"})
for variant, detail in details_by_variant.items():
    self.assertEqual(detail["argv"], expected_preflight_argv(variant))
```

- [ ] **Step 3: Keep execution assertions on real paths**

In the same test file, replace assertions against logged benchmark invocations with `expected_execution_argv(binary, seed)` so the test proves execution still used real files:

```python
self.assertEqual(binary_invocations, [expected_execution_argv(binary, seed) for seed in range(9)])
```

Do not change the preflight invocation-log checks that assert `--shots 1` on the real subprocess calls.

- [ ] **Step 4: Update checker temporary bundle helper**

In `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`, add:

```python
FIXTURE_REPO_PATH = fair_cli_contract.EXPECTED_CASE["canonical_input_path"]
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}
```

Add helper functions equivalent to the runner test helpers:

```python
def expected_recorded_argv(variant: str, seed: int) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(case["shots"]),
        "--seed",
        str(seed),
        "--out_format",
        case["output_format"],
        "--in",
        FIXTURE_REPO_PATH,
    ]


def expected_preflight_argv(variant: str) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        "1",
        "--seed",
        "0",
        "--out_format",
        case["output_format"],
        "--in",
        KNOWN_ANSWER_INPUT_TOKEN,
    ]
```

Change `write_valid_bundle` so raw records and `environment["round_argv"]` use `expected_recorded_argv(variant, seed)`, and environment uses:

```python
"runtime_identities": [
    {
        "role": "tool://stim",
        "version": case["stim_version"],
        "basename": "stim",
        "sha256": sha256_file(stim_binary),
    },
    {
        "role": "tool://rstim",
        "version": "rstim test",
        "basename": "rstim",
        "sha256": sha256_file(rstim_binary),
    },
],
```

Remove `stim_binary`, `stim_binary_sha256`, `rstim_binary`, and `rstim_binary_sha256` from the temporary environment. Change preflight detail argv to `expected_preflight_argv(variant)`.

- [ ] **Step 5: Add checker negative controls**

Add these tests to `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`:

```python
def test_rejects_host_absolute_raw_fixture_argument_before_artifact_hashes(self) -> None:
    records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
    records[0]["argv"][records[0]["argv"].index("--in") + 1] = "/tmp/copied-fixture.stim"
    (self.bundle / "raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("stim-cli-b8 argv contains a host-absolute path", result.stderr)
    self.assertNotIn("artifact-sha256.json", result.stderr)


def test_rejects_legacy_environment_binary_paths(self) -> None:
    def add_live_paths(environment: dict[str, Any]) -> None:
        environment["stim_binary"] = "/opt/homebrew/bin/stim"
        environment["stim_binary_sha256"] = "a" * 64
        environment["rstim_binary"] = "/tmp/rstim"
        environment["rstim_binary_sha256"] = "b" * 64

    rewrite_json(self.bundle / "environment.json", add_live_paths)
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("environment must not contain live runtime path fields", result.stderr)
```

- [ ] **Step 6: Run RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected: tests fail because the runner and checker still record/expect live paths.

- [ ] **Step 7: Commit tests**

If only tests changed, commit them:

```sh
git add benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py tools/test_check_rstim_vs_stim_fair_cli_evidence.py
git commit -m "test: require fair cli portable provenance"
```

### Task 2: Record Logical Provenance In The Runner

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`
- Test: `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`

**Interfaces:**
- Consumes: `time_cli(argv: list[str], *, cwd: Path) -> CliResult` still receives real execution argv.
- Produces: `recorded_argv(variant: str, *, case: dict[str, Any], shots: int, seed: int, input_token: str | None = None) -> list[str]` and `runtime_identity(role: str, version: str, binary: Path) -> dict[str, str]`.

- [ ] **Step 1: Add runner constants and helper functions**

In `run_fair_cli.py`, add after `KNOWN_ANSWER_OUTPUT`:

```python
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}
```

Add helpers after `_expand_argv`:

```python
def _recorded_argv(
    *,
    variant: str,
    case: dict[str, Any],
    shots: int,
    seed: int,
    input_token: str | None = None,
) -> list[str]:
    if variant not in TOOL_ROLES:
        raise RuntimeError(f"unsupported fair CLI variant: {variant}")
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        case["output_format"],
        "--in",
        input_token if input_token is not None else case["canonical_input_path"],
    ]


def _runtime_identity(*, role: str, version: str, binary: Path) -> dict[str, str]:
    return {
        "role": role,
        "version": version,
        "basename": binary.name,
        "sha256": _sha256_file(binary),
    }
```

- [ ] **Step 2: Record logical preflight argv**

In `_run_known_answer_preflight`, keep `argv = _expand_argv(...)` for `time_cli`.
Change the result append block to:

```python
results.append(
    {
        "variant": variant,
        "argv": _recorded_argv(
            variant=variant,
            case=case,
            shots=1,
            seed=0,
            input_token=KNOWN_ANSWER_INPUT_TOKEN,
        ),
        "exit_code": result.exit_code,
        "stdout_hex": result.stdout.hex(),
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "elapsed_ns": result.elapsed_ns,
    }
)
```

- [ ] **Step 3: Record logical raw argv**

In `_run_rounds`, keep the execution `argv = _expand_argv(...)` and pass it to `time_cli`. Change the `_raw_record` call to:

```python
records.append(
    _raw_record(
        case=case,
        variant=variant,
        phase=phase,
        round_index=round_index,
        seed=round_seed,
        argv=_recorded_argv(
            variant=variant,
            case=case,
            shots=case["shots"],
            seed=round_seed,
        ),
        result=result,
    )
)
```

- [ ] **Step 4: Replace environment live binary fields**

In `_collect_environment`, remove `stim_binary`, `stim_binary_sha256`,
`rstim_binary`, and `rstim_binary_sha256`. Add:

```python
"runtime_identities": [
    _runtime_identity(role="tool://stim", version=stim_version, binary=stim_binary),
    _runtime_identity(role="tool://rstim", version=rstim_version, binary=rstim_binary),
],
```

Keep `round_argv` derived from `records`, which now contain logical argv.

- [ ] **Step 5: Run runner tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected: all tests pass.

- [ ] **Step 6: Commit runner implementation**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_fair_cli.py benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py
git commit -m "feat: record fair cli logical provenance"
```

### Task 3: Validate Logical Provenance In The Checker

**Files:**
- Modify: `tools/check_rstim_vs_stim_fair_cli_evidence.py`
- Test: `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`

**Interfaces:**
- Consumes: raw/environment artifacts using `runtime_identities` and logical argv.
- Produces: `validate_bundle(results_dir: Path) -> tuple[int, int]` accepting portable fair bundles and rejecting host paths with semantic errors.

- [ ] **Step 1: Add checker constants and host-path detection**

In `tools/check_rstim_vs_stim_fair_cli_evidence.py`, import `PurePosixPath` and `PureWindowsPath` from `pathlib`.

Add:

```python
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}
EXPECTED_RUNTIME_IDENTITIES = (
    {
        "role": "tool://stim",
        "version": "1.15.0",
        "basename": "stim",
        "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323",
    },
    {
        "role": "tool://rstim",
        "version": "rstim 0.1.1",
        "basename": "rstim",
        "sha256": "2db6fa113495235829ca1dc7e4f8080befe3e6336f8effb61800b9e84510182a",
    },
)
LIVE_RUNTIME_PATH_FIELDS = frozenset(
    {"stim_binary", "stim_binary_sha256", "rstim_binary", "rstim_binary_sha256"}
)
POSIX_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])/(?!/)")
WINDOWS_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])([A-Za-z]:[\\/]|\\\\)")
```

Add helpers:

```python
def contains_host_absolute_path(value: object) -> bool:
    if isinstance(value, str):
        return (
            PurePosixPath(value).is_absolute()
            or bool(PureWindowsPath(value).drive and PureWindowsPath(value).is_absolute())
            or POSIX_ABSOLUTE_RE.search(value) is not None
            or WINDOWS_ABSOLUTE_RE.search(value) is not None
        )
    if isinstance(value, list):
        return any(contains_host_absolute_path(item) for item in value)
    if isinstance(value, tuple):
        return any(contains_host_absolute_path(item) for item in value)
    if isinstance(value, dict):
        return any(
            contains_host_absolute_path(key) or contains_host_absolute_path(item)
            for key, item in value.items()
        )
    return False
```

- [ ] **Step 2: Replace expected argv reconstruction**

Replace `_expected_argv` with:

```python
def _expected_argv(variant: str, *, seed: int) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(case["shots"]),
        "--seed",
        str(seed),
        "--out_format",
        case["output_format"],
        "--in",
        case["canonical_input_path"],
    ]


def _expected_preflight_argv(variant: str) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        "1",
        "--seed",
        "0",
        "--out_format",
        case["output_format"],
        "--in",
        KNOWN_ANSWER_INPUT_TOKEN,
    ]
```

In `validate_raw_semantics`, before comparing expected argv, add:

```python
argv = record.get("argv")
if contains_host_absolute_path(argv):
    raise ValueError(f"{variant} argv contains a host-absolute path")
expected_argv = _expected_argv(variant, seed=record["seed"])
require_equal(argv, expected_argv, f"{variant} argv must match canonical argv")
```

- [ ] **Step 3: Remove live binary path validation**

Delete `_validate_path_hash` usage for `stim_binary` and `rstim_binary`. Keep path/hash validation only for:

```python
("fair_manifest_path", "fair_manifest_sha256"),
("source_manifest_path", "source_manifest_sha256"),
("fixture_path", "fixture_sha256"),
```

Keep `_resolve_recorded_path` for repo-relative manifest/fixture fields.

- [ ] **Step 4: Add runtime identity validation**

Add:

```python
def _validate_runtime_identities(environment: dict[str, Any]) -> None:
    forbidden = sorted(set(environment) & LIVE_RUNTIME_PATH_FIELDS)
    if forbidden:
        raise ValueError("environment must not contain live runtime path fields")
    identities = environment.get("runtime_identities")
    if not isinstance(identities, list):
        raise ValueError("environment runtime_identities must be a list")
    if identities != list(EXPECTED_RUNTIME_IDENTITIES):
        raise ValueError("environment runtime_identities must match canonical runtime identities")
    for identity in identities:
        if set(identity) != {"role", "version", "basename", "sha256"}:
            raise ValueError("environment runtime identity must contain only role, version, basename, and sha256")
        if contains_host_absolute_path(identity):
            raise ValueError("environment runtime identity contains a host-absolute path")
        digest = identity.get("sha256")
        if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
            raise ValueError("environment runtime identity sha256 must be a lowercase SHA-256 digest")
```

Call `_validate_runtime_identities(environment)` in `validate_environment` after fixed environment fields are checked.

- [ ] **Step 5: Update preflight validation**

In `_validate_preflight_detail`, remove use of `environment["stim_binary"]` and `environment["rstim_binary"]`. Replace the argv shape check with:

```python
argv = detail.get("argv")
if contains_host_absolute_path(argv):
    raise ValueError(f"{variant} known-answer preflight argv contains a host-absolute path")
require_equal(argv, _expected_preflight_argv(variant), f"{variant} known-answer preflight argv must match canonical shape")
```

Keep exit code, stdout hex, stdout hash, and elapsed validation unchanged.

- [ ] **Step 6: Check round argv for host paths**

Before the `round_argv` equality check in `validate_environment`, add:

```python
if contains_host_absolute_path(environment.get("round_argv")):
    raise ValueError("environment round_argv contains a host-absolute path")
```

- [ ] **Step 7: Run checker tests**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected: all tests pass.

- [ ] **Step 8: Commit checker implementation**

```sh
git add tools/check_rstim_vs_stim_fair_cli_evidence.py tools/test_check_rstim_vs_stim_fair_cli_evidence.py
git commit -m "fix: validate fair cli portable provenance"
```

### Task 4: Migrate The Committed Fair Bundle

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/raw.jsonl`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`

**Interfaces:**
- Consumes: checker from Task 3.
- Produces: committed fair bundle accepted in the working checkout and after archive relocation.

- [ ] **Step 1: Write a one-off migration script in `/tmp`**

Create `/tmp/migrate_fair_cli_bundle.py` with logic that:

- loads `raw.jsonl`;
- for every record, replaces `argv[0]` with `tool://stim` or `tool://rstim`;
- replaces each `--in` value with `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`;
- preserves all other raw record fields and values;
- loads `environment.json`;
- removes `stim_binary`, `stim_binary_sha256`, `rstim_binary`, and `rstim_binary_sha256`;
- adds `runtime_identities` matching the existing published binary versions and SHA-256 values;
- rewrites `round_argv` from migrated raw records;
- rewrites each preflight detail argv to the logical preflight argv;
- writes JSON/JSONL using the repository's existing sorted JSON formatting.

The script's constants must be:

```python
FIXTURE = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {"stim-cli-b8": "tool://stim", "rstim-cli-b8": "tool://rstim"}
RUNTIME_IDENTITIES = [
    {
        "role": "tool://stim",
        "version": "1.15.0",
        "basename": "stim",
        "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323",
    },
    {
        "role": "tool://rstim",
        "version": "rstim 0.1.1",
        "basename": "rstim",
        "sha256": "2db6fa113495235829ca1dc7e4f8080befe3e6336f8effb61800b9e84510182a",
    },
]
```

- [ ] **Step 2: Run the migration script**

Run:

```sh
python3 /tmp/migrate_fair_cli_bundle.py
```

Expected: it rewrites only `raw.jsonl`, `environment.json`, and `artifact-sha256.json` in the fair bundle.

- [ ] **Step 3: Verify preserved summary and report digests**

Run:

```sh
python3 - <<'PY'
import hashlib
from pathlib import Path
base = Path("benchmarks/rstim_vs_stim_simulator/results/fair-cli-release")
for name in ("summary.json", "report.md"):
    print(name, hashlib.sha256((base / name).read_bytes()).hexdigest())
PY
```

Expected output contains:

```text
summary.json 131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07
report.md 1b28385ccf1523fac930feb4dc11542751884bdf99416e98815e0591d1960e51
```

- [ ] **Step 4: Refresh fair catalog artifact digests**

Update only the `fair-cli-release` artifact entries in
`benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`:

- `artifact-sha256.json`;
- `environment.json`;
- `raw.jsonl`.

Leave `summary.json` and `report.md` unchanged. Use SHA-256 of current file
bytes.

- [ ] **Step 5: Run bundle and catalog validation**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS fair CLI sampling evidence variants=2 measured=14
PASS portable evidence catalog bundles=4 schema=2
```

- [ ] **Step 6: Commit bundle migration**

```sh
git add benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/raw.jsonl \
  benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/environment.json \
  benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/artifact-sha256.json \
  benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
git commit -m "data: migrate fair cli evidence provenance"
```

### Task 5: Final Verification And Pull Request

**Files:**
- Modify only files required by failed verification.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: pushed branch and pull request targeting `master`.

- [ ] **Step 1: Run focused Python tests**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli tools.test_check_rstim_vs_stim_fair_cli_evidence benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: all tests pass.

- [ ] **Step 2: Run required working-checkout verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

Expected:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 3: Run required relocated archive verification**

Run:

```sh
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release)
```

Expected:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 4: Run required negative control**

In a temporary copied bundle, replace the first `stim-cli-b8` raw `--in` argument
with `/tmp/copied-fixture.stim`, refresh that copied bundle's artifact hashes,
and run the checker against the copy. Expected stderr contains:

```text
stim-cli-b8 argv contains a host-absolute path
```

- [ ] **Step 5: Run portable catalog validation**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

Expected:

```text
PASS portable evidence catalog bundles=4 schema=2
```

- [ ] **Step 6: Run cargo test**

Run:

```sh
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 7: Inspect diff for scope and path leaks**

Run:

```sh
git diff --check
rg -n "/Users/|/private/|/var/folders|/tmp/copied-fixture|/opt/homebrew" \
  benchmarks/rstim_vs_stim_simulator/results/fair-cli-release \
  tools/check_rstim_vs_stim_fair_cli_evidence.py \
  benchmarks/rstim_vs_stim_simulator/run_fair_cli.py
```

Expected: `git diff --check` has no output. The `rg` command has no matches in the migrated bundle/checker/runner except historical design/plan docs are not part of this search.

- [ ] **Step 8: Commit any verification fixes**

If verification required code or data changes, commit them with a focused message:

```sh
git add <changed-files>
git commit -m "fix: complete fair cli provenance verification"
```

- [ ] **Step 9: Push and create PR**

Use the finishing workflow to choose "Push and create a Pull Request". The PR body must include:

```markdown
## Summary
- Migrate fair CLI raw/environment provenance to schema-v2 logical argv and runtime identities.
- Update the runner and checker to use portable provenance without live runtime paths.
- Refresh fair bundle artifact/catalog hashes while preserving summary/report bytes.

## Verification
- `python3 tools/check_rstim_vs_stim_fair_cli_evidence.py --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release` -> `PASS fair CLI sampling evidence variants=2 measured=14`
- relocated `git archive HEAD` checker command -> `PASS fair CLI sampling evidence variants=2 measured=14`
- negative control with `/tmp/copied-fixture.stim` -> failed with `stim-cli-b8 argv contains a host-absolute path`
- `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli tools.test_check_rstim_vs_stim_fair_cli_evidence benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q` -> passed
- `python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml` -> `PASS portable evidence catalog bundles=4 schema=2`
- `cargo test` -> passed

Closes #480
```

## Self-Review

- Every #480 interface requirement is covered by Tasks 2 through 4.
- The negative control is covered by Task 1 tests and Task 5 verification.
- Summary/report SHA preservation is explicitly verified before final checks.
- The plan keeps execution paths internal to the runner and recorded provenance portable.
- No placeholders or unresolved choices remain.
