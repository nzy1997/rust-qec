# Issue 301 Surface-Code Atom-Loss and SVG Layout Design

## Context

Issue #301 asks for a first-class opt-in atom-loss channel in surface-code
generation and a renderer fix for the wide surface-code atom-loss timeline.
The current generator already supports depolarizing/Pauli-style noise through
`NoiseParams::after_clifford_depolarization`; that meaning must not change.
The current mixed-noise showcase gets its `LOSS(0.01)` block from a sparse
manual insertion in `rstim::showcase::mixed_noise_rotated_memory_x_d3_r3`.

Live GitHub issue and PR lookup is unavailable in this Agent Desk environment
because `gh` cannot reach the GitHub API through the configured proxy. The issue
body and checked-in atom-loss showcase artifacts are the binding context for this
run.

## Selected Approach

Add a separate `after_clifford_loss_probability: f64` field to
`rstim::codegen::NoiseParams`, default it to `0.0`, and keep
`NoiseParams::uniform(noise)` depolarization/flip-only. The new loss field will
be populated explicitly by the `rstim gen --after_clifford_loss_probability`
CLI flag and by the showcase helper. This preserves old default and
depolarization-only semantics.

Apply the loss channel to both rotated and unrotated surface-code schedules.
After every emitted one-qubit Clifford layer (`H` in the current surface-code
round schedule), emit a single `LOSS(p)` operation targeting all qubits touched
by that layer. After every emitted two-qubit Clifford layer (`CX`), emit
`LOSS(p)` on all qubits participating in that layer. Existing depolarization
emission remains adjacent to the same layer and independent of loss.

Update the mixed-noise showcase so its dense atom-loss coverage comes from the
new surface-code generation path, while preserving the existing sparse
`X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, and `DEPOLARIZE2` decorations near the
tail. Regenerate the committed `.stim`, base QP101 JSON, sample QP101 JSON, and
matching Rust QP101 fixtures through `cargo run -p rstim --example
mixed_noise_showcase`. No checked-in SVG preview exists in this checkout, so no
SVG artifact is regenerated.

For `rstim::qp101_svg`, replace the one-visible-column-per-operation layout with
a layer packer. Operations between `TICK`s can share a column when their rendered
lane footprint does not overlap. Single-qubit same-layer gates like many `H`
operations will therefore share one column. Multi-target or paired operations
such as batched `CX`/`DEPOLARIZE2` get split into render items and assigned to
non-overlapping columns within the current layer, preventing vertical boxes and
lines from visually colliding. Probability labels for known noise operations
will be decimal-only (`0.01`) and attached per rendered item, so every `D1` and
`LOSS` box has its own visible parameter label.

## Interfaces

- `NoiseParams` gains `after_clifford_loss_probability: f64`.
- Existing constructors keep old behavior:
  - `NoiseParams::none()` returns all zeros.
  - `NoiseParams::uniform(noise)` keeps loss at `0.0`.
- `rstim gen` gains `--after_clifford_loss_probability`, defaulting to `0`.
- `generate_common_circuit_text` and `run_gen` receive a `NoiseParams` value for
  common-code generation so the new CLI flag reaches surface-code generation.
- Public generator functions such as `rotated_memory_x(distance, rounds, noise)`
  keep their signatures and semantics.
- `rotated_memory_x_with_params`, `rotated_memory_z_with_params`,
  `unrotated_memory_x_with_params`, and `unrotated_memory_z_with_params`
  honor the new loss field.

## Tests

- Add `surface_code_after_clifford_atom_loss` in `rstim/tests/gen_surface_code.rs`.
  It constructs a distance-3, round-3 rotated-memory-X circuit with
  `after_clifford_loss_probability = 0.01` and verifies that each `H` and `CX`
  Clifford operation is immediately followed by `LOSS(0.01)` on the exact qubits
  touched by that Clifford layer.
- Add a CLI regression in `rstim/tests/cli_gen.rs` proving
  `--after_clifford_loss_probability 0.01` emits loss and
  `--after_clifford_depolarization 0.01` does not.
- Update QP101 fixture tests so the showcase contract expects generated
  after-Clifford loss, not a sparse final-tail loss block.
- Add `surface_code_atom_loss_svg_layout_regression` in
  `rstim/tests/qp101_svg.rs`. It renders a compact surface-code-like document and
  asserts packed same-layer single-qubit gates, non-overlapping boxes, one
  decimal-only label per `D1`/`LOSS` item, and no `p=` labels for known noise.
- Keep existing depolarizing-noise tests green.

## Verification

Run the issue-specified focused checks:

```sh
cargo test -p rstim --test gen_surface_code surface_code_after_clifford_atom_loss -q
cargo run -q -p rstim --bin rstim -- gen --code surface_code --task rotated_memory_x --distance 3 --rounds 3 --after_clifford_loss_probability 0.01 --out /tmp/rstim-surface-d3-r3-atom-loss.stim
cargo run -q -p rstim --bin rstim -- gen --code surface_code --task rotated_memory_x --distance 3 --rounds 3 --after_clifford_depolarization 0.01 --out /tmp/rstim-surface-d3-r3-depol-only.stim
cargo run -p rstim --example mixed_noise_showcase
cargo test -p rstim --test qp101_fixtures mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise -q
cargo test -p rstim --test qp101_svg surface_code_atom_loss_svg_layout_regression -q
```

Then run broader checks before opening the PR:

```sh
cargo test -p rstim --test qp101_export --test qp101_fixtures --test cli_export_json
typst compile --root qp101-viz qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ /tmp/surface-code-rotated-memory-x-d3-r3-atom-loss.pdf
cargo test
```

## Self-Review

- No unresolved placeholders remain.
- The design keeps depolarization semantics independent from atom loss.
- The work is scoped to generator CLI plumbing, surface-code schedule emission,
  showcase regeneration, and the built-in SVG renderer.
- QP101 JSON schema semantics do not change, so `rstim/doc/QP101-ZY.md` does not
  need an update.
