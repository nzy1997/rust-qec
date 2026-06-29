from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[3]


def write_manifest(path: Path, cases: str) -> None:
    path.write_text(
        textwrap.dedent(
            f"""
            manifest_version = 1
            suite = "qec_code_random_window"

            {cases}
            """
        ).lstrip(),
        encoding="utf-8",
    )


def inline_cell(ref: str, value: object) -> str:
    if isinstance(value, (int, float)):
        return f'<c r="{ref}"><v>{value}</v></c>'
    return f'<c r="{ref}" t="inlineStr"><is><t>{escape(str(value))}</t></is></c>'


def _sheet_xml(rows: list[list[object]]) -> str:
    row_xml = []
    for row_index, row in enumerate(rows, start=1):
        cells = []
        for col_index, value in enumerate(row, start=1):
            col = chr(ord("A") + col_index - 1)
            cells.append(inline_cell(f"{col}{row_index}", value))
        row_xml.append(f'<row r="{row_index}">{"".join(cells)}</row>')
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>{"".join(row_xml)}</sheetData>
</worksheet>"""


def write_xlsx(path: Path, sheet_name: str, rows: list[list[object]]) -> None:
    write_multi_sheet_xlsx(path, [(sheet_name, rows)])


def write_multi_sheet_xlsx(path: Path, sheets: list[tuple[str, list[list[object]]]]) -> None:
    workbook_sheets = "".join(
        f'<sheet name="{escape(sheet_name)}" sheetId="{index}" r:id="rId{index}"/>'
        for index, (sheet_name, _) in enumerate(sheets, start=1)
    )
    workbook_rels = "".join(
        f'<Relationship Id="rId{index}" '
        'Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" '
        f'Target="worksheets/sheet{index}.xml"/>'
        for index, _ in enumerate(sheets, start=1)
    )
    content_overrides = "".join(
        f'<Override PartName="/xl/worksheets/sheet{index}.xml" '
        'ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        for index, _ in enumerate(sheets, start=1)
    )
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(
            "[Content_Types].xml",
            f"""<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
{content_overrides}
</Types>""",
        )
        archive.writestr(
            "_rels/.rels",
            """<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>""",
        )
        archive.writestr(
            "xl/workbook.xml",
            f"""<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets>{workbook_sheets}</sheets>
</workbook>""",
        )
        archive.writestr(
            "xl/_rels/workbook.xml.rels",
            f"""<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{workbook_rels}
