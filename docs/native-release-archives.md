# Native release archives

The `Native release archives` workflow builds and verifies `rustqec` and `rstim`
archives for these platforms:

| Target | Runtime baseline |
| --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu 24.04 x86_64, glibc and standard system libraries |
| `aarch64-apple-darwin` | macOS 15 on Apple silicon, with system libraries supplied by macOS |

A pull request that changes the workflow runs a staging build automatically. It
uses the pull request tooling and an ephemeral tag for the pull request head. The
artifacts are attached to the Actions staging run and are not release assets.

To stage an existing immutable tag without uploading assets, run:

```sh
gh workflow run native-archives.yml --ref v0.3.0 -f tag=v0.3.0 -f publish=false
```

To build, verify, and attach new assets after reviewing the staging evidence, run:

```sh
gh workflow run native-archives.yml --ref v0.3.0 -f tag=v0.3.0 -f publish=true
```

`--ref` selects the reviewed workflow and release-tooling version; using the
release tag pins both tooling and source for v0.3.0. For a later release, select
the exact tooling ref reviewed for that release. The `tag` input independently
selects the source commit used for the locked native builds and the rebuilt
embedded Shot Lab assets. A production run accepts only an
annotated tag that passes the release gate, and it checks that the gate's peeled
commit is the commit packaged in the manifest.

Each release publishes two `.tar.gz` archives, `SHA256SUMS`,
`release-manifest.json`, and `verify_release_archive.py`. Download the verifier
for a tag from:

```text
https://github.com/nzy1997/rust-qec/releases/download/v0.3.0/verify_release_archive.py
```

Run it beside the downloaded manifest and checksums before using an archive:

```sh
python3 verify_release_archive.py \
  --archive rustqec-v0.3.0-x86_64-unknown-linux-gnu.tar.gz \
  --checksums SHA256SUMS \
  --manifest release-manifest.json \
  --expected-tag v0.3.0
```

Publication first checks the names of existing release assets and stops if any
requested name already exists. It never uses an overwrite option and leaves the
body and notes of an existing GitHub Release unchanged.

## Envelope support evidence bundle

A release whose gate promotes an envelope decoder also publishes
`envelope-support-evidence-<tag>.tar.gz` and its `.sha256` sidecar. The bundle
is assembled from the same candidate run that built the archives and freezes
one version-bound record of the support claim:

- the passing per-decoder release-gate report;
- the exact support matrix and compatibility policy the gate consumed;
- the support, correctness, and resource evidence and their retained raw
  resource observations;
- one installed-envelope report per official target, bound to its published
  archive by filename and SHA-256 (machine-local build paths are redacted);
- a human-readable `SUMMARY.md` of decoder maturity, scope, version,
  platforms, and limitations.

Decoder maturity and scope inside the bundle are derived from the gate
decision, never edited by hand. The workflow rejects mismatched tags or
commits, archive-hash drift, missing platforms, failed evidence, and
promotion claims the gate did not earn before anything is uploaded.

Download every asset of a release into one directory and verify the published
support without rerunning any evidence campaign:

```sh
python3 tools/check_envelope_publication.py \
  --release-dir <download-dir> --expect-decoder envelope-matching
```

The command reconciles the tag, source commit, release manifest, both archive
identities, the per-platform installed reports, and the gate decision, then
prints the decoder scope and version followed by `PASS envelope published
support`. `--expect-decoder` may be repeated. Releases that predate the bundle
(v0.3.0 is the only one) contain no evidence asset: the verifier reports them
as lacking a verified envelope promotion rather than treating them as
Supported. `python3 tools/check_envelope_publication.py --self-test` proves
offline that swapped platform reports, removed platforms, unsupported scope
claims, and failed gates are all rejected.
