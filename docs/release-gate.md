# Release gate

GitHub Release creation is allowed only after one version tag resolves to one full
commit and that exact commit satisfies the repository's release policy. The gate is
read-only: it never creates, edits, deletes, or uploads a GitHub resource.

## Version policy

[`tools/release_version_policy.json`](../tools/release_version_policy.json)
classifies every Cargo workspace member. The repository release version tracks the
four crates already updated by `make release`:

- `rstim`
- `rsinter`
- `rbposd`
- `rmatching`

Their `[package].version` values and Cargo.lock entries must equal the version in the
tag. The other workspace crates are explicitly independent. Their manifest versions
must match Cargo.lock, but they do not have to equal the repository release version.
Adding a workspace member without classifying it makes the gate fail.

Only stable tags in canonical `vMAJOR.MINOR.PATCH` form are supported. Prerelease and
build-metadata tags are rejected before any GitHub lookup. Supporting prereleases in
the future requires an explicit policy and a matching change to the Release action;
they must never silently produce a non-prerelease Release.

## Exact commit and CI

The checker peels lightweight or nested annotated tags to a full 40-character commit
SHA. It then reads `Cargo.toml`, every package manifest, and `Cargo.lock` from that
commit through the GitHub API. It requires the same five always-applicable checks as
the default-branch gate, with their GitHub Actions app identity, and verifies that
their supplying workflow run is a successful `push` run of
`.github/workflows/ci.yml` for the exact tagged commit. A successful run for the
parent commit does not count.

For reviewer use, run:

```sh
python3 tools/check_release_gate.py \
  --repo nzy1997/rust-qec \
  --tag v0.2.1 \
  --dry-run
```

The existing `v0.2.1` tag is annotated and resolves to
`cfc935fc13e73469f413a08e08d0c19ecad0e42a`. Its exact CI run is
[33947294824](https://github.com/nzy1997/rust-qec/actions/runs/33947294824).
The command remains a safe negative check until the reviewed tag ruleset below is
active; no part of this work retags or edits the existing release.

The Release workflow uses the same command with a bounded wait because `make release`
pushes the release commit and tag together, so branch CI can still be running when
the tag workflow begins. On success it exports a stable interface for downstream
release jobs:

- `commit`: the full peeled tag commit
- `version`: stable SemVer without the `v` prefix
- `prerelease`: `false`

Future native archive jobs should depend on `release-gate`, check out or fetch source
at `needs.release-gate.outputs.commit`, finish their own clean-runtime verification,
and perform uploads only after both gates pass.

## Tag immutability rule

[`tools/release_tag_ruleset.json`](../tools/release_tag_ruleset.json) targets only
`refs/tags/v*.*.*` and restricts update and deletion. It does not restrict initial tag
creation. Repository administrators have the sole exceptional bypass, using
`bypass_mode: always` because pull-request bypass mode does not apply to tags.

After reviewing and merging the implementation, a repository administrator can
apply the ruleset exactly once:

```sh
gh api \
  --method POST \
  repos/nzy1997/rust-qec/rulesets \
  --input tools/release_tag_ruleset.json
```

Save the returned ruleset ID. Update an existing equivalent rule with `PUT
repos/nzy1997/rust-qec/rulesets/RULESET_ID` instead of creating a duplicate.

Run the offline negative controls with:

```sh
python3 -m unittest tools.test_check_release_gate
```
