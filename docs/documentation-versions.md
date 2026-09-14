# Documentation editions

The public site currently has one edition: **Development · master**, at the
existing `/rust-qec/` root. No stable documentation edition is published. Package
releases and documentation editions are separate: installation commands, API
references, download URLs, and reproducible results retain their package versions.

## Configuration and builds

`site/versions.json` is the publication registry. The default entry occupies the
root. Additional editions require unique subdirectories and full commit SHAs,
so rebuilding the site does not silently move a released edition to newer code.

`make build-site` builds the current checkout into `_site`, including its version
label and a catalog advertising only that snapshot. For development previews this
is all that is needed. Serve `_site` using the normal local HTTP server.

The Pages workflow then runs:

```sh
python3 tools/site_versions.py build --site-root _site --output _pages
```

The output directory must not already exist. The command uses the existing build
for the default edition and builds other registered commits in temporary checkouts.
Each snapshot gets its own search index, diagrams, JavaScript, WebAssembly, and
base URL. Every additional build must pass `check_site_build.py` before assembly.
Pages uploads the combined `_pages` artifact in one deployment.

Each edition has a generated `versions.json` with relative edition roots, page
routes, and anchors. With one edition, the UI displays a plain version label.
With multiple editions, it displays a labeled native select. Switching preserves
the current page and query string when possible; a missing page falls back to the
target edition's homepage, and missing anchors are removed. If JavaScript or the
catalog request fails, the rendered current-edition label remains visible.

## Enabling a release edition later

1. Prepare a documentation snapshot against the intended release source. It must
   include this version infrastructure. If the release predates it, backport the
   documentation tooling and design to a documentation maintenance branch and
   validate every example against the release; do not relabel master as stable.
2. Commit the validated snapshot and record its full commit SHA. Add an entry to
   `site/versions.json`, for example an ID `v1.0.0`, label `Release · v1.0.0`, path
   `versions/v1.0.0`, and that SHA as `ref`. Keep master as the root/default entry.
3. Build and test the master site, then run the publication command above with a
   fresh output directory. The workflow fetches complete Git history so pinned
   commits are available. All snapshots need their own compatible build tools;
   update workflow tooling if a future snapshot requires different dependencies.
4. Serve `_pages` locally and check both directions of the version switch, search,
   downloads, and Shot Lab before publishing. Adding the registration activates
   the selector automatically; no placeholder release entry is needed today.

The assembler rejects missing snapshots, mismatched edition labels, overlapping
version directories, and directories that would overwrite existing root content.
To change which edition occupies the root later, plan redirects separately so
existing development URLs do not unexpectedly become release documentation.

## Verification

```sh
python3 -m unittest tools.test_site_versions
python3 tools/check_site_build.py _site
npm --prefix web/shot-viewer run test:e2e -- tests/docs-versions.spec.js
```

The Python tests assemble two distinct fixture snapshots and check asset/search
isolation and rejection cases. Browser tests exercise single-edition presentation,
catalog failure, and future switching with a fixture-only second edition.
