# Issue 100 rbposd Evidence Reconciliation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconcile `rbposd` evidence-facing docs and docs-contract tests with the tracked full-tier benchmark CSV.

**Architecture:** Add a CSV-backed docs contract that computes the paired `ldpc` and `rbposd` native full-tier rows directly from `benchmarks/surface_decoder_compare/results/full/results.csv`. Update the stale `rbposd` core performance design summary with a table derived from those rows and explicit scope language for tracked artifacts, current-speed claims, and remaining alignment gaps after the LSD and BP-option milestones.

**Tech Stack:** Python standard-library `csv` parsing, pytest/unittest tests, Markdown docs, existing Cargo workspace verification.

## Global Constraints

- Do not change decoder implementation code.
- Do not regenerate benchmark artifacts.
- Do not change benchmark plot styling or framework architecture.
- Ground all speed evidence in `benchmarks/surface_decoder_compare/results/full/results.csv`.
- Explicitly distinguish tracked checked-in results from current-speed claims.
- Explicitly distinguish speed evidence from remaining feature-alignment gaps versus upstream `ldpc`.
- The required verification commands are `python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py`, `python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py -k stale`, and `cargo test`.

---

## File Structure

- Modify `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`: add CSV-backed evidence checks and a stale-claim negative control.
- Modify `docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md`: replace the stale summary benchmark gap claim with a tracked-evidence update and post-LSD/BP-option alignment notes.

### Task 1: Add CSV-Backed Docs Contract Tests

**Files:**
- Modify: `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`

**Interfaces:**
- Consumes: tracked CSV rows from `benchmarks/surface_decoder_compare/results/full/results.csv`.
- Produces: unittest methods `test_rbposd_performance_doc_matches_tracked_full_csv` and `test_stale_rbposd_slower_claim_is_rejected`.

- [ ] **Step 1: Replace the docs contract test file with the expanded contract**

Use this exact file content:

