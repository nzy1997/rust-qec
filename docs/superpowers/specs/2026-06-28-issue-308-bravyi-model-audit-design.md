# Issue #308 Bravyi BB Effective Model Audit Design

Date: 2026-06-28
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #308, source-backed BB effective decoder model audit

## Context

Issue #308 depends on the Bravyi source contract added for issue #305. The
checked-in contract pins `sbravyi/BivariateBicycleCodes` at commit
`fa77e3333d3ec44c79d8f914dd24c040d1da471b` and records the upstream decoder
settings, trial-level failure predicate, and the `num_cycles + 2` noiseless
tail convention from `decoder_setup.py`.

The Rust BB path in `rsinter/src/bb_circuit_memory.rs` already builds the
effective decoder models through `build_effective_models()`, groups identical
augmented syndrome/logical columns in a `BTreeMap`, appends logical rows at
`first_logical_row = n2 * (num_cycles + BRAVYI_NOISELESS_TAIL_CYCLES)`, and
exports comparison models only as part of simulation exports. The issue asks
for a deterministic, cheap audit for BB72 at `p=0.003`, `num_cycles=6` that
does not run Monte Carlo trials.

The upstream Python dependencies `ldpc` and `bposd` are not installed in this
sandbox, and network installation is blocked. The design therefore uses the
pinned #305 contract plus a checked-in BB72 expected fixture with provenance and
model hashes rather than executing upstream Python during CI-style tests.

## Goals

- Add a Python audit command:
  `python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit`.
- Add a Python verifier command:
  `python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit`.
- Add a model-only Rust JSON export path so the audit can inspect effective
  models without sampling or decoding trials.
- Add a checked-in BB72 expected fixture for `code_id=bb72`,
  `physical_error_rate=0.003`, and `num_cycles=6`.
- Validate shape, schedule labels/counts, two noiseless tail cycles,
  `first_logical_row`, decoder dimensions, grouped column hashes, and grouped
  probability evidence.
- Include negative controls that prove verifier failures name tail-cycle and
  schedule/model drift.

## Non-Goals

- Do not fix any mismatch found by the audit.
- Do not run BB72 or BB144 Monte Carlo comparisons.
- Do not support BB108 or BB288 Rust constructors.
- Do not vendor or install the upstream Bravyi Python dependency stack.
- Do not change plot styling or benchmark result schemas unrelated to the
  audit artifact.

## Approach Options

### Recommended: Model-Only Rust Export Plus Python Audit Fixture

Add a `--json-model-audit` mode to the existing `rsinter bb-circuit-bposd-memory`
CLI. This mode builds the code, syndrome cycle, and effective models, then
serializes code shape, schedule metadata, tail-cycle convention, and the two
models. A Python audit wrapper runs that export, reduces large matrices to
stable hashes and probability totals, compares the result to a checked-in BB72
fixture, and writes a compact audit JSON.

This is the best fit because it is deterministic, cheap, source-backed through
the #305 Bravyi contract, and keeps large model payloads out of committed audit
artifacts.

### Alternative: Reuse `--json-compare-case`

The existing export already contains `z_model` and `x_model`, but it also runs
sampling and decoding. That violates the issue's "do not run Monte Carlo
trials" recommendation and makes the audit slower and less clearly scoped.

### Alternative: Execute Upstream `decoder_setup.py`

Running the pinned upstream setup would be the strongest direct comparison, but
this environment lacks `ldpc` and `bposd`, and the setup path is heavier than
the issue's CI-style audit requirement. This remains a future option if the
upstream dependency stack is intentionally provisioned.

## Design

### Rust Model Export

`rsinter::bb_circuit_memory` gains
`export_bravyi_model_audit_for_code(code_id, config)`. It validates the model
configuration, builds the BB code and syndrome cycle, builds effective models,
and returns a serializable export with:

- `code_id`, `physical_error_rate`, and configured `num_cycles`;
- `noiseless_tail_cycles = BRAVYI_NOISELESS_TAIL_CYCLES`;
- `num_cycles_plus_tail`;
- code shape: `ell`, `m`, `n2`, `n`, `k`, X/Z check counts, and data qubit
  count;
- schedule labels and operation counts by kind;
- `z_model` and `x_model` in the same shape as existing comparison model
  exports.

