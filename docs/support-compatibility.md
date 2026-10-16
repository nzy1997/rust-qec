# Support and compatibility contract

This contract covers the RustQEC 0.3 release line. The documentation version
selects the development branch or a stable release; individual crates may have
different patch versions. Package requirements such as `rstim >=0.3.1,<0.4.0`
apply to that crate.

## Support levels

**Supported** means the documented behavior is checked within its declared
limits. **Beta** means release verification is still pending. **Experimental**
identifies research workflows without a supported-interface promise.

| Surface | Level | Supported boundary |
| --- | --- | --- |
| `rstim` unified CLI and its capability/error envelopes | Supported | Commands and structured error codes advertised by `rstim capabilities --format json` |
| `rstim` circuit APIs and CLI | Supported | Documented simulator and CLI inputs within command-specific limits; support varies by analysis/export mode |
| Atom-loss envelope decoders | [Decoder-specific status below](#atom-loss-support-boundary) | Loss-visible Z readout circuits; build requirements and tested operating points below |
| Decoder experiments, benchmark harnesses, and optional visualization/research workflows | Experimental | Inputs and methodology are specific to each workflow |

## Atom-loss support boundary

The machine-readable [support matrix](envelope-support.json) records each
decoder's build requirements, circuit limits, applicable revision and expected
acceptance or rejection outcomes. Status is verified against published release
evidence; verification procedures are in the [maintainer reference](maintainer-reference.md).

### Decoder-specific support table

| Property | `envelope-matching` | `envelope-mle` |
| --- | --- | --- |
| Maturity | <span data-decoder-support-copy="envelope-matching">Beta unless publication verification succeeds</span> | <span data-decoder-support-copy="envelope-mle">Beta unless publication verification succeeds</span> |
| Build requirement | Default CLI builds | `--features ilp` or an official native archive |
| Objective | Minimum-weight matching on a loss-conditioned graph (an approximation; optimal ties allowed) | Exact most-likely fault configuration of the declared envelope model |
| Mid-SWAP family (`midswap` fixtures) | Flat memory-Z circuits within the circuit contract; tested operating points are in the [resource report](../benchmarks/atom_loss/readiness/resources/report.md). Untested combinations of size/loss points and latency on other machines are unpromised. | The four workload points in [MLE Supported scope](#mle-supported-scope) below |
| Conventional Stim-annotated family | Isolated checked example (pinned fixture only) | Excluded: rejects the pinned fixture as `unsupported_circuit` (candidate limit) before publishing any output file |
| Per-shot timeout | Rejected (`--shot-timeout-ms` is MLE-only) | `--shot-timeout-ms`; timeout stops the batch with `decode_timeout` (exit 3), writing diagnostic statistics but no predictions |
| Infeasible shot | Not applicable | `decode_infeasible` (exit 3), diagnostic statistics but no predictions |
| Unsupported input | `unsupported_circuit` (exit 2), no prediction or statistics files | Same |

### MLE Supported scope

The [MLE scope plan](envelope-mle-scope.json) covers Mid-SWAP distance 3:

| Rounds | Loss parameter | Batch shots |
| --- | --- | --- |
| 2 | 0.002 | 1,024 and 16,384 |
| 1 | 0.01 | 1,024 and 16,384 |

The conventional family and unlisted size/loss/batch points are outside the
Supported promise. There is no interpolation or universal timing guarantee.
Out-of-domain inputs can still be accepted; only hard limits such as candidate
count, `REPEAT` blocks and unsupported instructions guarantee rejection.
The per-shot timeout applies to solving.

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
`rstim` needs `--features ilp` for `envelope-mle`; default builds advertise
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
