# Support and compatibility contract

This document tracks the development branch. The installed CLI quickstart
targets v0.3.3; unchanged library API examples remain pinned to v0.3.0.
Use each release's notes for its frozen support boundary.
Development-branch wording describes the upcoming release state; a decoder
maturity is binding only for a published release whose evidence bundle
verifies it (v0.3.3 is the corrected candidate for both Matching and MLE). It is a
contract for using the shipped interfaces, not a claim that every research
component, circuit dialect, decoder, or benchmark result is ready for
publication-scale use.

## Support levels

| Surface | Level | Supported boundary |
| --- | --- | --- |
| `rustqec` unified CLI and its capability/error envelopes | Supported | Use the commands and structured error codes advertised by `rustqec capabilities --format json`. The CLI rejects unsupported inputs with a named error code instead of silently producing a result. |
| `rstim` circuit APIs and CLI | Supported | The documented simulator and CLI inputs are supported within their documented command-specific limits. The contract does not extend to every Stim extension or every analysis/export mode. |
| Atom-loss `envelope-matching` decoder | <span data-decoder-support-copy="envelope-matching">Beta unless v0.3.3 publication verification succeeds</span> | Flat loss-visible Mid-SWAP memory-Z circuits within the declared circuit contract and the measured operating envelope. The v0.3.1 and v0.3.2 candidates did not complete consistent publication verification. Finite tested size/loss points are not an untested Cartesian-product or universal latency guarantee. |
| Atom-loss `envelope-mle` decoder | <span data-decoder-support-copy="envelope-mle">Beta unless v0.3.3 publication verification succeeds</span> | Exactly four measured Mid-SWAP workload points declared by the executable scope plan [`docs/envelope-mle-scope.json`](envelope-mle-scope.json): d=3/r=2 at loss 0.002 and d=3/r=1 at loss 0.01, each at batches 1,024 and 16,384, with no interpolation. Requires the `ilp` feature or an official native archive; the conventional fixture and every unlisted size/loss/batch point are outside the Supported promise. |
| Decoder experiments, benchmark harnesses, and optional visualization/research workflows | Experimental | These are useful implementation and evidence tools. Their presence does not establish a universal decoder comparison, a universal Stim/PyMatching replacement, or a publication-scale result. |

## Atom-loss support boundary

The atom-loss support promise is defined per decoder by the executable matrix
[`docs/envelope-support.json`](envelope-support.json). The matrix records each
decoder's build features and maturity, the circuit contract (families, readout
basis, allowed instructions, observable/sweep/REPEAT restrictions), the
revision the promise applies to, and one acceptance or rejection control for
every declared behavior. It is verified by executing both decoders against the
declared controls:

```sh
cargo build --release --locked -p rustqec-cli --features ilp
python3 tools/check_envelope_support.py \
  --binary target/release/rustqec \
  --matrix docs/envelope-support.json \
  --out drafts/envelope-readiness/support.json
```

The checker exits nonzero unless every control matches, including structured
rejection codes and output-file rules; a binary without ILP fails the run
rather than silently skipping the MLE controls.

### Decoder-specific support table

| Property | `envelope-matching` | `envelope-mle` |
| --- | --- | --- |
| Maturity | <span data-decoder-support-copy="envelope-matching">Beta unless publication verification succeeds</span> (verified by its release evidence bundle) | <span data-decoder-support-copy="envelope-mle">Beta unless publication verification succeeds</span> (verified independently by its release evidence bundle and [MLE scope plan](envelope-mle-scope.json)) |
| Build requirement | Default CLI builds | `--features ilp` or an official native archive |
| Objective | Minimum-weight matching on a loss-conditioned graph (an approximation; allowed ties are defined by the independent correctness suite) | Exact most-likely fault configuration of the declared envelope model |
| Mid-SWAP family (`midswap` fixtures) | Checked acceptance domain; measured operating ranges in the [resource report](../benchmarks/atom_loss/readiness/resources/report.md) (workload/machine-specific) | Supported domain: the four measured d=3 points in the [MLE scope plan](envelope-mle-scope.json): r=2 at loss 0.002 and r=1 at loss 0.01, each with batches 1,024 and 16,384. No unmeasured loss/batch interpolation or universal timing guarantee. |
| Conventional Stim-annotated family | Isolated checked example (pinned fixture only) | Excluded: rejects the pinned fixture as `unsupported_circuit` (candidate limit) before publishing any output file |
| Per-shot timeout | Rejected (`--shot-timeout-ms` is MLE-only) | `--shot-timeout-ms`; timeout stops the batch with `decode_timeout` (exit 3), writing diagnostic statistics but no predictions |
| Infeasible shot | Not applicable | `decode_infeasible` (exit 3), diagnostic statistics but no predictions |
| Unsupported input | `unsupported_circuit` (exit 2), no prediction or statistics files | Same |

