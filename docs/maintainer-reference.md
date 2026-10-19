# Maintainer reference

Run these checks from a configured repository checkout. The release gate
checks whether the evidence meets a decoder’s support promise; publication
verification checks the files actually shipped. User-facing limits live in
[Support and compatibility](support-compatibility.md).

Envelope MLE (maximum-likelihood estimation) uses an ILP (integer linear
programming) solver; the controls below build it explicitly.

## CI selection

The five required checks keep their existing names: `test`, `coverage`,
`rsmp-v1-readiness`, `checked-evidence-portability`, and
`publication-evidence-check`. Do not add workflow-level path filters to their
`CI` workflow: the repository and release gates require successful reports.

On PRs, `tools/ci_scope.py` compares the PR head with its merge base. Site-only
changes and recognized README, license, and agent-instruction files skip Rust
setup and compilation inside `test`; repository policy tests still run.
Unknown files, Rust files, build inputs, assets, test fixtures, top-level
`docs/`, and package `doc/` and `docs/` trees select the Rust gate. Documentation
can be embedded by `include_str!` or read as executable examples by integration
tests, so Markdown under those documentation trees keeps Rust validation.
Deleted and renamed files count, and an unreadable diff fails the check.
The Rust gate uses `make test` plus the minimal-feature and optional-backend checks.

`coverage` reports the policy in its job summary on PRs. Coverage generation
runs on master pushes, weekly CI runs, and manual runs, alongside full Rust
tests. The other three required evidence/readiness checks still run on every
PR. No ruleset changes are needed for this selection policy.

| Workflow | PR selection | Full runs |
| --- | --- | --- |
| Native support | Rust/build/native-support inputs; Linux MSRV and macOS stable | Four platform/toolchain cells on relevant master pushes, weekly, or manually |
| Crates.io readiness | Rust, manifests, library READMEs, and package tooling | Every master push or manual run |
| Near-Clifford diagnostic evidence | Near-Clifford implementation, simulator, benchmark evidence/protocol, artifact tooling, Cargo manifests/lockfiles, build scripts, and toolchain/Cargo configuration | Relevant master pushes, weekly, or manually |
| Atom-loss correctness reference | Sampler, circuit, decoder, atom-loss evidence, and envelope inputs; candidate correctness and smoke resources | Weekly/manual runs also execute the full contract, historical replay, and resource campaigns |
| Native release archives | Manifests, release/build/install tooling, and envelope policy | Manual immutable-tag builds retain all release and publication gates |
| Interactive shot quality | Viewer and documentation build inputs | Weekly or manually |
| Interactive shot fuzz | Rust viewer dependencies and fuzz inputs; five-second budget per target | Weekly with a sixty-second budget, or manually |

Ordinary site edits run the required checks and browser/site checks without
launching the native, packaging, historical benchmark, or fuzz matrices.
Native archives cancel superseded PR runs but preserve manual release runs;
atom-loss runs also cancel superseded checks on the same branch.

The full native support verifier still requires all four cells. Use a full
master, scheduled, or manual run when collecting support evidence; the two-cell
PR run does not certify the complete support matrix.

Validate CI selection locally with:

```sh
python3 -m unittest tools.test_ci_scope tools.test_check_repository_gate tools.test_check_release_gate
actionlint -shellcheck= -pyflakes=
```

## Atom-loss regression controls

Validate the finite MLE scope plan before running the decoder controls. The
plan covers only the four declared distance-3 loss/batch points; passing this
check does not promote the decoder or widen that grid:

```sh
python3 -m unittest tools.test_envelope_mle_scope
```

Build the ILP-capable CLI before running the complete envelope support matrix:

```sh
cargo build --release --locked -p rstim --features ilp
python3 tools/check_envelope_support.py \
  --binary target/release/rstim \
  --matrix docs/envelope-support.json \
  --out drafts/envelope-readiness/support.json
```

The pinned Mid-SWAP MLE positive control must decode successfully:

```sh
cargo test --locked -p rstim --test external_fixtures \
  current_rstim_atom_loss_midswap_envelope_mle_decodes_unmodified -- --exact
```

The conventional candidate-explosion fixture is outside the MLE support
boundary and must fail before publishing predictions or statistics:

```sh
cargo test --locked -p rstim --test external_fixtures \
  current_rstim_atom_loss_conventional_envelope_mle_rejects_candidate_explosion -- --exact
```

## Decoder release-readiness gate

Promotion requires every scoped case, the release gate at the candidate revision,
and post-publication asset verification to pass. Later Supported releases repeat
the same gate. The offline suite's `in-domain` and `outside-supported-domain`
labels classify support scope.

Promotion is decided per decoder. The gate consumes the support matrix,
independent correctness report, measured resource envelope, and installed
artifact reports for `x86_64-unknown-linux-gnu` and
`aarch64-apple-darwin`:

