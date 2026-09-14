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

## Independent-agent audit and build-environment fixes

Three independent reviewers examined fairness/statistics, correctness coverage,
and evidence integrity. The first two found no unresolved material issue in
scope. The evidence reviewer reproduced two defects: unrecorded Cargo overrides
could produce an unoptimized release build, and false Pauli-noise metadata could
survive coherent resealing and replay.

Source commit `ce196f09e` closes both paths. Evidence generation now uses a fresh
Cargo home and an allowlisted build/measurement environment; external ancestor
configuration and ignored/untracked checkout configuration are rejected. The
regression builds an actual release executable with hostile parent profile
settings and requires optimization without debug assertions. Every corpus now
records Pauli probability 0.001, validators enforce it, and generator replay uses
the declared probability. Original attack scripts were independently rerun;
all identified bypasses were rejected before evidence regeneration.

All experiments were regenerated from this clean source. All 345 prediction
files and all 64 sets of public samples/masks/answers remain byte-identical;
timing values are freshly measured. Local logs are `drafts/round10-generation.log`,
`drafts/round10-tests.log`, and the round10 replay/site/browser logs. Independent
review reports and exact attack scripts are retained under
`drafts/agent-audit-fairness/`, `drafts/agent-audit-correctness/`, and
`drafts/agent-audit-evidence/`.

Post-fix local checks: 44 regression tests, all 213 current decoder/corpus
combinations covering 345 archived predictions, both verifier modes, eight site
checks, and Chromium/Firefox browser checks passed. Each initial agent finding
was retested with its original reproduction; final independent verdicts are
retained in the audit directories against the regenerated artifact commit.

## 2026-09-14: figures bound to validated data

Source commit `3624d7aa0f659bbd57d12f1b6d72726e32820a02` adds presentation
verification after the numerical and source checks. All eight SVGs are checked
against a deterministic redraw, and all eight PNGs must match its decoded pixels
and metadata after complete loading. PNG chunk CRCs are checked too.
The downloaded methodology must equal the source-bound README. Plotting uses
pinned dependencies, bundled DejaVu fonts and FreeType 2.6.1, reset styles,
fixed SVG IDs and no generated timestamps. macOS ARM and Linux x86-64 showed
one axis endpoint rounded apart by 0.000001 pt, while every PNG pixel matched.
The verifier therefore permits at most one serialized decimal unit of absolute
roundoff for anonymous absolute-path coordinates. Commands, separators, number
counts and all other SVG bytes remain exact; no relative tolerance, text/style/
transform relaxation, glyph-path tolerance or pixel tolerance is allowed.
Boundary controls accept 0.000001 pt and reject 0.000002 pt, large-coordinate
relative-tolerance attacks, changed commands/text/styles/transforms, relative
paths and arc flags.

Regressions reject a fabricated SVG, changed PNG pixels, swapped figures and
altered methodology after their checksums are coherently updated. Both normal
and optimized Python must reject these mutations. Independent review additionally
found trailing PNG EXIF orientation/text and invalid IDAT CRC cases; both were
fixed and the original probes were rerun successfully. The reviewer found no
unresolved material issue in this focused presentation-integrity scope. These
checks establish data-to-presentation consistency, not universal experimental
correctness or protection against rewriting the source and its checks together.

All evidence was regenerated from the clean source with 742 bound inputs and
five fresh binaries. All 64 public sample/mask/answer sets and all 345 predictions
remain byte-identical to the previous revision; timings were freshly measured.
The page now explicitly states the incomplete balance of three timing rotations
and the finite d=3/two-round/four-history independent physical oracle. The
fixed-weight ablation and workflow-only performance interpretation remain.

Local validation completed:

- `python -m unittest benchmarks.atom_loss.test_reference benchmarks.atom_loss.test_source_contract benchmarks.atom_loss.test_figure_contract`: 51 tests passed (`drafts/round12-tests.log`).
- All 64 corpora regenerated; 213 current-decoder/corpus combinations matched all 345 prediction files (`drafts/round12-sample-replay.log`, `drafts/round12-decoder-replay.log`).
- Ordinary and `python -O` bundle checks passed; the original resealed SVG attack failed specifically at figure comparison (`drafts/round12-final-verify.log`, `drafts/round12-original-repro.log`).
- Eight site checks and Chromium/Firefox browser tests passed. The results capture was refreshed and mobile overflow checked (`drafts/round12-site.log`, `drafts/round12-browser.log`, `drafts/round12-capture.log`).
- Independent final bundle review passed normal/optimized verification and all seven figure tests (`drafts/round11-agent-audit/REPORT.md`).

These are local checks; hosted CI runs on the subsequent pushed artifact commit.


## Exported-graph physical oracle closure (2026-09-14)

Clean source `f32221e41489cc8dc7b2a283c5f6960178d1f7b0` binds 744 build/source
inputs. All evidence was regenerated with five fresh binaries from a clean
checkout. The 64 public sample/mask/answer sets and all 345 prediction files
remain byte-identical to the previous evidence; workflow timings were measured
again.

