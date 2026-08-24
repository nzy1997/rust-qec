# Loss-Visible Circuit Subset v1

Status: **stable contract** (versioned). This document is the public
specification of the circuit subset accepted by `rustqec decode` and produced
by `rustqec dataset export`. It exists so that dataset producers outside this
repository can generate decodable datasets without reading the compiler
source.

The reference implementation is
[`rustqec-cli/src/decode/compiler.rs`](../../rustqec-cli/src/decode/compiler.rs)
(`compile_circuit` and `normalize_supported_circuit`). Where this document and
the implementation disagree, the implementation is wrong or this document is
stale; both are pinned together by the test suite, so please file an issue.

## 1. Scope and versioning

- Subset version: **v1**.
- The decoder accepts exactly one circuit per dataset, stored as
  `circuit.stim` inside a public dataset bundle (see §7).
- Any circuit meeting every requirement in §2–§6 is accepted, regardless of
  which software generated it. Acceptance is capability-checked per circuit;
  there is no allow-list of generator programs or code families.
- Widening the subset (new instructions, `REPEAT`, additional loss-visible
  bases) requires a new subset version. Rejections that v1 specifies must stay
  rejections within v1.

## 2. File-level requirements

| Requirement | Error code on violation |
|---|---|
| Circuit is valid UTF-8 and parses under `rstim::validation::parse_and_validate` | `invalid_dataset` / `unsupported_circuit` |
| Circuit is flat: no `REPEAT` blocks | `unsupported_circuit` ("outside the flat native Mid-SWAP subset") |
| Measurement/detector/observable/sweep-bit counts match `manifest.json` | `invalid_dataset` |
| No sweep bits; between 1 and 64 observables | `unsupported_circuit` |

## 3. Instruction subset

Each instruction is classified as **kept for analysis** (present in the DEM
extraction circuit), **structural** (consumed by the loss compiler itself), or
**rejected**.

| Instruction | Class | Notes |
|---|---|---|
| `LOSS q...` | structural | Declares a loss-opportunity site. Not itself analyzed; it opens the window in which a later loss-visible readout may herald a loss. Targets must be plain qubits. |
| `ML`, `MZL` | structural → `M` | Loss-visible Z readout. Emits **two** measurement records: flag then value (see §4). |
| `MRL`, `MRZL` | structural → `MR` | Loss-visible Z readout with reset. Same two-record layout; closes the loss window for that wire. |
| `MXL`, `MYL`, `MRXL`, `MRYL` | rejected | Non-Z loss-visible bases are future work. |
| `H` | kept | Also tracked as a basis-change site for envelope compilation. |
| `CX`, `CNOT`, `ZCX` | kept (decomposed to `H`–`CZ`–`H`) | Targets must form complete, pairwise-disjoint qubit pairs within one instruction. |
| `R`, `RZ` | kept | Closes any open loss window for the targeted wires. |
| `X_ERROR`, `DEPOLARIZE1`, `DEPOLARIZE2` | kept | The only noise channels in v1. All probabilities must be finite and `< 0.5` at DEM level. |
| `QUBIT_COORDS`, `TICK`, `DETECTOR`, `OBSERVABLE_INCLUDE` | kept | Annotations; see §5. |
| anything else (incl. `Y_ERROR`, `Z_ERROR`, `PAULI_CHANNEL_*`, `CORRELATED_ERROR`, `MPP`, `S`, `SWAP`, …) | rejected | `unsupported_circuit` naming the instruction. |

Loss-visible readouts with inline noise arguments (`ML(p) ...`) are rejected.

## 4. Loss-record layout

For every loss-visible readout the measurement record interleaves:

1. **flag record** — 1 iff the atom was heralded lost at this readout;
2. **value record** — the measurement value bit.

The flag occupies the earlier measurement index. During compilation the flag
position is materialized as an `MPAD` placeholder so record indices stay
stable. Consequences:

- `DETECTOR` and `OBSERVABLE_INCLUDE` must reference **value** records, never
  flag records (`unsupported_circuit`, "detectors and observables must
  reference value bits, not loss flags").
- Each readout must have at least one `LOSS` site on that wire since the last
  reset (`unsupported_circuit`, "no LOSS opportunity since reset").
- A readout without reset (`ML`/`MZL`) is **terminal** for its wire: no
  subsequent instruction may target that wire (`unsupported_circuit`,
  "ML must be terminal for each measured physical wire").

## 5. Detector error model requirements

The kept-for-analysis circuit (loss structure removed, noiseless variant used
for reference samples) must yield a DEM satisfying:

- every detector carries at least `x, y, t` coordinates;
- every error probability is finite and `< 0.5` (zero-probability errors are
  ignored);
- after DEM decomposition, every error component touches at most two
  detectors (graphlike). Non-graphlike remnants are rejected
  (`unsupported_circuit`, "non-graphlike Pauli effect");
- observable-only error components are rejected for the matching backend
  ("observable-only Pauli effects are unsupported by envelope-matching");
- at least one decodable effect and one graph edge exist.

Edges are classified for diagnostics: same `(x, y)` → time-like, otherwise
space-like, single-detector → boundary.

## 6. Resource limits

| Limit | Value | Error on exceed |
|---|---|---|
| Envelope candidates per loss measurement | 100 000 | `unsupported_circuit` ("exceeds candidate limit") |
| Primitive loss probes | 10 000 | `unsupported_circuit` ("exceeds primitive probe limit") |
| Measurements / detectors | 10 000 000 each | layout error |
| Observables | min(64, 1 000 000) | `unsupported_circuit` |
| Parity terms in measurement transforms | 100 000 000 | layout error |
| Transform working memory | 512 MiB transform / 256 MiB block | layout error |

A primitive loss probe that fans out into more than one DEM mechanism is
rejected ("a primitive loss probe produced multiple DEM mechanisms").

## 7. Dataset bundle contract (consumer side)

`rustqec decode` accepts a directory containing exactly:

- `manifest.json` — format `rstim_decoder_dataset` schema v1, or
  `qude_decoder_dataset` schema v3 (Decoder-Server interchange);
- `circuit.stim` — a v1-subset circuit;
- `shots.b8` — `lsb_first` bit-packed measurement rows, width equal to the
  circuit's measurement count, zero padding.

SHA-256 hashes of both files, row widths, shot count, file size, and (for the
rstim format) the derived `dataset_id` are verified before compilation.
Violations report `missing_dataset_file`, `invalid_dataset`, or
`unsupported_dataset_mode`. Decode-time failures report `decode_timeout` or
`decode_infeasible` (exit code 3); all other failures exit 2.

## 8. Relationship to built-in generators

`rustqec circuit gen --code surface_code --task rotated_memory_z_midswap`
emits circuits inside this subset, and the decode regression suite replays
them. **Membership in the subset is the contract, not the generator.** The
compiler performs no generator identification: any circuit satisfying §2–§6 is
accepted, whether hand-written, generated by this repository, or produced by
third-party tooling. Conformance fixtures that bypass the built-in generators
live under `rustqec-cli/tests/` and are added together with this
specification's rollout.

## 9. Error-code summary

| Code | Meaning | Exit |
|---|---|---|
| `unsupported_circuit` | circuit violates §2–§6 | 2 |
| `invalid_dataset` | manifest/hash/layout mismatch | 2 |
| `missing_dataset_file` | bundle file absent or unreadable | 2 |
| `unsupported_dataset_mode` | dataset is not `measurements_blinded` | 2 |
| `decode_timeout` | per-shot time limit hit | 3 |
| `decode_infeasible` | model infeasible for a shot | 3 |
