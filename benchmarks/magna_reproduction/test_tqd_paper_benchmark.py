"""TQD deposit schedules and paired-draw verification."""

from copy import deepcopy
from fractions import Fraction
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from cultivation_statistics import ONE
import decoder_workloads_benchmark as workloads
import reference
from remaining_contracts import read_json
import tqd_paper_benchmark as benchmark
from tqd_author_approximation import target_z_infidelity
from tqd_paper_statistics import WARMUP, fault_stream, summarize, verify_readout_witness


HERE = Path(__file__).resolve().parent


class TqdPaperBenchmarkTests(unittest.TestCase):
    def test_production_covers_every_contiguous_point_and_nine_serial_trials(self):
        plan = read_json(HERE/'paper/tqd.json')
        benchmark.validate_manifest(plan, reference.load_manifest())
        batches = list(benchmark.cases(plan))
        timing = batches[:9]
        precision = batches[9:]
        self.assertEqual(len({case['id'] for case in batches}), len(batches))
        self.assertEqual({case['kind'] for case in timing}, {'timing'})
        self.assertEqual({case['shots'] for case in timing}, {256})
        self.assertEqual(sum(case['shots'] for case in precision), 18432)
        for name, family in plan['families'].items():
            self.assertEqual(len(family['grid']), 5)
            for point in family['grid']:
                ranges = [case for case in precision if case['family'] == name and case['point'] == point['point']]
                self.assertEqual([case['start'] for case in ranges], list(range(0, point['draws'], 64)))
                self.assertEqual(sum(case['shots'] for case in ranges), point['draws'])
                self.assertEqual({case['seed'] for case in ranges}, {point['seed']})

    def test_paired_uncertainty_retains_flagged_draws_in_the_full_curve(self):
        def mass(value):
            return Fraction(value), Fraction(0)

        rows = [(ONE, mass('1/8'), True, mass('1/16'), False),
                (ONE, mass('3/8'), True, mass('1/16'), True),
                (ONE, mass('1/2'), False, mass(0), True),
                (ONE, mass(0), False, mass(0), False)]
        report = summarize(rows)
        self.assertEqual(target_z_infidelity(), Fraction(24, 25))
        self.assertEqual((report['draws'], report['flux_success']), (4, 2))
        self.assertEqual((report['odd_after_all'], report['odd_after_unflagged']), (2, 1))
        self.assertEqual(report['full']['mean'], 1/4)
        self.assertEqual(report['matched_author']['mean'], 1/16)
        self.assertEqual(report['paired_difference']['mean'], 3/16)
        self.assertAlmostEqual(report['paired_difference']['standard_error']**2, 1/96)

    def test_witness_rejects_true_syndrome_in_place_of_reported_syndrome(self):
        original = dict(name='jit_b_0', call=0, inputs=[False, True])
        flipped = dict(original, inputs=[True, True])
        payload = dict(configuration=dict(size=3, start=0, seed=202610380000),
                       readout_witness=dict(index=WARMUP-1, fault=0, unflipped=original, flipped=flipped))
        entries = [dict(kind='X', color='b', edge=2, record=4)]
        verify_readout_witness(payload, entries)
        flipped['inputs'][0] = False
        with self.assertRaisesRegex(ValueError, 'flipped reported bit'):
            verify_readout_witness(payload, entries)

    def test_fault_reference_has_stable_independent_draw_addresses(self):
        first = fault_stream(202610380000, 0, Fraction(3, 1000), 756)
        second = fault_stream(202610380000, 1, Fraction(3, 1000), 756)
        self.assertEqual(first, [193, 357, 451, 556, 632, 640])
        self.assertEqual(second, [472, 526, 695])

    def test_author_replay_ignores_ambient_python_import_settings(self):
        with TemporaryDirectory() as scratch:
            root = Path(scratch).resolve()
            (root/'config').mkdir()
            stage = dict(payload=dict(rows=[]))

            def run_process(argv, timeout, directory):
                self.assertEqual(argv[:5], ['python3','-E','-s','-B',str(HERE/'tqd_author_approximation.py')])
                self.assertEqual(read_json(Path(argv[-1])), stage['payload'])
                self.assertEqual((timeout,directory),(1800,root/'config'))
                return dict(status='complete')

            case = dict(kind='precision',timeout_seconds=1800)
            benchmark.author_stage(stage,case,root,'python3',run_process)
            self.assertEqual(stage['author']['status'],'complete')

    def test_combined_checker_requires_the_tqd_results_and_full_plan(self):
        plan = read_json(HERE/'paper/decoder_workloads.json')
        with TemporaryDirectory() as scratch:
            root = Path(scratch)
            for name in ('logical-distillation', 'code-switching'):
                (root/name).mkdir()
            with self.assertRaisesRegex(ValueError, 'exactly the three full campaigns'):
                workloads.check_capture(plan, root)
            incomplete = deepcopy(plan)
            del incomplete['campaigns']['tqd']
            with self.assertRaisesRegex(ValueError, 'omits or changes a full campaign'):
                workloads.check_capture(incomplete, root)


if __name__ == '__main__':
    unittest.main()
