# RSMP v1 Showcase Website Design

Date: 2026-07-24  
Status: approved in conversation

## Purpose

Create a dedicated, deployable single-page website that explains RSMP v1 to a
mixed audience. A project lead should understand the storage value in the first
viewport, while a QEC or Rust developer can continue into the reversible
transform, real CLI workflow, exact evidence, and operational limits.

The page is a product showcase, not a replacement for the normative format
document and not a claim that every circuit compresses by the same ratio.

## Audience

The primary audience combines:

- research and project leads evaluating whether RSMP is useful at scale;
- QEC researchers who need to retain decoder-relevant sample results;
- developers integrating `pack_samples`, `unpack_samples`, or the archive
  library.

The first reading layer emphasizes value and confidence. Technical details,
exact commands, and limitations remain directly accessible on the same page.

## Product boundary

The website presents the implemented RSMP v1 contract:

- every measurement result is recovered losslessly;
- detectors and logical observables are derived using the supplied circuit;
- unpack and verify require the original circuit, not a DEM;
- sweep-bit circuits are unsupported in v1;
- archives are sequential and do not provide random shot access;
- integrity is checked, but producer authenticity is not guaranteed.

The site does not add an RSMP service, execute uploaded circuits, store visitor
data, or modify the archive implementation.

## Narrative structure

### 1. First-viewport conclusion

The hero introduces RSMP as lossless, circuit-bound sample compression for QEC.
It leads with the measured surface-code result:

- RSMP archive size is 11.98% of raw `b8`;
- RSMP uses 57.14% of the direct-Zstandard size;
- the fixed case is surface-code distance 11, 100 rounds, and 1024 shots.

The case identity remains visible beside the headline so the result is not
mistaken for a universal claim.

### 2. Why circuit knowledge helps

A compact process diagram explains:

```text
measurements
  -> XOR noiseless reference
  -> independent detector values + free measurement coordinates
  -> adaptive dense/sparse syndrome codec
  -> independent Zstandard frames
```

The section explains that the transform is reversible. It recovers measurements
first, then derives all detector and observable results. CSS blocks and
connectors illustrate the transformation without a large formula wall or
decorative technical imagery.

### 3. Real CLI workflow

The page demonstrates the natural sequence:

```text
sample -> pack_samples -> verify_only -> unpack_samples
```

A terminal component provides Pack, Verify, and Unpack tabs. Each tab contains
a real command, its purpose, and observed output. Copy buttons support the
commands.

The four-shot compatibility case is explicitly labeled a mechanics
demonstration. Its eight raw bytes become a larger archive because the fixed
envelope dominates tiny inputs; it is never used as compression evidence.

### 4. Measured compression

An honest zero-based horizontal comparison shows:

| Representation | Bytes | Relative to raw |
|---|---:|---:|
| Raw b8 | 1,552,384 | 100.00% |
| Direct Zstandard | 325,565 | 20.97% |
| RSMP v1 | 186,028 | 11.98% |

The section states both savings:

- 88.02% fewer bytes than raw `b8`;
- 42.86% fewer bytes than direct Zstandard.

The high-entropy control is shown separately:

| Representation | Bytes | Relative to raw |
|---|---:|---:|
| Raw b8 | 1,048,576 | 100.00% |
| RSMP v1 | 1,049,064 | 100.0465% |

This control demonstrates bounded overhead, not compression.

### 5. Large-scale estimator

An interactive calculator accepts a shot count and offers shortcuts for one
million, 100 million, and one billion shots. It reports:

- raw `b8` storage;
- direct-Zstandard storage;
- RSMP storage;
- serial pack time;
- serial unpack time;
- storage saved relative to raw and direct Zstandard.

Every result is calculated from the pinned 1024-shot evidence. No projected
value is embedded as an independent claim.

The estimator displays `Projected` prominently and includes this limitation:

> Linear projection from one 1024-shot observation on the pinned surface-code
> case. It is not a large-scale measurement or performance guarantee.

### 6. Confidence and limitations

The final evidence section reports:

- seven required semantic roles;
- 27 named corruption recipes;
- 491 generated truncation checks;
- eight generated bit flips;
- one immutable two-block compatibility fixture covering sparse and dense
  codecs.

It links to the repository, normative format document, CLI guide, evidence
bundle, and feature-test report.

The readiness disclosure is exact:

- the complete 19-command readiness suite passes in a clean exported checkout;
- the ordinary working tree can fail evidence validation when a different
  current `target/release/rstim` exists at the historical producer path;