The real d=3/two-round Mid-SWAP chain now runs all 1,504 witness rows through
native streaming matching, native MLE, actual PyMatching batch, and native
offline batch. Every prediction is scored against the independently constructed
allowed answers. Empty exported edges fail through both batch consumers;
empty loss mappings produce 74 rejected rows per adapter; observable-labelled
weights scaled by 1e-6 with the mean recomputed produce 308 rejected rows each.
The report retains raw predictions and allowed answers. Its standard-library
contract recomputes summaries and requires all backends and controls. CI reruns
the chain, requires unchanged oracle definitions and revalidates observations;
alternative equally optimal predictions remain valid.

The original empty-graph bypass now raises `Unreachable fired detector`.
A focused subagent re-review found no unresolved material issue in this scope.
Five coherently resealed report attacks (missing adapter, missing control,
wrong summary, invalid decoder-error outcome and truncated predictions) all
failed specifically at the chain contract in normal and optimized Python.
See the local audit at `drafts/round13-final-review.md`.

Completed local checks:

- All 62 tests passed: `python -m unittest benchmarks.atom_loss.test_reference benchmarks.atom_loss.test_source_contract benchmarks.atom_loss.test_figure_contract benchmarks.atom_loss.test_chain_reference` (`drafts/round13-tests.log`).

- Both sample archives regenerated: all 64 corpora passed (`drafts/round13-sample-replay.log`).
- Current decoder replay passed 213 backend/corpus combinations covering all 345 predictions (`drafts/round13-decoder-replay.log`).
- Full bundle checks passed normally and with `python -O`; fresh chain comparison passed (`drafts/round13-verify.log`).
- All eight figures matched Linux/macOS redraws; eight site checks and both Chromium/Firefox evidence tests passed. Desktop capture and mobile overflow checks passed (`drafts/round13-linux-render.log`, `drafts/round13-site.log`, `drafts/round13-browser.log`, `drafts/round13-capture.log`).

The 1,504 oracle witnesses include placeholder pairs and are not additional IID
accuracy samples. Coverage remains this fixture and four loss histories; this
change does not establish general compiler correctness or a kernel-speed ranking.


### Deterministic witness follow-up

The first hosted run at `557caad51` passed the 62 tests but exposed a portability
error in the new fresh-report comparison: seeded Stim sampling can produce
different records under macOS polyfill and Linux SSE2. Each platform passed its
own oracle, while the row-indexed answer arrays differed. The cross-platform
probe reproduced the old measurement/syndrome mismatch.

Clean source `0594f8f6e8d6889025f6fb0db3290a12df158808` replaces those stochastic
correctness witnesses with one canonical `reference_sample()` per physical
fault circuit. It produces 2,998 traces and 752 rows after deduplication and
placeholder pairing, with measurement SHA-256
`f179f618bd3a18bcab28565103c4717c0111d3ae3a534d326200698e8d118cea`.
This supersedes the earlier 1,504-row witness report; these rows remain finite
correctness probes, never IID accuracy samples. Four backends pass; empty loss
mappings are rejected on 54 rows by each batch adapter, and corrupted relative
weights on 186. Empty graphs raise a PyMatching error or produce 110 rejected
offline predictions. The full report comparison remains strict about oracle
definitions, now including witness generation and measurement-row hash.

A Linux x86-64 Python/Stim process independently reconstructed the same rows,
syndromes and allowed-answer sets using captured native graph/model inputs from
macOS. Both reports passed and their definitions matched exactly; this local
probe does not substitute for running a Linux native binary in hosted CI.
All evidence was again regenerated from 744 clean bound inputs and five fresh
binaries. All 64 sample/mask/answer sets and all 345 predictions remain byte
identical to the prior revision; only workflow times were remeasured.

The deterministic final bundle passed ordinary/optimized verification and fresh
chain comparison (`drafts/round14-verify.log`). All eight Linux/macOS figure
comparisons, eight site checks and both browser tests passed. A final independent
subagent review accepted the published report against both portability probes
and the fresh local report, and repeated all ten coherently resealed report
attacks with the intended rejection (`drafts/round14-final-review.md`). No
unresolved material issue remained within this exported-graph oracle scope.

All 62 tests passed again after the deterministic-witness change
(`drafts/round14-tests.log`, 236.403 seconds). Hosted CI on the pushed artifact
commit must complete the final Linux native build and all archived-corpus replays.


### Repeated fresh-context review of the complete PR

The original user review prompt was recovered from four other Codex tasks:
“审核PR712 重点看里面做的benchmark是否公平合理 有没有会被攻击的漏洞”.
Ten new reviewers were run in five rounds, two per round, without inherited
conversation or prior finding lists. Each reviewed the complete PR diff, not
only the latest patch. Confirmed findings were fixed before freezing clean
source and regenerating all evidence for the next round.

