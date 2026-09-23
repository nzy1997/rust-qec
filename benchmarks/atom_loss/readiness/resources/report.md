# Envelope decoder operating envelope

Measured 2026-09-23T05:23:11Z on `macOS-27.0-arm64-arm-64bit` (arm64, 10 CPUs, 32.00 GiB).
Binary `rustqec 0.3.3` sha256 `a386a97ce6f76368…`.

These are workload- and machine-specific measurements, not universal latency guarantees. The stress budget was declared before the run: per-case wall ≤ 900 s, total ≤ 3600 s, peak RSS ≤ 4.00 GiB.

| Case | Decoder | Kind | Loss | Shots | Wall (s) | Compile (s) | Decode (s) | Patterns | Cache builds | Cache hits | Eviction rebuilds | Peak RSS watermark |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| matching-d3r2-p002-b1024 | envelope-matching | batch-scale | 0.002 | 1024 | 0.01 | 0.001 | 0.00 | 49 | 49 | 975 | 0 | 7.5 MiB |
| matching-d3r2-p002-b16384 | envelope-matching | batch-scale | 0.002 | 16384 | 0.03 | 0.001 | 0.02 | 245 | 245 | 16139 | 0 | 13.2 MiB |
| matching-d3r2-p002-b65536 | envelope-matching | batch-scale | 0.002 | 65536 | 0.08 | 0.001 | 0.07 | 433 | 433 | 65103 | 0 | 17.3 MiB |
| matching-d3r2-p020-b1024 | envelope-matching | batch-scale | 0.02 | 1024 | 0.02 | 0.001 | 0.01 | 598 | 598 | 426 | 0 | 22.4 MiB |
| matching-d3r2-p020-b16384 | envelope-matching | batch-scale | 0.02 | 16384 | 0.20 | 0.001 | 0.18 | 4684 | 7219 | 9165 | 2535 | 41.8 MiB |
| matching-d3r2-p020-b65536 | envelope-matching | batch-scale | 0.02 | 65536 | 3.22 | 0.001 | 3.20 | 11669 | 28107 | 37429 | 16438 | 41.8 MiB |
| matching-d3r2-p100-b1024 | envelope-matching | batch-scale | 0.1 | 1024 | 0.07 | 0.002 | 0.05 | 1024 | 1024 | 0 | 0 | 41.8 MiB |
| matching-d3r2-p100-b16384 | envelope-matching | batch-scale | 0.1 | 16384 | 1.56 | 0.001 | 1.54 | 16244 | 16381 | 3 | 137 | 43.2 MiB |
| matching-d3r2-p100-b65536 | envelope-matching | batch-scale | 0.1 | 65536 | 6.35 | 0.002 | 6.32 | 65536 | 65519 | 17 | 0 | 43.2 MiB |
| matching-d3r3-p020-b16384 | envelope-matching | circuit-scale | 0.02 | 16384 | 1.50 | 0.004 | 1.44 | 9059 | 12326 | 4058 | 3267 | 88.1 MiB |
| matching-d5r3-p020-b16384 | envelope-matching | circuit-scale | 0.02 | 16384 | 9.71 | 0.011 | 9.54 | 16314 | 16384 | 0 | 70 | 235.1 MiB |
| matching-eviction-wires24 | envelope-matching | cache-eviction | synthetic | 1601 | 0.05 | 0.001 | 0.02 | 1401 | 1402 | 199 | 1 | 235.1 MiB |
| mle-d3r2-p002-b1024 | envelope-mle | repeated-patterns | 0.002 | 1024 | 12.30 | 0.004 | 12.28 | 45 | 45 | 979 | 0 | 235.1 MiB |
| mle-d3r2-p002-b16384 | envelope-mle | repeated-patterns | 0.002 | 16384 | 51.36 | 0.005 | 51.33 | 267 | 267 | 16117 | 0 | 235.1 MiB |
| mle-eviction-wires24 | envelope-mle | cache-eviction | synthetic | 1601 | 1.23 | 0.000 | 1.20 | 1401 | 1402 | 199 | 1 | 235.1 MiB |

## Failure semantics (tested against the real CLI)

| Case | Decoder | Exit | Error code | Stats written | Predictions |
| --- | --- | --- | --- | --- | --- |
| fail-mle-candidate-limit | envelope-mle | 2 | unsupported_circuit | no | none installed |
| fail-mle-solve-timeout | envelope-mle | 3 | decode_timeout | yes | none installed |
| fail-mle-infeasible | envelope-mle | 3 | decode_infeasible | yes | none installed |
| fail-stale-output-overwrite | envelope-matching | 2 | output_error | no | none installed |

## Recommended operating ranges (this machine, this workload)

### envelope-matching
- Shots per batch: ≤ 65536 measured.
- Loss rate: ≤ 0.1 measured.
- decode stays near-linear in shots at fixed pattern count; cache eviction rebuilds were measured and remained within budget.

### envelope-mle
- Shots per batch: ≤ 16384 measured.
- Loss rate: ≤ 0.002 measured.
- cost is dominated by ILP build+solve per distinct loss pattern; use --shot-timeout-ms so a slow pattern stops the batch with decode_timeout instead of running unbounded.

### Exact failure semantics
- Compilation rejection (`unsupported_circuit`, exit 2): neither predictions nor statistics are published; compilation is outside `--shot-timeout-ms`.
- MLE solve timeout (`decode_timeout`, exit 3): diagnostic statistics are written (including `compile_seconds`, `attempted_shot_count`, `timeout_count=1`) and no prediction file is published; completed shots count as zero.
- MLE infeasible shot (`decode_infeasible`, exit 3): same output rule as timeout, with `infeasible_shot_count=1`.
- Pre-existing outputs: the CLI refuses to overwrite (`output_error`, exit 2) and leaves stale files byte-identical; never read a stale prediction as this run’s success. Counts after timeout/infeasible are attempts, never completed shots.

## Exclusions

- mle-d3r2-p020/p100-batches: ILP model build+solve per distinct loss pattern (~1.4e-1 s each on the reference machine) times tens of thousands of patterns exceeds the declared stress budget; no timing is invented for these omitted points
- mle-scale-beyond-d3r2-real-circuits: larger real circuits widen the ILP model per pattern; only the synthetic 24-wire eviction corpus is measured at scale for MLE in this campaign
- matching-batches-above-65536: not measured in this campaign; the recommended range stops at the largest measured successful batch

## Hard code limits (separate from measurements)

- `max_envelope_candidates` = 100000
- `max_primitive_probes` = 100000
- `max_primitive_symptom_terms` = 10000000
- `max_conditioned_decoder_artifacts` = 1024
- `observables` = 1..=64
- `sweep_bits` = 0
