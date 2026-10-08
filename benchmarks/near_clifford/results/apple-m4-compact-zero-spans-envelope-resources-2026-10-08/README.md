# CLI operating-envelope regression evidence

This independent 19-case full CLI campaign binds clean measured runtime source
`d989531d837fe874a77ac05745f5d77ea5639d52`. It retains 15 successful workload
cases, four expected failures and all raw measurements. See [report.md](report.md).
CLI wall sum is 44.068 s, whole campaign is 57.470 s including setup, and peak
process RSS watermark is 212041728 bytes. This is an envelope-decoder CLI regression
check, not near-Clifford peer throughput or comparative engine-memory evidence.
The measured CLI binary remains in ignored evidence; portable replay validates
historical Git inputs and recorded digests, not a current rebuilt executable.

```sh
python3 -m benchmarks.atom_loss.readiness_resources --verify benchmarks/near_clifford/results/apple-m4-compact-zero-spans-envelope-resources-2026-10-08/manifest.json --historical-source
python3 -O -m benchmarks.atom_loss.readiness_resources --verify benchmarks/near_clifford/results/apple-m4-compact-zero-spans-envelope-resources-2026-10-08/manifest.json --historical-source
```

PR #792 replaced the default [CLI resource archive](../../../atom_loss/readiness/resources/manifest.json)
with a fresh 19-case campaign measured at
`bacdcfa3a0c86619087a0f30944f08e2c4527974`, binding 201 source inputs. The default
CI verifier checks current source identity without `--historical-source`.
This archive preserves the unchanged historical `d989531d837fe874a77ac05745f5d77ea5639d52`
campaign and its 200 source inputs; use the historical-source commands above for it.

```sh
python3 -m benchmarks.atom_loss.readiness_resources --verify benchmarks/atom_loss/readiness/resources/manifest.json
python3 -O -m benchmarks.atom_loss.readiness_resources --verify benchmarks/atom_loss/readiness/resources/manifest.json
```
