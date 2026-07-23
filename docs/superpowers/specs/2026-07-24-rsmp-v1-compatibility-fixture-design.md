# rsmp v1 Compatibility Fixture Design

Date: 2026-07-24
Status: approved by Agent Desk standing answer policy for issue #532

## Need

`rsmp v1` readers need one immutable, reviewed archive that proves future reader
changes still accept bytes produced by the already-approved v1 format. A current
writer round trip is not enough, because writer output may change with writer
implementation details or Zstandard versions. The fixture is therefore a
reader-compatibility specimen, not a promise that future writers reproduce the
same bytes.

## Approaches Considered

The selected approach is a committed two-block fixture plus a manifest-driven
reader test. It keeps every source and expected output under
`rstim/tests/fixtures/rsmp/v1/`, pins all hashes in `manifest.toml`, references
the archive and manifest from the shared catalog, and makes the compatibility
test read the archive directly through `SampleArchiveReader`.

A writer round-trip test was rejected because it would only prove the current
writer and reader agree. A generated corruption-corpus base was rejected for
this issue because #530 owns generated mutations and needs a reviewed valid
archive as input.

## Fixture

The fixture ID is `compat_v1_two_block_sparse_dense`. It contains the exact
circuit, four measurement shots, the exact measurement `b8` bytes consumed by
initial generation, the immutable `compat-v1.rsmp` archive, expected detector
and observable `01` outputs, a TOML manifest, and an append-only README.

The archive uses `block_shots = 2`, so it has two blocks. The first block stores
zero selected-detector values and must select the sparse syndrome codec. The
second block stores all nine selected-detector values and must select the dense
syndrome codec. The free-coordinate stream is dense in both blocks and is not
byte-aligned, which exercises zero final padding.

## Compatibility Test

`rstim/tests/rsmp_v1_compatibility.rs` owns the reader-only compatibility path.
It parses `manifest.toml`, recomputes source-file, canonical-circuit,
measurement, expected-output, archive-file, internal trailer, and block logical
payload hashes, validates v1 format and transform identifiers, checks block
order and codec IDs, and decodes the committed archive with `SampleArchiveReader`.
Decoded measurements, detector events, and observable flips are compared
directly as `BitTable` values against the committed expected data.

The test must not construct `SampleArchiveWriter` or regenerate archive bytes.
Negative controls copy fixtures into temporary directories, mutate only the
temporary copies, and verify the same checker rejects a changed archive payload
byte and a changed expected-measurement hash.

## Catalog And Policy

`rstim/tests/fixtures/rsmp/catalog.json` gains one additive case that references
the compatibility manifest and archive by repo-relative path and pins their
SHA-256 values without duplicating archive bytes. The fixture README records
that existing compatibility fixtures are append-only after merge: reader or
writer changes must not replace `compat-v1.rsmp` or relax hashes to make a
failure pass. New coverage gets a new fixture ID and new files.

## Verification

The focused verification command is:

```console
cargo test --locked -p rstim --test rsmp_v1_compatibility -- --nocapture
```

It must print exactly one line:

```text
PASS rsmp v1 compatibility fixtures=1 blocks=2 codecs=sparse,dense
```

The branch also keeps the existing catalog checker valid and runs the full
repository Rust gate with `cargo test`.
