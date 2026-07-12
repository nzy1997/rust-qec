# Issue 492 Paired Frame-Noise Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a same-machine paired runner that compares baseline and candidate `rstim` revisions on the canonical frame-noise sampling workload.

**Architecture:** Create a dedicated Python module under `benchmarks/rstim_vs_stim_simulator` rather than extending the Stim-vs-rstim fair CLI runner. The runner materializes both revisions with `git archive`, builds each with an isolated `CARGO_TARGET_DIR`, validates the exact `rstim sample --skip_reference_sample --out_format b8` command, alternates paired round order, and derives portable raw/summary/report/environment artifacts.

**Tech Stack:** Python 3 standard library, unittest, existing benchmark fixture metadata, Cargo release builds for `rstim`, Rust workspace verification through `cargo test`.

## Global Constraints

- Baseline revision is pinned to `f10d1ed024d3519318ed244c9095724074519595`.
- CLI module name is `benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise`.
- Required interface is `python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise --baseline-rev <sha> --candidate-rev <sha|HEAD> --fixture <path> --shots 1024 --warmup-rounds 2 --measure-rounds 7 --out-dir <dir>`.
- Materialize baseline and candidate through `git archive`; do not switch or modify the current checkout.
- Build revisions into separate temporary target directories.
- Invoke `rstim sample --skip_reference_sample --out_format b8`.
- Use identical fixture, shots, and seeds for each paired baseline/candidate round.
- Alternate baseline-first and candidate-first ordering by round index.
- Time process spawn through complete stdout/stderr drain and exit.
- Require exactly `1552384` stdout bytes for every benchmark child process.
- Write portable `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and `artifact-sha256.json`.
- Same resolved baseline/candidate revision must fail with `baseline and candidate revisions must differ`.
- Fake candidate output one byte short must fail before summary generation.
- Removing `--skip_reference_sample` must fail canonical command validation.
- Do not expose a production legacy-noise switch.
- Do not include reference-build timing.
- Do not impose an absolute timing threshold.
- Required success line is exactly `PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384`.
- Required final verification includes `cargo test`.

---

### Task 1: Paired Frame-Noise Runner

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_paired_frame_noise.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_paired_frame_noise.py`

**Interfaces:**
- Consumes:
  - canonical fixture at `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`;
  - `git`, `cargo`, and the built `rstim` binary from each archived revision.
- Produces:
  - `time_cli(argv: list[str], *, cwd: Path) -> CliResult`
  - `validate_canonical_command(argv: list[str], *, variant: str, fixture: Path, shots: int, seed: int) -> None`
  - `materialize_revision(revision: str, *, repo_root: Path, temp_root: Path, label: str) -> RevisionBuild`
  - `build_revision(revision: RevisionBuild) -> Path`
  - `run_paired_frame_noise(args: argparse.Namespace, *, repo_root: Path = REPO_ROOT) -> dict[str, Any]`
  - CLI `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Write failing unit tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_paired_frame_noise.py` with tests that import the new module:

```python
from benchmarks.rstim_vs_stim_simulator import run_paired_frame_noise
```

Include these constants:

```python
ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
FIXTURE_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
BASELINE_REV = "f10d1ed024d3519318ed244c9095724074519595"
BASELINE_COMMIT = "0" * 40
CANDIDATE_COMMIT = "1" * 40
EXPECTED_BYTES = 1_552_384
EXPECTED_SHA256 = hashlib.sha256(bytes(range(256)) * 6064).hexdigest()
```

Add a fake CLI writer that enforces the command contract and can emit success,
short output, delayed drain, or version output:

