# Issue 495 Sampler Performance Milestone Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the sampler-performance readiness gate verify the eight required GitHub milestones are closed, then close those live milestones after offline readiness passes.

**Architecture:** Extend the existing Python readiness checker's optional GitHub path from open-issue probing to exact milestone-object state verification. Keep the irreversible live milestone closure outside the checker and run it only after the offline readiness command succeeds.

**Tech Stack:** Python 3 standard library, unittest, GitHub CLI/API, Cargo test suite.

## Global Constraints

- Close exactly these milestone titles:
  - `P0: Fair CLI Benchmark`
  - `P1A: Reusable Compiled Sampler`
  - `P1B: Packed Inverse Reference Tableau`
  - `P1C: Instruction-wide Sparse Noise`
  - `M1: Portable Evidence Foundation`
  - `M2: Direct Inverse Measurement`
  - `M3: Repeat-Aware Reference Sampling`
  - `M4: Measured Optimization Closure`
- Do not change issues #38, #379, or #406.
- Apply milestone state changes only after the offline readiness command passes.
- Do not edit issue bodies, update the site, or add project-board/`auto-resolve` wiring.
- Preserve the existing readiness PASS line exactly: `PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05`.
- When `--verify-github` succeeds, print exactly: `PASS milestone closure closed=8 open=0`.
- Negative control with any one required milestone marked open must fail with `milestone remains open: <title>`.
- Required final GitHub verification command is exactly:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --verify-github nzy1997/rstim \
  --out /tmp/rstim-sampler-readiness.json
```

---

### Task 1: Milestone State Verification

**Files:**
- Modify: `tools/check_sampler_performance_readiness.py`
- Modify: `tools/test_check_sampler_performance_readiness.py`

**Interfaces:**
- Consumes: existing `build_readiness(catalog_path: Path, verify_github: str | None = None, github_json: Path | None = None) -> dict[str, object]`.
- Produces:
  - `MILESTONE_TITLES: tuple[str, ...]`.
  - `MILESTONE_PASS_LINE = "PASS milestone closure closed=8 open=0"`.
  - `read_github_milestones(repo: str, github_json: Path | None) -> list[dict[str, Any]]`.
  - `verify_milestone_closure(repo: str, github_json: Path | None) -> dict[str, object]`.
  - CLI prints `MILESTONE_PASS_LINE` only when `--verify-github` was supplied and all required milestones are closed.

- [ ] **Step 1: Write failing milestone tests**

In `tools/test_check_sampler_performance_readiness.py`, replace the existing mocked GitHub issue tests with milestone-state tests and add the two missing edge controls:

```python
MILESTONE_TITLES = (
    "P0: Fair CLI Benchmark",
    "P1A: Reusable Compiled Sampler",
    "P1B: Packed Inverse Reference Tableau",
    "P1C: Instruction-wide Sparse Noise",
    "M1: Portable Evidence Foundation",
    "M2: Direct Inverse Measurement",
    "M3: Repeat-Aware Reference Sampling",
    "M4: Measured Optimization Closure",
)
MILESTONE_PASS_LINE = "PASS milestone closure closed=8 open=0\n"


def milestone_payload(open_title: str | None = None) -> list[dict[str, object]]:
    return [
        {
            "number": index,
            "title": title,
            "state": "open" if title == open_title else "closed",
            "open_issues": 1 if title == open_title else 0,
            "closed_issues": index,
        }
        for index, title in enumerate(MILESTONE_TITLES, start=28)
    ]
```

Add these tests:

```python
def test_mocked_closed_github_milestones_succeed(self) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        github_json = Path(tmp) / "milestones.json"
        github_json.write_text(json.dumps(milestone_payload()), encoding="utf-8")
        out = Path(tmp) / "readiness.json"

        result = self.run_checker(
            "--catalog",
            str(CATALOG),
            "--out",
            str(out),
            "--verify-github",
            "nzy1997/rstim",
            "--github-json",
            str(github_json),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, PASS_LINE + MILESTONE_PASS_LINE)
        milestone = json.loads(out.read_text(encoding="utf-8"))["issues"]["milestone"]
        self.assertEqual(milestone["status"], "closed")
        self.assertEqual(milestone["closed"], 8)
        self.assertEqual(milestone["open"], 0)
        self.assertEqual([item["title"] for item in milestone["milestones"]], list(MILESTONE_TITLES))