</Relationships>""",
        )
        for index, (_, rows) in enumerate(sheets, start=1):
            archive.writestr(f"xl/worksheets/sheet{index}.xml", _sheet_xml(rows))


def run_importer(
    args: list[str], env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.qec_code_random_window.import_paper_baselines",
            *args,
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        env=merged_env,
    )


class ImportPaperBaselinesTest(unittest.TestCase):
    def test_readme_documents_external_data_setup(self) -> None:
        readme = (
            ROOT / "benchmarks" / "qec_code_random_window" / "README.md"
        ).read_text(encoding="utf-8")

        self.assertIn("https://github.com/m-webster/codeDistancePYPI", readme)
        self.assertIn("paper results", readme)
        self.assertIn("openpyxl", readme)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", readme)
        self.assertIn("unmapped", readme)
        self.assertIn('"/path/to/codeDistancePYPI/paper results"', readme)

    def test_selected_workbook_and_alias_headers_convert(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bivariate-results.xlsx",
                "Bivariate Bicycle",
                [
                    ["case", "method", "ub", "runtime_s"],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,bivariate-results.xlsx,Bivariate Bicycle,2\n",
            )

    def test_synthetic_xlsx_converts_to_exact_canonical_csv(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_fixture"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 12
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                    ["bb144", "QDistEvol", 12, 30],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stderr, "")
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,bb-summary.xlsx,BB summary,2\n"
                "bb144_fixture,bb144,QDistEvol,12,30,bb-summary.xlsx,BB summary,3\n",
            )

    def test_missing_required_sheet_exits_nonzero_and_names_sheet(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(paper_dir / "notes.xlsx", "Other Data", [["note"], ["ignore"]])

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required sheet", result.stderr)

    def test_unrelated_workbook_is_ignored_and_missing_selected_sheet_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "notes.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required sheet", result.stderr)
            self.assertIn("bb", result.stderr)
            self.assertIn("bivariate", result.stderr)
            self.assertIn("summary", result.stderr)

    def test_selected_workbook_with_unrelated_sheet_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "Other Data",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required sheet", result.stderr)
            self.assertIn("bb", result.stderr)
            self.assertIn("bivariate", result.stderr)
            self.assertIn("summary", result.stderr)

    def test_selected_bad_workbook_is_not_silently_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-renamed.xlsx",
                "Other Data",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required sheet", result.stderr)

    def test_missing_required_column_exits_nonzero_and_names_field(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [["paper_case", "baseline_method", "baseline_upper_bound"], ["bb72", "QDistRndMW", 6]],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("baseline_elapsed_s", result.stderr)

    def test_selected_sheets_across_one_workbook_are_all_imported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_fixture"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 12
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """,
            )
            write_multi_sheet_xlsx(
                paper_dir / "bb-summary.xlsx",
                [
                    (
                        "BB summary",
                        [
                            [
                                "paper_case",
                                "baseline_method",
                                "baseline_upper_bound",
                                "baseline_elapsed_s",
                            ],
                            ["bb72", "QDistRndMW", 6, 12.5],
                        ],
                    ),
                    (
                        "Bivariate detail",
                        [
                            [
                                "paper_case",
                                "baseline_method",
                                "baseline_upper_bound",
                                "baseline_elapsed_s",
                            ],
                            ["bb144", "QDistEvol", 12, 30],
                        ],
                    ),
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,bb-summary.xlsx,BB summary,2\n"
                "bb144_fixture,bb144,QDistEvol,12,30,bb-summary.xlsx,Bivariate detail,2\n",
            )

    def test_any_selected_sheet_missing_required_column_fails_nonzero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_multi_sheet_xlsx(
                paper_dir / "bb-summary.xlsx",
                [
                    (
                        "BB summary",
                        [
                            [
                                "paper_case",
                                "baseline_method",
                                "baseline_upper_bound",
                                "baseline_elapsed_s",
                            ],
                            ["bb72", "QDistRndMW", 6, 12.5],
                        ],
                    ),
                    (
                        "Bivariate detail",
                        [
                            [
                                "paper_case",
                                "baseline_method",
                                "baseline_upper_bound",
                            ],
                            ["bb72", "QDistRndMW", 6],
                        ],
                    ),
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("baseline_elapsed_s", result.stderr)

    def test_paper_case_variants_normalize_to_canonical_aliases(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_fixture"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 12
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["BB72", "QDistRndMW", 6, 12.5],
                    ["bb 144", "QDistEvol", 12, 30],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,bb-summary.xlsx,BB summary,2\n"
                "bb144_fixture,bb144,QDistEvol,12,30,bb-summary.xlsx,BB summary,3\n",
            )

    def test_dash_separated_paper_case_variant_matches_alias(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb-72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(
                "bb72_fixture,bb72,QDistRndMW,6,12.5",
                out.read_text(encoding="utf-8"),
            )

    def test_qc_named_workbook_and_sheet_are_selected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "qc-results.xlsx",
                "QC",
                [
                    ["case", "method", "ub", "runtime_s"],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,qc-results.xlsx,QC,2\n",
            )

    def test_unmapped_case_is_omitted_not_silently_matched(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "surface_fixture"
            code_id = "surface_rotated:d=5"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 5
            target_upper_bound = 5
            baseline_key = "unmapped:surface_rotated_d5"
            baseline_required = false
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["surface_rotated_d5", "QDistRndMW", 5, 1],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n",
            )

    def test_env_var_supplies_paper_results_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "bb-summary.xlsx",
                "BB summary",
                [
                    [
                        "paper_case",
                        "baseline_method",
                        "baseline_upper_bound",
                        "baseline_elapsed_s",
                    ],
                    ["bb72", "QDistRndMW", 6, 12.5],
                ],
            )

            result = run_importer(
                ["--cases", str(manifest), "--out", str(out)],
                env={"CODEDISTANCE_PAPER_RESULTS_DIR": str(paper_dir)},
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(
                "bb72_fixture,bb72,QDistRndMW,6,12.5", out.read_text(encoding="utf-8")
            )


if __name__ == "__main__":
    unittest.main()
