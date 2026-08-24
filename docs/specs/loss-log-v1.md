# Loss-Log Sidecar v1

Status: **stable contract** (versioned). Companion to
`loss-visible-circuit-subset-v1.md`.

`rustqec dataset import --loss-log <path>` accepts a JSON sidecar in which an
external producer declares, per shot, which loss-visible readouts heralded a
loss. The importer cross-checks the declaration against the flag bits in the
shot payload and rejects drift with `loss_log_mismatch` (exit 2).

## Schema: `rustqec.loss-log.v1`

```json
{
  "schema_version": "rustqec.loss-log.v1",
  "shots": [[], [0], [0, 3], []]
}
```

- `schema_version` — exactly `"rustqec.loss-log.v1"`.
- `shots` — one entry per shot in the payload, in shot order. The array
  length must equal the shot count.
- Each entry is a list of **loss-visible readout ordinals**: the i-th
  loss-visible measurement (`ML`, `MZL`, `MRL`, `MRZL`) in circuit order has
  ordinal i. Ordinals are not measurement-record indices; the importer
  translates them through the compiled flag layout.

A shot entry must equal the set of ordinals whose flag record is 1 in the
packed shot row. Extra, missing, or misordered entries fail the import and
nothing is published.

## Why a sidecar instead of flag bits alone

Real loss sources (experiment control software, third-party simulators) emit
loss events in their own coordinates. The sidecar lets a producer state its
claims explicitly, so a packaging bug that drops or invents a loss is caught
at import time rather than silently changing the decoding problem.
