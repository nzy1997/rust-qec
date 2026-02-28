# RStim Phases 8–11 Roadmap

**Goal:** Close the remaining gaps between rstim and Stim by adding `sweep[]` target support,
`ptb64` output format, surface/color code generators, format conversion CLI commands, and
error explanation.

---

## Phase 8 — `sweep[]` Targets + `ptb64` Output Format

### sweep[] targets

`sweep[k]` is a classical control input target used in batch experiments. Stim treats it as
a per-shot bit that can flip measurement results. In rstim's single-shot tableau simulator it
is a no-op (always 0); in the frame simulator it can be wired to a sweep-bit table.

**Files:**
- Modify: `src/ir.rs` — add `StimTarget::Sweep(u32)` variant; update `circuit_to_string`
- Modify: `src/parser.rs` — parse `sweep[k]` tokens (non-negative integer index)
- Modify: `src/executor.rs` — accept `Sweep` targets without error (no-op in tableau path;
  frame path treats as 0 unless a sweep table is provided)
- Modify: `src/sim/frame.rs` — accept `Sweep` targets in measurement instructions
- Test: `tests/sweep.rs`

### ptb64 output format

Partially-transposed bit-packed binary: bits are packed 64 shots × 1 bit per u64 word,
transposed so that all shots for detector 0 come before detector 1, etc. Used for SIMD
decoders.

**Files:**
- Modify: `src/output.rs` — add `Format::Ptb64`; implement `write_shots_ptb64`
- Modify: `src/cli.rs` — accept `"ptb64"` in `--out_format` for `sample`, `detect`, `sample_dem`
- Test: `tests/output_formats.rs`

---

## Phase 9 — Surface Code + Color Code Circuit Generators

Extends `rstim gen` to support the two most-used QEC codes.

### surface_code

Tasks: `rotated_memory_x`, `rotated_memory_z`, `unrotated_memory_x`, `unrotated_memory_z`.

Layout follows Stim's convention: data qubits on a 2d grid, ancilla qubits interleaved.
Each round: reset ancillas, apply stabilizer CNOTs in canonical order, measure ancillas,
emit DETECTORs. Final round measures data qubits and emits OBSERVABLE_INCLUDE.

**Files:**
- Create: `src/gen/surface_code.rs`
- Modify: `src/gen/mod.rs` (or `src/circuit_gen.rs`) — register new generators
- Modify: `src/cli.rs` — accept `--code surface_code` with the four tasks
- Test: `tests/gen_surface_code.rs`

### color_code

Task: `memory_xyz`.

Triangular color code on a 2d grid. Three-body stabilizers (X, Y, Z type).

**Files:**
- Create: `src/gen/color_code.rs`
- Modify: `src/gen/mod.rs` — register
- Modify: `src/cli.rs` — accept `--code color_code --task memory_xyz`
- Test: `tests/gen_color_code.rs`

---

## Phase 10 — `convert` + `m2d` CLI Subcommands

### convert

Reads a shot file in one format and writes it in another. Supports all 6 formats:
`01`, `b8`, `r8`, `hits`, `dets`, `ptb64`.

Requires knowing the number of bits per shot (passed via `--bits` flag, or inferred from
a companion circuit/DEM file via `--circuit` or `--dem`).

**Files:**
- Modify: `src/cli.rs` — add `Commands::Convert` with `--in_format`, `--out_format`,
  `--bits` (or `--circuit`/`--dem`), `--in`, `--out`
- Create: `src/convert.rs` — `read_shots(format, bits, reader)` → `BitTable`;
  `write_shots(format, table, writer)`
- Modify: `src/output.rs` — add corresponding readers (`read_shots_01`, `read_shots_b8`, etc.)
- Test: `tests/convert.rs`

### m2d

Converts raw measurement output to detection events, given a circuit. Equivalent to
`stim m2d`. Reads measurement bits, runs the deterministic (noiseless) reference simulation
to get expected measurement values, XORs to produce detection events.

**Files:**
- Modify: `src/cli.rs` — add `Commands::M2d` with `--circuit`, `--in_format`,
  `--out_format`, `--in`, `--out`, `--append_observables`
- Create: `src/m2d.rs` — `measurements_to_detections(circuit, meas_table)` → `(det_table, obs_table)`
- Test: `tests/m2d.rs`

---

## Phase 11 — `explain_errors` CLI Subcommand

Given a circuit and a set of detection events (one shot), finds the minimal set of error
mechanisms from the DEM that could explain the observed detectors firing.

Stim's `explain_errors` runs the circuit→DEM conversion, then for each provided detection
event set finds matching error mechanisms via a greedy/exact search.

**Files:**
- Modify: `src/cli.rs` — add `Commands::ExplainErrors` with `--circuit`, `--dem` (optional),
  `--in` (detection events), `--out`
- Create: `src/explain_errors.rs` — `explain(dem, detectors_fired)` → `Vec<ExplainedError>`;
  `ExplainedError` holds the DEM error probability + detector targets + circuit-level location
- Test: `tests/explain_errors.rs`

---

## Dependency Order

```
Phase 8  (sweep[], ptb64)
  └─► Phase 10 (convert needs ptb64; m2d is independent)

Phase 9  (surface_code, color_code)  ← independent

Phase 11 (explain_errors)  ← builds on existing DEM/ErrorAnalyzer
```