| Round | Candidate | Confirmed new evidence-contract gaps |
| --- | --- | --- |
| 1 | `afaa9347` | Incomplete decoder-oracle observations; contradictory runtime provenance. |
| 2 | `dc8313cb` | Sampling negative-control summaries not reconstructed from complete affected observations. |
| 3 | `2ac3490d` | Work/graph counters not tied to public loss patterns and current decoder observations. |
| 4 | `e28b515e` | Standalone ZIP scoring accepted missing comparators and inconsistent score/paired fields. |
| 5 | `88bb87bf` | Neither independent reviewer found an unresolved material issue within the declared scope. |

The fixes retain raw oracle and negative-control observations, independently
recompute their conclusions, bind workload counts to public inputs and current
replay, and enforce complete independently scored standalone archives. A
regression in validation error ordering was also corrected; the final complete
suite passes. Current published accuracy values were repeatedly independently
recomputed and found correct throughout these reviews.

The final measured source is
`b7c274252438e015ea5a2987b06bc8afb2ceb63d`, with **750 bound inputs**. The clean
run freshly rebuilt five binaries and regenerated all evidence and timings.
All 537 `.b8` archive entries remain byte-identical to the previous revision,
including all 64 public corpora/masks/answers and 345 prediction files.

Final local validation:

- **87 tests passed**, across the eight test modules in the evidence CI workflow
  (`drafts/round18-tests.log`, 487.546 seconds).
- Both ordinary and optimized-Python full verification passed.
- Current generator replay matched all 64 corpora. Current decoder replay
  matched all 213 backend/corpus combinations, all 345 predictions, scores and
  the original non-time work counters.
- All eight figures matched Linux/macOS redraws; eight site checks, Chromium
  and Firefox evidence tests, desktop capture and mobile overflow checks passed.
- Both final reviewers independently rescored all 64 corpora and 345 predictions
  and recomputed paired intervals. Each also executed the ten d7/high-loss and
  tradeoff decoder combinations against current binaries.
- Reviewer 09 added a separate seven-wire, 16,384-row integer-cost oracle:
  three matching backends passed; incorrect loss mappings, logical labels and
  conditioning weights were rejected. It independently recomputed 784 Fisher
  tests and the low-probability binomial intervals.
- Reviewer 10 flipped all 110,990 lost-value placeholder bits in the d7/high-loss
  corpus; four backends retained byte-identical predictions.

Local reports and reproduction scripts are under
`drafts/fresh-pr712-audit-09/` and `drafts/fresh-pr712-audit-10/`;
`drafts/fresh-pr712-root/final.md` records the review loop. Review scripts are
scratch artifacts, not production or CI dependencies. The final conclusion is
scoped to this workload and evidence contract: it does not authenticate past
wall-clock time or prove arbitrary-circuit physical correctness. Existing
single-machine, incomplete timing rotation, low-event-count, shared compiler
and finite independent physical-oracle limits remain explicitly disclosed.

### Complete sweep as the default figure

The results page now displays the full d=3/5/7 sweep, including zero-event
upper limits, by default. The nonzero d=3/5 crop is an optional detail. The
additional-seed explanation explicitly attributes the observed tradeoff benefit
to conditioned weights: native and PyMatching envelope each have 35 failures,
versus 61 for fixed weights in 15,000 shots; their pooled totals match at all
16 settings. These statements were checked against `accuracy-seeds.json`.

This is a presentation-only change; no measured source or evidence file was
edited. Eight site checks and both Chromium/Firefox evidence tests passed,
including default full-figure visibility, optional detail expansion and mobile
overflow. The desktop capture was refreshed and the default full figure was
visually inspected (`drafts/round19-main-figure.png`).

### Sampling cost table and explicit matching legends

The sampling chart is replaced on the page by a table of milliseconds per
256-shot batch: median and min/max of the three recorded sample-plus-packing
runs for each backend at d=3/5/7. The template computes cells from
`sampling.json`; both browser tests independently recompute all six cells.
A highlighted note above the table states that the Python + Stim correctness
reference is unoptimized and includes parsing, loss-history grouping, circuit
construction and per-history Stim compilation, while Rust parsing is excluded.
The table does not claim native Stim performance or a backend speedup.

The accuracy/time figure now names both native entries explicitly as
“RustQEC envelope matching (streaming)” and “RustQEC envelope matching (batch)”.
The original separate sampling plots remain downloadable evidence artifacts.

Because the plot and methodology are bound source inputs, all evidence was
regenerated from clean source `41fad414a` with 750 inputs and fresh binaries.
All 537 binary archive entries, including the 64 corpora and 345 predictions,
remain byte-identical to the preceding run. Workflow times were remeasured.
Ordinary and optimized full verification, all eight Linux/macOS figure
comparisons, eight site checks and both browser tests passed. Desktop/mobile
captures were refreshed, including inspection of the warning, table and legend.
Logs and focused screenshots are in `drafts/round20-*`.
