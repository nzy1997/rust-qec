# Interactive Shot Visualizer Design

Date: 2026-08-12
Status: Product design approved
Scope: Interactive, single-shot editing on top of rstim's existing QP101/SVG circuit visualization

## Summary

Add an interactive shot viewer that keeps a parsed Stim circuit fixed, samples one
execution of that circuit, and lets the user change the outcome of an existing
noise opportunity. Each edit re-executes the circuit so downstream measurements,
detectors, and observables immediately reflect the edited physical error.

The feature deliberately reuses the current QP101/SVG renderer. The SVG remains
the visual document; a separate `ViewSnapshot` contains interactive state, and
stable IDs connect the two. The browser UI is a thin HTML/CSS/ES-module shell over
a Rust `ShotSession`, compiled to WASM for the hosted page.

Two entry modes share the same application:

- a GitHub Pages page with one repository-configured, immutable circuit; and
- a local-file page that starts blank and accepts a user-owned `.stim` file.

For fully offline use, `rstim shot_viewer` serves the same application on
`127.0.0.1` and opens it in the user's browser. No circuit is sent to an external
service.

The first version keeps the renderer's existing behavior of fully expanding
`REPEAT` blocks. It does not attempt to support arbitrarily large circuits. A
preflight expansion budget rejects oversized inputs before simulation or DOM
construction; the final default budget is set from browser performance evidence.

## Product Need

The existing `rstim render_svg --sample_shot` path produces a useful and visually
clear static diagram, but investigating a different physical error currently
requires creating another run and comparing another output. The desired workflow
is closer to a small laboratory:

1. hold one circuit fixed;
2. inspect a sampled shot or a no-error baseline;
3. change one existing noise outcome;
4. immediately observe the downstream detector behavior; and
5. save the exact view as SVG or vector PDF.

The same workflow must be useful for debugging, teaching, and producing figures.
Those are not separate modes: all three consume the same current-shot state and
the same renderer.

## Confirmed Semantics

### The circuit is immutable

Loading or configuring a circuit creates a `CircuitSession`. After that point the
circuit text, instruction order, targets, probabilities, and repeat counts are
read-only. Ordinary gates such as `H` and `CX` may be inspected but cannot be
edited.

An interactive edit changes the realized outcome of a noise opportunity already
present in the fixed circuit. It does not insert an `X`, `Y`, or `Z` gate after an
arbitrary operation, and it does not rewrite the source `.stim` file.

DEM construction and DEM error selection are out of scope for this version.

### A shot has a base and sparse overrides

Each session has exactly one current shot:

```text
CurrentShot
├── base: Sampled(seed) | Noiseless(seed)
├── overrides: NoiseEventId -> NoiseOutcome
├── undo stack
└── redo stack
```

`Sample` creates `Sampled(new_seed)`, clears overrides, and clears both history
stacks. All circuit noise and intrinsic quantum measurement choices are sampled.

`Clear` creates `Noiseless(new_seed)`, clears overrides, and clears both history
stacks. Every modeled physical or classical noise choice is forced to its
no-error branch, while intrinsic quantum measurement randomness is still sampled
from the new seed. This makes the result a no-error execution, not a hard-coded
all-zero measurement record.

After either action, a manual edit changes one entry in `overrides`. Every other
stochastic choice remains keyed to the same shot seed. The circuit is re-executed,
and only changes causally downstream of the edited physical outcome can alter the
result.

Removing an override restores that event to the current base result. It does not
perform a global `Clear`.

Undo and redo apply only to manual edits in the current shot. Neither operation
crosses a `Sample` or `Clear` boundary.

### Applicability is distinct from the requested outcome

Some noise opportunities are dynamically inapplicable. For example, a Pauli
error targeting a qubit that is already lost has no physical effect. The state
model therefore distinguishes:

- `base_outcome`: the outcome selected by the shot's base policy;
- `override_outcome`: the user's optional requested outcome; and
- `effective_outcome`: the outcome actually applied by the executor.

An override may remain recorded while temporarily ineffective. If an earlier edit
makes the event applicable again, the same override becomes effective without
being silently discarded. The UI labels an ineffective override and explains the
reason.

## First-Version Scope

### Included

