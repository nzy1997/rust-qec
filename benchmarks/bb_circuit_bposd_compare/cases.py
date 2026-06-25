from __future__ import annotations

from dataclasses import dataclass

CSV_HEADER = [
    "case_id",
    "runner",
    "decoder_impl",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
    "error",
]


@dataclass(frozen=True)
class CompareCase:
    case_id: str
    code_id: str
    p: float
    num_cycles: int
    num_trials: int
    seed: int = 12345
    bp_method: str = "ms"
    max_iter: int = 10000
    osd_method: str = "osd_cs"
    osd_order: int = 7


SMOKE_CASES = (
    CompareCase("bb72-p0005-c1-t1-seed12345", "bb72", 0.0005, 1, 1),
    CompareCase("bb90-p0005-c1-t1-seed12345", "bb90", 0.0005, 1, 1),
)
