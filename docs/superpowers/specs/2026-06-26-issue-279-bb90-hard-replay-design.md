# Issue 279 BB90 hard-syndrome replay design

Issue: #279 Replay the BB90 hard syndrome against Python ldpc and Rust ldpc_cs

Date: 2026-06-26

## Context

Issues #276, #277, and #278 are merged. `rbposd` now has an explicit
`LdpcCombinationSweep` OSD-CS planner, channel-prior candidate scoring in that
mode, and profile counters for OSD and GF(2) work. The existing
`benchmarks/bb_circuit_bposd_compare` smoke runner already exports Rust BB
effective decoder models, replays those models through Python
`ldpc.BpOsdDecoder`, records skipped Python rows when dependencies are missing,
and verifies paired smoke CSV rows.

The gap for #279 is narrower than a campaign: the checked-in BB90 hard-syndrome
fixture exercises a difficult Z-basis BP-OSD path, but the compare runner does
not yet replay that exact syndrome through both upstream Python `ldpc` and Rust
`rbposd` in the new `ldpc`-compatible OSD-CS mode.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming review
gates use the standing answer policy:

- No visual companion is needed because this is a decoder replay and CSV
  verifier change, not a visual design.
- The design is approved from the issue text, the existing hard-syndrome
  fixture tests, and the merged #276/#277/#278 contracts.
- Use the `osd_cs` spelling in CSV rows and CLI arguments. It is the upstream
  Python spelling already documented as equivalent to the repository's older
  `ldpc_cs` label and maps to `OsdVariant::LdpcCombinationSweep` in Rust.
- Use a dedicated `hard-replay` runner tier rather than broadening smoke. This
  keeps the diagnostic cheap and makes the hard fixture contract explicit.
- Keep missing Python dependencies as skipped rows from the runner and a
  verifier failure by default. Add an explicit verifier allow-missing mode only
  for local Rust-only diagnostics.

## Approaches Considered

1. Add a focused hard-replay tier to `run_compare.py`, reuse the existing Rust
   JSON model export, add per-trial Rust logical predictions/stats to that
   export, and add a new `verify_replay.py`.
   This is recommended because it touches the smallest existing surfaces while
   proving both decoders used the same model, syndrome, basis, and settings.
2. Add a new Rust-only fixture replay CLI that directly consumes the fixture
   JSON and prints a separate hard-replay schema.
   This would be clean for Rust, but it creates a second JSON protocol beside
   the compare export and does not help the existing Python replay path.
3. Rebuild the BB90 effective model in Python from the fixture and compare that
   against Rust.
   This duplicates the repository-owned BB circuit/model implementation and
   increases the risk that the comparison uses different models.

## Design

`rsinter` will keep the existing `bb-circuit-bposd-memory --json-compare-case`
export as the shared model source. The command gains an optional
`--osd-method` argument. When omitted, existing behavior is preserved by using
the current default Rust OSD behavior. When set to `osd_cs`, the command parses
the method through `rbposd::OsdVariant::from_method_name` and configures the
Rust decoders with `LdpcCombinationSweep`.

The comparison export will include per-trial Rust logical predictions and
per-basis decode stats for collected trials. Existing consumers can ignore the
new JSON fields. The hard-replay runner will use the Z-basis fields from the
first exported BB90 trial, check that the exported syndrome support and sampled
logical match the checked-in fixture, and write one Rust row for that basis.

`benchmarks/bb_circuit_bposd_compare` will add:

- `HARD_REPLAY_CASES` with one BB90 fixture case:
  `bb90-p006-c10-seed12345-order7-hard-syndrome`, basis `Z`,
  `p = 0.006`, `cycles = 10`, `seed = 12345`, `max_iter = 10000`,
  `bp_method = ms`, `osd_method = osd_cs`, and `osd_order = 7`.
- A `--tier hard-replay` mode in `run_compare.py`.
- A `--rust-binary` option so issue verification can run an existing
  `target/release/rsinter` instead of always invoking `cargo run`.
- CSV columns for replay metadata and counters:
  `basis`, `syndrome_weight`, `syndrome_support`, `logical_prediction`,
  `expected_logical`, `bp_seconds`, `osd_seconds`, `decode_call_count`,
  `bp_iteration_count`, `osd_use_count`, `osd_candidate_count`,
  `gf2_solve_count`, and `gf2_full_elimination_count`.

The existing smoke rows will keep working with the expanded CSV header. Hard
replay rows will fill the replay metadata and the Rust counter fields. Python
rows will fill timing and logical prediction fields; Rust-specific counters stay
blank for Python.

## Data Flow

For the hard replay:

1. `run_compare.py --tier hard-replay` loads the checked-in fixture JSON.
2. The runner invokes `rsinter bb-circuit-bposd-memory --json-compare-case`
   with BB90 fixture settings, `--num-trials 1`, and `--osd-method osd_cs`.
3. The Rust export returns the effective Z/X models, sampled trial, Rust
   logical predictions, and per-basis decode stats.
4. The Python row builds exactly one `ldpc.BpOsdDecoder` for the fixture basis,
   decodes the same syndrome against the exported model, and records the
   predicted logical vector.
5. `results.csv` contains one `rbposd` row and one `ldpc_bposd` row for the same
   hard `case_id`.
6. `verify_replay.py` asserts pairing, pinned settings, matching fixture
   syndrome metadata, matching logical predictions, timing fields, and Rust
   OSD/GF(2) counters.

## Error Handling

If Python `ldpc`, `bposd`, `numpy`, or their `BpOsdDecoder` surface is missing,
the runner records a skipped `ldpc_bposd` row and exits nonzero unless
`--allow-missing-python` is passed. `verify_replay.py` rejects skipped Python
rows by default; `--allow-missing-python` permits that local diagnostic mode
only when the Rust row is otherwise complete and the skipped Python row has an
explicit dependency error.

Rust export failures become Rust error rows and make the runner fail. Fixture
metadata mismatches, missing per-trial prediction fields, and malformed replay
CSV data are verifier failures with messages that mention either unpaired
Rust/Python replay rows or mismatched logical predictions.

## Testing

Add Python tests for:

- `run_compare` hard-replay acceptance with fake Rust export data and fake
  Python `ldpc`.
- hard-replay missing Python dependency rows and nonzero default status.
- `verify_replay.py` accepting a paired hard replay.
- `verify_replay.py` rejecting skipped Python rows without allow-missing.
- `verify_replay.py` rejecting unpaired syndrome metadata and mismatched logical
  predictions.

Add Rust tests for:

- `rsinter bb-circuit-bposd-memory --json-compare-case --osd-method osd_cs`
  emitting per-trial logical prediction and per-basis decode counter fields.
- Existing CLI behavior without `--osd-method` remains compatible.

Required verification:

```bash
cargo test
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-replay \
  --rust-binary target/release/rsinter
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.verify_replay \
  /tmp/rstim-bb90-hard-replay/results.csv
```
