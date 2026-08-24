# Third-party conformance fixtures

These fixtures pin the decoder's acceptance boundary to the published
loss-visible circuit subset v1
(`docs/specs/loss-visible-circuit-subset-v1.md`) rather than to rstim's
built-in generators. The circuit structure was produced by an independent
codebase: Google's Stim 1.16.0 (`stim.Circuit.generated`).

| File | Content | Producer |
|---|---|---|
| `stim_rotated_memory_z_d3_r2.stim` | Flattened distance-3 rotated-surface memory-Z circuit, 2 rounds, `after_clifford_depolarization=0.001`. Pure Stim dialect, unmodified. | Stim 1.16.0 |
| `stim_rotated_memory_z_d3_r2.dem` | Detector error model computed by Stim itself (`detector_error_model(decompose_errors=False)`) for the circuit above. | Stim 1.16.0 |
| `stim_rotated_memory_z_d3_r2_loss_visible.stim` | The same circuit annotated into subset v1: `MR`→`MRL`, terminal `M`→`ML`, `LOSS(0.01)` after each CX layer, `LOSS(0.02)` before each readout, `rec[-k]`→`rec[-(2k-1)]` for the inserted flag records, plus the `# RSTIM_LOGICAL_FLIP_POINT` marker after the initial reset. | `tools/annotate_loss_visible.py` |

Consumed by `rustqec-cli/tests/external_fixtures.rs`:

- end-to-end export + dual-backend decode of the annotated circuit, with the
  exact envelope-MLE backend reproducing private answers on a seeded dataset;
- semantic DEM parity between rstim's error analyzer and Stim's own output on
  the extension-free circuit;
- rejection behavior when a spec invariant is violated.

Regenerate all three files with:

```sh
pip install stim==1.16.0
python3 tools/annotate_loss_visible.py
```

The regeneration is deterministic; a diff after regeneration means either the
fixture drifted or the installed Stim version changed.
