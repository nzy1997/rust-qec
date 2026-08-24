# Error-Trace Sidecar v1

Status: **stable contract** (versioned). Companion to the decoder-dataset
export (`rustqec dataset export` / `rstim export_decoder_dataset`).

`rustqec dataset export --error-trace` writes an additional file
`trace.jsonl` into the **private** bundle. It records, for every shot, the
complete noise realization that produced the shot: every Pauli error branch
that fired and every atom-loss onset, in execution order. Together with the
public `shots.b8` and the private `answers.b8` / `masks.b8`, this makes the
dataset a fully labeled training set for learned decoders — the model sees
the syndrome, the trainer sees exactly which physical errors caused it.

The trace is **private** because it reveals the ground-truth error process
per shot. Nothing about the public bundle changes when the flag is on.

## File: `trace.jsonl`

One JSON object per line, one line per shot, in shot order. The number of
lines equals the `shots` count in both manifests.

```json
{"schema_version":"rstim.error-trace.v1","shot":1,"events":[{"op":"X_ERROR","targets":[0],"branch":"X","path":[1],"iterations":[]},{"op":"LOSS","targets":[0],"branch":"L","path":[2],"iterations":[]}]}
```

- `schema_version` — exactly `"rstim.error-trace.v1"`, repeated on every
  line so each line is self-describing.
- `shot` — zero-based global shot index, matching the shot order of
  `shots.b8`, `answers.b8`, and `masks.b8`. Export writes shots in bounded
  batches (`--batch_shots`); trace line indices are global, not per-batch.
- `events` — the noise events that **occurred** in this shot, in execution
  order. Events that did not occur are implicit (absent). An empty array
  means a noiseless shot.

Each event:

- `op` — the noise instruction name as written in the circuit
  (`X_ERROR`, `DEPOLARIZE1`, `PAULI_CHANNEL_2`, `CORRELATED_ERROR`,
  `ELSE_CORRELATED_ERROR`, `LOSS`, ...).
- `targets` — the qubit indices the event acted on.
- `branch` — the sampled branch label. For Pauli channels this is the Pauli
  string that was applied (`"X"`, `"Y"`, `"Z"`, `"IX"`, ...). For `LOSS`
  this is exactly `"L"` and marks the **loss onset**: the qubit in
  `targets` is lost from this point until its next reset.
- `path` — instruction path: the index of the instruction within its
  enclosing block, descending through `REPEAT` bodies.
- `iterations` — the `REPEAT` iteration indices, one per enclosing
  `REPEAT` level, so an event inside a repeated syndrome round is pinned to
  its round.

The injected logical flip of `measurements_blinded` mode is a gate, not a
noise event: it never appears in `events`. Its per-shot presence is exactly
the `masks.b8` bit, so the hidden flip remains recoverable without
polluting the physical noise record.

## Manifest entry

When the trace is written, the private `manifest.json` gains a `trace_file`
entry (absent otherwise, so older private manifests still parse):

```json
"trace_file": {
  "file": "trace.jsonl",
  "sha256": "...",
  "schema": "rstim.error-trace.v1",
  "lines": 1000000
}
```

- `sha256` — hex digest of the exact `trace.jsonl` bytes.
- `lines` — equals the dataset shot count.

## Sampling semantics

`--error-trace` switches export from the batch sampler to a per-shot traced
executor. The two paths consume the physical RNG differently, so the same
`--seed` produces a **different** batch with and without the flag. Within a
traced export, `trace.jsonl`, `shots.b8`, `answers.b8`, and `masks.b8` are
guaranteed to describe the same shots, and a fixed seed (with the same
`--batch_shots`) reproduces the bundle byte-for-byte.

Traced export is per-shot simulation and is therefore slower than batch
sampling; use it when the training labels are the point.

## Consistency guarantees (checked by the exporter)

For every shot, in both modes:

- the public `shots.b8` row (detections or blinded measurements) is the
  readout of the same execution recorded in that shot's trace line;
- the private `answers.b8` bit is the observable of that execution with the
  hidden logical flip (if any) removed;
- in `measurements_blinded` mode the executed measurement record equals
  `answer XOR mask` up to the physical noise recorded in the trace.
