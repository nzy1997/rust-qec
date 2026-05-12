# Sample Visualization Design

**Date:** 2026-05-12

## Goal

Add a single-shot sample visualization path that:

1. renders circuits containing the new `LOSS` gate correctly
2. annotates one sampled execution directly on the circuit
3. marks only the noise events that actually occurred
4. shows the concrete branch for ambiguous noise such as `DEPOLARIZE1/2` and `PAULI_CHANNEL`
5. shows every measurement result
6. shows only detector flips

This feature is for explanation and debugging, not bulk sampling throughput.

## User-Confirmed Display Contract

The visualization must follow these rules.

### Error annotations

- annotate only noise events that actually happened in the sampled shot
- if the sampled branch is ambiguous at the instruction level, show the resolved branch:
  - `DEPOLARIZE1` -> `X`, `Y`, or `Z`
  - `DEPOLARIZE2` -> `XX`, `XY`, ..., `ZZ`
  - `PAULI_CHANNEL_1` -> `X`, `Y`, or `Z`
  - `PAULI_CHANNEL_2` -> `IX`, `IY`, ..., `ZZ` as applicable
- `LOSS` is shown explicitly as loss, not collapsed into a generic error marker

### Measurement annotations

- annotate every measurement result
- for ordinary measurement instructions, if the sampled bit is `1` because the qubit was already lost, show that cause explicitly
- for loss-visible measurement instructions, show both the `loss_flag` and the `value_bit`

### Detector annotations

- annotate only detector flips
- non-flipping detectors stay unmarked

## Scope

This design covers:

- `rstim` executor-side single-shot tracing
- `rstim` QP101 export for sample overlays
- `qp101-viz` timeline rendering updates needed for sample overlays
- support for `LOSS` in circuit rendering

This design does not cover:

- DEM extraction for loss
- batch visualization of many shots
- new coordinate-layout rendering
- heatmaps or statistical summaries

## Recommended Architecture

The implementation should be split into three layers.

### Layer 1: Executor Trace

Add a single-shot trace path in `rstim` that runs the circuit once and records:

- which noise sites fired
- which branch each ambiguous noise site chose
- which measurement bits were produced
- whether a measurement result came from loss semantics
- which detectors flipped

This layer owns execution truth. It should not depend on renderer behavior.

### Layer 2: QP101 Export With Sample Overlay

Continue to export the circuit through the existing QP101 document shape. Do not replace the current exporter. Instead, add a sample-aware export variant that:

- emits the same circuit structure as `export_qp101`
- injects sample annotations using the existing `annotations` mechanism
- preserves exact source locations through `op_path`, `repeat_iterations`, and `target_slots`

### Layer 3: Typst Rendering

`qp101-viz` should render the sample overlay as presentation only. It must not reimplement execution logic. The renderer consumes already-resolved annotations and draws:

- fired noise branches
- measurement values
- loss-caused measurements
- detector flips

## Core Design Decision

Do not attempt to reconstruct sample semantics from `sample_batch` outputs.

The existing batch sampler only returns measurement, detection, and observable bit tables. It does not retain enough information to explain a shot:

- it cannot recover which noise site fired
- it cannot recover which branch a depolarizing or Pauli-channel instruction chose
- it cannot distinguish a normal `1` measurement from a `1` caused by loss

Therefore the visualization path must add an executor trace instead of trying to infer the shot afterward.

## Trace Data Model

Introduce a trace object for one sampled shot.

```text
SampleTrace
  noise_events: Vec<NoiseEvent>
  measurement_events: Vec<MeasurementEvent>
  detector_events: Vec<DetectorEvent>
```

The location model should reuse the existing DEM provenance coordinate scheme:

- `op_path`
- `repeat_iterations`
- `target_slots`
- `target_qubits`

This avoids inventing a second addressing system and lets the sample overlay match the existing repeat-aware QP101 annotation flow.

### NoiseEvent

Each noise event represents one natural target group from one instruction in one sampled shot.

Fields should include:

- `op_path`
- `repeat_iterations`
- `instr_name`
- `target_slots`
- `target_qubits`
- `occurred: bool`
- `branch_label: Option<String>`

Examples:

- `X_ERROR` -> `branch_label = "X"`
- `DEPOLARIZE1` -> `branch_label = "Y"`
- `DEPOLARIZE2` -> `branch_label = "XZ"`
- `LOSS` -> `branch_label = "L"`
- non-fired events remain in trace only if useful internally, but only fired ones are exported as annotations

### MeasurementEvent

Each measurement event represents one emitted measurement bit.

Fields should include:

- `op_path`
- `repeat_iterations`
- `target_slot`
- `target_qubit`
- `instr_name`
- `measurement_index`
- `bit`
- `loss_cause`
- `component`

`component` is needed for loss-visible measurement families:

- `value`
- `loss_flag`

This lets one instruction target emit either one logical measurement annotation or a two-part annotation without losing ordering.

### DetectorEvent

Each detector event should include:

- `op_path`
- `repeat_iterations`
- `detector_index`
- `flipped`

Only `flipped = true` becomes a visible annotation.

## Executor Changes

The executor is the right place to record trace details because the information only exists at execution time.

### Trace-capable execution entry point

Add a trace-aware single-shot API instead of forcing all callers through the current `ExecOutput` only path. The exact naming is flexible, but the behavior should be:

- execute the circuit once
- return the ordinary `ExecOutput`
- return a `SampleTrace`

### Preserve execution context through repeats

The current repeat execution path creates a nested executor and only appends the nested measurements and detector outputs. That is too weak for sample visualization because it loses:

- full `op_path`
- `repeat_iterations`
- any direct trace event stream
- potentially other local execution state that should stay in one trace session

The sample trace path should execute repeats recursively within one shared trace context.

### Reuse existing loss semantics

Do not redesign the loss behavior. The trace should report what the executor already does:

- `LOSS` can mark qubits as lost
- gates and noise skip lost qubits or groups as already defined
- ordinary measurements on lost qubits output `1`
- loss-visible measurements emit explicit loss metadata

The trace simply records these outcomes.

## QP101 Export Design

Keep the document shape stable and reuse `Qp101Annotation`.

### Circuit operation classification

Update the exporter so `LOSS` is treated as a `noise` operation in QP101 export. Without this, circuits containing `LOSS` will not render through the specialized noise path.

### Sample annotation strategy

Export sample overlays as `marker` annotations.

#### Noise markers

- attach to the corresponding `noise` operation
- fill `target_slots`
- label with the resolved sampled branch
- style them as danger/red

Examples:

- `X`
- `YZ`
- `L`

#### Measurement markers

Attach measurement results to the corresponding measurement operation and target slot.

For ordinary measurement families:

- `0`
- `1`
- `1[L]` when the result is `1` because the qubit was lost

For loss-visible measurement families:

- show both outputs in one compact annotation
- recommended text form: `L=0 | M=1` or `L=1 | M=1`

Do not create two separate boxes on the timeline for `ML/MRL` output bits. The circuit operation is still one measurement action at one moment.

#### Detector markers

- attach to the detector operation
- annotate only when flipped
- use the existing blue symptom style

## Typst Rendering Semantics

`qp101-viz` already has useful annotation infrastructure. Extend it instead of creating a parallel rendering system.

### Noise rendering

The existing noise renderer should stay responsible for drawing the gate body.

Add `LOSS` to the supported noise classification so it renders as a compact per-target noise operator instead of falling back to a generic gate form.

### Measurement rendering

The current render model only recognizes a narrow set of measurement gates. Extend `measurement-targets(op)` so it covers:

- `M`, `MX`, `MY`, `MZ`
- `MR`, `MRX`, `MRY`, `MRZ`
- `ML`, `MXL`, `MYL`, `MZL`
- `MRL`, `MRXL`, `MRYL`, `MRZL`

Measurement boxes should continue to represent one circuit operation target. Sample result text is an overlay label, not a second structural measurement box.

### Detector rendering

Reuse the existing detector annotation styling path. Sample detector flips should look like symptoms, not like gates.

## CLI And API Shape

The design does not require a final CLI surface yet, but the implementation should be structured so a caller can request:

1. one sampled shot with a fixed seed
2. one QP101 document with the sample overlay embedded

This keeps the feature usable both from tests and from a future CLI command or export flag.

## Risks

### 1. Sampling truth split across layers

If the renderer starts inferring execution details from raw QP101, the visual output will drift from the simulator. Keep all sampling truth in the executor trace.

### 2. Repeat addressing drift

If sample annotations do not reuse `op_path` and `repeat_iterations`, repeat-body overlays will land on the wrong moment. The design should keep one addressing scheme across DEM and sample overlays.

### 3. Overloaded measurement visuals

If loss-visible measurement outputs are rendered as multiple structural gates, the timeline will stop matching the circuit. Keep the gate count stable and place extra information in labels only.

## Verification Strategy

Verification should cover four layers.

### 1. Executor trace tests

Add targeted tests that lock down:

- `LOSS` firing records `L`
- `DEPOLARIZE1` records `X/Y/Z`
- `DEPOLARIZE2` records a concrete two-qubit branch
- `PAULI_CHANNEL_1/2` record the chosen branch
- ordinary measurement on a lost qubit records `bit = 1` and `loss_cause = true`
- `ML/MRL`-family measurements record two components in the right order
- only flipped detectors become positive detector events

### 2. QP101 export tests

Add JSON assertions that:

- `LOSS` exports as `type: "noise"`
- fired noise branches produce red sample markers
- measurement results produce the expected annotation text
- flipped detectors produce blue symptom markers
- repeat-localized sample annotations carry the expected `repeat_iterations`

### 3. Typst checks

Add or extend visual checks so the rendered output shows:

- `LOSS` on the circuit
- a fired ambiguous noise branch with its resolved label
- measurement overlays including `1[L]`
- a flipped detector marker

### 4. End-to-end sample fixture

Create at least one fixed-seed fixture circuit that includes:

- one ambiguous noise instruction
- one loss event
- one ordinary measurement affected by loss
- one loss-visible measurement
- one detector flip

The resulting QP101 output should be stable under the fixed seed.

## Out Of Scope

This first version does not include:

- sample overlays for many shots at once
- aggregated frequency views
- automatic CLI image generation
- DEM-aware loss analysis
- a redesign of the QP101 base schema

## Implementation Direction

The smallest credible implementation is:

1. add executor trace support for one shot
2. export sample overlays through QP101 annotations
3. teach the renderer about `LOSS` and the wider measurement family
4. add fixed-seed tests and Typst fixtures

That is enough to support the requested debugging workflow without reopening the protocol design.