def test_mocked_open_github_milestone_fails_with_title(self) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        open_title = "M4: Measured Optimization Closure"
        github_json = Path(tmp) / "milestones.json"
        github_json.write_text(json.dumps(milestone_payload(open_title)), encoding="utf-8")
        out = Path(tmp) / "readiness.json"

        result = self.run_checker(
            "--catalog",
            str(CATALOG),
            "--out",
            str(out),
            "--verify-github",
            "nzy1997/rstim",
            "--github-json",
            str(github_json),
        )

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"not ready: milestone remains open: {open_title}", result.stderr)


def test_mocked_missing_github_milestone_fails_with_title(self) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        missing_title = "M3: Repeat-Aware Reference Sampling"
        payload = [item for item in milestone_payload() if item["title"] != missing_title]
        github_json = Path(tmp) / "milestones.json"
        github_json.write_text(json.dumps(payload), encoding="utf-8")
        out = Path(tmp) / "readiness.json"

        result = self.run_checker(
            "--catalog",
            str(CATALOG),
            "--out",
            str(out),
            "--verify-github",
            "nzy1997/rstim",
            "--github-json",
            str(github_json),
        )

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"not ready: sampler-performance milestone missing: {missing_title}", result.stderr)


def test_mocked_duplicate_github_milestone_fails_with_title(self) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        duplicate_title = "P0: Fair CLI Benchmark"
        payload = milestone_payload()
        payload.append(dict(payload[0]))
        github_json = Path(tmp) / "milestones.json"
        github_json.write_text(json.dumps(payload), encoding="utf-8")
        out = Path(tmp) / "readiness.json"

        result = self.run_checker(
            "--catalog",
            str(CATALOG),
            "--out",
            str(out),
            "--verify-github",
            "nzy1997/rstim",
            "--github-json",
            str(github_json),
        )

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"not ready: duplicate sampler-performance milestone title: {duplicate_title}", result.stderr)
```

- [ ] **Step 2: Run RED test**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: FAIL because the checker still reads open milestone issues and does not print `PASS milestone closure closed=8 open=0`.

- [ ] **Step 3: Implement milestone verification**

In `tools/check_sampler_performance_readiness.py`, add constants near `PASS_LINE`:

```python
MILESTONE_TITLES = (
    "P0: Fair CLI Benchmark",
    "P1A: Reusable Compiled Sampler",
    "P1B: Packed Inverse Reference Tableau",
    "P1C: Instruction-wide Sparse Noise",
    "M1: Portable Evidence Foundation",
    "M2: Direct Inverse Measurement",
    "M3: Repeat-Aware Reference Sampling",
    "M4: Measured Optimization Closure",
)
MILESTONE_PASS_LINE = "PASS milestone closure closed=8 open=0"
```

Replace `read_github_issues` with:

```python
def read_github_milestones(repo: str, github_json: Path | None) -> list[dict[str, Any]]:
    if github_json is not None:
        with github_json.open(encoding="utf-8") as handle:
            value = json.load(handle)
    else:
        value = []
        for state in ("open", "closed"):
            completed = subprocess.run(
                [
                    "gh", "api", "-X", "GET", "--paginate",
                    f"repos/{repo}/milestones",
                    "-f", f"state={state}",
                ],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            if completed.returncode != 0:
                raise ValueError(f"GitHub milestone query failed: {completed.stderr.strip()}")
            loaded = json.loads(completed.stdout)
            if not isinstance(loaded, list):
                raise ValueError("GitHub milestone response must be a JSON array")
            value.extend(loaded)
    if not isinstance(value, list) or not all(isinstance(milestone, dict) for milestone in value):
        raise ValueError("GitHub milestone response must be a JSON array of objects")
    return value
```

Add:

```python
def verify_milestone_closure(repo: str, github_json: Path | None) -> dict[str, object]:
    milestones = read_github_milestones(repo, github_json)
    by_title: dict[str, list[dict[str, Any]]] = {title: [] for title in MILESTONE_TITLES}
    for milestone in milestones:
        title = milestone.get("title")
        if isinstance(title, str) and title in by_title:
            by_title[title].append(milestone)

    entries: list[dict[str, object]] = []
    open_count = 0
    for title in MILESTONE_TITLES:
        matches = by_title[title]
        if not matches:
            raise ReadinessError(f"not ready: sampler-performance milestone missing: {title}")
        if len(matches) > 1:
            raise ReadinessError(f"not ready: duplicate sampler-performance milestone title: {title}")
        milestone = matches[0]
        state = str(milestone.get("state", "")).lower()
        if state != "closed":
            open_count += 1
            raise ReadinessError(f"not ready: milestone remains open: {title}")
        entries.append({
            "number": milestone.get("number"),
            "title": title,
            "state": state,
            "open_issues": milestone.get("open_issues"),
            "closed_issues": milestone.get("closed_issues"),
        })

    return {
        "status": "closed",
        "repo": repo,
        "closed": len(entries),
        "open": open_count,
        "milestones": entries,
    }
```

In `build_readiness`, replace the old open-issues block with:

```python
issues: dict[str, object] = {"status": "not_checked", "open": []}
if verify_github is not None:
    issues = verify_milestone_closure(verify_github, github_json)
```

In `main`, after printing `PASS_LINE`, add:

```python
if args.verify_github is not None:
    print(MILESTONE_PASS_LINE)
```

- [ ] **Step 4: Run GREEN test**

Run:

```sh
python3 -m unittest tools.test_check_sampler_performance_readiness -q
```

Expected: PASS.

- [ ] **Step 5: Run offline readiness gate before live changes**

Run:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --out /tmp/rstim-sampler-readiness-offline.json
```

Expected output:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
```

- [ ] **Step 6: Commit code changes**

Run:

```sh
git add tools/check_sampler_performance_readiness.py tools/test_check_sampler_performance_readiness.py
git commit -m "test: verify sampler milestone closure"
```

### Task 2: Live Milestone Closure and Verification

**Files:**
- No repository file changes.

**Interfaces:**
- Consumes: the exact required milestone title list from Task 1.
- Produces: live GitHub milestone state where the eight named milestones are closed and the verification command prints both PASS lines.

- [ ] **Step 1: Confirm prerequisite gate already passed**

Use the output from Task 1 Step 5. If it did not pass, stop and fix the readiness failure before closing any live milestone.

- [ ] **Step 2: Close exactly the required milestones**

Use live milestone numbers discovered by exact title, then PATCH only those milestone numbers:

```sh
gh api -X GET --paginate repos/nzy1997/rstim/milestones -f state=open
gh api -X PATCH repos/nzy1997/rstim/milestones/<number> -f state=closed
```

Required live numbers observed before closure on 2026-07-13:

```text
28 P0: Fair CLI Benchmark
29 P1A: Reusable Compiled Sampler
30 P1B: Packed Inverse Reference Tableau
31 P1C: Instruction-wide Sparse Noise
32 M1: Portable Evidence Foundation
33 M2: Direct Inverse Measurement
34 M3: Repeat-Aware Reference Sampling
35 M4: Measured Optimization Closure
```

- [ ] **Step 3: Run required live verification**

Run:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --verify-github nzy1997/rstim \
  --out /tmp/rstim-sampler-readiness.json
```

Expected output:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
PASS milestone closure closed=8 open=0
```

- [ ] **Step 4: Run issue non-mutation spot checks**

Run:

```sh
gh issue view 38 --repo nzy1997/rstim --json number,title,state,milestone
gh issue view 379 --repo nzy1997/rstim --json number,title,state,milestone
gh issue view 406 --repo nzy1997/rstim --json number,title,state,milestone
```

Expected: all three issues remain open and have no milestone mutation from this task.

- [ ] **Step 5: Commit any remaining tracked changes**

No files should change in this task. If `sampler-performance-readiness.md` is rewritten with identical content, do not commit metadata-only churn.
