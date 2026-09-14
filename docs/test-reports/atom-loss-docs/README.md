# Atom-loss documentation and version navigation

Local Chromium captures of the built site at 1440 × 1000 (desktop) and
390 × 844 (mobile), taken on 2026-09-11. Full-page captures show the homepage's
single atom-loss destination and the complete dedicated walkthrough.

- [Homepage](home.png)
- [Atom-loss guide](guide.png)
- [Atom-loss guide on mobile](guide-mobile.png)
- [Benchmark results with analytic noise controls, real-circuit oracle and timing breakdown](results.png)

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

The results capture shows the final d = 3 / d = 5 logarithmic loss-sweep
figure. Zero-failure points are omitted from this display; each panel uses
its visible x range with equal fractional margins. The full d = 3, 5, 7
measurements also appear in an expandable full-sweep figure with exact one-sided
95% upper limits at zero-event points. Raw data remain in `site/static/data/atom-loss/`, alongside sampling
throughput, accuracy/time, correctness reports, and reproducibility metadata.

The updated accuracy/time figure includes both fixed-weight batch and loop APIs.
All 16 original corpora and backend prediction hashes are unchanged. The decoder
oracle now checks 1,088 rows and rejects an actual ignore-conditioning mutation.

The results-section capture hides the sticky navigation during capture so it
does not overlay the long section; the mobile check uses the normal navigation.
The latest capture includes analytic channel-deletion controls and a finite real
Mid-SWAP chain check covering matching and MLE. A supplementary Python phase
chart separates graph construction from decode_batch calls. The sampling and
accuracy/time SVGs state their comparison limits directly inside the figure.

The results capture was refreshed on 2026-09-13 after bulk PyMatching graph
construction and rotated-order retiming. Sparse topology preparation is included
in each run. The downloadable 1.5 MB shot archive contains all 16 corpora and
150 predictions; the standalone standard-library rescorer verifies every result.
Regression controls reject resealed native timing corruption, missing required
checksums/provenance, changed source snapshots, and altered or omitted predictions.

The 2026-09-13 channel-coverage revision adds Bell Pauli-component probes,
X/Y/Z joint/marginal checks, both loss directions and actual wrong-channel
mutations. The original IX-only reproduction now fails the overall report.
The main sampling figure shows absolute Rust throughput; reference cost is a
separate supplement. All 15 timing settings appear in a new workflow figure,
with 135 timing/cache records downloadable as CSV. Browser tests exercise both
new figures and the distribution-probe report. Timing records and the shot
archive are unchanged.

The low-probability revision adds 15 primitive-rate controls (including two-qubit
half rates down to 0.00005) and a separate 65,536-shot blinded-export comparison.
Pauli-only, loss-only and combined low-probability deletions must fail both
analytic and real-circuit checks. The screenshot includes this scope, while
performance figures and benchmark sample sizes remain unchanged. Summary CSV
regressions cover every field, headers and missing/duplicate/extra rows.

The answer/manifest revision independently derives every scoring key from the
measurement observable and input mask. The download's standalone checker now
validates row formats, lengths, padding, metadata and this scoring relation.
Correctness reports retain histogram counts; a standard-library verifier
recomputes reported statistics and decisions. The updated figures label pLoss
as a configuration parameter and document the two-qubit target half rate.
Validation: 28 benchmark tests, eight site checks, and both Chromium/Firefox
browser cases passed. Original corpus, predictions and timing records are
unchanged; only the archive's checker and index were updated.

## 2026-09-14: origin, policy and independent-seed checks

The current screenshot includes the native offline batch comparator and a
separate paired-accuracy figure for three predeclared new seeds. Both offline
adapters include prediction write/flush; native streaming is labelled as a
separate policy. Timing values were freshly measured on all original corpora.
All 150 existing prediction hashes stayed unchanged; the original-corpus archive
now includes 48 additional offline-native predictions (198 total).