```python
import csv
import unittest
from dataclasses import dataclass
from pathlib import Path


README_PATH = Path("benchmarks/surface_decoder_compare/README.md")
MAKEFILE_PATH = Path("Makefile")
PERFORMANCE_DOC_PATH = Path(
    "docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md"
)
FULL_RESULTS_PATH = Path("benchmarks/surface_decoder_compare/results/full/results.csv")
TRACKED_CASES = (
    ("3", "3", "0.002"),
    ("3", "3", "0.005"),
    ("3", "3", "0.01"),
    ("5", "5", "0.002"),
    ("5", "5", "0.005"),
    ("5", "5", "0.01"),
)


@dataclass(frozen=True)
class PairedResult:
    distance: str
    rounds: str
    p: str
    ldpc_decode_us_per_shot: str
    rbposd_decode_us_per_shot: str

    @property
    def rbposd_ratio(self) -> float:
        return float(self.rbposd_decode_us_per_shot) / float(
            self.ldpc_decode_us_per_shot
        )

    @property
    def markdown_row(self) -> str:
        return (
            f"| {self.distance} | {self.rounds} | {float(self.p):.3f} | "
            f"{self.ldpc_decode_us_per_shot} | "
            f"{self.rbposd_decode_us_per_shot} | {self.rbposd_ratio:.3f} |"
        )


class DocsContractTest(unittest.TestCase):
    def test_readme_and_makefile_document_both_tiers(self) -> None:
        readme = README_PATH.read_text()
        makefile = MAKEFILE_PATH.read_text()

        self.assertIn("make surface-decoder-compare-smoke", readme)
        self.assertIn("make surface-decoder-compare-full", readme)
        self.assertIn("surface-decoder-compare-smoke:", makefile)
        self.assertIn("surface-decoder-compare-full:", makefile)

    def test_readme_and_makefile_document_rsinter_surface_benchmark_flow(self) -> None:
        readme = README_PATH.read_text()
        makefile = MAKEFILE_PATH.read_text()

        self.assertIn("make bench-surface-smoke", readme)
        self.assertIn("make bench-surface-full", readme)
        self.assertIn("bench-surface-smoke:", makefile)
        self.assertIn("bench-surface-full:", makefile)

    def test_rbposd_performance_doc_matches_tracked_full_csv(self) -> None:
        doc = PERFORMANCE_DOC_PATH.read_text()
        assert_no_stale_rbposd_slower_claim(doc)

        self.assertIn(str(FULL_RESULTS_PATH), doc)
        self.assertIn("tracked checked-in full-tier native rows", doc)
        self.assertIn("not a fresh claim about current local machine speed", doc)
        self.assertIn("does not contain checked-in timing rows", doc)
        self.assertIn("rbposd_lsd_order1", doc)
        self.assertIn("rbposd_product_sum_serial", doc)

        for paired in paired_rbposd_ldpc_results():
            self.assertLess(
                paired.rbposd_ratio,
                1.0,
                f"tracked CSV no longer shows rbposd slower for {paired}",
            )
            self.assertIn(paired.markdown_row, doc)

    def test_stale_rbposd_slower_claim_is_rejected(self) -> None:
        stale_doc = """The current checked-in `full` benchmark results show
        `rbposd` decode time per shot trailing `ldpc` by roughly:
        - `39.7x` at `distance=3, p=0.002`
        - `67.6x` at `distance=3, p=0.005`
        - `85.8x` at `distance=3, p=0.010`
        - `104.0x` at `distance=5, p=0.002`
        - `121.8x` at `distance=5, p=0.005`
        - `131.0x` at `distance=5, p=0.010`
        """

        with self.assertRaisesRegex(
            AssertionError, "stale rbposd slower-than-ldpc claim"
        ):
            assert_no_stale_rbposd_slower_claim(stale_doc)


def paired_rbposd_ldpc_results() -> list[PairedResult]:
    rows = list(csv.DictReader(FULL_RESULTS_PATH.open(newline="")))
    pairs = []
    for distance, rounds, p in TRACKED_CASES:
        ldpc = result_row(rows, "ldpc", distance, rounds, p)
        rbposd = result_row(rows, "rbposd", distance, rounds, p)
        pairs.append(
            PairedResult(
                distance=distance,
                rounds=rounds,
                p=p,
                ldpc_decode_us_per_shot=ldpc["decode_us_per_shot"],
                rbposd_decode_us_per_shot=rbposd["decode_us_per_shot"],
            )
        )
    return pairs


def result_row(
    rows: list[dict[str, str]],
    decoder: str,
    distance: str,
    rounds: str,
    p: str,
) -> dict[str, str]:
    matches = [
        row
        for row in rows
        if row["tier"] == "full"
        and row["decoder"] == decoder
        and row["backend"] == "native"
        and row["distance"] == distance
        and row["rounds"] == rounds
        and row["p"] == p
    ]
    if len(matches) != 1:
        raise AssertionError(
            f"expected one full/native {decoder} row for "
            f"distance={distance}, rounds={rounds}, p={p}; got {len(matches)}"
        )
    return matches[0]


def assert_no_stale_rbposd_slower_claim(text: str) -> None:
    stale_ratios = ("39.7x", "67.6x", "85.8x", "104.0x", "121.8x", "131.0x")
    stale_context = (
        "current checked-in" in text
        and "trailing `ldpc`" in text
        and all(ratio in text for ratio in stale_ratios)
    )
    if stale_context:
        raise AssertionError(
            "stale rbposd slower-than-ldpc claim contradicts tracked CSV"
        )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the focused docs contract and observe the red state**

Run:

```bash
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py
```

Expected: FAIL in `test_rbposd_performance_doc_matches_tracked_full_csv` because the performance doc still contains the stale `trailing ldpc` claim and lacks the CSV-derived table.

- [ ] **Step 3: Run the negative control**

Run:

```bash
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py -k stale
```

Expected: PASS. The synthetic stale statement is rejected by `assert_no_stale_rbposd_slower_claim`.

### Task 2: Update rbposd Performance Evidence Text

**Files:**
- Modify: `docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md`
- Test: `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`

**Interfaces:**
- Consumes: table rows required by Task 1.
- Produces: performance design summary that no longer makes the stale categorical slower-than-`ldpc` claim.

- [ ] **Step 1: Replace the stale summary evidence block**

In `docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md`, replace the paragraph beginning `The current checked-in \`full\` benchmark results show` through the six stale ratio bullets with this text:

