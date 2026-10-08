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

The default [CLI resource archive](../../../atom_loss/readiness/resources/manifest.json)
contains the same unchanged 19-case measurements and retains the original producer
revision. Its 200 measured source inputs match the current runtime, so the default
CI verifier checks current source identity without `--historical-source`. The prior
canonical measurements remain available in Git history.

```sh
python3 -m benchmarks.atom_loss.readiness_resources --verify benchmarks/atom_loss/readiness/resources/manifest.json
python3 -O -m benchmarks.atom_loss.readiness_resources --verify benchmarks/atom_loss/readiness/resources/manifest.json
```
