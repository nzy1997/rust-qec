# Issue 216 Rstim Render SVG Atom Loss Showcase Design

Issue: #216

## Context

The repository already documents and tests `rstim render_svg` in
`rstim/doc/cli.md` and `rstim/tests/cli_render_svg.rs`. The showcase framework
from #211 and the uncertain-claim policy from #220 are merged, so this issue can
add one individual showcase page under `docs/showcases/` without changing the
checker contract.

Dependencies #211 and #220 are complete. There is no existing PR for #216.

## Approaches Considered

1. Add one concise showcase page that uses inline and committed Stim examples,
   references existing generated-output markers, and points verification to the
   checker plus focused render tests. This is the chosen approach because it
   satisfies the issue while avoiding duplicate generated SVG artifacts.
2. Commit rendered SVG output files alongside the page. This would make the
   page more visual in the repository tree, but it adds generated artifacts and
   extra churn when renderer layout changes.
3. Expand the showcase into a broader QP101/Typst migration guide. This is out
   of scope because the issue explicitly says not to migrate the Pages gallery
   or replace Typst.

## Design

Create `docs/showcases/rstim-render-svg-atom-loss.md` as an individual showcase
page using the required sections: `What This Shows`, `Run It`,
`Expected Result`, `Code`, `Verification`, and `Limits`.

The page will cover three user-facing workflows:

- plain `rstim render_svg` from a small inline circuit,
- seeded sample-shot rendering with `--sample_shot --seed 7` using
  `qp101-viz/examples/atom-loss-sample.stim`,
- DEM-origin highlighting with `--highlight_dem_error 0` on a compact inline
  noisy circuit.

The expected-result text will name stable markers already asserted by
`rstim/tests/cli_render_svg.rs`, such as `<svg`, `q0`, `H`, `M`, `>LOSS</text>`,
`marker: X`, `marker: L`, `marker: D0`, and
`data-annotation-tags="dem-origin query-result"`. It will describe generated SVG
paths as local outputs rather than committing those outputs.

## Links

The showcase page will link to:

- `rstim/doc/cli.md`
- `rstim/tests/cli_render_svg.rs`
- `rstim/tests/qp101_svg.rs`
- `qp101-viz/examples/atom-loss-sample.stim`

`qp101-viz` will appear only as optional legacy/prototype context for users who
need the older Typst fixture path.

## Limits

The `Limits` section must stay concrete. It will state that the showcase covers
the supported plain SVG path, the supported sample-shot annotation path, and the
supported single-DEM-error highlight path. It will also say that `--seed` is
only valid with `--sample_shot`, that `--sample_shot` and
`--highlight_dem_error` are mutually exclusive, and that this page does not
promise a full gallery migration or full Typst replacement.

## Verification

Run these commands:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-render-svg-atom-loss.md
cargo test -p rstim --test cli_render_svg --test qp101_svg -q
```

The focused Rust tests already include the required negative control:
`rstim render_svg --seed 7` without `--sample_shot` fails and preserves the
existing output file.

## Approval

Automatic approval under the Agent Desk standing policy: choose the conservative
page-only design because it is narrow, reversible, and follows the merged
showcase/checker conventions.