- this is an existence-dependent, non-hermetic checker behavior, not a
  recomputation failure of the committed compression arithmetic.

## Evidence contract

The website maintains one typed evidence object as the only source for measured
and projected numbers.

### Measured case

```text
circuit: surface-code d=11, rounds=100
shots: 1024
M: 12121
D: 12000
L: 1
rank: 12000
free width: 121
raw b8: 1552384 bytes
direct Zstandard: 325565 bytes
RSMP: 186028 bytes
pack throughput: 15443821 raw-input bytes/second
unpack throughput: 37218025 raw-input bytes/second
logical block working-set model: 11900649 bytes
```

Pack throughput corresponds to approximately 10,187 shots per second. Unpack
throughput corresponds to approximately 24,550 shots per second.

### Projection formulas

For a requested shot count `S`:

```text
scale = S / 1024
raw bytes = 1552384 * scale
direct Zstandard bytes = 325565 * scale
RSMP bytes = 186028 * scale
pack seconds = raw bytes / 15443821
unpack seconds = raw bytes / 37218025
```

The UI may round display values but retains full-precision arithmetic internally.
Units distinguish decimal `GB/TB` from raw byte counts.

### Timing limitations

The measured timing is one observation without repeats, confidence intervals,
warm-up control, or cross-platform comparison. It was recorded on arm64 macOS
with Rust 1.93.1. Timing includes process startup, circuit parsing, transform
construction, and file I/O. Unpack produced measurements, detectors, and
observables; the number is not codec-only throughput.

The 11.9 MB value is a logical working-set model, not measured RSS.

## Visual design

The visual language resembles a precise scientific instrument rather than a
generic dashboard.

- Background: deep ink blue.
- Primary text: warm off-white.
- RSMP accent: cyan-green.
- Raw baseline: muted steel blue.
- Direct-Zstandard accent: amber.
- Measured state: green label.
- Projected state: amber label.

The first viewport uses a large `11.98%` figure paired with compact evidence
cards. Narrow reading sections alternate with full-width evidence regions.
Typography carries most of the identity; no decorative stock imagery or
model-authored SVG illustration is required.

Charts begin at zero and reflect actual byte ratios. The interface does not use
truncated axes, three-dimensional bars, or decorative chart effects.

## Interaction and accessibility

- The terminal tabs are operable by keyboard and touch.
- Copy controls expose accessible labels and transient success feedback.
- The shot estimator combines numeric input, slider, and scale shortcuts.
- Invalid or empty estimator input falls back to a clear inline validation
  state and never emits `NaN` or infinite results.
- Mobile layout avoids horizontal page overflow; code regions scroll locally.
- Color is never the sole carrier of measured/projected or pass/caveat status.
- Motion is limited to small numeric and state transitions and respects
  `prefers-reduced-motion`.

## Architecture

The site lives at `sites/rsmp-v1-showcase/` as a separate project surface
inside the repository. It uses the Sites starter architecture and produces a
Cloudflare Worker-compatible build.

The first version has one route and no persistence, authentication, uploads,
runtime environment variables, or external data dependency. Page content and
the evidence object are compiled into the site.

The implementation should favor:

- one page component for narrative composition;
- small focused components for the terminal, compression comparison, evidence
  badges, and estimator;
- one primary stylesheet for layout, visual tokens, and responsive behavior;
- deterministic formatting utilities for byte counts, ratios, and durations.

## Validation

### Repository feature evidence

Before publishing the website:

- retain the independent RSMP feature-test report;
- do not describe the ordinary working-tree readiness command as passing while
  the historical binary-path defect remains;
- preserve the distinction between clean-checkout readiness and current-tree
  readiness.

### Website behavior

The release check must establish:

- the production build exits successfully;
- the estimator matches independent expected values for one million,
  100 million, and one billion shots;
- all displayed measured byte counts and ratios match the evidence object;
- terminal tabs and copy controls are keyboard-operable;
- invalid estimator values show a readable error;
- desktop and mobile CSS prevent page-level horizontal overflow;
- measured and projected content remain visibly distinct.

### Publishing

Publish the validated version using Sites. The deployed URL is the primary
deliverable. The site source remains in the repository as an independently
buildable project.

## Out of scope

- Fixing the non-hermetic readiness checker.
- Regenerating or changing the compression evidence.
- Claiming a universal compression ratio.
- Claiming that projected large-scale timing was measured.
- Adding a circuit upload or live compression service.
- Adding user accounts, analytics, database storage, or external APIs.
- Modifying RSMP archive semantics or CLI behavior.
