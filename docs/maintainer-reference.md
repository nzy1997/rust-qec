# Maintainer reference

This document contains release gates, regression commands, and evidence
bookkeeping for RustQEC maintainers. User-facing compatibility promises live in
`docs/support-compatibility.md`.

## Atom-loss regression controls

Validate the finite MLE scope plan before running the decoder controls. The
plan covers only the four declared distance-3 loss/batch points; passing this
check does not promote the decoder or widen that grid:

```sh
python3 -m unittest tools.test_envelope_mle_scope
```

Build the ILP-capable CLI before running the complete envelope support matrix:

```sh
cargo build --release --locked -p rustqec-cli --features ilp
python3 tools/check_envelope_support.py \
  --binary target/release/rustqec \
  --matrix docs/envelope-support.json \
  --out drafts/envelope-readiness/support.json
```

The pinned Mid-SWAP MLE positive control must decode successfully:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures \
  current_rstim_atom_loss_midswap_envelope_mle_decodes_unmodified -- --exact
```

The conventional candidate-explosion fixture is outside the MLE support
boundary and must fail before publishing predictions or statistics:

```sh
cargo test --locked -p rustqec-cli --test external_fixtures \
  current_rstim_atom_loss_conventional_envelope_mle_rejects_candidate_explosion -- --exact
```

## Decoder release-readiness gate

Promotion is decided per decoder. The gate consumes the support matrix,
independent correctness report, measured resource envelope, and installed
artifact reports for `x86_64-unknown-linux-gnu` and
`aarch64-apple-darwin`:

```sh
python3 tools/check_envelope_release.py \
  --evidence-dir drafts/envelope-readiness \
  --matrix docs/envelope-support.json \
  --policy docs/envelope-compatibility-policy.md \
  --candidate-revision "$(git rev-parse HEAD)" \
  --out drafts/envelope-readiness/release-gate.json
```

Produce each installed-artifact report from the extracted release archive,
never from a binary found on `PATH`:

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

A missing decoder, platform report, control, correctness check, or matching
revision fails the gate. Retained resource measurements are reusable only when
their revision is an ancestor of the candidate and all measurement-relevant
sources remain identical.

## Publication evidence bundle

Every native release that promotes a decoder publishes a checksummed
`envelope-support-evidence-<tag>.tar.gz` containing the gate report, matrix,
policy, correctness and resource evidence, and archive-bound platform reports.
Verify a downloaded release directory with:

```sh
python3 tools/check_envelope_publication.py \
  --release-dir <dir> \
  --expect-decoder <name>
```

Releases without that bundle, including v0.3.0, do not carry a verified
envelope promotion.

## Publication gaps and tracked exclusions

Passing a structural evidence checker is not the same as
`publication_ready=true`. The following work remains outside the published
claims:

- [#601](https://github.com/nzy1997/rust-qec/issues/601): publication-grade,
  multi-platform benchmark evidence.
- [#209](https://github.com/nzy1997/rust-qec/issues/209): BB circuit BP-OSD
  runtime gap investigation.
- [#550](https://github.com/nzy1997/rust-qec/issues/550): high-rate classical
  APM smoke-case runtime investigation.

## Release checklist

1. Confirm the candidate versions in every public crate manifest.
2. Run the support matrix and independent correctness checks.
3. Produce both native archive reports from the candidate artifacts.
4. Run the release-readiness gate at the candidate revision.
5. Publish and checksum the evidence bundle with the release.
6. Verify the published bundle and update the user-facing support contract.
