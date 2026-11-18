"""Schedule and interpreter admission checks for decoder capture."""

import json
from pathlib import Path
from subprocess import CompletedProcess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import decoder_campaign
import logical_distillation_benchmark as benchmark
import reference
from remaining_contracts import read_json


class LogicalDistillationBenchmarkTests(unittest.TestCase):
    def test_production_uses_one_unsharded_timing_run_and_contiguous_precision(self):
        directory = Path(__file__).resolve().parent
        plan = read_json(directory/'paper/logical-distillation.json')
        benchmark.validate_manifest(plan, reference.load_manifest())
        timing, *precision = list(benchmark.cases(plan))
        self.assertEqual(timing['kind'], 'timing')
        self.assertEqual((timing['start'], timing['shots'], timing['seed']), (0, 16384, 2026100801))
        self.assertEqual(len(precision), 64)
        self.assertEqual([case['start'] for case in precision], list(range(0, 16384, 256)))
        self.assertEqual({case['seed'] for case in precision}, {2026100631})
        self.assertEqual({case['shots'] for case in precision}, {256})

    def test_worker_probe_preserves_the_virtual_environment_launcher(self):
        with TemporaryDirectory() as scratch:
            root = Path(scratch)
            target = root/'base-python'
            target.write_bytes(b'python executable')
            launcher = root/'venv-python'
            launcher.symlink_to(target)
            environment = dict(prefix='venv', base_prefix='base', version='test', packages={'pymatching':'test'})
            probe = CompletedProcess([], 0, json.dumps(environment), '')
            with patch.object(decoder_campaign.subprocess,'run',return_value=probe) as run:
                invoked, captured = decoder_campaign.python_environment(launcher)
            self.assertEqual(invoked, launcher)
            self.assertEqual(run.call_args.args[0][0], launcher)
            self.assertEqual(captured['launcher'], str(launcher))
            self.assertEqual(captured['sha256'], reference.digest(target))
            self.assertEqual(captured['packages'], environment['packages'])


if __name__ == '__main__':
    unittest.main()