```sh
python3 tools/check_envelope_release.py \
  --evidence-dir drafts/envelope-readiness \
  --matrix docs/envelope-support.json \
  --policy docs/envelope-compatibility-policy.md \
  --candidate-revision "$(git rev-parse HEAD)" \
  --out drafts/envelope-readiness/release-gate.json
```

Produce each installed-artifact report from the extracted release archive,
never from a binary found on `PATH`:

```sh
python3 tools/check_installed_envelope.py \
  --bin-dir extracted/<archive-root>/bin \
  --matrix docs/envelope-support.json \
  --target aarch64-apple-darwin \
  --archive <archive.tar.gz> \
  --source-sha <candidate-revision> \
  --expect-ilp \
  --out drafts/envelope-readiness/installed-aarch64-apple-darwin.json
```

A missing decoder, platform report, control, correctness check, or matching
revision fails the gate. Retained resource measurements are reusable only when
their revision is an ancestor of the candidate and all measurement-relevant
sources remain identical.

## Publication evidence bundle

Every native release that promotes a decoder publishes a checksummed
`envelope-support-evidence-<tag>.tar.gz` containing the gate report, matrix,
policy, correctness and resource evidence, and archive-bound platform reports.
Verify a downloaded release directory with:

```sh
python3 tools/check_envelope_publication.py \
  --release-dir <dir> \
  --expect-decoder <name>
```

Releases without that bundle, including v0.3.0, do not carry a verified
envelope promotion.

## Publication gaps and tracked exclusions

Passing a structural evidence checker is not the same as
`publication_ready=true`. The following work remains outside the published
claims:

- [#601](https://github.com/nzy1997/rust-qec/issues/601): publication-grade,
  multi-platform benchmark evidence.
- [#209](https://github.com/nzy1997/rust-qec/issues/209): BB circuit BP-OSD
  runtime gap investigation.
- [#550](https://github.com/nzy1997/rust-qec/issues/550): high-rate classical
  APM smoke-case runtime investigation.

## Release checklist

1. Confirm the candidate versions in every public crate manifest.
2. Run the support matrix and independent correctness checks.
3. Produce both native archive reports from the candidate artifacts.
4. Run the release-readiness gate at the candidate revision.
5. Publish and checksum the evidence bundle with the release.
6. Verify the published bundle and update the user-facing support contract.

## RSMP compression evidence

Committed compression evidence lives under
`benchmarks/rstim_vs_stim_simulator/results/rsmp-v1/`. The checked gate names
are `benchmark_raw_lt_20pct`, `benchmark_zstd_lt_75pct`, and
`high_entropy_raw_le_102pct`; their arithmetic is integer cross-multiplication
over recorded byte counts.

The evidence checker is:

```console
python3 tools/check_rsmp_v1_compression_evidence.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/rsmp-v1
```

The reproduction command for regenerating the evidence remains separate from
readiness:

```console
python3 -m benchmarks.rstim_vs_stim_simulator.run_rsmp_compression \
  --rstim target/release/rstim \
  --catalog rstim/tests/fixtures/rsmp/catalog.json \
  --case stim_surface_d11_r100 \
  --shots 1024 \
  --seed 7 \
  --zstd-level 3 \
  --out-dir /tmp/rstim-rsmp-v1-evidence
```

The benchmark row also carries Stim-format baselines for exactly `b8`, `r8`,
and `ptb64`. All three serialize the same canonical 1024-shot, seed-7
measurement batch; `r8` is produced with the pinned `stim convert` command and
`ptb64` with the pinned stim Python API (the Stim CLI streams single records
and cannot write `ptb64`). Each baseline records its raw byte count, its
direct level-3 Zstandard byte count under the same recorded contract, the
conversion argv, the artifact SHA-256, and a round-trip `b8` SHA-256 that must
equal the canonical measurement SHA-256. The summary and report state byte
counts and ratios relative to the RSMP archive only; no universal
cross-format compression superiority is claimed. The pinned Stim binary
identity, version, and every conversion command are recorded in
`environment.json`.

The runner writes the bundle to a temporary output directory so the evidence
check above stays separate from regeneration. To publish a regenerated bundle
over the committed location, copy the five generated files:

```console
cp /tmp/rstim-rsmp-v1-evidence/{raw.jsonl,summary.json,report.md,environment.json,artifact-sha256.json} \
  benchmarks/rstim_vs_stim_simulator/results/rsmp-v1/
```

Regeneration must run from a clean Git tree so the recorded environment has
`git.dirty=false`.

The recorded environment includes producer identity, Git state, Rust target,
Cargo.lock hash, zstd crate versions, native zstd version, full command argv,
and artifact SHA-256 values. These gates prove only the pinned `rsmp v1`
evidence cases under that recorded producer and zstd contract. No fixed
wall-clock performance gate, cross-version byte-for-byte writer determinism,
or broader compression claim is made.