Unsupported input, timeout, and infeasible outcomes are distinct results:
compilation rejection produces neither predictions nor statistics, while MLE
timeout/infeasible may emit diagnostic statistics with zero completed
predictions. For any circuit outside the checked domain, treat
`unsupported_circuit` as a support-boundary result: do not reinterpret it as a
prediction, and do not rely on absent output files.

### MLE Supported scope

The MLE Supported domain and its evidence requirements are declared as a
machine-readable plan, [`docs/envelope-mle-scope.json`](envelope-mle-scope.json)
(issue #721). The plan pins the finite supported grid — Mid-SWAP distance 3,
r=2 at loss 0.002 and r=1 at loss 0.01, each at batches 1,024 and 16,384 —
the required correctness/resource/installed-platform evidence for
every declared point, the candidate-limit and solve-only timeout semantics,
and the standing exclusion of the conventional family. An offline suite
validates the plan and classifies decode jobs against the domain boundary
(`in-domain` vs `outside-supported-domain`, a support statement rather than a
decoder prediction):

```sh
python3 -m unittest tools.test_envelope_mle_scope
```

Out-of-domain inputs are unpromised, not automatically rejected: only the
hard limits (candidate count, REPEAT blocks, unsupported instructions) produce
a guaranteed structured rejection. v0.3.3 can record the first promotion only
after every required case, the release gate on the exact source, and the
post-publication asset verification succeed. Later Supported releases must
pass the same gate again.

### Regression controls

The Mid-SWAP MLE positive control:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures current_rstim_atom_loss_midswap_envelope_mle_decodes_unmodified -- --exact
```

The conventional candidate-explosion fixture is explicitly excluded from the
MLE route. It must fail before publishing prediction or statistics files,
using the existing `unsupported_circuit` structured failure:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures current_rstim_atom_loss_conventional_envelope_mle_rejects_candidate_explosion -- --exact
```

Successful decoding of the pinned fixtures shows that these support paths
still work. It is not publication-scale validation, a logical-error-rate
campaign, or evidence that arbitrary atom-loss circuits are supported.

### Release-readiness gate

Promotion from Beta to Supported is decided per decoder by the
release-readiness gate (issue #716), not by the matrix alone. The gate
aggregates the revision-bound evidence bundle — the support-matrix result, the
independent correctness suite, the measured resource envelope, and one
installed-artifact contract report per official native target
(`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`) — and promotes a decoder
only when every artifact passes and was produced from the release candidate
revision (or a recorded source-equivalence check):

```sh
python3 tools/check_envelope_release.py \
  --evidence-dir drafts/envelope-readiness \
  --matrix docs/envelope-support.json \
  --policy docs/envelope-compatibility-policy.md \
  --candidate-revision "$(git rev-parse HEAD)" \
  --out drafts/envelope-readiness/release-gate.json
```

The installed-artifact reports are produced from the verified release archive
of each platform (never a PATH binary):

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

A missing expected decoder, a missing platform report, a partially executed
control set, hollowed correctness coverage, or a revision mismatch fails the
gate; the decoder then remains Beta with its blocking gaps recorded in the
gate report. The retained full resource report supports a candidate only when
its measurement revision is an ancestor of the candidate with identical
measurement-relevant sources (the measurement script, the decoder crates and
`Cargo.lock`); otherwise the full campaign must be rerun. What the promoted
surface freezes — CLI arguments, dataset interpretation, prediction packing,
structured error codes, statistics semantics, and the evolution/deprecation
rules — is defined by
[`docs/envelope-compatibility-policy.md`](envelope-compatibility-policy.md).

A passing gate decides candidacy; the durable proof travels with the release.
Each native release whose gate promotes a decoder publishes a checksummed,
version-bound evidence bundle (`envelope-support-evidence-<tag>.tar.gz`)
freezing the gate report, the consumed matrix/policy, the MLE scope plan, the
support/correctness/general-resource/MLE-resource evidence, and one
archive-bound installed report per platform.
Verify a downloaded release with
`python3 tools/check_envelope_publication.py --release-dir <dir> --expect-decoder <name>`;
releases without the bundle (v0.3.0) are reported as lacking a verified
promotion. See the
[native archive guide](https://github.com/nzy1997/rust-qec/blob/master/docs/native-release-archives.md#envelope-support-evidence-bundle).

## Mid-SWAP configuration migration

The pre-1.0 Mid-SWAP API no longer accepts the old catch-all
`pauli_probability` field. Replace it with the four named Pauli channels and
initialize `before_round_data_loss_probability` explicitly:

```compile_fail
use rstim::codegen::MidSwapConfig;

// Old API: this does not compile against the current release line.
let config = MidSwapConfig {
    distance: 3,
    rounds: 2,
    before_round_data_depolarization: 0.001,
    before_round_data_loss_probability: 0.0,
    after_clifford_depolarization: 0.001,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
    operation_loss_probability: 0.0,
    measurement_loss_probability: 0.0,
    pauli_probability: 0.001,
};
```

```rust
use rstim::codegen::{MidSwapConfig, rotated_memory_z_midswap};

let config = MidSwapConfig {
    distance: 3,
    rounds: 2,
    before_round_data_depolarization: 0.001,
    before_round_data_loss_probability: 0.0,
    after_clifford_depolarization: 0.001,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
    operation_loss_probability: 0.0,
    measurement_loss_probability: 0.0,
};
let circuit = rotated_memory_z_midswap(config).unwrap();
assert!(!circuit.is_empty());
```

The valid form is compiled by the `MidSwapConfig` rustdoc test. The obsolete
form is intentionally marked `compile_fail`; it documents a migration rather
than a compatibility shim.

## Compatibility and deprecation policy

RustQEC is pre-1.0. Public Rust APIs, JSON schemas, CLI arguments, defaults,
and generated formats can change between release lines. A compatibility promise
exists only where a format reference, a version field, or a structured CLI
contract says so. In particular, QP101-ZY is governed by
[`rstim/doc/QP101-ZY.md`](../rstim/doc/QP101-ZY.md); other experimental exports
should be treated as release-line specific unless they state a versioned
compatibility policy.

Deprecations are documented in the affected API or command reference with a
migration path when one exists. The Mid-SWAP field rename above is an example:
the removed field remains a compile-time error so callers must choose the four
separate channels rather than receive an implicit mapping.

The repository tag identifies a RustQEC source release. Workspace crates are
independently versioned packages: inspect each crate's `Cargo.toml` and Cargo
metadata when selecting a dependency version. A repository tag therefore does
not imply that every workspace package has the same package version.

## Evidence and known exclusions

The publication-evidence checker can be structurally consistent while still
reporting `publication_ready=false`; it currently reports gaps rather than
freshly reproduced publication-grade measurements. Passing that consistency
check is not publication readiness.

The following open work remains outside this contract:

- [#601](https://github.com/nzy1997/rust-qec/issues/601): publication-grade,
  multi-platform benchmark evidence.
- [#209](https://github.com/nzy1997/rust-qec/issues/209): BB circuit BP-OSD
  runtime gap investigation.
- [#550](https://github.com/nzy1997/rust-qec/issues/550): high-rate classical
  APM smoke-case runtime investigation.

These links preserve the known limitations; they do not claim that their
historical measurements have been rerun for this release line.

<span id="development-cargo-features-before-the-first-registry-release"></span>

## Cargo features in the 0.3 release line

The 0.3 source package defaults keep native solver dependencies optional.
`rustqec-cli` needs `--features ilp` for `envelope-mle`; default builds advertise
only the available decoder choices. Official native archives retain ILP support.
`rsinter` needs explicit runner/plotting features, or `full` for the previous
complete research setup. These changes do not alter immutable v0.2.1 archives.
See the [crate guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md) for installation and migration commands.

`rmatching::Matching::try_decode_shots_bit_packed` returns a typed error for
incorrect dimensions, buffer lengths, or batch-size overflow. It counts declared
unused detectors/observables as part of the graph. The older infallible method
remains available and documents its panic conditions. The DEM importer supports
probabilities `0 <= p < 1`; probability-1 errors require a different representation
and are rejected explicitly. This restriction does not apply to rstim sampling
or DEM generation, including the probability-1 installed quickstart.

Library defaults now omit compatibility CLI parsing, CSS generation, the local
viewer, and native ILP backends. Enable `rstim` features `cli`, `codegen-css`,
and `shot-viewer` where needed; `qec-code/cli` enables its executable and
`qec-ilp-core/highs` enables its open-source solver. Full native archives retain
the viewer. `rmatching` benchmark binaries moved to the unpublished
`rmatching-bench-tools` workspace member. See the
[feature migration guide](https://github.com/nzy1997/rust-qec/blob/master/docs/crates-io.md#library-feature-boundaries).
