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
- Full CSV treatment: preserved because the full paired rerun is too expensive for this PR.
- Controlled rerun artifact: `benchmarks/bb_circuit_bposd_compare/results/controlled/results.csv` with 16 rows and 8 paired groups.
- Controlled command: `MPLCONFIGDIR=/tmp/rstim-mplconfig /private/tmp/rstim-ldpc-venv/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke --output-dir benchmarks/bb_circuit_bposd_compare/results/controlled --rust-binary target/release/rsinter --batch-size 10`
- Python environment: `/private/tmp/rstim-ldpc-venv/bin/python (ldpc 2.4.1, bposd 2.1, numpy 2.5.0)`
- Rust binary: `target/release/rsinter`
- Rust source commit: `1d2df4bc97f63f4f87308b54a05c6bb8e06a5067`

## Per-Row LER Table

| code_id | p | cycles | decoder | shots | logical_errors | LER | status | stop_reason |
| --- | ---: | ---: | --- | ---: | ---: | ---: | --- | --- |
| bb144 | 0.003 | 12 | rbposd | 40000 | 200 | 0.005 | ok | errors_budget_reached |
| bb144 | 0.003 | 12 | ldpc_bposd | 40000 | 138 | 0.00345 | ok | errors_budget_reached |
| bb144 | 0.004 | 12 | rbposd | 4500 | 209 | 0.04644444444444444 | ok | errors_budget_reached |
| bb144 | 0.004 | 12 | ldpc_bposd | 4500 | 159 | 0.035333333333333335 | ok | errors_budget_reached |
| bb144 | 0.005 | 12 | rbposd | 1000 | 224 | 0.224 | ok | errors_budget_reached |
| bb144 | 0.005 | 12 | ldpc_bposd | 1000 | 183 | 0.183 | ok | errors_budget_reached |
| bb144 | 0.006 | 12 | rbposd | 500 | 272 | 0.544 | ok | errors_budget_reached |
| bb144 | 0.006 | 12 | ldpc_bposd | 500 | 246 | 0.492 | ok | errors_budget_reached |
| bb72 | 0.003 | 6 | rbposd | 7000 | 201 | 0.028714285714285713 | ok | errors_budget_reached |
| bb72 | 0.003 | 6 | ldpc_bposd | 7000 | 182 | 0.026 | ok | errors_budget_reached |
| bb72 | 0.004 | 6 | rbposd | 2500 | 212 | 0.0848 | ok | errors_budget_reached |
| bb72 | 0.004 | 6 | ldpc_bposd | 2500 | 194 | 0.0776 | ok | errors_budget_reached |
| bb72 | 0.005 | 6 | rbposd | 1000 | 214 | 0.214 | ok | errors_budget_reached |
| bb72 | 0.005 | 6 | ldpc_bposd | 1000 | 205 | 0.205 | ok | errors_budget_reached |
| bb72 | 0.006 | 6 | rbposd | 1000 | 405 | 0.405 | ok | errors_budget_reached |
| bb72 | 0.006 | 6 | ldpc_bposd | 1000 | 390 | 0.39 | ok | errors_budget_reached |

## Rust/Python Delta Table

| code_id | p | cycles | Rust LER | Python LER | Rust-Python delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| bb144 | 0.003 | 12 | 0.005 | 0.00345 | 0.00155 |
| bb144 | 0.004 | 12 | 0.04644444444444444 | 0.035333333333333335 | 0.011111111111111105 |
| bb144 | 0.005 | 12 | 0.224 | 0.183 | 0.041 |
| bb144 | 0.006 | 12 | 0.544 | 0.492 | 0.052 |
| bb72 | 0.003 | 6 | 0.028714285714285713 | 0.026 | 0.002714285714285713 |
| bb72 | 0.004 | 6 | 0.0848 | 0.0776 | 0.0072 |
| bb72 | 0.005 | 6 | 0.214 | 0.205 | 0.009 |
| bb72 | 0.006 | 6 | 0.405 | 0.39 | 0.015 |

## Final Verdict For #303

**Final verdict for #303:** Implementation checks pass on the current artifacts, but the preserved BB72/BB144 full run is not directly comparable to the paper/reference target. The checked-in full rows are batched, error-budget-stopped comparison rows rather than a fresh fixed-shot reproduction of the pinned Bravyi curve, and the controlled rerun is intentionally smoke-sized evidence that the post-#307 path still executes paired Rust/Python rows. No specific remaining implementation gap is identified by this report.