```python
def write_fake_rstim(path: Path, *, mode: str = "success") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{sys.executable}
            import os
            import sys
            import time

            MODE = {mode!r}
            EXPECTED_BYTES = {EXPECTED_BYTES}

            if not sys.argv[1:]:
                print("rstim 0.1.1-test")
                sys.exit(0)

            log_path = os.environ.get("PAIRED_FAKE_INVOCATIONS")
            if log_path:
                with open(log_path, "a", encoding="utf-8") as log:
                    log.write("CLI\\t" + sys.argv[0] + "\\t" + " ".join(sys.argv[1:]) + "\\n")

            argv = sys.argv[1:]
            if argv[0] != "sample":
                print("expected sample", file=sys.stderr)
                sys.exit(2)
            for flag in ("--skip_reference_sample", "--shots", "--seed", "--out_format", "--in"):
                if flag not in argv:
                    print(f"missing {{flag}}", file=sys.stderr)
                    sys.exit(2)
            if argv[argv.index("--out_format") + 1] != "b8":
                print("expected b8", file=sys.stderr)
                sys.exit(2)
            if argv[argv.index("--shots") + 1] != "1024":
                print("expected 1024 shots", file=sys.stderr)
                sys.exit(2)

            payload = bytes(range(256)) * (EXPECTED_BYTES // 256)
            if MODE == "short-output":
                sys.stdout.buffer.write(payload[:-1])
                sys.exit(0)
            if MODE == "delayed":
                sys.stdout.buffer.write(payload[:-1])
                sys.stdout.buffer.flush()
                time.sleep(0.15)
                sys.stdout.buffer.write(payload[-1:])
                sys.stdout.buffer.flush()
                sys.stdout.close()
                time.sleep(0.15)
                sys.stderr.buffer.write(b"drained stderr\\n" * 1024)
                sys.stderr.buffer.flush()
                sys.exit(0)

            sys.stdout.buffer.write(payload)
            sys.exit(0)
            """
        ),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path
```

Add fake build helpers:

```python
def fake_builds(root: Path) -> dict[str, run_paired_frame_noise.RevisionBuild]:
    return {
        "baseline": run_paired_frame_noise.RevisionBuild(
            label="baseline",
            requested_rev=BASELINE_REV,
            resolved_commit=BASELINE_COMMIT,
            source_dir=root / "baseline-src",
            target_dir=root / "baseline-target",
            binary_path=write_fake_rstim(root / "baseline-target/release/rstim"),
        ),
        "candidate": run_paired_frame_noise.RevisionBuild(
            label="candidate",
            requested_rev="HEAD",
            resolved_commit=CANDIDATE_COMMIT,
            source_dir=root / "candidate-src",
            target_dir=root / "candidate-target",
            binary_path=write_fake_rstim(root / "candidate-target/release/rstim"),
        ),
    }
```

Add a runner helper that patches `materialize_revision` and `build_revision`:

```python
def run_with_fake_builds(out_dir: Path, builds: dict[str, run_paired_frame_noise.RevisionBuild]) -> dict[str, object]:
    def materialize(revision: str, *, repo_root: Path, temp_root: Path, label: str) -> run_paired_frame_noise.RevisionBuild:
        return builds[label]

    def build(revision: run_paired_frame_noise.RevisionBuild) -> Path:
        return revision.binary_path

    with (
        mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.materialize_revision", side_effect=materialize),
        mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.build_revision", side_effect=build),
    ):
        return run_paired_frame_noise.run_paired_frame_noise(
            argparse.Namespace(
                baseline_rev=BASELINE_REV,
                candidate_rev="HEAD",
                fixture=FIXTURE,
                shots=1024,
                warmup_rounds=2,
                measure_rounds=7,
                out_dir=out_dir,
            ),
            repo_root=ROOT,
        )
```

Cover these behaviors:

```python
def test_same_revision_rejected() -> None:
    with self.assertRaisesRegex(ValueError, "baseline and candidate revisions must differ"):
        run_paired_frame_noise.ensure_distinct_revisions("a" * 40, "a" * 40)
```

```python
def test_canonical_command_requires_skip_reference_sample() -> None:
    argv = [
        "tool://rstim-baseline",
        "sample",
        "--shots",
        "1024",
        "--seed",
        "0",
        "--out_format",
        "b8",
        "--in",
        FIXTURE_REPO_PATH,
    ]
    with self.assertRaisesRegex(ValueError, "--skip_reference_sample"):
        run_paired_frame_noise.validate_canonical_command(
            argv,
            variant="baseline-rstim-frame-noise-b8",
            fixture=Path(FIXTURE_REPO_PATH),
            shots=1024,
            seed=0,
        )
```

