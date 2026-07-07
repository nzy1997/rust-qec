# Issue 391 rstim-vs-Stim Simulator Showcase Design

## Objective

Add `docs/showcases/rstim-vs-stim-simulator.md` as a reviewer-readable evidence
page for the `rstim`-vs-Stim simulator benchmark family. The page must explain
the workload, how to rerun speed and correctness workflows, what evidence files
mean, which code owns the workflows, and where the claim boundary stops.

This is documentation and docs-checker work only. It must not optimize
`rstim`, claim broad performance parity, or require the first checked
performance run to be good or complete.

## Selected Approach

Use the existing showcase page contract and extend the showcase checker with a
targeted contract for this page. The new page will:

- follow the required showcase sections: `What This Shows`, `Run It`,
  `Expected Result`, `Code`, `Verification`, and `Limits`;
- identify the canonical circuit input as the checked Stim-generated fixture
  from issue #385;
- link umbrella issue #38 as historical benchmark context;
- document the correctness verifier from issue #386 and the selected speed
  runner plus summarize/report workflow from issues #389 and #390;
- state that bad, slow, failed, or incomplete performance is still evidence,
  not a documentation failure;
- state that claims apply only to recorded workloads and recorded
  environments.

The checker will keep its existing generic showcase validation and add one
specialized rule set for
`docs/showcases/rstim-vs-stim-simulator.md`: the page must contain the
correctness command entry point
`python3 -m benchmarks.rstim_vs_stim_simulator.verify_correctness` and the
speed command entry point `cargo run -p rstim --bin rstim -- perf run`. The
existing required-section validation already makes removal of `Limits` fail
with `missing required section: Limits`.

This approach is recommended over two alternatives:

- A documentation-only page without checker updates would satisfy the prose
  need but would not give the issue verification command teeth when either the
  speed or correctness command is accidentally removed.
- A broad checker framework for arbitrary command contracts would add
  abstraction before there is a second specialized showcase with the same
  needs. A small path-specific contract matches the current risk and can be
  generalized later if repeated.

## Page Content

`What This Shows` will frame the page as an evidence guide, not an optimization
claim. It will name the Stim-style surface-code sample case
`stim-style-surface-sample-d11-r100-b1024`, the checked fixture catalog, the
statistical correctness workflow, and the speed report workflow.

`Run It` will include exact commands from the repository root:

- fixture catalog validation for `cases.smoke.toml`;
- correctness verification writing `/tmp/rstim-vs-stim-correctness.json`;
- selected speed run writing `/tmp/rstim-vs-stim-speed.jsonl`;
- speed summarization writing `/tmp/rstim-vs-stim-summary.json`;
- report rendering writing `/tmp/rstim-vs-stim-report.md`.

`Expected Result` will describe PASS/WARN/FAIL correctness evidence, JSON
status fields, selected-case raw JSONL records, summary rates, report-only
Stim comparison ratios, and explicit unavailable statuses. It will make clear
that slow, failed, or incomplete performance output is still valid evidence
when recorded honestly.

`Code` will link the fixture manifests, canonical `.stim` fixture,
correctness verifier, tests, perf registry, perf tests, benchmark README, and
issues #38 and #385.

`Verification` will include the docs checker command from the issue and the
negative-control expectations: removing `Limits`, the speed command, or the
correctness command must fail validation.

`Limits` will explicitly bound claims to recorded workloads and recorded
environments only. It will also note that smoke checks are wiring checks, Stim
comparisons are report-only context, the page does not prove generator parity,
and optimization remains out of scope.

## Checker Contract

`tools/check_showcase_docs.py` will gain constants for the page path and the
two command substrings. `validate_showcase_page` will append specialized
errors only when validating that exact page:

```text
missing rstim-vs-Stim correctness command link
missing rstim-vs-Stim speed command link
```

The contract uses direct substring checks instead of parsing shell code blocks
because the page is Markdown prose and the desired behavior is specifically
that the commands remain visible to reviewers. Link validation and section
validation remain unchanged for all showcase pages.

The built-in self-test will add fixture pages that remove each required command
from the specialized page content and assert the corresponding errors. This
keeps the negative controls runnable without mutating the real showcase file.

## Testing

Use TDD for the checker change:

1. Add self-test fixtures proving the specialized page passes only when both
   command entry points are present.
2. Run `python3 tools/check_showcase_docs.py --self-test` and observe failure
   before the checker has the new specialized validation.
3. Implement the smallest checker change that makes the self-test pass.
4. Add the showcase page and README link.
5. Run the issue verification command:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-vs-stim-simulator.md
```

Also run the checker self-test, the full showcase directory check, and
`cargo test` as the repository-level gate required by the Agent Desk task.

## Scope Limits

Do not add benchmark optimization work. Do not generate or commit new benchmark
outputs. Do not claim broad `rstim` performance parity. Do not make
`rstim`-vs-Stim speed a hard CI gate. Do not change the correctness verifier or
perf runner behavior unless the docs checker requires a command-discovery
contract.

## Self-Review

- No placeholder text remains.
- The required showcase sections are explicit.
- The canonical Stim-generated fixture and claim boundary are explicit.
- The design includes both positive verification and the issue's negative
  controls.
- The checker extension is scoped to the issue page and does not affect
  unrelated showcase pages.
