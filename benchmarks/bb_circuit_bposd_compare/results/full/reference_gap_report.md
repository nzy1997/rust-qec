# BB72/BB144 Circuit BP-OSD Reference-Gap Report

## Source Contract

- Bravyi contract commit: `fa77e3333d3ec44c79d8f914dd24c040d1da471b`
- Upstream repository: `sbravyi/BivariateBicycleCodes`
- Source-backed contract URLs:
- https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/README.md#L16-L21
- https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_setup.py#L511-L618
- https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L67-L72
- https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L329-L349
- https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L364-L415

## Audit Status

| Check | Status | Evidence |
| --- | --- | --- |
| Bravyi contract audit | PASS | `verify_bravyi_contract` accepted commit `fa77e3333d3ec44c79d8f914dd24c040d1da471b`. |
| Bravyi LER audit | PASS | `verify_bravyi_ler` accepted 16 rows. |
| Batched accounting audit | PASS | `verify_batched_accounting` accepted 8 paired groups. |
| Bravyi model audit | PASS - #308 checked in `reference/bravyi_model_audit_bb72_p003_c6.json` and `verify_model_audit` verifies fresh audit artifacts against that fixture. | BB72 effective-model audit remains the #308 gate. |
| Hard replay parity | PASS - #307 fixed the pinned BB90 hard-replay parity; `verify_replay` and `verify_replay_trace` remain the gates for regenerated replay artifacts. | BB90 hard replay remains the #306/#307 gate. |

## Regeneration Evidence

- Full results CSV: `benchmarks/bb_circuit_bposd_compare/results/full/results.csv`
- Full results rows: 16
- Paired comparison groups: 8
- Full CSV treatment: fresh full paired rerun completed for the checked-in benchmark evidence.
- Controlled rerun artifact: `benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv` with 16 rows and 8 paired groups.
- Controlled command: `MPLCONFIGDIR=/tmp/rstim-mplconfig /private/tmp/rstim-ldpc-venv/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/controlled --rust-binary target/release/rsinter --batch-size 10`
- Python environment: `/private/tmp/rstim-ldpc-venv/bin/python (ldpc 2.4.1, bposd 2.1, numpy 2.5.0)`
- Rust binary: `target/release/rsinter`
- Rust source commit: `cc6ee302523c0810bef71ee891eca77fb9396508`

## Per-Row LER Table

| code_id | p | cycles | decoder | shots | logical_errors | LER | status | stop_reason |
| --- | ---: | ---: | --- | ---: | ---: | ---: | --- | --- |
| bb144 | 0.003 | 12 | rbposd | 56000 | 204 | 0.003642857142857143 | ok | errors_budget_reached |
| bb144 | 0.003 | 12 | ldpc_bposd | 56000 | 204 | 0.003642857142857143 | ok | errors_budget_reached |
| bb144 | 0.004 | 12 | rbposd | 5500 | 200 | 0.03636363636363636 | ok | errors_budget_reached |
| bb144 | 0.004 | 12 | ldpc_bposd | 5500 | 200 | 0.03636363636363636 | ok | errors_budget_reached |
| bb144 | 0.005 | 12 | rbposd | 1500 | 272 | 0.18133333333333335 | ok | errors_budget_reached |
| bb144 | 0.005 | 12 | ldpc_bposd | 1500 | 272 | 0.18133333333333335 | ok | errors_budget_reached |
| bb144 | 0.006 | 12 | rbposd | 500 | 238 | 0.476 | ok | errors_budget_reached |
| bb144 | 0.006 | 12 | ldpc_bposd | 500 | 238 | 0.476 | ok | errors_budget_reached |
| bb72 | 0.003 | 6 | rbposd | 8000 | 216 | 0.027 | ok | errors_budget_reached |
| bb72 | 0.003 | 6 | ldpc_bposd | 8000 | 217 | 0.027125 | ok | errors_budget_reached |
| bb72 | 0.004 | 6 | rbposd | 3000 | 233 | 0.07766666666666666 | ok | errors_budget_reached |
| bb72 | 0.004 | 6 | ldpc_bposd | 3000 | 233 | 0.07766666666666666 | ok | errors_budget_reached |
| bb72 | 0.005 | 6 | rbposd | 1500 | 300 | 0.2 | ok | errors_budget_reached |
| bb72 | 0.005 | 6 | ldpc_bposd | 1500 | 300 | 0.2 | ok | errors_budget_reached |
| bb72 | 0.006 | 6 | rbposd | 1000 | 384 | 0.384 | ok | errors_budget_reached |
| bb72 | 0.006 | 6 | ldpc_bposd | 1000 | 383 | 0.383 | ok | errors_budget_reached |

## Rust/Python Delta Table

| code_id | p | cycles | Rust LER | Python LER | Rust-Python delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| bb144 | 0.003 | 12 | 0.003642857142857143 | 0.003642857142857143 | 0 |
| bb144 | 0.004 | 12 | 0.03636363636363636 | 0.03636363636363636 | 0 |
| bb144 | 0.005 | 12 | 0.18133333333333335 | 0.18133333333333335 | 0 |
| bb144 | 0.006 | 12 | 0.476 | 0.476 | 0 |
| bb72 | 0.003 | 6 | 0.027 | 0.027125 | -0.000125 |
| bb72 | 0.004 | 6 | 0.07766666666666666 | 0.07766666666666666 | 0 |
| bb72 | 0.005 | 6 | 0.2 | 0.2 | 0 |
| bb72 | 0.006 | 6 | 0.384 | 0.383 | 0.001 |

## Final Verdict For #303

**Final verdict for #303:** Implementation checks pass on the current artifacts, but the checked-in BB72/BB144 full rows are not directly comparable to the paper/reference target. The checked-in full rows are batched, error-budget-stopped comparison rows rather than a fresh fixed-shot reproduction of the pinned Bravyi curve, and the controlled rerun is intentionally smoke-sized evidence that the post-#307 path still executes paired Rust/Python rows. No specific remaining implementation gap is identified by this report.
