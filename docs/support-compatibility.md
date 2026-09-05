# Support and compatibility contract

This page describes the support boundary for the current RustQEC release line.
It is a contract for using the shipped interfaces, not a claim that every
research component, circuit dialect, decoder, or benchmark result is ready for
publication-scale use.

## Support levels

| Surface | Level | Supported boundary |
| --- | --- | --- |
| `rustqec` unified CLI and its capability/error envelopes | Supported | Use the commands and structured error codes advertised by `rustqec capabilities --format json`. The CLI rejects unsupported inputs with a named error code instead of silently producing a result. |
| `rstim` circuit APIs and CLI | Supported | The documented simulator and CLI inputs are supported within their documented command-specific limits. The contract does not extend to every Stim extension or every analysis/export mode. |
| Atom-loss envelope decoding | Beta | The checked Mid-SWAP fixture below is supported by the current native compiler and `envelope-mle`; other circuit shapes remain limited by the compiler's explicit acceptance rules. |
| Decoder experiments, benchmark harnesses, and optional visualization/research workflows | Experimental | These are useful implementation and evidence tools. Their presence does not establish a universal decoder comparison, a universal Stim/PyMatching replacement, or a publication-scale result. |

## Atom-loss support boundary

The native atom-loss compiler accepts the flat, loss-visible Mid-SWAP subset
used by `rustqec-cli/tests/fixtures/current_rstim_atom_loss/midswap_canonical_mle/`.
The supported regression control is intentionally narrow: it exercises the
current d=5/r=15 Mid-SWAP circuit and one pinned row with `envelope-mle`.

Run the positive control:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures current_rstim_atom_loss_midswap_envelope_mle_decodes_unmodified -- --exact
```

Successful decoding of that small fixture shows that this current support path
still works. It is not publication-scale validation, a logical-error-rate
campaign, or evidence that arbitrary atom-loss circuits are supported.

The conventional candidate-explosion fixture is explicitly excluded from this
route. It must fail before publishing prediction or statistics files, using the
existing `unsupported_circuit` structured failure:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures current_rstim_atom_loss_conventional_envelope_mle_rejects_candidate_explosion -- --exact
```

For any other circuit, treat `unsupported_circuit` as a support-boundary result:
do not reinterpret it as a prediction, and do not rely on absent output files.

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
    pauli_probability: 0.001,
    operation_loss_probability: 0.0,
    measurement_loss_probability: 0.0,
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
let circuit = rotated_memory_z_midswap(config)?;
# Ok::<(), rstim::codegen::MidSwapError>(())
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
