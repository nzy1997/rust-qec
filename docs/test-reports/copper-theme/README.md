# Copper theme and homepage workflow

The site now extends the approved 01C logo palette to its navigation, links,
buttons, documentation surfaces, and Shot Lab controls. Scientific diagram
colors and success/error states retain their meanings. The README's stacked
logo is 200px wide and retains its light/dark variants.

The homepage uses a text-only introduction and presents four tasks: Define
codes, Construct circuits, Simulate circuits, and Decode & benchmark. Circuit
diagrams and Shot Lab are grouped under circuit construction. On desktop the
subtitle uses the available width; small screens wrap naturally.

## Visual evidence

Screenshots captured from the built site using Chromium at 1440×1000, or
390×1000 for mobile:

- [Homepage](home.png)
- [Mobile homepage](home-mobile.png)
- [Get started](get-started.png)
- [Shot Lab](shot-lab.png)

## Verification

- `make -o build-shot-viewer build-site` — builds the site using the existing,
  unchanged WebAssembly bundle. Updated viewer CSS is synchronized to the
  embedded asset tree and its manifest.
- `python3 tools/check_site_build.py _site` — 8 checks passed.
- `python3 tools/shot_viewer_assets.py` — asset manifest verified.
- `npm --prefix web/shot-viewer run test:e2e -- --workers=3` — 96 passed across Chromium and Firefox.
- `cargo test --locked -p rstim --no-default-features --test site_contract --test workspace_brand` — 15 passed.
- `git diff --check`

Measured contrast ratios: white primary-button text 5.60:1, copper links on
page background 7.52:1, secondary text on muted surfaces 5.28:1, and active
navigation text 6.54:1. Desktop subtitle is one line at 1280px and 1440px;
390px mobile layout has no page-level horizontal overflow.