The independent Python checker reconstructs seeded masks, including per-batch
shuffling. A coherently rewritten mask/answer pair that forces perfect decoder
accuracy is rejected. Explicit evidence exceptions remain active under Python
optimization. A new archive contains 48 independent-seed corpora and 147
predictions, with per-seed results and conservative paired difference intervals.
All 64 original/new corpora were regenerated and their public shots, masks and
answers matched byte for byte.

Validation: 33 benchmark tests passed; normal and `python -O` artifact checks
passed; both archives rescore under `python -I -S`; all eight site checks and
Chromium/Firefox browser tests passed. Mobile overflow on the new seed figure
was fixed using the existing responsive figure styles. The long results capture
hides sticky navigation only while taking the screenshot; mobile tests keep it.

Commands and logs used locally:

- `python -m unittest benchmarks.atom_loss.test_reference` — `drafts/round8-tests.log`.
- `python -m benchmarks.atom_loss.remeasure ...` — `drafts/round8-remeasure.log`.
- `python -m benchmarks.atom_loss.replay` (both archives) — `drafts/round8-replay.log`, `drafts/round8-seed-replay.log`.
- `python -m benchmarks.atom_loss.verify` and `python -O -m benchmarks.atom_loss.verify`.
- `make -o build-shot-viewer build-site` and `python3 tools/check_site_build.py _site`.
- `npx playwright test tests/atom-loss-evidence.spec.js --workers 2` — both browser projects.

## 2026-09-14: clean source binding and current-decoder replay

The complete benchmark was regenerated from clean source commit
`b61d92575d10686fcc8ad298329e3bd266876a3b` in an isolated detached worktree.
The new manifest inventories all 740 production/harness/build inputs and all
five freshly built binaries. Sampling, correctness, all original timing cases
and all 48 independent-seed cases now refer to this same clean source.
All 345 predictions and all 64 public sample/mask/answer sets match the previous
release of the experiment byte for byte; timing values are freshly measured.

The artifact commit follows the source commit. CI verifies equal source/build
input trees rather than requiring an artifact to contain its own commit hash.
Keep the measured source commit in history when merging this PR (use a merge
commit, not a squash/rebase that removes its ancestor relationship). Any later
change to the bound inputs requires new evidence; a successful prediction
replay alone does not validate historical timing.

Current-decoder replay covers 213 backend/corpus combinations and compares all
345 archived prediction files, scores and recorded paired discordances. The
negative control executes the real native decoder behind a wrapper that flips
one prediction, leaving the archive untouched; replay rejects it. Other controls
reject changed decoder/build inputs, omitted inventory entries, dirty or newly
added inputs, and optimized-Python bypass attempts.

The page exposes the source/build manifest alongside the provenance downloads.
Fixed-weight PyMatching is explicitly a weight-conditioning ablation, retaining
the shared canonical syndromes and graph. Sampling remains absolute Rust
throughput, with unoptimized reference cost confined to the supplement.

Local reproduction and validation logs:

- `python -m benchmarks.atom_loss.evidence_run ...` — `drafts/round9-generation.log`.
- `python -m benchmarks.atom_loss.decoder_replay --root drafts/round9-results` — `drafts/round9-decoder-replay.log`.
- `python -m unittest benchmarks.atom_loss.test_reference benchmarks.atom_loss.test_source_contract` — `drafts/round9-tests.log`.
- Both sample replay commands — `drafts/round9-sample-replay.log`.
- Ordinary and optimized verifier, then site and Chromium/Firefox checks.

Validation completed locally: 39 reference/source/replay regression tests, all
64 sample regenerations, all 213 current-decoder combinations, normal and
optimized bundle verification, both standalone archives under `python -I -S`,
eight site checks and both Chromium/Firefox cases passed. Desktop/mobile
captures were refreshed; the mobile viewport has no horizontal overflow.
These are local results; hosted CI must run on the pushed artifact commit.
