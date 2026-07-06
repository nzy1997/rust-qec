# Issue 367 QEC-Code And Future Benchmark Status Design

Issue: #367

## Context

The static documentation site already has a benchmark methodology section, a
manifest copied from `site/benchmark-site.json` into `_site/data/`, and a Rust
site contract test for the manifest-backed claims policy. The qec-code
random-window benchmark pipeline has committed source docs, manifests, tests,
and Make targets, but its generated outputs are ignored under
`benchmarks/out/`. The site must expose the family without presenting those
ignored outputs as checked benchmark artifacts.

## Automatic Brainstorming Choices

No visual companion is needed because the problem is classification and contract
coverage, not layout exploration.

The issue body is specific enough to avoid interactive clarification. The
accepted scope is: add site-facing content for qec-code random-window and future
`rstim` versus Stim simulator benchmarks, keep qec-code status local-only or
partial, keep future simulator status future, and add a focused negative-control
contract test.

## Approaches Considered

Recommended: extend the existing manifest and static site sections, then lock
the classifications with a focused `rstim/tests/site_contract.rs` test. This
matches the site architecture added by issues #361, #363, and #365, keeps data
in the manifest, and avoids copying generated benchmark outputs.

Alternative: add prose-only cards in `site/index.html` without changing the
manifest. This would be simpler, but it would bypass the manifest-backed claims
policy and make future regressions harder to catch.

Alternative: promote generated qec-code benchmark output into checked artifacts.
This is out of scope for #367 because the issue explicitly says
`benchmarks/out/` output remains untracked unless a separate policy issue
changes that.

## Design

The site will present qec-code random-window as local-only evidence. Its section
will link the existing showcase and README, list all relevant local Make
targets, and state that generated output remains under the ignored
`benchmarks/out/qec_code_random_window/` tree. The manifest family may remain
`local-only`; any qec-code evidence item must be `local-only` or `partial` and
must not list checked artifacts unless those artifact paths are tracked and
non-ignored.

The site will present `rstim` versus Stim simulator benchmarks as future work.
The manifest family and evidence item will remain `future`, with no checked
artifacts and no current-result language. The static site will name likely
future comparison areas only as planning scope: sampling, detection, DEM
extraction, conversion, repeat-heavy circuits, and memory footprint.

The manifest checker already rejects checked artifacts that are ignored or
untracked, including entries under `benchmarks/out/`. The new Rust contract test
will add a site-level negative control for qec-code and future simulator
classifications: it will fail if qec-code is presented as an existing checked
full artifact without tracked artifacts, if any checked artifact points under
`benchmarks/out/`, or if the simulator family is presented as anything other
than future.

## Testing

The implementation will start with the focused Rust site contract test. The red
test will require qec-code/future copy and manifest classifications that are not
fully present yet. The production changes will then update `site/index.html` and
`site/benchmark-site.json` to satisfy the contract without touching generated
benchmark output.

Verification will run the commands from #367:

- `make build-site`
- `python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q`
- `python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json`
- `cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q`

The broader applicable gate is `cargo test`.