```markdown
An evidence update from issue #100 supersedes the original benchmark-gap
summary in this design. The tracked artifact at
`benchmarks/surface_decoder_compare/results/full/results.csv` is the source for
the current checked-in full-tier comparison rows. It is evidence of the
checked-in benchmark artifact, not a fresh claim about current local machine
speed.

In the tracked checked-in full-tier native rows, `rbposd` has lower
`decode_us_per_shot` than `ldpc` for every paired `distance in {3, 5}` and
`p in {0.002, 0.005, 0.010}` case:

| distance | rounds | p | ldpc decode_us_per_shot | rbposd decode_us_per_shot | rbposd / ldpc |
| --- | --- | --- | ---: | ---: | ---: |
| 3 | 3 | 0.002 | 9.28949949957314 | 5.533358 | 0.596 |
| 3 | 3 | 0.005 | 15.255653700023686 | 9.888312299999999 | 0.648 |
| 3 | 3 | 0.010 | 22.875337890445515 | 18.083490234375002 | 0.791 |
| 5 | 5 | 0.002 | 194.81863700011675 | 128.28873740000003 | 0.659 |
| 5 | 5 | 0.005 | 386.04600850012497 | 322.0114498 | 0.834 |
| 5 | 5 | 0.010 | 737.693568638826 | 639.9339513020834 | 0.867 |

The table should not be read as a machine-independent speed promise or as proof
that all `rbposd` configurations are faster than upstream `ldpc`. It only says
that the tracked native full-tier comparison artifact no longer supports the
old claim that default `rbposd` trails `ldpc` on every checked-in case.
```

- [ ] **Step 2: Add post-milestone alignment scope after the summary table**

Immediately after the replacement text above and before `The goal of this work is`, add:

```markdown
The LSD and BP-option milestone work also changes the alignment story. The repo
now has LSD execution and result-row coverage, BP method/schedule configuration,
behavioral teeth for `product_sum` plus `serial`, and checked-in `rsinter`
benchmark spec entries named `rbposd_lsd_order1` and
`rbposd_product_sum_serial`.

Those milestones do not mean the tracked comparison CSV covers every expanded
decoder surface. `benchmarks/surface_decoder_compare/results/full/results.csv`
does not contain checked-in timing rows for `rbposd_lsd_order1` or
`rbposd_product_sum_serial`, and the implemented option surface is still a
narrow subset of upstream `ldpc` rather than full feature parity.
```

- [ ] **Step 3: Update success criteria wording**

In the `Success Criteria` section, replace:

```markdown
- `surface_decoder_compare` shows a substantial reduction in `rbposd`
  `decode_us_per_shot`
- the remaining performance gap to `ldpc`, if any, is no longer dominated by
  avoidable allocation and matrix-rebuild costs
```

with:

```markdown
- fresh `surface_decoder_compare` runs, when regenerated for performance work,
  should be reported separately from the tracked CSV artifact cited above
- any remaining `ldpc` comparison claim is grounded in the specific benchmark
  artifact being discussed, not copied forward from stale checked-in numbers
```

- [ ] **Step 4: Run the docs contract to observe green**

Run:

```bash
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py -k stale
```

Expected: both commands PASS.

### Task 3: Final Verification And Commit

**Files:**
- Commit: `benchmarks/surface_decoder_compare/tests/test_docs_contract.py`
- Commit: `docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md`
- Commit: `docs/superpowers/plans/2026-06-21-issue-100-rbposd-evidence-reconciliation.md`

**Interfaces:**
- Consumes: completed Tasks 1 and 2.
- Produces: committed evidence reconciliation implementation ready for PR.

- [ ] **Step 1: Run required issue verification**

Run:

```bash
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py
python3 -m pytest benchmarks/surface_decoder_compare/tests/test_docs_contract.py -k stale
```

Expected: both commands PASS.

- [ ] **Step 2: Run required Agent Desk verification**

Run:

```bash
cargo test
```

Expected: PASS for the whole workspace.

- [ ] **Step 3: Check diff hygiene**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; only the intended implementation files are modified or untracked before the commit.

- [ ] **Step 4: Commit the implementation**

Run:

```bash
git add benchmarks/surface_decoder_compare/tests/test_docs_contract.py docs/superpowers/specs/2026-06-06-rbposd-core-performance-design.md docs/superpowers/plans/2026-06-21-issue-100-rbposd-evidence-reconciliation.md
git commit -m "docs: reconcile rbposd benchmark evidence"
```

Expected: commit succeeds on the worker branch.
