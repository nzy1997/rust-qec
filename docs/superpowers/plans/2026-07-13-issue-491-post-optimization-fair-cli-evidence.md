# Issue 491 Post-Optimization Fair CLI Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the `fair-cli-release` evidence slot with a pinned pre-optimization baseline, a new post-optimization candidate run, and a checked derived comparison.

**Architecture:** Keep the existing fair CLI raw/summary/report format for the candidate run. Add baseline, comparison, and M3-3 reference cross-link fields around that format, then teach the checker to derive and validate the comparison from raw candidate evidence and the pinned baseline.

**Tech Stack:** Python 3 standard library, unittest, existing `benchmarks.rstim_vs_stim_simulator.run_fair_cli` runner, existing portable evidence catalog, Rust workspace verification through Cargo.

## Global Constraints

- Preserve the current `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json` as `baseline-summary.json` with SHA-256 `131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07`.
- Generate a new symmetric `b8`, 1024-shot, process-spawn-through-exit candidate run.
- Candidate variants remain exactly `stim-cli-b8` and `rstim-cli-b8`.
- Candidate raw records remain two warmups plus seven measured records per variant, seeds `0` through `8`, for `measured=14`.
- Add `comparison.json` derived from baseline and candidate summaries.
- `comparison.json` and the checker must record `baseline_rstim_over_stim == 3.576`.
- `comparison.json` and the checker must record the candidate ratio and candidate-minus-baseline change without making either value a threshold.
- Cross-link candidate `environment.json` to `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json`.
- The reference cross-link strategy must be exactly `direct_inverse_repeat_folded`.
- Reject candidate summary reuse with `candidate summary must differ from pinned baseline summary`.
- Reject a mismatched reference-evidence hash.
- Reject checked prose containing parity wording while candidate ratio is greater than `1.0`.
- Do not update site metadata, close #406, rewrite #406 history, or impose a cross-machine timing threshold.
- Required checker pass line is exactly `PASS fair CLI sampling evidence variants=2 measured=14`.

---

### Task 1: Runner Comparison Artifacts

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`

**Interfaces:**
- Consumes: existing `_summary(records, case=...)`, existing `_render_report(summary)`, committed baseline summary, committed reference-build summary.
- Produces:
  - `BASELINE_SUMMARY_SHA256: str`
  - `REFERENCE_SUMMARY_REPO_PATH: str`
  - `REFERENCE_STRATEGY: str`
  - `_rstim_over_stim(summary: dict[str, Any]) -> float`
  - `_reference_evidence(*, repo_root: Path) -> dict[str, str]`
  - `_comparison(baseline_summary: dict[str, Any], candidate_summary: dict[str, Any], reference_evidence: dict[str, str]) -> dict[str, Any]`
  - `_render_report(summary: dict[str, Any], comparison: dict[str, Any] | None = None) -> str`
  - runner writes `baseline-summary.json`, `comparison.json`, `report.md`, `environment.json`, and `artifact-sha256.json`.

- [ ] **Step 1: Write failing runner tests**

In `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`, add constants near the existing path constants:

```python
BASELINE_SUMMARY = ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json"
REFERENCE_SUMMARY = ROOT / "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json"
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
REFERENCE_STRATEGY = "direct_inverse_repeat_folded"
```

Extend `assert_artifacts` after reading `summary.json`:

```python
baseline = json.loads((out_dir / "baseline-summary.json").read_text(encoding="utf-8"))
comparison = json.loads((out_dir / "comparison.json").read_text(encoding="utf-8"))
self.assertEqual(
    hashlib.sha256((out_dir / "baseline-summary.json").read_bytes()).hexdigest(),
    BASELINE_SUMMARY_SHA256,
)
self.assertEqual(comparison["baseline_rstim_over_stim"], 3.576)
self.assertGreater(comparison["candidate_rstim_over_stim"], 1.0)
self.assertEqual(
    comparison["ratio_delta_from_baseline"],
    round(comparison["candidate_rstim_over_stim"] - comparison["baseline_rstim_over_stim"], 3),
)
self.assertEqual(comparison["reference_strategy"], REFERENCE_STRATEGY)
self.assertEqual(
    comparison["reference_summary_path"],
    "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json",
)
self.assertEqual(
    comparison["reference_summary_sha256"],
    hashlib.sha256(REFERENCE_SUMMARY.read_bytes()).hexdigest(),
)
self.assertNotEqual(summary, baseline)
```

Extend the report assertions:

```python
self.assertIn("Baseline rstim/Stim ratio: 3.576x", report_text)
self.assertIn("Candidate rstim/Stim ratio:", report_text)
self.assertIn("Change from baseline:", report_text)
self.assertNotRegex(report_text, r"(?i)\bparity\b")
```

Extend environment assertions:

```python
reference_evidence = environment["reference_evidence"]
self.assertEqual(reference_evidence["slot"], "reference-build-release")
self.assertEqual(reference_evidence["summary_path"], "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json")
self.assertEqual(reference_evidence["summary_sha256"], hashlib.sha256(REFERENCE_SUMMARY.read_bytes()).hexdigest())
self.assertEqual(reference_evidence["reference_variant"], "rstim-direct-repeat-reference-b8")
self.assertEqual(reference_evidence["reference_strategy"], REFERENCE_STRATEGY)
self.assertEqual(reference_evidence["checker"], "tools/check_rstim_vs_stim_reference_build_evidence.py")
```

At the end of `assert_artifacts`, add artifact hash coverage:

```python
artifact_hashes = json.loads((out_dir / "artifact-sha256.json").read_text(encoding="utf-8"))
self.assertEqual(
    set(artifact_hashes),
    {"raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json"},
)
for filename, digest in artifact_hashes.items():
    self.assertEqual(digest, hashlib.sha256((out_dir / filename).read_bytes()).hexdigest())
