import csv
import re
import unittest
from dataclasses import dataclass
from pathlib import Path


README_PATH = Path("benchmarks/surface_decoder_compare/README.md")
MAKEFILE_PATH = Path("Makefile")
BENCHMARK_EVIDENCE_SHOWCASE_PATH = Path("docs/showcases/benchmark-evidence.md")
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
        compare_targets = makefile_targets(
            makefile,
            "surface-decoder-compare-smoke",
            "surface-decoder-compare-full",
        )
        self.assertIn("bench plot-surface-compare-csv", compare_targets)
        self.assertNotIn("benchmarks.surface_decoder_compare.plot_compare", compare_targets)

    def test_readme_and_makefile_document_rsinter_surface_benchmark_flow(self) -> None:
        readme = README_PATH.read_text()
        makefile = MAKEFILE_PATH.read_text()

        self.assertIn("make bench-surface-smoke", readme)
        self.assertIn("make bench-surface-full", readme)
        self.assertIn("bench-surface-smoke:", makefile)
        self.assertIn("bench-surface-full:", makefile)

    def test_benchmark_evidence_showcase_links_required_evidence(self) -> None:
        doc = BENCHMARK_EVIDENCE_SHOWCASE_PATH.read_text()

        self.assertIn("benchmarks/surface_decoder_compare/README.md", doc)
        self.assertIn("docs/bb144_circuit_bposd_reproduction.md", doc)
        self.assertIn("benchmarks/surface_decoder_compare/results/full/results.csv", doc)
        self.assertIn(
            "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
            doc,
        )
        self.assertIn("implementation smoke evidence", doc)
        self.assertIn("not statistical reproduction", doc)
        self.assertIn("bb-circuit-bposd-memory", doc)
        assert_valid_bb_circuit_command_keys(doc)

    def test_benchmark_evidence_showcase_rejects_bb_circuit_command_typo(self) -> None:
        doc = BENCHMARK_EVIDENCE_SHOWCASE_PATH.read_text().replace(
            "cargo run -p rsinter -- bb-circuit-bposd-memory \\",
            "cargo run -p rsinter -- bb-circuit-bposd-memroy \\",
            1,
        )

        with self.assertRaisesRegex(
            AssertionError, "unknown BB circuit command key: bb-circuit-bposd-memroy"
        ):
            assert_valid_bb_circuit_command_keys(doc)

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
    with FULL_RESULTS_PATH.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
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


def makefile_targets(makefile: str, *target_names: str) -> str:
    lines = makefile.splitlines()
    blocks: list[str] = []
    for index, line in enumerate(lines):
        if not any(line == f"{target}:" for target in target_names):
            continue
        block = [line]
        for next_line in lines[index + 1 :]:
            if next_line and not next_line.startswith(("\t", " ")):
                break
            block.append(next_line)
        blocks.append("\n".join(block))
    return "\n".join(blocks)


def assert_valid_bb_circuit_command_keys(text: str) -> None:
    known = {"bb-circuit-bposd-memory"}
    keys = set(re.findall(r"\bbb-circuit-bposd-[A-Za-z0-9_-]+\b", text))
    unknown = sorted(keys - known)
    if unknown:
        raise AssertionError(f"unknown BB circuit command key: {', '.join(unknown)}")


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
