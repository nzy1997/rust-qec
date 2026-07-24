# Issue 531 rsmp CLI Publication Design

Date: 2026-07-24
Status: approved by non-interactive Agent Desk standing policy

## Need

`pack_samples` and `unpack_samples` already stage file outputs in sibling
temporary files, but the publication logic is still local to each target and
does not fully enforce the v1 CLI path contract. `unpack_samples` also lacks
the no-output verification mode needed by reviewers and corruption checks.

## Selected approach

Use a single CLI preflight pass to validate stream usage, parse result formats,
capture the current directory once, lexically normalize every non-`-` path, and
compare normalized file inputs against final output paths before opening any
input or creating any temporary file.

Keep all file output writes behind sibling temporary files. After archive
writer or reader `finish()` succeeds, finish and close every staged writer,
then publish file outputs in command order: archive for pack, and measurements,
detectors, observables for unpack. Each rename is an independent atomic
publication. If a later rename fails, return `RSMP_IO`, keep already-published
files in place, remove only unpublished temporaries through RAII cleanup, and
name the already-published paths in the diagnostic.

Add `unpack_samples --verify_only` as a separate preflight mode. It accepts no
result-output options, creates no result targets, drains the archive through
`SampleArchiveReader::finish()`, and prints one stable success line from the
validated archive summary:

```text
PASS rsmp version=1.0 shots=<shots> blocks=<blocks> M=<M> D=<D> L=<L> circuit=<12-hex>
```

## Alternatives considered

1. Publish each file immediately after its writer finishes. This keeps less
   state, but it can expose partial output before archive `finish()` has
   validated the trailer, final counts, whole-archive digest, EOF, and trailing
   data.
2. Try to roll back earlier successful renames when a later rename fails. This
   falsely suggests filesystem-wide transaction semantics and can delete valid
   output that this invocation already published.
3. Use canonical filesystem paths for collision detection. This would detect
   more aliases on some systems, but it requires filesystem access before
   preflight completes and exceeds the v1 syntactic contract.

## Testing

Add `rstim/tests/cli_rsmp_publication.rs` as the focused issue gate. The test
drives the real `rstim` binary, uses the committed #532 compatibility fixture,
uses #530 named corruption recipes for archive failures, verifies no leaked
sibling temporary files, and injects a second-rename failure through a hidden
debug-build-only CLI environment hook.

The required focused command is:

```console
cargo test --locked -p rstim --test cli_rsmp_publication -- --nocapture
```

The required success line is:

```text
PASS rsmp CLI publication pack=1 unpack=1 duplicate_paths=1 normalized_paths=4 rename_failure=1 verify_only=1
```