- fixed circuit loading and immutable session identity;
- seeded global sampling and a no-error reset;
- deterministic keyed randomness for stable re-execution;
- manual editing of supported existing noise opportunities;
- immediate measurement, detector, and observable updates;
- current-shot undo and redo;
- full expansion of `REPEAT`, including nested repeat identity;
- the current QP101/SVG layout and styling with added stable IDs;
- pan, zoom, selection, keyboard access, filters, and a detail sidebar;
- direct SVG download and browser-side vector PDF export;
- fixed-circuit GitHub Pages mode and blank local-file mode;
- an offline localhost launcher; and
- native/WASM equivalence, fuzzing, property tests, browser tests, visual
  regression tests, export tests, and a performance matrix.

### Editable noise outcomes

The initial interactive outcome picker supports:

| Instruction | One event applies to | Choices |
| --- | --- | --- |
| `X_ERROR` | one target slot | `I`, `X` |
| `Y_ERROR` | one target slot | `I`, `Y` |
| `Z_ERROR` | one target slot | `I`, `Z` |
| `DEPOLARIZE1` | one target slot | `I`, `X`, `Y`, `Z` |
| `DEPOLARIZE2` | one target pair | `II` plus the 15 non-identity Pauli pairs |
| `LOSS` | one target slot | present, lost |

The executor must still key and replay every other stochastic choice that it
supports, because `Sample`, `Clear`, and edit isolation depend on that property.
Initially, measurement-flip arguments, complete Pauli channels, heralded errors,
and correlated-error chains are sampled and cleared correctly but are shown as
read-only stochastic sites. The loader reports these limitations without
rejecting an otherwise executable circuit.

### Deferred

- circuit editing or inserting new error instructions;
- DEM visualization;
- manually editing correlated, heralded, measurement-flip, or complete Pauli
  channel outcomes;
- collapsed or virtualized repeat rendering and Repeat Focus;
- propagation animation;
- multi-shot statistics, comparison, or a persistent shot gallery;
- re-importing an exported SVG/PDF as an editable session;
- Canvas, WebGL, Typst, or a second circuit renderer;
- server-side circuit storage or sharing; and
- arbitrary-scale circuit support.

## Existing Foundation

The implementation extends, rather than replaces, these existing paths:

- `rstim/src/executor.rs` owns circuit execution and already returns
  measurements, detectors, and observables.
- `rstim/src/sample_trace.rs` records occurred noise events, measurement events,
  and detector events.
- `rstim/src/qp101.rs` converts circuits and sampled traces into QP101.
- `rstim/src/qp101_svg.rs` renders QP101 as the existing SVG diagram, including
  fully expanded repeat groups and iteration boundaries.
- `rstim render_svg --sample_shot --seed ...` is the compatibility reference for
  static sampled output.
- the repository already has a Zola site and GitHub Pages build pipeline.

The design borrows one architectural lesson from Scenery: keep a small,
host-agnostic Rust state engine and put a thin browser interface around it. It
does not adopt Scenery's Canvas drawing, custom serialization, or alternative
rendering stack.

## Architecture

```text
fixed .stim source
       │
       ▼
CircuitSession ─── NoiseEventCatalog
       │                    │
       └──────────┬─────────┘
                  ▼
            EditableShot
       keyed base + sparse overrides
                  │
                  ▼
        Executor::run_with_choices
                  │
                  ▼
             ViewSnapshot
        ┌─────────┴──────────┐
        ▼                    ▼
  QP101/SVG renderer     JSON sidecar
        │                    │
        └─────────┬──────────┘
                  ▼
      HTML/CSS/ES-module application
        ┌─────────┴──────────┐
        ▼                    ▼
 fixed GitHub page     local-file page
                              │
                       rstim shot_viewer
```

The native engine is the source of truth. The WASM interface wraps the same Rust
types; JavaScript never simulates a gate, detector, or error.

## Module Design

### 1. `CircuitSession`

`CircuitSession` owns the immutable source and all reusable derived data:

```rust
pub struct CircuitSession {
    source: String,
    source_sha256: [u8; 32],
    circuit_digest: CircuitDigest,
    instructions: Vec<StimInstr>,
    catalog: NoiseEventCatalog,
    expansion: ExpansionSummary,
}
```

`source_sha256` identifies the exact loaded bytes for provenance.
`circuit_digest` hashes a canonical serialization of parsed IR so stable event
identity is independent of whitespace and comments.

Opening a session performs parsing, instruction validation, catalog construction,
and expansion-budget validation before creating simulator or SVG state. A failed
open leaves the previous session untouched.

