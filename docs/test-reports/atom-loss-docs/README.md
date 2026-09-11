# Atom-loss documentation and version navigation

Local Chromium captures of the built site at 1440 × 1000 (desktop) and
390 × 844 (mobile), taken on 2026-09-11. Full-page captures show the homepage's
single atom-loss destination and the complete dedicated walkthrough.

- [Homepage](home.png)
- [Atom-loss guide](guide.png)
- [Atom-loss guide on mobile](guide-mobile.png)

The tutorial command test builds the workspace CLI and runs the exact four
command blocks from the page in a temporary directory. The seeded example
produces 64 decoded shots, 10 loss patterns, and 0/64 logical errors. Negative
controls reject truncated predictions and tampered public dataset files.
This small example establishes workflow continuity, not a performance estimate.

Browser checks cover homepage navigation, the dedicated page, mobile overflow,
search destinations, version switching with fixture editions, and favicon assets.
The only published edition configured by this change is Development · master.
Freezing a v0.3.0 documentation edition remains deferred in issue #711.

## Reproduce

```sh
make build-site
python3 tools/check_site_build.py _site
python3 -m unittest tools.test_atom_loss_guide tools.test_site_versions
npm --prefix web/shot-viewer run test:e2e
```
