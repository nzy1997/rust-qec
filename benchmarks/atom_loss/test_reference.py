"""Semantic controls for the benchmark/reference harness, without timing assertions."""
import unittest
from unittest.mock import patch
import numpy as np
from . import reference
from .verify import require_complete_sweep
from .run import logical_x, score, wilson, measure_python


class ReferenceTests(unittest.TestCase):
    def test_loss_is_persistent_and_reset_restores(self):
        rows=reference.sample('R 0 1\nX 0\nLOSS(1) 0\nCX 0 1\nML 0 1\nR 0\nX 0\nML 0',128)
        np.testing.assert_array_equal(rows,np.tile([1,1,0,0,0,1],(128,1)))

    def test_lost_entangled_partner_does_not_collapse_survivor_to_zero(self):
        rows=reference.sample('R 0 1\nH 0\nCX 0 1\nLOSS(1) 0\nML 0 1',8192)
        self.assertTrue(np.all(rows[:,:3]==[1,1,0]))
        self.assertLess(abs(rows[:,3].mean()-.5),.04)

    def test_lost_two_qubit_noise_is_skipped(self):
        rows=reference.sample('R 0 1\nLOSS(1) 0\nDEPOLARIZE2(1) 0 1\nML 0 1',128)
        np.testing.assert_array_equal(rows,np.tile([1,1,0,0],(128,1)))

    def test_repeat_and_inverted_measurement(self):
        rows=reference.sample('R 0\nREPEAT 2 {\nX 0\n}\nLOSS(1) 0\nML !0\nMRL 0\nML 0',16)
        np.testing.assert_array_equal(rows,np.tile([1,0,1,1,0,0],(16,1)))

    def test_unsupported_operations_and_noise_fail(self):
        for circuit in ['R 0\nS 0\nM 0','R 0\nML(0.1) 0','REPEAT 2 {\nR 0']:
            with self.assertRaises(ValueError): reference.sample(circuit,4)

    def test_incomplete_scoring_cannot_report_success(self):
        with self.assertRaises(ValueError): score(np.array([0]),np.array([0,0]))
        with self.assertRaises(ValueError): score(np.array([2]),np.array([0]))
        self.assertGreater(wilson(0,5000)[1],0)

    def test_backend_failure_preserves_timings_without_partial_accuracy(self):
        successful = (np.array([0,1], dtype=np.uint8), {'decode_seconds':.1})
        with patch('benchmarks.atom_loss.run.python_decode', side_effect=[successful, ValueError('unreachable syndrome')]):
            result = measure_python({'compile_seconds':.01,'transform_seconds':.01}, True, np.array([0,1]), 3)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(len(result['runs']), 1)
        self.assertIn('unreachable syndrome', result['error'])
        self.assertNotIn('logical_error_rate', result)
        self.assertNotIn('errors', result)
        self.assertNotIn('total_seconds', result)

    def test_incomplete_comparison_cannot_be_published_as_a_curve(self):
        cases=[{'distance':d,'loss_probability':p,'decoders':{name:{'status':'ok'} for name in
                ['envelope-matching','pymatching-fixed','pymatching-envelope']}}
               for d in [3,5,7] for p in [.0001,.0003,.001,.003,.01]]
        require_complete_sweep(cases)
        with self.assertRaises(ValueError): require_complete_sweep(cases[:-1])
        cases[0]['decoders']['pymatching-fixed']['status']='failed'
        with self.assertRaises(ValueError): require_complete_sweep(cases)
        del cases[0]['decoders']['pymatching-fixed']
        with self.assertRaises(ValueError): require_complete_sweep(cases)

    def test_logical_support_is_derived_from_coordinates(self):
        text='QUBIT_COORDS(1,5) 15\nQUBIT_COORDS(2,2) 9\nQUBIT_COORDS(1,1) 1\nQUBIT_COORDS(1,3) 8'
        self.assertEqual(logical_x(text,3),'15,1,8')
        with self.assertRaises(ValueError): logical_x(text,5)


if __name__=='__main__': unittest.main()