The session records warnings for executable but non-editable stochastic
instructions. Errors are structured and include a source span or operation path
where available.

### 2. `NoiseEventCatalog`

Static position and dynamic execution identity remain separate:

```rust
pub struct NoiseSiteId {
    circuit_digest: CircuitDigest,
    op_path: Vec<usize>,
    target_slots: Vec<usize>,
}

pub struct NoiseEventId {
    site: NoiseSiteId,
    repeat_iterations: Vec<u64>,
}
```

`op_path` addresses the instruction through nested repeat bodies.
`target_slots` differentiates independently sampled targets or target pairs in a
single instruction. `repeat_iterations` identifies the concrete dynamic instance.
Indices are zero-based internally; the UI renders friendly one-based labels.

Although the SVG fully expands repeats in this version, the catalog API keeps
static sites and instances distinct. Overrides are stored sparsely and do not
require a dense outcome array:

```rust
list_noise_sites() -> &[NoiseSite]
list_instances(site_id, cursor, limit) -> Page<NoiseEventSummary>
get_event(event_id) -> NoiseEventDetail
```

The current renderer budget bounds how many dynamic instances can be exposed in
one `ViewSnapshot`.

The shipped defaults also cap the tableau at 256 qubits before allocation and
charge wide instruction target lists against the 100,000-node SVG budget. The
remaining defaults are 5,000 expanded operations, 5,000 noise events, and 5,000
measurement results.

### 3. Keyed randomness and `EditableShot`

Sequential RNG replay is not a valid edit model: changing an earlier physical
state can change which later random calls execute and shift the entire tape.
Instead, each stochastic decision has a semantic key:

```rust
pub struct RandomKey {
    circuit_digest: CircuitDigest,
    op_path: Vec<usize>,
    repeat_iterations: Vec<u64>,
    target_slots: Vec<usize>,
    choice_kind: ChoiceKind,
    subchoice: u16,
}
```

`choice_kind` separates noise occurrence, noise branch, intrinsic quantum
measurement, measurement flip, loss-related behavior, herald, and other executor
choices. `subchoice` separates multiple decisions belonging to the same dynamic
event.

The keyed generator is a specified, cross-platform PRF based on SHA-256 over a
versioned binary encoding of `(shot_seed, RandomKey)`. Float and bounded-integer
conversion algorithms are part of the format contract; Rust's `DefaultHasher`
and platform-dependent serialization are forbidden. This guarantees matching
native and WASM results.

The executor gains a choice-source abstraction:

```rust
pub trait ChoiceSource {
    fn bernoulli(&mut self, key: &RandomKey, probability: f64) -> bool;
    fn uniform_below(&mut self, key: &RandomKey, upper: u32) -> u32;
}
```

The existing `Executor::run(rng)` and seeded CLI behavior remain available via a
sequential RNG adapter, preserving current CLI compatibility. Interactive
execution uses `run_with_choices`, whose keyed source is independent of control
flow and dynamic applicability.

```rust
pub enum ShotBase {
    Sampled { seed: u64 },
    Noiseless { seed: u64 },
}

pub struct EditableShot {
    base: ShotBase,
    overrides: BTreeMap<NoiseEventId, NoiseOutcome>,
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
}
```

The no-error choice source forces every declared noise decision to its identity
branch but delegates intrinsic quantum choices to the keyed source. Overrides are
then applied at their exact `NoiseEventId`.

Every state-changing method is transactional: invalid event IDs, unsupported
outcomes, or execution failures return an error and preserve the prior shot and
history.

### 4. Execution trace and results

The current trace records only noise events that occurred. Interactive display
needs all editable opportunities and their state, so the catalog supplies the
opportunity list while execution supplies applicability and realized outcomes.

The result model includes:

```rust
pub struct ShotResult {
    noise_events: Vec<NoiseEventState>,
    measurements: Vec<MeasurementEvent>,
    detectors: Vec<DetectorEvent>,
    observables: Vec<ObservableEvent>,
}
```

`ObservableEvent` is added to `ExecOutput`'s execution-event coverage because
aggregate observables already appear there but per-operation observable events
do not belong to the existing public `SampleTrace` contract.

After an edit, the engine diffs the new result against the previous result and
reports changed measurement, detector, and observable IDs. This is presented as
"changed by the last action," not as a claim that a complete symbolic causal cone
was computed.