The new CLI flag is mutually exclusive in behavior with `--json-compare-case`.
When set, it writes the model audit export and does not build decoders, sample
trials, or decode trials.

### Python Audit Command

`bravyi_model_audit.py` accepts:

```bash
--code-id bb72
--physical-error-rate 0.003
--num-cycles 6
--out /tmp/rstim-bb-model-audit/model_audit.json
```

Optional `--rust-binary` mirrors existing benchmark wrappers. Without it, the
command runs:

```bash
cargo run -q -p rsinter --bin rsinter -- bb-circuit-bposd-memory \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --num-trials 1 \
  --max-bp-iterations 10000 \
  --osd-order 7 \
  --json-model-audit
```

`num_trials` is passed only because the existing CLI argument shape includes
it; the Rust model-audit path ignores it after model validation. The audit
normalizes the Rust export into:

- input and upstream provenance from `bravyi_contract.json`;
- observed code shape and schedule evidence;
- `syndrome_tail` with configured cycles, tail cycles, and
  `num_cycles_plus_tail`;
- per-basis model summaries with decoder rows/columns, `first_logical_row`,
  sparse-row hash, augmented-column hash, channel-probability hash, grouped
  probability total, and min/max probabilities;
- expected fixture summary, mismatch list, and top-level `status`.

### Expected Fixture

`benchmarks/bb_circuit_bposd_compare/reference/bravyi_model_audit_bb72_p003_c6.json`
stores the expected normalized summary for the BB72 audit point. It includes
the Bravyi commit, issue #305 contract version, source URLs, input parameters,
schedule labels/counts, tail-cycle convention, first logical rows, decoder
dimensions, and grouped model hashes/probability totals.

The fixture intentionally stores hashes and counts, not full model matrices.
This keeps review noise low while still catching drift in syndrome histories,
logical-row offsets, grouping, schedule conventions, and channel probabilities.

### Verifier

`verify_model_audit.py` loads an audit artifact and independently compares it
to the checked-in fixture. It rejects:

- unsupported audit version or status not equal to `pass`;
- wrong Bravyi upstream commit or contract version;
- input, code-shape, schedule label/count, or tail-cycle mismatches;
- X/Z `first_logical_row`, decoder rows/columns, grouped column count, hashes,
  or probability-total mismatches.

On success it prints a PASS line naming BB72 shape, schedule count,
`num_cycles_plus_tail=8`, X/Z `first_logical_row`, decoder dimensions, and
grouped probability evidence. On failure it prints each mismatch to stderr and
exits nonzero. The negative control changes either `syndrome_tail` or schedule
evidence and must fail with the changed field named.

### Tests

`benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py` covers:

- audit summary construction from a fake Rust export;
- CLI writing an audit JSON while the Rust export call is mocked;
- verifier success on a generated-good audit;
- verifier negative controls for tail-cycle and schedule/model drift;
- Rust command argument construction for the model-only export.

Rust unit tests cover the model-only export on a tiny BB72 configuration and
assert that it reports `num_cycles + 2`, expected first logical rows, and no
sample/decode profile.

## Verification

Required commands:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_model_audit.py
python3 -m benchmarks.bb_circuit_bposd_compare.bravyi_model_audit \
  --code-id bb72 \
  --physical-error-rate 0.003 \
  --num-cycles 6 \
  --out /tmp/rstim-bb-model-audit/model_audit.json
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/rstim-bb-model-audit/model_audit.json
cargo test
```

Negative control:

```bash
cp /tmp/rstim-bb-model-audit/model_audit.json /tmp/model_audit_bad.json
python3 - <<'PY'
import json
from pathlib import Path
path = Path("/tmp/model_audit_bad.json")
data = json.loads(path.read_text())
data["observed"]["syndrome_tail"]["noiseless_tail_cycles"] = 1
path.write_text(json.dumps(data, indent=2) + "\n")
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_model_audit \
  /tmp/model_audit_bad.json
```

Expected: verifier exits nonzero and names the tail-cycle mismatch.

## Approval

The run is non-interactive. The standing answer policy chooses the recommended
model-only Rust export plus Python audit fixture because it satisfies the
issue's deterministic audit interface while avoiding unavailable upstream
Python dependencies and Monte Carlo work.
