# Issue 492 Paired Frame-Noise Runner Design

Issue: #492

## Context

Issue #492 adds a same-machine benchmark runner for the frame-noise sampling
path. Existing checked frame-noise evidence proves that instruction-wide
iterator builds replace per-target setup work, but it does not measure the
wall-clock effect. The new runner compares two `rstim` revisions directly:

- baseline: `f10d1ed024d3519318ed244c9095724074519595`, the last commit before
  instruction-wide noise routing;
- candidate: a user-supplied revision, normally `HEAD`.

Dependency #491 is closed and merged into the current branch history. Repository
instructions live in `.AGENTS/AGENTS.md`; the QP101 visualization rules are not
relevant because this issue only adds a Python benchmark runner and tests.

## Approaches Considered

1. Add a dedicated paired frame-noise runner. The runner materializes revisions
   through `git archive`, builds both into separate temporary target
   directories, validates the canonical `rstim sample --skip_reference_sample
   --out_format b8` command, alternates execution order, and writes raw,
   summary, report, environment, and artifact hash files.

2. Reuse `run_fair_cli.py` and treat baseline/candidate rstim binaries as two
   fair CLI variants. This would borrow artifact conventions, but the fair CLI
   runner is explicitly a Stim-vs-rstim runner and would mix two different
   benchmark contracts.

3. Add a production legacy-noise switch and compare one binary in two modes.
   This would simplify builds, but it is out of scope and would expose legacy
   behavior through the production CLI.

The selected approach is option 1. It keeps the baseline/candidate revision
isolation explicit, avoids production switches, and keeps the benchmark
contract independent from later reference-build changes.

## Interface

Add module `benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise` with:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise \
  --baseline-rev <sha> --candidate-rev <sha|HEAD> \
  --fixture <path> --shots 1024 \
  --warmup-rounds 2 --measure-rounds 7 --out-dir <dir>
```

The runner resolves both revisions with `git rev-parse`. It rejects equal
resolved commits with:

```text
baseline and candidate revisions must differ
```

The runner accepts only the canonical full fixture contract:

- fixture path:
  `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`;
- fixture SHA-256:
  `a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229`;
- shots: `1024`;
- output format: `b8`;
- expected output bytes: `1552384`;
- warmup rounds: nonnegative;
- measure rounds: positive.

## Revision Builds

Each revision is materialized without switching the current checkout:

1. `git archive <resolved-rev>` streams a tar archive.
2. The archive is extracted into a temporary source directory under the runner
   temp root.
3. `cargo build --release -p rstim --bin rstim` runs with a revision-specific
   `CARGO_TARGET_DIR`.
4. The runner records the resulting `target/release/rstim` identity by version,
   basename, SHA-256, and logical role.

Candidate `HEAD` is also materialized through `git archive`; the active
worktree is not built in place and is not modified.

## Command Contract

Every timed child process uses exactly this logical command shape:

```text
tool://rstim-<variant> sample --skip_reference_sample --shots <shots> --seed <seed> --out_format b8 --in <fixture>
```

The executable path is absolute only in the actual subprocess argv. Recorded
argvs use logical tool roles and repository-relative fixture paths, matching
the portable evidence conventions used by the existing benchmark runners.

Before running any round, a canonical command validator checks that each
expanded argv contains:

- command `sample`;
- `--skip_reference_sample`;
- `--shots <shots>`;
- `--seed <seed>`;
- `--out_format b8`;
- `--in <fixture>`.

The validator rejects missing `--skip_reference_sample`, asymmetric output
format, missing values, unknown variant names, and fixture mismatches before
spawning the timed process.

## Round Scheduling

The variants are `baseline-rstim-frame-noise-b8` and
`candidate-rstim-frame-noise-b8`.

For each round index, both variants use the same seed. Warmup seeds are
`0..warmup_rounds-1`, measured seeds continue immediately after the warmups,
and with the required arguments the measured seeds are `2..8`.

Ordering alternates by round:

- even round indexes run baseline first, candidate second;
- odd round indexes run candidate first, baseline second.

Warmup and measured records are written to `raw.jsonl`, but `summary.json`
derives statistics only from measured records. Each timed process is measured
from before `subprocess.Popen` through `communicate()` completion and child
exit, so stdout and stderr are fully drained.

Each successful round must exit with code `0` and produce exactly `1552384`
stdout bytes. A short output, including one byte fewer from a fake candidate,
raises before `summary.json` is written.

## Artifacts

The runner writes these files under `--out-dir`:

- `raw.jsonl`
- `summary.json`
- `report.md`
- `environment.json`
- `artifact-sha256.json`

`raw.jsonl` contains one JSON object per warmup or measured child process. Each
record includes case identity, variant, phase, round index, seed, ordering
slot, logical argv, shots, measurement count, output format, timer scope,
elapsed nanoseconds, actual output bytes, stdout SHA-256, exit code, and
resolved revision.

`summary.json` records case metadata, revision metadata, measured record count
`14`, expected bytes `1552384`, and per-variant elapsed nanosecond summaries
with samples, min, max, mean, median, total output bytes, and stdout SHA-256
lists.

`report.md` renders the same derived summary in a short markdown table. It
does not impose or imply an absolute timing threshold.

`environment.json` records OS, CPU, Rust/Cargo versions, current repository
commit, current dirty flag, fixture path and hash, baseline/candidate revision
inputs and resolved commits, build target isolation, runtime identities, round
argvs, and the exact runner argv.

`artifact-sha256.json` maps the other four artifact filenames to SHA-256
digests.

The CLI prints exactly:

```text
PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384
```

for the required verification command.

## Error Handling

Validation runs before summary generation. The runner fails with a nonzero exit
code and stderr message for:

- equal baseline and candidate revisions;
- missing fixture or noncanonical fixture hash;
- invalid warmup or measured round counts;
- canonical command validation failure;
- build failure for either revision;
- nonzero child exit;
- child stdout byte count other than `1552384`.

No failure path writes `summary.json`, `report.md`, `environment.json`, or
`artifact-sha256.json` after a semantic round failure.

## Testing

Add `benchmarks/rstim_vs_stim_simulator/tests/test_run_paired_frame_noise.py`
with unit coverage for:

- same-revision rejection with the exact message;
- `git archive` materialization and separate baseline/candidate target dirs;
- alternating baseline-first and candidate-first measured ordering;
- identical fixture, shots, and seed use across paired variants;
- process timing through complete stdout/stderr drain and exit;
- exact 1,552,384 byte enforcement before summary generation;
- canonical command validator rejecting missing `--skip_reference_sample`;
- artifact shape and summary derivation from raw records;
- CLI success line for the fake-rstim path.

Run focused verification:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_paired_frame_noise -q
```

Run the issue verification:

```sh
rm -rf /tmp/rstim-paired-frame-noise
python3 -m benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise \
  --baseline-rev f10d1ed024d3519318ed244c9095724074519595 \
  --candidate-rev HEAD \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --shots 1024 --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-paired-frame-noise
```

Run the required Rust verification:

```sh
cargo test
```

## Scope Limits

- Do not expose a production legacy-noise switch.
- Do not include reference-build timing.
- Do not switch or modify the current checkout while materializing revisions.
- Do not impose an absolute timing threshold.
- Do not update checked benchmark result catalogs or site metadata.

## Self Review

- No incomplete markers remain.
- The baseline revision and output byte count match the issue exactly.
- The design uses `git archive` and separate target directories.
- The command contract requires `--skip_reference_sample --out_format b8`.
- Short output and same-revision negative controls fail before summary
  generation.
- The selected approach keeps production code and reference-build timing out of
  scope.
