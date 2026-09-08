# qec-code

`qec-code` provides Rust data structures and algorithms for constructing and
analyzing quantum error-correcting codes. Its dependency-facing APIs include:

- CSS matrix validation and construction;
- exact distance through optional HiGHS or Gurobi ILP backends;
- randomized CSS distance upper bounds; and
- packed GF(2) row operations, reduced row spaces, and reusable kernel
  workspaces under `qec_code::packed_gf2`.

## Installation

Add the library from crates.io:

```toml
[dependencies]
qec-code = "0.3.0"
```

Enable the open-source exact ILP backend only when it is needed:

```toml
qec-code = { version = "0.3.0", features = ["distance-ilp-highs"] }
```

The default build is a library-only build with no command-line parser or native
solver dependency. Available opt-in features are:

- `cli` exposes `qec_code::cli` and builds the `qec-code` binary;
- `distance-ilp-highs` enables exact distance with HiGHS; and
- `distance-ilp-gurobi` enables exact distance with Gurobi without also pulling
  in HiGHS.

Applications that previously invoked the package binary or imported
`qec_code::cli` must add `features = ["cli"]`. Features compose, so a CLI using
HiGHS can enable both `cli` and `distance-ilp-highs`.

This crate is covered by the repository-wide
[Apache-2.0 license](LICENSE), which is also declared in the workspace
package metadata. Enable the distance features above only when needed.

## Deterministic regular matrices

`qec_code::regular_classical` provides the versioned pure-Rust generator for
regular binary parity-check matrices used by random code families. The exact
version-1 stream, bounded-index rule, retry behavior, and seed-7 fixture are
documented in [`doc/regular_classical.md`](doc/regular_classical.md).

## Packed GF(2) example

```rust
use qec_code::packed_gf2::{PackedRow, ReducedRowSpace};

let generators = vec![vec![1, 1, 0], vec![0, 1, 1]];
let space = ReducedRowSpace::from_dense_rows(&generators, 3)?;
let target = PackedRow::from_dense(&[1, 0, 1])?;
assert!(space.contains(&target)?);
# Ok::<(), qec_code::QecError>(())
```

The public wrappers intentionally hide the packed storage layout so internal
elimination code can evolve without breaking downstream crates.
