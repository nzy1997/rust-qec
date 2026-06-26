from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    SMALL_LDPC_CASES,
    SMOKE_CASES,
)

__all__ = [
    "CATALOG_HEADER",
    "CSV_HEADER",
    "SMALL_LDPC_CASES",
    "SMOKE_CASES",
    "verify_rows",
    "write_summary",
]


def __getattr__(name: str):
    if name == "verify_rows":
        from benchmarks.bb_circuit_bposd_compare.verify_smoke import verify_rows

        return verify_rows
    if name == "write_summary":
        from benchmarks.bb_circuit_bposd_compare.summary import write_summary

        return write_summary
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