### 5. `ViewSnapshot` sidecar

Interaction state is not folded into the portable QP101 schema. A snapshot
contains the rendered document and browser-facing data side by side:

```rust
pub struct ViewSnapshot {
    revision: u64,
    svg: String,
    shot: ShotSummary,
    noise_sites: Vec<NoiseSiteSummary>,
    noise_events: Vec<NoiseEventSummary>,
    measurements: Vec<MeasurementSummary>,
    detectors: Vec<DetectorSummary>,
    observables: Vec<ObservableSummary>,
    changed_by_last_action: ChangedSet,
    provenance: Provenance,
    warnings: Vec<SessionWarning>,
}
```

`revision` increases after every accepted state change. The UI discards a stale
snapshot whose revision is older than the most recently requested operation.

QP101 remains the renderer input and continues to own visual annotations. The
sidecar owns selection details, available outcomes, edit state, warnings, and
history availability.

### 6. QP101/SVG identity extension

The existing renderer keeps its geometry, colors, labels, annotation styles, and
expanded-repeat layout. It adds stable, escaped attributes to the relevant SVG
groups:

```html
<g class="noise-site"
   data-noise-site-id="..."
   data-noise-event-id="...">

<g class="measurement-anchor" data-measurement-id="m24">
<g class="detector" data-detector-id="d7">
<g class="observable" data-observable-id="l0">
```

IDs come from explicit render metadata, never DOM position, CSS class order, or
JavaScript coordinate inference. Attribute values use a compact versioned string
encoding and are treated as opaque by the browser.

The renderer receives an optional interaction index. Calling the existing plain
render path without it produces the current static SVG apart from intentionally
approved, deterministic markup additions. Existing sample-shot rendering remains
supported.

The entire SVG may be regenerated after an edit. Pan, zoom, focus, and sidebar
selection live outside the SVG and are restored after replacement. This keeps
the Rust renderer stateless and avoids fragile DOM patch logic.

### 7. WASM boundary

The browser holds a persistent WASM session rather than sending the full circuit
and edit map on every action:

```rust
#[wasm_bindgen]
pub struct ShotSession { /* opaque */ }

#[wasm_bindgen]
impl ShotSession {
    pub fn open(source: &str, limits: JsValue) -> Result<ShotSession, JsValue>;
    pub fn sample(&mut self) -> Result<JsValue, JsValue>;
    pub fn clear(&mut self) -> Result<JsValue, JsValue>;
    pub fn set_noise(&mut self, event_id: &str, outcome: &str)
        -> Result<JsValue, JsValue>;
    pub fn restore_noise(&mut self, event_id: &str) -> Result<JsValue, JsValue>;
    pub fn undo(&mut self) -> Result<JsValue, JsValue>;
    pub fn redo(&mut self) -> Result<JsValue, JsValue>;
    pub fn snapshot(&self) -> Result<JsValue, JsValue>;
}
```

`wasm-bindgen` and `serde` form the boundary. Public JSON is versioned and uses
strings for opaque IDs. Pan/zoom and transient popover state are not sent to
Rust.

The first version runs WASM on the main browser thread. Operations set a busy
state before invoking it. The API is message-shaped and avoids DOM references so
the session can later move into a Web Worker without redesigning the engine.

### 8. Web application

The application uses native HTML, CSS, and ES Modules. It does not introduce a UI
framework. Its responsibilities are limited to:

- choosing the configured entry mode;
- loading circuit text;
- calling `ShotSession`;
- mounting the returned SVG;
- delegating clicks and keyboard events by stable ID;
- preserving viewport and selection across snapshots;
- rendering the toolbar, popover, sidebar, warnings, and errors; and
- invoking export.

Rust remains the only source of circuit semantics. JavaScript never predicts an
outcome or mutates detector state locally.

## User Experience

### Fixed online mode

The hosted `/interactive/` entry reads a static configuration such as:

```json
{
  "mode": "fixed",
  "circuitUrl": "./circuits/demo.stim",
  "allowLocalFile": false
}
```

It loads exactly that circuit and exposes no file picker or circuit replacement
action. The configured circuit is an ordinary versioned site asset.

### Local-file mode

The `/interactive/local/` entry starts with an empty canvas and a file drop zone.
It accepts `.stim` text through the browser File API. The hosted version does not
upload or persist the file and must not require analytics or third-party network
requests to operate after its own assets load.