```python
def test_time_cli_includes_complete_stderr_drain_and_exit() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        cli = write_fake_rstim(root / "rstim", mode="delayed")
        result = run_paired_frame_noise.time_cli([
            str(cli),
            "sample",
            "--skip_reference_sample",
            "--shots",
            "1024",
            "--seed",
            "0",
            "--out_format",
            "b8",
            "--in",
            str(FIXTURE),
        ], cwd=ROOT)
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(len(result.stdout), EXPECTED_BYTES)
        self.assertIn(b"drained stderr", result.stderr)
        self.assertGreaterEqual(result.elapsed_ns, 300_000_000)
```

```python
def test_runner_writes_paired_artifacts_and_alternates_order() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        out_dir = root / "out"
        invocations = root / "invocations.txt"
        builds = fake_builds(root)
        with mock.patch.dict(os.environ, {"PAIRED_FAKE_INVOCATIONS": str(invocations)}):
            summary = run_with_fake_builds(out_dir, builds)

        self.assertEqual(summary["measured_record_count"], 14)
        self.assertEqual({path.name for path in out_dir.iterdir()}, {
            "raw.jsonl",
            "summary.json",
            "report.md",
            "environment.json",
            "artifact-sha256.json",
        })
        records = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text(encoding="utf-8").splitlines()]
        self.assertEqual(len(records), 18)
        self.assertEqual({record["variant"] for record in records}, {
            "baseline-rstim-frame-noise-b8",
            "candidate-rstim-frame-noise-b8",
        })
        measured = [record for record in records if record["phase"] == "measured"]
        self.assertEqual(len(measured), 14)
        for phase in ("warmup", "measured"):
            phase_records = [record for record in records if record["phase"] == phase]
            for round_index in sorted({record["round_index"] for record in phase_records}):
                pair = [record for record in phase_records if record["round_index"] == round_index]
                expected = [
                    "baseline-rstim-frame-noise-b8",
                    "candidate-rstim-frame-noise-b8",
                ]
                if round_index % 2 == 1:
                    expected.reverse()
                self.assertEqual([record["variant"] for record in pair], expected)
                self.assertEqual(len({record["seed"] for record in pair}), 1)
        for record in records:
            self.assertEqual(record["actual_output_bytes"], EXPECTED_BYTES)
            self.assertEqual(record["stdout_sha256"], EXPECTED_SHA256)
            self.assertIn("--skip_reference_sample", record["argv"])
            self.assertEqual(record["argv"][record["argv"].index("--out_format") + 1], "b8")
            self.assertEqual(record["argv"][record["argv"].index("--in") + 1], FIXTURE_REPO_PATH)

        environment = json.loads((out_dir / "environment.json").read_text(encoding="utf-8"))
        self.assertEqual(environment["baseline_revision"]["resolved_commit"], BASELINE_COMMIT)
        self.assertEqual(environment["candidate_revision"]["resolved_commit"], CANDIDATE_COMMIT)
        self.assertEqual(environment["fixture_path"], FIXTURE_REPO_PATH)
        self.assertEqual(environment["expected_output_bytes"], EXPECTED_BYTES)
        self.assertNotIn(str(root), json.dumps(records))
        self.assertNotIn(str(root), json.dumps(summary))

        hashes = json.loads((out_dir / "artifact-sha256.json").read_text(encoding="utf-8"))
        self.assertEqual(set(hashes), {"raw.jsonl", "summary.json", "report.md", "environment.json"})
        for filename, digest in hashes.items():
            self.assertEqual(digest, hashlib.sha256((out_dir / filename).read_bytes()).hexdigest())
```