```

In `test_main_writes_symmetric_artifacts_for_all_rounds`, add:

```python
self.assertTrue((out_dir / "baseline-summary.json").is_file())
self.assertTrue((out_dir / "comparison.json").is_file())
self.assertTrue((out_dir / "artifact-sha256.json").is_file())
```

Add this preservation-specific test:

```python
def test_preserves_existing_summary_as_baseline_before_candidate_write(self) -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        fake_bin = root / "bin"
        fake_bin.mkdir()
        write_fake_cli(fake_bin / "stim", mode="success")
        rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
        out_dir = root / "out"
        out_dir.mkdir()
        (out_dir / "summary.json").write_bytes(BASELINE_SUMMARY.read_bytes())
        with (
            mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
            mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
        ):
            run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

        self.assertEqual((out_dir / "baseline-summary.json").read_bytes(), BASELINE_SUMMARY.read_bytes())
        self.assertNotEqual(
            hashlib.sha256((out_dir / "summary.json").read_bytes()).hexdigest(),
            BASELINE_SUMMARY_SHA256,
        )
```

- [ ] **Step 2: Run runner tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected: FAIL because the runner does not yet write `baseline-summary.json`, `comparison.json`, `reference_evidence`, or expanded artifact hashes.

- [ ] **Step 3: Implement runner helpers and artifacts**

In `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`, add constants near `FAIR_MANIFEST_REPO_PATH`:

```python
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
BASELINE_RATIO = 3.576
REFERENCE_SUMMARY_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json"
REFERENCE_VARIANT = "rstim-direct-repeat-reference-b8"
REFERENCE_STRATEGY = "direct_inverse_repeat_folded"
REFERENCE_CHECKER = "tools/check_rstim_vs_stim_reference_build_evidence.py"
```

Add:

```python
def _variant_summary(summary: dict[str, Any], variant: str) -> dict[str, Any]:
    matches = [item for item in summary["variants"] if item["variant"] == variant]
    if len(matches) != 1:
        raise RuntimeError(f"summary must contain exactly one {variant} variant")
    return matches[0]