After a circuit loads it is fixed. Replacing it requires the explicit "Close
circuit" then "Open circuit" flow, which prevents an accidental drop from erasing
the current shot.

For offline use:

```sh
rstim shot_viewer
```

starts an HTTP server bound only to `127.0.0.1` on an available port, serves a
version-matched embedded web bundle, and opens the local-file entry. Options such
as `--no-open` and `--port` support headless or controlled environments. The
release build pipeline produces and embeds the web bundle; a clean-regeneration
check prevents source/bundle drift.

### Toolbar

The primary toolbar contains:

- `Sample`
- `Clear`
- `Undo`
- `Redo`
- `Export SVG`
- `Export PDF`

`Sample` and `Clear` require no confirmation because they are explicit shot
boundaries, but their labels and tooltips state that manual edit history will be
cleared. Disabled undo/redo controls expose their state accessibly.

### Editing a noise event

Clicking an editable noise element opens a small anchored popover. For example:

```text
DEPOLARIZE1 · q3
Current: I (sampled)

[ I ] [ X ] [ Y ] [ Z ]
```

Choosing an outcome updates the session immediately. A visible marker
distinguishes manually overridden results from sampled results. A "Restore sampled
result" action removes the override.

The detail sidebar shows:

- instruction and target;
- nested operation path and repeat iteration;
- configured probability;
- base, override, and effective outcomes;
- applicability information;
- results changed by the last action; and
- the restore action.

`DEPOLARIZE2` uses a compact 4-by-4 Pauli-pair grid. A read-only stochastic site
explains that manual outcome selection is deferred instead of silently ignoring a
click.

Clicking an ordinary gate only selects it and shows static information. It never
opens an error picker.

### Expanded repeats

Repeats remain fully expanded exactly as in the existing renderer. Each iteration
has a distinct `NoiseEventId`; nested repeat iteration vectors preserve identity.
There is no focus selector or collapsed representation in this version.

Before accepting a circuit, `ExpansionSummary` estimates at least:

- expanded operation columns;
- dynamic noise events;
- measurement events; and
- estimated SVG node count.

If any configured budget is exceeded, the session returns a structured
`CircuitTooLarge` error before building the expanded QP101 document or SVG. The UI
shows the estimated count, the active limit, and the fact that collapsed repeats
are not yet supported. It must not offer a "continue anyway" button that can hang
the page.

Budgets are configurable for development and deployment. The default is selected
from the slowest supported-browser/device tier in the performance matrix, with
headroom rather than from a guessed line-count limit.

### Accessibility and failure behavior

- Every clickable SVG element is keyboard reachable and has an accessible label.
- Outcome buttons use text in addition to color.
- Detector changes are not communicated by color alone.
- Focus returns to the selected element after an SVG refresh when it still exists.
- Parser, size, execution, WASM, and export errors are shown without discarding a
  previously valid session.
- Long operations show busy state after 500 ms and prevent conflicting commands.
- A static explanatory fallback remains visible if WASM initialization fails.

## Export and Provenance

The current SVG is the only rendering master:

```text
ViewSnapshot -> QP101/SVG -> .svg
                         └-> SVG-to-PDF -> vector .pdf
```

Only the circuit figure is exported; toolbar, popover, and sidebar chrome are not.
The figure includes the current base outcomes, overrides, measurements,
detectors, observables, and expanded repeat view.

SVG export adds compact metadata:

```text
format version
rstim version
exact source SHA-256
canonical circuit digest
shot mode and seed
sorted manual overrides
export timestamp
```

The embedded metadata is provenance, not a supported re-import format. It must
not contain the entire undo stack or unneeded source text.

PDF conversion runs in the browser from the exact exported SVG using a pinned,
locally bundled SVG-to-PDF dependency whose license is reviewed during
implementation. External CDNs are not used. Fonts, strokes, text, and vector
shapes remain vector objects; rasterizing the SVG into a canvas is not an
acceptable fallback.

Suggested filenames are sanitized and deterministic apart from the timestamp:

```text
<circuit>-shot-<seed>-edited-<yyyymmdd>.svg
<circuit>-shot-<seed>-edited-<yyyymmdd>.pdf
```

Noiseless shots use `noiseless-<seed>` in place of `shot-<seed>`.