```python
def test_short_candidate_output_fails_before_summary_generation() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        out_dir = root / "out"
        builds = fake_builds(root)
        builds["candidate"] = dataclasses.replace(
            builds["candidate"],
            binary_path=write_fake_rstim(root / "candidate-target/release/rstim", mode="short-output"),
        )
        with self.assertRaisesRegex(RuntimeError, "1552384|output bytes"):
            run_with_fake_builds(out_dir, builds)
        self.assertFalse((out_dir / "summary.json").exists())
```

```python
def test_materialize_revision_uses_git_archive_without_checkout() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        calls: list[list[str]] = []

        def fake_run(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[:2] == ["git", "rev-parse"]:
                return subprocess.CompletedProcess(argv, 0, stdout=(CANDIDATE_COMMIT + "\n").encode(), stderr=b"")
            if argv[:2] == ["git", "archive"]:
                import tarfile
                archive = root / "archive.tar"
                source_file = root / "Cargo.toml"
                source_file.write_text("[workspace]\n", encoding="utf-8")
                with tarfile.open(archive, "w") as tar:
                    tar.add(source_file, arcname="Cargo.toml")
                return subprocess.CompletedProcess(argv, 0, stdout=archive.read_bytes(), stderr=b"")
            raise AssertionError(argv)

        with mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.subprocess.run", side_effect=fake_run):
            build = run_paired_frame_noise.materialize_revision("HEAD", repo_root=ROOT, temp_root=root, label="candidate")

        self.assertEqual(build.resolved_commit, CANDIDATE_COMMIT)
        self.assertTrue((build.source_dir / "Cargo.toml").is_file())
        self.assertTrue(any(call[:2] == ["git", "archive"] for call in calls))
        self.assertFalse(any(item == "checkout" for call in calls for item in call))
```

```python
def test_main_prints_required_success_line() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        out_dir = root / "out"
        builds = fake_builds(root)
        with (
            mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.materialize_revision", side_effect=lambda revision, *, repo_root, temp_root, label: builds[label]),
            mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.build_revision", side_effect=lambda revision: revision.binary_path),
            mock.patch("sys.stdout", new_callable=io.StringIO) as stdout,
        ):
            code = run_paired_frame_noise.main([
                "--baseline-rev", BASELINE_REV,
                "--candidate-rev", "HEAD",
                "--fixture", str(FIXTURE),
                "--shots", "1024",
                "--warmup-rounds", "2",
                "--measure-rounds", "7",
                "--out-dir", str(out_dir),
            ])
        self.assertEqual(code, 0)
        self.assertEqual(
            stdout.getvalue().strip(),
            "PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384",
        )
```

- [ ] **Step 2: Run unit tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise -q
```

Expected: FAIL with an import error because `run_paired_frame_noise.py` does not exist yet.

- [ ] **Step 3: Implement the runner**

Create `benchmarks/rstim_vs_stim_simulator/run_paired_frame_noise.py` with these pieces:

```python
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from io import BytesIO
```

Define constants:

```python
PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
MODULE_NAME = "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise"
CANONICAL_FIXTURE_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
CANONICAL_FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
PINNED_BASELINE_REV = "f10d1ed024d3519318ed244c9095724074519595"
CASE_ID = "stim_surface_d11_r100"
MEASUREMENT_COUNT = 12_121
OUTPUT_FORMAT = "b8"
BYTES_PER_SHOT = 1_516
EXPECTED_OUTPUT_BYTES = 1_552_384
TIMER_SCOPE = "process_spawn_stdout_stderr_drain_exit"
BASELINE_VARIANT = "baseline-rstim-frame-noise-b8"
CANDIDATE_VARIANT = "candidate-rstim-frame-noise-b8"
VARIANT_LABELS = {
    BASELINE_VARIANT: "baseline",
    CANDIDATE_VARIANT: "candidate",
}
TOOL_ROLES = {
    BASELINE_VARIANT: "tool://rstim-baseline-frame-noise",
    CANDIDATE_VARIANT: "tool://rstim-candidate-frame-noise",
}
ARTIFACT_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")
```

Add data classes:

```python
@dataclass(frozen=True)
class CliResult:
    exit_code: int
    stdout: bytes
    stderr: bytes
    elapsed_ns: int


