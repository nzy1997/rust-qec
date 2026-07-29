# qec-code

`qec-code` provides Rust data structures and algorithms for constructing and
analyzing quantum error-correcting codes. Its dependency-facing APIs include:

- CSS matrix validation and construction;
- exact distance through optional HiGHS or Gurobi ILP backends;
- randomized CSS distance upper bounds; and
- packed GF(2) row operations, reduced row spaces, and reusable kernel
  workspaces under `qec_code::packed_gf2`.

## Dependency status

This crate is currently developed inside the RustQEC workspace and is not
published independently. A sibling local checkout can use a path dependency:

```toml
[dependencies]
qec-code = { path = "../rust-qec/qec-code" }
```

Downstream Git-based prototypes should pin an exact repository revision instead
of following a moving branch:

```toml
[dependencies]
qec-code = { git = "https://github.com/nzy1997/rust-qec.git", rev = "<reviewed-commit>" }
```

Enable the open-source exact ILP backend only when it is needed:

```toml
qec-code = { git = "https://github.com/nzy1997/rust-qec.git", rev = "<reviewed-commit>", features = ["distance-ilp-highs"] }
```

This crate is covered by the repository-wide
[Apache-2.0 license](../LICENSE), which is also declared in the workspace
package metadata. The crate is not yet published independently; downstream
users should continue to pin a reviewed repository revision.

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
