# Envelope Decoder Compatibility Policy

This policy defines what the envelope-decoder release promise freezes, what may
evolve compatibly, and what requires a schema or release-line change. It is the
contract referenced by the release-readiness gate
(`tools/check_envelope_release.py`, issue #716) and applies to the source
revisions for which that gate passes. Per-decoder maturity (Beta vs Supported)
is decided by the gate; this document defines the surface each maturity level
promises to keep stable. The first published Supported promises are
`envelope-matching` in v0.3.1 and the finite `envelope-mle` scope in v0.3.2;
their frozen surfaces are the ones this policy defines. Each release's evidence
bundle binds this document by SHA-256.

Scope: the `rustqec` CLI envelope-decoder surface — `decode`, `capabilities`,
`circuit stats` — as exercised by `docs/envelope-support.json`. Anything not
listed here is not part of the frozen promise.

## Frozen CLI arguments

The following arguments, their names, value domains, and exit-code conventions
are frozen for the envelope decode path:

- `rustqec decode --decoder <name> --dataset <dir> --out <file> --stats-out <file>`
- `rustqec decode --shot-timeout-ms <ms>` — MLE solve path only; passing it to
  `envelope-matching` is rejected with `invalid_arguments`.
- `rustqec capabilities --format json` — must list every decoder the binary
  can execute under `commands[].decoders`; a decoder that cannot execute must
  not be advertised.
- `rustqec circuit stats --format json --in <file>`

Exit-code convention: `0` success; `2` argument/dataset/compile/output errors;
`3` per-shot decode failures (`decode_timeout`, `decode_infeasible`). Successful
runs write nothing to stdout. `--out` and `--stats-out` name files that must
not already exist: a pre-existing output file is an `output_error` (exit `2`)
and the pre-existing file is left byte-identical.

## Dataset interpretation

The frozen dataset format is `rstim_decoder_dataset` `schema_version` 1 in mode
`measurements_blinded`: `circuit.stim`, `shots.b8`, and `manifest.json`, where
the manifest binds the circuit and shots by SHA-256 and declares
`dataset_id` over the pinned material string. Only public records are inputs;
private loss answers are never consumed. Shots are measurement rows packed as
declared by the manifest `row` block (`b8`, `lsb_first`, `bytes_per_shot`);
a payload that is not a whole number of rows is rejected.

## Prediction packing

Prediction files (`--out`) contain exactly one row per input shot. Each row
holds one bit per logical observable declared by the circuit, packed `b8`
with `lsb_first` bit order and tail bits zero-padded to the byte boundary.
The byte length per row is `ceil(num_observables / 8)`. Rejection paths write
no prediction file; timeout/infeasible paths write no prediction file.

## Structured error codes

Errors are reported as a single JSON envelope on stderr with
`schema_version: rustqec.cli.v1`, `status: error`, and an `error.code` field.
The frozen codes and their artifact rules:

| code | exit | predictions | stats | meaning |
| --- | --- | --- | --- | --- |
| `invalid_arguments` | 2 | none | none | argument combination not supported (e.g. `--shot-timeout-ms` with matching) |
| `invalid_dataset` | 2 | none | none | dataset fails manifest/hash/shape validation |
| `unsupported_dataset_mode` | 2 | none | none | dataset mode is not `measurements_blinded` |
| `unsupported_circuit` | 2 | none | none | circuit outside the declared contract (e.g. MLE candidate explosion) |
| `decode_timeout` | 3 | none | written | per-shot solve exceeded `--shot-timeout-ms` |
| `decode_infeasible` | 3 | none | written | per-shot solve proved infeasible |
| `output_error` | 2 | none | none | output file already exists; pre-existing file preserved |

## Statistics semantics

Statistics files (`--stats-out`) use schema `rustqec.decode-stats.v1`. On the
timeout and infeasible paths the shot counters are *attempted* counts, not
completed counts: they record how many shots entered the solve path before the
run stopped, and must not be read as successful decodes. On success the
counters are completed counts. Timing and cache-counter fields are
informational: they are never part of semantic comparisons and may vary
between machines and revisions.

## Compatible evolution

The following changes are compatible and may ship within a release line
without a schema change:

- Adding optional fields to JSON outputs (stats, capabilities, error
  envelopes). Consumers must ignore unknown fields.
- Adding new controls to the support matrix, new evidence artifacts, and new
  recommended operating ranges derived from measured data.
- Adding a new decoder with its own maturity track.
- Adding new error codes for previously failing operations, provided the
  exit-code convention (`2` vs `3`) is preserved.
- Narrowing a Beta decoder's documented limitations (more precise, not weaker).

## Breaking changes

The following require a schema-version bump of the affected artifact and an
explicit note in the release line's changelog:

- Renaming, removing, or changing the meaning of any frozen CLI argument,
  error code, exit code, or artifact rule above.
- Changing the dataset format version, the `dataset_id` material string, or
  the prediction packing (bit order, padding, row layout).
- Changing stats semantics, including the attempted-vs-completed distinction.
- Promoting or demoting a decoder's maturity outside the release-readiness
  gate, or widening a Supported decoder's promised circuit domain without
  re-running the gate at the new candidate revision.

## Deprecation notices

Any planned breaking change is announced in the release notes and in this
document one release line before it takes effect. The notice must name the
frozen item, the replacement, and the release line in which the old behavior
stops being guaranteed. Deprecated items remain frozen until the announced
release line ships.
