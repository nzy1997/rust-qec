# Native release archives

The `Native release archives` workflow builds and verifies `rustqec` and `rstim`
archives for these platforms:

| Target | Runtime baseline |
| --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu 24.04 x86_64, glibc and standard system libraries |
| `aarch64-apple-darwin` | macOS 15 on Apple silicon, with system libraries supplied by macOS |

A pull request that changes the workflow runs a staging build automatically. It
uses the pull request tooling and an ephemeral tag for the pull request head. The
artifacts remain private to the Actions run.

To stage an existing immutable tag without uploading assets, run:

```sh
gh workflow run native-archives.yml --ref master -f tag=v0.2.1 -f publish=false
```

To build, verify, and attach new assets after reviewing the staging evidence, run:

```sh
gh workflow run native-archives.yml --ref master -f tag=v0.2.1 -f publish=true
```

`--ref` selects the reviewed workflow and release-tooling version. The `tag`
input independently selects the source commit used for the locked native builds
and the rebuilt embedded Shot Lab assets. A production run accepts only an
annotated tag that passes the release gate, and it checks that the gate's peeled
commit is the commit packaged in the manifest.

Each release publishes two `.tar.gz` archives, `SHA256SUMS`,
`release-manifest.json`, and `verify_release_archive.py`. Download the verifier
for a tag from:

```text
https://github.com/nzy1997/rust-qec/releases/download/v0.2.1/verify_release_archive.py
```

Run it beside the downloaded manifest and checksums before using an archive:

```sh
python3 verify_release_archive.py \
  --archive rustqec-v0.2.1-x86_64-unknown-linux-gnu.tar.gz \
  --checksums SHA256SUMS \
  --manifest release-manifest.json \
  --expected-tag v0.2.1
```

Publication first checks the names of existing release assets and stops if any
requested name already exists. It never uses an overwrite option and leaves the
body and notes of an existing GitHub Release unchanged.
