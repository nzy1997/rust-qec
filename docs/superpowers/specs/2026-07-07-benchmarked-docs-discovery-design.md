# Benchmarked Documentation Discovery Design

## Context

Issue #369 asks for the broader benchmarked documentation site to be discoverable
from the repository front door while preserving the existing GitHub Pages build
path. The current static site already lives under `site/` and `make build-site`
publishes it to `_site/` with QP101 assets, benchmark manifest data, copied
checked artifacts, and gallery assets. The current README links showcase docs
and benchmark evidence, but it does not name the benchmarked documentation site
or tell readers how the Pages site is built and checked.

## Chosen Approach

Use a narrow discovery-and-contract update.

Alternative 1, the recommended approach, adds explicit links from `README.md`
and `docs/showcases/README.md` to the live GitHub Pages documentation site,
mentions the local `make build-site` and `tools/check_site_build.py _site`
contract, and adds focused Rust site-contract tests for README discovery and
Pages workflow wiring.

Alternative 2 would add generated badges or release artifacts. That would make
the site more prominent but adds moving parts that are unnecessary for this
issue.

Alternative 3 would introduce a separate frontend toolchain or deployment path.
That conflicts with the issue guidance to keep Pages focused on `make
build-site`.

## Design

`README.md` gets a first-page link to the live benchmarked documentation site at
`https://nzy1997.github.io/rstim/`. The copy makes clear that the site is the
broader documentation surface, not only the legacy QP101 browser, and it names
the local build path: `make build-site` followed by `python3
tools/check_site_build.py _site`.

`docs/showcases/README.md` gets matching discovery copy near the top of the
showcase index. It links readers from runnable Markdown workflows to the same
benchmarked documentation site, and it preserves the showcase index as the
source for category-specific runnable examples.

`.github/workflows/deploy-pages.yml` remains centered on `run: make build-site`.
The workflow may rename the build step to clarify that it builds the benchmarked
documentation site, but it must not add a package manager install or replace the
Makefile contract.

`Makefile` keeps `build-site` as the complete static-site entry point. The help
text should name the benchmarked documentation site, while the target continues
to copy QP101 resources, benchmark manifest data, checked benchmark artifacts,
and gallery assets into `_site/`.

`rstim/tests/site_contract.rs` gets two focused contracts:

- `readme_links_benchmarked_site` requires README and showcase-index discovery
  markers: the phrase "benchmarked documentation site", the live Pages URL,
  benchmark evidence, QP101 integration, `make build-site`, and `python3
  tools/check_site_build.py _site`.
- `pages_workflow_builds_benchmarked_site` requires the Pages workflow to invoke
  `make build-site`, upload `_site`, deploy through GitHub Pages actions, and
  keeps the Makefile target wired to benchmark manifest copying and checked
  artifact publication.

## Testing

The implementation should follow TDD by adding the two Rust tests first and
watching them fail against the current docs. After updating docs and the build
contract copy, run:

```sh
make build-site
python3 tools/check_showcase_docs.py --readme README.md
python3 tools/check_site_build.py _site
cargo test -p rstim --test site_contract readme_links_benchmarked_site -q
cargo test -p rstim --test site_contract pages_workflow_builds_benchmarked_site -q
cargo test
```

If cargo attempts network access in this restricted Agent Desk environment, use
offline Cargo verification for local evidence and record the exact network
failure separately.

## Scope Boundaries

No new frontend framework, JavaScript bundler, package-manager install, project
board wiring, or benchmark-data interpretation changes are part of this work.

## Self-Review

This spec has no placeholders. It is limited to repository discovery docs,
workflow/build contract copy, and focused contract tests. It does not require
changing site architecture or benchmark claims.
