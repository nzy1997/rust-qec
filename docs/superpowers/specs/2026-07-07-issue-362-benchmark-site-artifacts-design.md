# Issue 362 Benchmark Site Artifact Publishing Design

Issue: #362 Publish benchmark site data and checked artifacts through make build-site

Date: 2026-07-07

## Context

The repository already has a plain static site build target. `make build-site`
refreshes `_site/`, copies the QP101 browser resources, and generates the QP101
gallery. Issue #361 added `site/benchmark-site.json` plus
`tools/check_site_manifest.py`, which validates checked artifact paths against
git tracking and ignored-output policy. Issue #363 preserved the QP101 browser
inside the broader documentation shell.

The missing piece is publishing the checked benchmark manifest and checked
artifacts into `_site/` during the same build, without treating ignored local
benchmark outputs such as `benchmarks/out/` as checked evidence.

## Automatic Answers

This Agent Desk run is non-interactive, so the standing answer policy resolves
the normal Superpowers gates:

- No visual companion is needed because this is a static-build and manifest
  wiring change.
- The design is approved from issue #362, merged issue #361, merged issue #363,
  and local repository state.
- Use a small Python copy helper rather than a static site framework.
- Extend `tools/check_site_manifest.py` so source-manifest validation, copied
  site validation, and the copy helper share one set of manifest rules.
- Preserve benchmark artifact paths under `_site/benchmarks/...`, and copy the
  manifest to `_site/data/benchmark-site.json`.
- Do not regenerate benchmark campaigns and do not copy ignored local-only
  outputs.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.

## Approaches Considered

1. Add a copy helper that imports the validator, copies the manifest and checked
   artifacts, and then asks the validator to check copied site paths. This is
   the chosen approach because it keeps `make build-site` plain, reuses #361's
   checked-artifact rules, and keeps copy behavior focused.
2. Put all copy behavior directly in the Makefile with `cp` commands for each
   artifact. This is simple for today's artifacts but would duplicate manifest
   knowledge and drift as `site/benchmark-site.json` changes.
3. Replace the build with a static site framework. This is out of scope and
   would add dependency and layout churn for a path-copying requirement.

## Design

Add `tools/copy_site_benchmark_data.py`. Its CLI accepts
`--repo-root`, `--site-root`, and the source manifest path. It validates the
source manifest first using `tools.check_site_manifest.validate_manifest`. If
validation fails, it prints those errors and exits nonzero before copying
anything. On success it copies:

- `site/benchmark-site.json` to `_site/data/benchmark-site.json`.
- Every checked artifact path listed in the manifest to `_site/<artifact path>`,
  preserving the repository-relative path under `_site/`.

The helper then calls copied-site validation and exits nonzero if any copied
site path is missing. Local-only and future evidence items are not copied
because they have empty artifact lists and the validator rejects checked
artifacts under ignored paths before the helper copies them.

Extend `tools/check_site_manifest.py` with:

- `iter_checked_artifact_paths(manifest)`, yielding valid checked artifact
  source paths from an already-loaded manifest.
- Optional copied-site validation that checks `_site/<artifact path>` exists for
  every checked artifact when `--site-root` is supplied.
- CLI support for:

```sh
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
```

`--site-root` does not replace source validation. The copied manifest still
uses the same source path rules, so ignored or untracked checked artifacts are
rejected even when the manifest lives under `_site/data/`.

Update `make build-site` to run the copy helper after the QP101 static resources
and gallery are produced. The existing QP101 resource copies remain unchanged.

## Error Handling

The copy helper fails before copying if source manifest validation fails. If a
copy operation fails, the Python exception propagates as a nonzero command and
the site build fails. Copied-site validation accumulates errors in the same
style as the existing validator, for example
`site: checked artifact benchmarks/.../results.csv was not copied to _site`.

## Testing

Use TDD around the new behavior:

1. Add unit coverage that source-manifest validation with `site_root` rejects a
   missing copied checked artifact.
2. Add unit coverage that the copy helper writes `data/benchmark-site.json`,
   preserves checked artifact paths under `benchmarks/...`, and does not copy an
   ignored `benchmarks/out/` file that is only present in the fixture.
3. Keep the #361 negative controls for a missing checked artifact path and a
   checked artifact path under `benchmarks/out/`.
4. Run the issue verification commands:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
```

Required repository verification also includes:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
cargo test
```

Out of scope: redesigning the page layout, changing benchmark results,
generating fresh benchmark runs, or publishing ignored local-only benchmark
outputs.

## Self Review

- Placeholder scan: no unresolved markers or incomplete sections.
- Consistency check: the chosen helper calls the same validator before and after
  copy, matching the issue's no-drift requirement.
- Scope check: the design only touches the Makefile and the small Python site
  manifest tooling.
- Ambiguity check: output paths are explicit:
  `_site/data/benchmark-site.json` and `_site/<manifest artifact path>`.
