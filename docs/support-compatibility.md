# Support and compatibility contract

This document defines the support contract for the RustQEC 0.3 release line.
The documentation edition banner identifies whether a rendered copy follows
the development branch or a frozen stable release.
The coordinated repository release and individual crate patch versions are
separate: a requirement such as `rustqec-cli >=0.3.1,<0.4.0` applies to that
package without renaming the whole documentation set. A decoder maturity is
binding only for a published package release whose evidence bundle verifies
it. This is a contract for using the shipped interfaces, not a claim that every
research component, circuit dialect, decoder, or benchmark result is ready for
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
revision the promise applies to, and the expected acceptance or rejection for
every declared behavior. Users should treat this matrix as the authoritative
machine-readable boundary. The commands that verify it before a release live
in the [maintainer reference](maintainer-reference.md).

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
pass the same gate again. Maintainer regression, gate, and publication-bundle
commands live in the [maintainer reference](maintainer-reference.md).

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

The repository tag identifies a coordinated RustQEC source release. Published
workspace crates share its major/minor release line, while package-only patch
releases can advance independently. Inspect each crate's `Cargo.toml` and Cargo
metadata when reproducing an exact package version.

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