def _rstim_over_stim(summary: dict[str, Any]) -> float:
    stim_median = float(_variant_summary(summary, "stim-cli-b8")["elapsed_ns"]["median"])
    rstim_median = float(_variant_summary(summary, "rstim-cli-b8")["elapsed_ns"]["median"])
    if stim_median <= 0:
        raise RuntimeError("stim median must be positive")
    return rstim_median / stim_median


def _rounded_ratio(value: float) -> float:
    return round(value, 3)


def _reference_evidence(*, repo_root: Path) -> dict[str, str]:
    summary_path = repo_root / REFERENCE_SUMMARY_REPO_PATH
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    direct = next(
        (
            variant for variant in summary["variants"]
            if variant["variant"] == REFERENCE_VARIANT
        ),
        None,
    )
    if direct is None or direct.get("backend") != REFERENCE_STRATEGY:
        raise RuntimeError("reference-build summary does not record direct inverse repeat folded strategy")
    return {
        "slot": "reference-build-release",
        "summary_path": REFERENCE_SUMMARY_REPO_PATH,
        "summary_sha256": _sha256_file(summary_path),
        "reference_variant": REFERENCE_VARIANT,
        "reference_strategy": REFERENCE_STRATEGY,
        "checker": REFERENCE_CHECKER,
    }
```

Add:

```python
def _comparison(
    baseline_summary: dict[str, Any],
    candidate_summary: dict[str, Any],
    reference_evidence: dict[str, str],
) -> dict[str, Any]:
    baseline_ratio = _rounded_ratio(_rstim_over_stim(baseline_summary))
    candidate_ratio = _rounded_ratio(_rstim_over_stim(candidate_summary))
    baseline_stim = _variant_summary(baseline_summary, "stim-cli-b8")["elapsed_ns"]["median"]
    baseline_rstim = _variant_summary(baseline_summary, "rstim-cli-b8")["elapsed_ns"]["median"]
    candidate_stim = _variant_summary(candidate_summary, "stim-cli-b8")["elapsed_ns"]["median"]
    candidate_rstim = _variant_summary(candidate_summary, "rstim-cli-b8")["elapsed_ns"]["median"]
    return {
        "baseline_rstim_over_stim": baseline_ratio,
        "candidate_rstim_over_stim": candidate_ratio,
        "ratio_delta_from_baseline": _rounded_ratio(candidate_ratio - baseline_ratio),
        "baseline_median_ns": {
            "stim-cli-b8": baseline_stim,
            "rstim-cli-b8": baseline_rstim,
        },
        "candidate_median_ns": {
            "stim-cli-b8": candidate_stim,
            "rstim-cli-b8": candidate_rstim,
        },
        "reference_summary_path": reference_evidence["summary_path"],
        "reference_summary_sha256": reference_evidence["summary_sha256"],
        "reference_variant": reference_evidence["reference_variant"],
        "reference_strategy": reference_evidence["reference_strategy"],
        "claim": f"Candidate rstim/Stim median ratio is {candidate_ratio:.3f}x, change from baseline is {candidate_ratio - baseline_ratio:+.3f}x.",
    }
```

Update `_render_report` to accept `comparison: dict[str, Any] | None = None`.
After the variant table, append:

```python
if comparison is not None:
    lines.extend(
        [
            "## Baseline comparison",
            "",
            f"Baseline rstim/Stim ratio: {comparison['baseline_rstim_over_stim']:.3f}x",
            f"Candidate rstim/Stim ratio: {comparison['candidate_rstim_over_stim']:.3f}x",
            f"Change from baseline: {comparison['ratio_delta_from_baseline']:+.3f}x",
            f"Reference strategy: {comparison['reference_strategy']}",
            "",
        ]
    )
