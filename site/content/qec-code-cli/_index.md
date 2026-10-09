+++
title = "qec-code CLI"
template = "cli-help.html"
[extra]
root = ".."
nav = "qec-cli"
cli_key = "qec_code"
+++
Use `qec-code` to construct and inspect codes, export CSS check matrices, and search for logical operators. Circuit generation and sampling belong to `rstim`.

## Inputs and distance results

`code css export` writes sparse-row JSON matrices. Custom inputs to `code css-distance` take `--hx` and `--hz` files; each row specifies the nonzero column indices. See the [CSS experiment](../css-codes/) for a complete example. Matrix dimensions and commuting X/Z checks determine whether these are valid stabilizers.

The built-in small-code `distance` command is an exact search. `randomized-upper-bound` produces a witness and an upper bound; it does not prove the absence of lighter logical operators. `exact` requires a configured solver for solver-backed inputs. A timeout or an incomplete search is not a certified distance.

## Optional solvers

`distance-ilp-highs` enables the open-source native solver; `distance-ilp-gurobi` additionally requires an installed licensed Gurobi environment. The [installation section](../get-started/#install) centralizes optional builds. Inspect the selected command's help for backend selection and output options.

## Output and failures

Use `--json` where offered for status, bound type, witness, options and provenance. Successful commands exit 0; parse, input or solver errors exit nonzero and print diagnostics to stderr. Do not assume `rstim`'s JSON error-envelope contract applies to this executable.