## Publishing and Build Integration

One application build serves both modes. Mode-specific static configuration, not
forked JavaScript, selects behavior.

The existing Zola and GitHub Pages pipeline copies the versioned JS, CSS, WASM,
fixed circuit, and configuration into the built `_site` tree. All URLs are
relative so the page works under a GitHub project subpath. CI rejects missing or
unhashed assets and verifies that fixed mode cannot enable local-file loading by
query parameter alone.

The WASM package is built from a small `rstim-shot-web` wrapper crate depending on
the native engine. The core types live in `rstim`; browser-only bindings do not
enter the simulator or renderer modules.

The offline launcher bundle and hosted bundle are produced from the same build
output. Release tooling records a manifest and SHA-256 for every embedded asset.

## Quality Strategy: Enhanced Verification (C)

### Deterministic unit and integration tests

- identical circuit, seed, and override commands produce byte-equivalent semantic
  snapshots;
- modifying one event leaves every unrelated keyed stochastic choice unchanged;
- `Sample`, `Clear`, restore, undo, and redo obey the stated boundaries;
- `Clear` removes all modeled noise but retains intrinsic quantum randomness;
- nested `op_path`, `repeat_iterations`, and `target_slots` form unique IDs;
- unavailable events preserve requested overrides and correctly reactivate;
- measurement, detector, and observable events update after physical edits;
- invalid commands are transactional; and
- the existing sequential-RNG CLI fixtures remain unchanged.

Golden test vectors include the circuit source, shot seed, edit-command sequence,
and expected semantic snapshot. Native Rust and WASM execute the same vectors and
must agree exactly after excluding presentation-only fields such as export time.

### Property-based tests

Generate bounded valid Clifford circuits with supported noise, measurements,
detectors, observables, and nested repeats. Check properties including:

- stable IDs are unique and deterministic;
- set then restore returns to the original semantic snapshot;
- edit then undo returns to the previous semantic snapshot;
- undo then redo restores the edited snapshot;
- two edits to distinct event IDs commute when their physical operations commute,
  with the property generator responsible for establishing that precondition;
- no-error mode never reports an effective declared-noise branch unless it is
  manually overridden; and
- serialization round trips do not change IDs or outcomes.

Seeds and minimized failing cases are printed for reproduction.

### Fuzzing

Add persistent fuzz targets for:

- opening arbitrary circuit text;
- decoding opaque event IDs and edit commands;
- long valid command sequences over a generated session;
- QP101 interaction metadata to SVG rendering; and
- SVG metadata/provenance escaping.

Fuzz invariants include no panic, no out-of-bounds target access, no unbounded
allocation before expansion-budget rejection, valid UTF-8/JSON responses, and no
unescaped user text entering XML. A scheduled CI job runs longer fuzz budgets;
pull requests run a deterministic smoke corpus.

### SVG visual regression

Existing static fixtures are rendered before and after the identity extension.
Approved changes are limited to deterministic interaction attributes and new
current-shot markers. Geometry, labels, repeat groups, and annotation styling are
compared through normalized SVG structure and representative browser screenshots.

### Browser end-to-end tests

Automated tests cover Chromium and Firefox:

- fixed mode loads the configured circuit and cannot replace it;
- local mode starts blank, accepts a `.stim`, and makes no circuit-upload request;
- clicking an editable event changes downstream results;
- restore, undo, redo, Sample, and Clear work with the stated history boundaries;
- expanded repeat instances receive distinct IDs;
- pan, zoom, selection, and focus survive rerendering;
- oversized input is rejected before SVG creation;
- SVG and PDF downloads are valid; and
- reload resets the transient session rather than implying persistence.

Safari receives a release smoke test on macOS. Accessibility automation checks
keyboard reachability, labels, contrast, and non-color state indicators.

### PDF validation

Each supported browser exports representative diagrams. Tests inspect the PDF
structure to confirm that circuit paths and text remain vector content, then
render pages for visual comparison. Multi-page output is rejected in this version;
the circuit is scaled to one page with preserved aspect ratio and documented
minimum margins.

### Long state-sequence tests

Model-based tests generate hundreds or thousands of valid actions. A simple
reference model tracks shot boundaries and sparse overrides; after every action
it compares base mode, overrides, undo/redo availability, revision, and semantic
results with `ShotSession`.

### Performance matrix

Benchmarks span:

- qubit count;
- expanded column count;
- noise-event count;
- measurement/detector count;
- nested repeat depth; and
- SVG byte/node count.

For each representative circuit, record parse/catalog time, initial execution,
edit re-execution, QP101 construction, SVG rendering, DOM mount, peak native/WASM
memory, SVG size, and PDF conversion time.

The normal interactive target is a snapshot update around 200 ms on the reference
desktop. A busy indicator appears after 500 ms. Expansion budgets are selected so
the slowest supported tier remains responsive and comfortably below its memory
limit. Regressions above an agreed percentage fail CI only after the benchmark
environment is proven stable; otherwise they emit a blocking review artifact.

If accepted circuits routinely exceed 500 ms in WASM, moving the already
message-shaped session API to a Web Worker becomes a follow-up requirement. It is
not silently added to first-version scope.

## Security and Privacy

- Hosted local-file mode performs all parsing and execution in WASM.
- It sends no circuit contents, shot state, or exports to a server.
- The app has no runtime CDN requirement.
- A restrictive Content Security Policy is used where GitHub Pages permits it.
- SVG text and metadata are escaped; event IDs are parsed as data, never HTML.
- Imported circuit size is checked before expansion or rendering.
- The offline launcher binds only to loopback and rejects non-local host headers.
- Download filenames are sanitized and never interpreted as paths by the browser.

## Compatibility and Versioning

- Existing `rstim render_svg`, `export_json`, and seeded sequential-RNG behavior
  remain compatible.
- Plain QP101 does not acquire browser session state.
- `RandomKey`, opaque ID, sidecar, and provenance encodings each carry explicit
  versions.
- Export metadata records the rstim and encoding versions needed to interpret the
  provenance.
- WASM and web assets are version locked; the app refuses a mismatched boundary
  version with a clear cache-refresh message.

## Acceptance Criteria

The feature is complete when all of the following are demonstrated:

1. A configured GitHub Pages page opens one fixed circuit and offers no circuit
   replacement path.
2. The local page starts blank and loads a user-selected `.stim` without an
   external upload.
3. `rstim shot_viewer` provides the same local-file workflow offline on loopback.
4. Sample produces a new global shot; Clear produces a new no-error shot with
   intrinsic measurement randomness retained.
5. Editing one supported existing noise event updates downstream measurements,
   detectors, and observables while all other keyed choices remain fixed.
6. Undo and redo work within one shot and do not cross Sample/Clear.
7. Existing SVG styling and fully expanded repeat rendering are retained, with
   stable event IDs added for interaction.
8. Oversized expanded circuits fail preflight with an actionable error and do not
   hang the page.
9. The current figure downloads as valid SVG and vector PDF with compact
   provenance.
10. Native and WASM golden vectors match, and all enhanced verification suites
    described above pass.

## Recommended Delivery Order

1. Define versioned IDs, random keys, outcomes, and golden vectors.
2. Refactor executor randomness behind `ChoiceSource` while preserving the legacy
   RNG adapter.
3. Implement `CircuitSession`, catalog, expanded-size preflight, and
   `EditableShot`.
4. Complete trace/result coverage for all opportunities and observables.
5. Add `ViewSnapshot` and SVG stable identity without changing layout.
6. Add WASM bindings and native/WASM parity fixtures.
7. Build the thin local-mode UI and end-to-end edit workflow.
8. Add fixed-mode configuration and integrate the GitHub Pages build.
9. Add SVG/PDF export and provenance.
10. Add the offline launcher and version-matched asset packaging.
11. Complete property, fuzz, visual, browser, PDF, sequence, accessibility, and
    performance gates; use the measurements to set the default expansion budget.

## Explicit Decisions

- Existing SVG renderer: reuse it.
- Circuit editing: no.
- Manual error meaning: override an existing noise outcome in the current shot.
- DEM: deferred.
- Randomness: keyed base choices plus sparse overrides.
- Repeat rendering: fully expanded; oversized inputs are unsupported and rejected.
- Browser boundary: persistent WASM `ShotSession`, initially on the main thread.
- UI stack: native HTML/CSS/ES Modules.
- Online/local codebase: one app selected by static configuration.
- Offline local entry: `rstim shot_viewer` on loopback.
- Export master: current SVG; PDF derives from it and remains vector.
- History: current-shot manual edits only.
- Quality level: enhanced verification scheme C.