```

Add helpers:

```python
def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _preserve_baseline(out_dir: Path, *, repo_root: Path) -> Path:
    baseline_path = out_dir / "baseline-summary.json"
    if not baseline_path.exists():
        existing_summary = out_dir / "summary.json"
        source = existing_summary if existing_summary.exists() else repo_root / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/baseline-summary.json"
        baseline_path.write_bytes(source.read_bytes())
    if _sha256_file(baseline_path) != BASELINE_SUMMARY_SHA256:
        raise RuntimeError("baseline-summary.json SHA-256 mismatch")
    return baseline_path


def _write_artifact_hashes(out_dir: Path) -> None:
    artifact_files = ("raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json")
    _write_json(out_dir / "artifact-sha256.json", {filename: _sha256_file(out_dir / filename) for filename in artifact_files})
```

Update `_collect_environment` to accept `reference_evidence: dict[str, str]` and include `"reference_evidence": reference_evidence` in the returned object.

In `run_fair_cli`, create `out_dir` before running, preserve baseline after preflight and records succeed but before writing new `summary.json`, compute `reference_evidence`, compute `comparison`, write `comparison.json`, call `_render_report(summary, comparison)`, and write artifact hashes after all other artifacts.

- [ ] **Step 4: Run runner tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected: PASS.

- [ ] **Step 5: Commit runner changes**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/run_fair_cli.py benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py
git commit -m "feat: derive fair cli comparison artifacts"
```

---

### Task 2: Fair CLI Evidence Checker

**Files:**
- Modify: `tools/check_rstim_vs_stim_fair_cli_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`

**Interfaces:**
- Consumes: Task 1 runner functions `_comparison`, `_reference_evidence`, `_render_report(summary, comparison)`.
- Produces: checker validates `baseline-summary.json`, `comparison.json`, candidate/reference cross-link, no unsupported parity wording, and returns a result object with `variants`, `measured`, `baseline_rstim_over_stim`, `candidate_rstim_over_stim`, and `reference_strategy`.

- [ ] **Step 1: Write failing checker tests**

In `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`, set:

```python
REQUIRED_ARTIFACTS = ("raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json")
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
REFERENCE_SUMMARY = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json"
```

In `write_valid_bundle`, copy the committed baseline:

```python
baseline_source = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/baseline-summary.json"
if not baseline_source.exists():
    baseline_source = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json"
(path / "baseline-summary.json").write_bytes(baseline_source.read_bytes())
baseline_summary = json.loads((path / "baseline-summary.json").read_text(encoding="utf-8"))
reference_evidence = run_fair_cli._reference_evidence(repo_root=REPO_ROOT)
comparison = run_fair_cli._comparison(baseline_summary, summary, reference_evidence)
(path / "comparison.json").write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
(path / "report.md").write_text(run_fair_cli._render_report(summary, comparison), encoding="utf-8")
```

Add `"reference_evidence": reference_evidence` to the temporary environment.

Add:

```python
def test_committed_bundle_records_comparison_details(self) -> None:
    checker = __import__("tools.check_rstim_vs_stim_fair_cli_evidence", fromlist=["validate_bundle"])
    result = checker.validate_bundle(REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release")
    self.assertEqual(result["baseline_rstim_over_stim"], 3.576)
    self.assertGreater(result["candidate_rstim_over_stim"], 1.0)
    self.assertEqual(result["reference_strategy"], "direct_inverse_repeat_folded")
```

Add:

```python
def test_rejects_candidate_summary_reused_from_baseline(self) -> None:
    (self.bundle / "summary.json").write_bytes((self.bundle / "baseline-summary.json").read_bytes())
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("candidate summary must differ from pinned baseline summary", result.stderr)
```

Add:

```python
def test_rejects_mismatched_reference_evidence_hash(self) -> None:
    def break_reference_hash(environment: dict[str, Any]) -> None:
        environment["reference_evidence"]["summary_sha256"] = "0" * 64

    rewrite_json(self.bundle / "environment.json", break_reference_hash)
    comparison = json.loads((self.bundle / "comparison.json").read_text(encoding="utf-8"))
    comparison["reference_summary_sha256"] = "0" * 64
    (self.bundle / "comparison.json").write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("reference_evidence summary_sha256 does not match reference summary", result.stderr)
```