@dataclass(frozen=True)
class RevisionBuild:
    label: str
    requested_rev: str
    resolved_commit: str
    source_dir: Path
    target_dir: Path
    binary_path: Path
```

Implement helpers:

```python
def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8")


def _record_path(path: Path, *, repo_root: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(repo_root).as_posix()
    except ValueError:
        return str(resolved)


def _probe_stdout(argv: list[str], *, cwd: Path) -> str:
    completed = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"{argv[0]} exited with code {completed.returncode}: {detail}")
    return completed.stdout.strip()


def _version_or_failed(argv: list[str], *, cwd: Path) -> str:
    try:
        return _probe_stdout(argv, cwd=cwd)
    except (OSError, RuntimeError) as error:
        return f"failed: {error}"


def _cpu_model() -> str:
    try:
        completed = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True, check=False)
    except OSError:
        completed = None
    if completed is not None and completed.returncode == 0 and completed.stdout.strip():
        return completed.stdout.strip()
    return platform.processor() or platform.machine() or "unknown"
```

Implement revision and build handling:

```python
def _git_stdout(argv: list[str], *, repo_root: Path) -> bytes:
    completed = subprocess.run(argv, cwd=repo_root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if completed.returncode != 0:
        detail = completed.stderr.decode(errors="replace").strip()
        raise RuntimeError(f"{' '.join(argv)} failed: {detail or completed.returncode}")
    return completed.stdout


def resolve_revision(revision: str, *, repo_root: Path) -> str:
    return _git_stdout(["git", "rev-parse", revision], repo_root=repo_root).decode("ascii").strip()


def ensure_distinct_revisions(baseline_commit: str, candidate_commit: str) -> None:
    if baseline_commit == candidate_commit:
        raise ValueError("baseline and candidate revisions must differ")


def materialize_revision(revision: str, *, repo_root: Path, temp_root: Path, label: str) -> RevisionBuild:
    resolved = resolve_revision(revision, repo_root=repo_root)
    source_dir = temp_root / f"{label}-source"
    target_dir = temp_root / f"{label}-target"
    source_dir.mkdir(parents=True, exist_ok=False)
    target_dir.mkdir(parents=True, exist_ok=False)
    archive = _git_stdout(["git", "archive", "--format=tar", resolved], repo_root=repo_root)
    with tarfile.open(fileobj=BytesIO(archive), mode="r:") as tar:
        tar.extractall(source_dir)
    return RevisionBuild(
        label=label,
        requested_rev=revision,
        resolved_commit=resolved,
        source_dir=source_dir,
        target_dir=target_dir,
        binary_path=target_dir / "release" / "rstim",
    )


def build_revision(revision: RevisionBuild) -> Path:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(revision.target_dir)
    subprocess.run(
        ["cargo", "build", "--release", "-p", "rstim", "--bin", "rstim"],
        cwd=revision.source_dir,
        env=env,
        check=True,
    )
    if not revision.binary_path.is_file():
        raise FileNotFoundError(f"expected rstim binary not found: {revision.binary_path}")
    return revision.binary_path
```

Implement command, timing, raw, summary, report, and environment logic matching
the design. Use `subprocess.Popen(..., stdout=subprocess.PIPE,
stderr=subprocess.PIPE)` and `communicate()` in `time_cli`. Use logical
recorded argv roles from `TOOL_ROLES` so artifacts do not contain temporary
binary paths.

- [ ] **Step 4: Run unit tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise -q
```

Expected: PASS.

- [ ] **Step 5: Run issue verification command**

Run:

```sh
rm -rf /tmp/rstim-paired-frame-noise
python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise \
  --baseline-rev f10d1ed024d3519318ed244c9095724074519595 \
  --candidate-rev HEAD \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --shots 1024 --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-paired-frame-noise
```

Expected stdout:

```text
PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384
```

- [ ] **Step 6: Run required workspace verification**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/run_paired_frame_noise.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_paired_frame_noise.py
git commit -m "feat: add paired frame-noise runner"
```
