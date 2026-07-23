# rsmp v1 Compression Evidence

Verdict: PASS

## Gates
- benchmark_raw_lt_20pct: PASS (930140 < 1552384; denominator=benchmark_raw_b8_bytes)
- benchmark_zstd_lt_75pct: PASS (744112 < 976695; denominator=benchmark_direct_zstd_bytes)
- high_entropy_raw_le_102pct: PASS (52453200 <= 53477376; denominator=high_entropy_raw_b8_bytes)

## Byte Counts
- Benchmark archive/raw: 186028 / 1552384 (11.98%).
- Benchmark archive/direct-zstd: 186028 / 325565 (57.14%).
- High-entropy archive/raw: 1049064 / 1048576 (100.04%).
- High-entropy direct Zstandard bytes: 1048616 (diagnostic only, not an acceptance denominator).

## Cases
| case | role | shots | M | D | L | rank | free | raw b8 | direct zstd | archive | syndrome codecs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| nonzero_reference | nonzero_reference | 4 | 1 | 1 | 0 | 1 | 0 | 4 | 17 | 338 | syndrome_dense_v1 |
| rank_zero | rank_zero | 4 | 1 | 0 | 0 | 0 | 1 | 4 | 17 | 338 | empty |
| dependent_detectors | dependent_detectors | 4 | 1 | 2 | 0 | 1 | 0 | 4 | 17 | 338 | syndrome_dense_v1 |
| repeat_records | repeat_records | 4 | 2 | 2 | 0 | 2 | 0 | 4 | 17 | 338 | syndrome_dense_v1 |
| observable_recovery | observable_recovery | 4 | 1 | 0 | 1 | 0 | 1 | 4 | 17 | 338 | empty |
| loss_visible_measurements | loss_visible_measurements | 4 | 2 | 2 | 0 | 2 | 0 | 4 | 17 | 338 | syndrome_dense_v1 |
| stim_surface_d11_r100 | surface_d11_r100 | 1024 | 12121 | 12000 | 1 | 12000 | 121 | 1552384 | 325565 | 186028 | syndrome_sparse_leb128_v1 |
| high_entropy_control | high_entropy_control | 8192 | 1024 | 0 | 0 | 0 | 1024 | 1048576 | 1048616 | 1049064 | empty,empty |

## Environment
The exact producer, Git state, Rust target, Cargo.lock hash, zstd package versions, and complete command argv values are recorded in environment.json. This report is rendered from raw.jsonl-derived counts and gate arithmetic.

## Throughput Observations
- nonzero_reference: encode 518 B/s, decode 545 B/s, peak logical block working set 8388639 bytes.
- rank_zero: encode 1253 B/s, decode 986 B/s, peak logical block working set 8388639 bytes.
- dependent_detectors: encode 1095 B/s, decode 885 B/s, peak logical block working set 8388639 bytes.
- repeat_records: encode 1121 B/s, decode 1098 B/s, peak logical block working set 8388655 bytes.
- observable_recovery: encode 1410 B/s, decode 1248 B/s, peak logical block working set 8388639 bytes.
- loss_visible_measurements: encode 1387 B/s, decode 1315 B/s, peak logical block working set 8388655 bytes.
- stim_surface_d11_r100: encode 15443821 B/s, decode 37218025 B/s, peak logical block working set 11900649 bytes.
- high_entropy_control: encode 11398829 B/s, decode 24977742 B/s, peak logical block working set 10485788 bytes.

## Claim Limitations
- These gates prove the pinned `rsmp v1` evidence cases under the recorded producer and zstd settings.
- Direct Zstandard for the high-entropy control is reported only as a diagnostic; the acceptance denominator is raw b8 bytes.
- No fixed wall-clock performance gate or cross-version byte-for-byte writer determinism is claimed.