Add:

```python
def test_rejects_unsupported_parity_wording_when_ratio_exceeds_one(self) -> None:
    report = (self.bundle / "report.md").read_text(encoding="utf-8") + "\nThis candidate reaches parity with Stim.\n"
    (self.bundle / "report.md").write_text(report, encoding="utf-8")
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("unsupported parity claim while candidate ratio exceeds 1.0", result.stderr)
```

Add:

```python
def test_rejects_comparison_not_derived_from_candidate(self) -> None:
    comparison = json.loads((self.bundle / "comparison.json").read_text(encoding="utf-8"))
    comparison["baseline_rstim_over_stim"] = 9.999
    (self.bundle / "comparison.json").write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("comparison.json does not match comparison derived from baseline and candidate summaries", result.stderr)
```

- [ ] **Step 2: Run checker tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected: FAIL because the checker does not yet require baseline/comparison/reference artifacts.

- [ ] **Step 3: Implement checker validation**

In `tools/check_rstim_vs_stim_fair_cli_evidence.py`:

Update required files:

```python
REQUIRED_FILES = ("raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json", "artifact-sha256.json")
ARTIFACT_FILES = REQUIRED_FILES[:-1]
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
BASELINE_RATIO = 3.576
REFERENCE_SUMMARY_REPO_PATH = run_fair_cli.REFERENCE_SUMMARY_REPO_PATH
REFERENCE_VARIANT = run_fair_cli.REFERENCE_VARIANT
REFERENCE_STRATEGY = run_fair_cli.REFERENCE_STRATEGY
REFERENCE_CHECKER = run_fair_cli.REFERENCE_CHECKER
PARITY_WORD_RE = re.compile(r"\bparity\b", re.IGNORECASE)
```

Add:

```python
def validate_baseline_and_candidate(results_dir: Path, candidate_summary: dict[str, Any]) -> dict[str, Any]:
    baseline_path = results_dir / "baseline-summary.json"
    if sha256_file(baseline_path) != BASELINE_SUMMARY_SHA256:
        raise ValueError("baseline-summary.json SHA-256 must match pinned pre-optimization summary")
    if sha256_file(results_dir / "summary.json") == BASELINE_SUMMARY_SHA256:
        raise ValueError("candidate summary must differ from pinned baseline summary")
    baseline = load_json_object(baseline_path, "baseline-summary.json")
    if run_fair_cli._rounded_ratio(run_fair_cli._rstim_over_stim(baseline)) != BASELINE_RATIO:
        raise ValueError("baseline_rstim_over_stim must be 3.576")
    return baseline
```

Add:

```python
def validate_reference_evidence(reference_evidence: object) -> dict[str, str]:
    if not isinstance(reference_evidence, dict):
        raise ValueError("environment reference_evidence must be an object")
    expected = {
        "slot": "reference-build-release",
        "summary_path": REFERENCE_SUMMARY_REPO_PATH,
        "reference_variant": REFERENCE_VARIANT,
        "reference_strategy": REFERENCE_STRATEGY,
        "checker": REFERENCE_CHECKER,
    }
    for field, value in expected.items():
        require_equal(reference_evidence.get(field), value, f"environment reference_evidence {field} must be {value}")
    if contains_host_absolute_path(reference_evidence):
        raise ValueError("environment reference_evidence contains a host-absolute path")
    digest = reference_evidence.get("summary_sha256")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ValueError("environment reference_evidence summary_sha256 must be a lowercase SHA-256 digest")
    reference_path = REPO_ROOT / REFERENCE_SUMMARY_REPO_PATH
    if sha256_file(reference_path) != digest:
        raise ValueError("reference_evidence summary_sha256 does not match reference summary")
    reference_summary = load_json_object(reference_path, "reference summary")
    direct = next((item for item in reference_summary.get("variants", []) if isinstance(item, dict) and item.get("variant") == REFERENCE_VARIANT), None)
    if direct is None or direct.get("backend") != REFERENCE_STRATEGY:
        raise ValueError("reference summary must record direct_inverse_repeat_folded strategy")
    return {key: str(value) for key, value in reference_evidence.items()}
```

