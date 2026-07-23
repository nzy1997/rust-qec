# rsmp v1 Compatibility Fixture

`compat-v1.rsmp` is an immutable reader-compatibility specimen for `rsmp v1`.
It was generated once from the committed `compat.stim` circuit and
`compat-measurements.b8` input with `block_shots = 2`.

The fixture covers a nonzero noiseless reference measurement, nine selected
detector coordinates, one free measurement coordinate, one observable, sparse
then dense syndrome block selection, two-block sequencing, trailer totals, and
non-byte-aligned syndrome and free-stream padding.

## Source Vectors

Measurement shots:

```text
1000000000
0111111111
1101010101
0010101010
```

Expected detector shots:

```text
000000000
000000000
111111111
111111111
```

Expected observable-flip shots:

```text
0
1
1
0
```

The manifest pins source-file, canonical-circuit, archive-file,
whole-archive, block logical-payload, measurement, and expected-output hashes.
Those hashes are part of the fixture contract.

## Append-Only Policy

After this fixture is merged, current-writer changes must not regenerate or
replace `compat-v1.rsmp`. Its archive SHA-256 and expected decoded hashes must
not be updated merely to make a failing reader test pass.

Additional coverage is added as a new fixture with a new ID and new files. An
existing fixture may be removed or declared invalid only through a separately
reviewed format-contract change explaining why the bytes were never valid
`rsmp v1`.

Documentation-only provenance corrections must not alter archive bytes or hide
a hash mismatch. If a manifest correction is necessary, reviewers must verify
that `compat-v1.rsmp` and the expected decoded-output files are unchanged.
