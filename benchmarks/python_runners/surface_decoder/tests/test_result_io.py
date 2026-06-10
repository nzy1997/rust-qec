import io
import unittest

from benchmarks.python_runners.surface_decoder.result_io import (
    read_results_jsonl,
    write_results_jsonl,
)


class ResultIoTest(unittest.TestCase):
    def test_write_and_read_results_jsonl_round_trip(self) -> None:
        rows = [
            {
                "benchmark": "surface_decoder",
                "runner": "pymatching",
                "language": "python",
                "status": "ok",
                "params": {"distance": 3, "p": 0.002},
                "case_summary": {"num_dets": 24},
                "metrics": {"shots_used": 20, "logical_error_rate": 0.05},
                "artifacts": {},
                "error": None,
            }
        ]

        buf = io.StringIO()
        write_results_jsonl(rows, buf)
        loaded = read_results_jsonl(io.StringIO(buf.getvalue()))
        self.assertEqual(loaded[0]["runner"], "pymatching")
        self.assertEqual(loaded[0]["metrics"]["shots_used"], 20)


if __name__ == "__main__":
    unittest.main()