Add:

```python
def validate_comparison(
    results_dir: Path,
    baseline_summary: dict[str, Any],
    candidate_summary: dict[str, Any],
    reference_evidence: dict[str, str],
) -> dict[str, Any]:
    expected = run_fair_cli._comparison(baseline_summary, candidate_summary, reference_evidence)
    actual = load_json_object(results_dir / "comparison.json", "comparison.json")
    if actual != expected:
        raise ValueError("comparison.json does not match comparison derived from baseline and candidate summaries")
    if actual["baseline_rstim_over_stim"] != BASELINE_RATIO:
        raise ValueError("comparison.json baseline_rstim_over_stim must be 3.576")
    if actual["reference_strategy"] != REFERENCE_STRATEGY:
        raise ValueError("comparison.json reference_strategy must be direct_inverse_repeat_folded")
    return actual
```

Add:

```python
def _string_values(value: object) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [item for entry in value for item in _string_values(entry)]
    if isinstance(value, dict):
        return [item for entry in value.values() for item in _string_values(entry)]
    return []


def validate_no_unsupported_parity_claim(report_text: str, comparison: dict[str, Any]) -> None:
    candidate_ratio = comparison["candidate_rstim_over_stim"]
    if candidate_ratio <= 1.0:
        return
    checked_text = [report_text, *_string_values(comparison)]
    if any(PARITY_WORD_RE.search(text) for text in checked_text):
        raise ValueError("unsupported parity claim while candidate ratio exceeds 1.0")
```

Update `validate_environment` to return the validated reference evidence:

```python
reference_evidence = validate_reference_evidence(environment.get("reference_evidence"))
return reference_evidence
```

Update `validate_bundle` to:

```python
summary = derive_summary(records)
...
baseline_summary = validate_baseline_and_candidate(results_dir, summary)
reference_evidence = validate_environment(environment, records)
comparison = validate_comparison(results_dir, baseline_summary, summary, reference_evidence)
report_text = (results_dir / "report.md").read_text(encoding="utf-8")
validate_no_unsupported_parity_claim(report_text, comparison)
if report_text != render_report(summary, comparison):
    raise ValueError("report.md does not match report derived from raw.jsonl")
...
return {
    "variants": len(VARIANTS),
    "measured": summary["measured_record_count"],
    "baseline_rstim_over_stim": comparison["baseline_rstim_over_stim"],
    "candidate_rstim_over_stim": comparison["candidate_rstim_over_stim"],
    "reference_strategy": comparison["reference_strategy"],
}
```

Update `render_report` to accept `comparison` and call `run_fair_cli._render_report(summary, comparison)`.

Update `main` to print from the returned dict:

```python
print(f"PASS fair CLI sampling evidence variants={result['variants']} measured={result['measured']}")
```

- [ ] **Step 4: Run checker tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected: PASS.

- [ ] **Step 5: Commit checker changes**

Run:

```sh
git add tools/check_rstim_vs_stim_fair_cli_evidence.py tools/test_check_rstim_vs_stim_fair_cli_evidence.py
git commit -m "test: enforce fair cli comparison evidence"
```

---

### Task 3: Publish Candidate Evidence

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/raw.jsonl`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/baseline-summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/comparison.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/report.md`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/artifact-sha256.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`
- Modify: `tools/check_all_portable_evidence.py`
- Modify: `tools/test_check_all_portable_evidence.py`

**Interfaces:**
- Consumes: Task 1 runner output and Task 2 checker return dict.
- Produces: committed refreshed candidate evidence and portable catalog hashes that include `baseline-summary.json` and `comparison.json`.

- [ ] **Step 1: Verify the current summary is the pinned baseline**

Run:

```sh
python3 - <<'PY'
import hashlib
from pathlib import Path
path = Path("benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json")
print(hashlib.sha256(path.read_bytes()).hexdigest())
PY
```

Expected:

```text
131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07
```

- [ ] **Step 2: Run the refreshed candidate benchmark**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_fair_cli \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 \
  --profile release \
  --warmup-rounds 2 \
  --measure-rounds 7 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

Expected:

```text
PASS symmetric fair CLI runner variants=2 warmups=4 measured=14 bytes_per_run=1552384
```

- [ ] **Step 3: Update the portable catalog for fair-cli-release**

Run:

```sh
python3 - <<'PY'
import hashlib
from pathlib import Path

catalog = Path("benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml")
root = Path("benchmarks/rstim_vs_stim_simulator/results/fair-cli-release")
text = catalog.read_text(encoding="utf-8")
start = text.index('id = "fair-cli-release"')
end = text.index('[[bundles]]', start + 1)
section = text[start:end]
for filename in [
    "artifact-sha256.json",
    "baseline-summary.json",
    "comparison.json",
    "environment.json",
    "raw.jsonl",
    "report.md",
    "summary.json",
]:
    digest = hashlib.sha256((root / filename).read_bytes()).hexdigest()
    marker = f'path = "{filename}"'
    if marker in section:
        before, rest = section.split(marker, 1)
        sha_line_start = rest.index('sha256 = "') + len('sha256 = "')
        sha_line_end = rest.index('"', sha_line_start)
        rest = rest[:sha_line_start] + digest + rest[sha_line_end:]
        section = before + marker + rest
    else:
        insert = f'[[bundles.artifacts]]\npath = "{filename}"\nsha256 = "{digest}"\n\n'
        artifact_start = section.index('[[bundles.artifacts]]')
        logical_start = section.index('[[bundles.logical_executables]]')
        section = section[:logical_start] + insert + section[logical_start:]
text = text[:start] + section + text[end:]
catalog.write_text(text, encoding="utf-8")
PY
```

Then normalize artifact order manually if the insertion placed `baseline-summary.json`
or `comparison.json` away from the other fair CLI artifacts. The final fair
artifact order should be:

```toml
artifact-sha256.json
baseline-summary.json
comparison.json
environment.json
raw.jsonl
report.md
summary.json
```

- [ ] **Step 4: Update aggregate checker return handling**

In `tools/check_all_portable_evidence.py`, update `_fair_cli_pass_line`:

```python
def _fair_cli_pass_line(result: Any) -> str:
    return f"PASS fair CLI sampling evidence variants={result['variants']} measured={result['measured']}"
```

In `tools/test_check_all_portable_evidence.py`, set:

```python
FAIR_CLI_ARTIFACTS = ("raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json")
```

The aggregate expected stdout remains:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 5: Run focused fair CLI verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
python3 tools/check_all_portable_evidence.py --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_all_portable_evidence benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected first line:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 6: Commit published evidence**

Run:

```sh
git add \
  benchmarks/rstim_vs_stim_simulator/results/fair-cli-release \
  benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  tools/check_all_portable_evidence.py \
  tools/test_check_all_portable_evidence.py
git commit -m "data: publish post-optimization fair cli evidence"
```

---

### Task 4: Final Verification and PR Readiness

**Files:**
- No planned source edits.

**Interfaces:**
- Consumes: Tasks 1 through 3.
- Produces: verification evidence for PR body.

- [ ] **Step 1: Run issue-required checker command**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

Expected:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 2: Run issue-required unit command**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected: exit code `0`.

- [ ] **Step 3: Run portable evidence checks**

Run:

```sh
python3 tools/check_all_portable_evidence.py --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_all_portable_evidence benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
```

Expected: exit code `0` for both commands.

- [ ] **Step 4: Run required Rust workspace verification**

Run:

```sh
cargo test
```

Expected: exit code `0`.

- [ ] **Step 5: Run diff hygiene**

Run:

```sh
git diff --check
git status --short
```

Expected: `git diff --check` exits `0`; `git status --short` is clean.

