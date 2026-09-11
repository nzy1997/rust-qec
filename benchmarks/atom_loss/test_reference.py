"""Semantic controls for the benchmark/reference harness, without timing assertions."""
import unittest
from unittest.mock import patch
import numpy as np
from . import reference
from .verify import require_complete_sweep
from .run import ROOT, build_matching, logical_x, score, wilson, measure_python, python_decode
from . import decoder_reference, chain_reference, correctness, noise_controls
import itertools


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
            result = measure_python(lambda rep: {'compile_seconds':.01,'transform_seconds':.01}, True, np.array([0,1]), 3)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(len(result['runs']), 1)
        self.assertIn('unreachable syndrome', result['error'])
        self.assertNotIn('logical_error_rate', result)
        self.assertNotIn('errors', result)
        self.assertNotIn('total_seconds', result)

    def test_each_repetition_remeasures_common_stages(self):
        calls=[]
        def export(rep):
            calls.append(rep)
            return {'compile_seconds':rep+1.,'transform_seconds':rep+.5}
        successful=(np.array([0,1],dtype=np.uint8),{'decode_seconds':.1})
        with patch('benchmarks.atom_loss.run.python_decode', side_effect=lambda *a, **kw: (successful[0],successful[1].copy())):
            result=measure_python(export,False,np.array([0,1]),3)
        self.assertEqual(calls,[0,1,2])
        np.testing.assert_allclose(result['total_seconds'],[1.6,3.6,5.6])

    def test_actual_ignore_conditioning_mutation_fails_oracle(self):
        with patch('benchmarks.atom_loss.decoder_reference.build_matching', side_effect=lambda graph, losses: build_matching(graph,[])):
            result=decoder_reference.run(ROOT/'target/release/rustqec',ROOT/'target/release/examples/export_matching_benchmark')
        self.assertEqual(result['status'],'FAIL')
        five=result['cases'][1]
        self.assertEqual(five['rejected_rows']['native'],[])
        self.assertIn(21,five['rejected_rows']['pymatching'])
        self.assertEqual(decoder_reference.oracle(21,False,5)[0],{0})
        self.assertEqual(decoder_reference.oracle(21,True,5)[0],{1})

    def test_batch_matches_loop_and_preserves_interleaved_shot_order(self):
        graph={'edges':[{'u':0,'v':None,'observables':[0],'weight':1.,'loss_factor':.5},
                        {'u':0,'v':1,'observables':[],'weight':1.,'loss_factor':.5},
                        {'u':1,'v':None,'observables':[],'weight':1.,'loss_factor':.5}],
               'loss_edges':[[0],[1],[2]],'mean_weight':1.,'syndromes':[],'losses':[]}
        for raw in range(64):
            _,syndrome,flags=decoder_reference.oracle(raw)
            graph['syndromes'].append(syndrome)
            graph['losses'].append([i for i,f in enumerate(flags) if f])
        for conditioned in [False,True]:
            batch,timing=python_decode(graph,conditioned)
            phases=[timing[k] for k in ['preprocess_seconds','graph_build_seconds','matching_seconds','output_seconds','adapter_overhead_seconds']]
            self.assertGreaterEqual(min(phases),0)
            self.assertAlmostEqual(sum(phases),timing['decode_seconds'])
            loop,_=python_decode(graph,conditioned,batch=False)
            np.testing.assert_array_equal(batch,loop)
        graph['syndromes'][0].append(1)
        for row in graph['syndromes'][1:]: row.append(0)
        with self.assertRaisesRegex(ValueError,'Unreachable'):
            python_decode(graph,False)

    def test_deleted_two_qubit_channel_fails_same_acceptance(self):
        original=correctness.rust_rows
        def defective(binary,text,shots,seed,work):
            text='\n'.join(line for line in text.splitlines() if not line.startswith('DEPOLARIZE2('))
            return original(binary,text,shots,seed,work)
        with patch('benchmarks.atom_loss.correctness.rust_rows',side_effect=defective):
            report=correctness.run(ROOT/'target/release/rustqec')
        self.assertEqual(report['status'],'FAIL')
        analytic=report['analytic_noise_controls']
        case=next(r for r in analytic['cases'] if r['case']=='DEPOLARIZE2_alive')
        self.assertEqual(case['status'],'FAIL')
        self.assertEqual(case['rust_probability'],0.)
        self.assertAlmostEqual(case['expected_probability'],8*.17/15)

    def test_exact_parity_costs_match_exhaustive_fault_choices(self):
        terms=[(1,1.1),(2,.3),(3,.4),(5,.7),(1,.2)]
        expected=np.full(8,np.inf)
        for choices in itertools.product([0,1],repeat=len(terms)):
            mask=0; cost=0.
            for choose,(effect,weight) in zip(choices,terms):
                if choose: mask^=effect; cost+=weight
            expected[mask]=min(expected[mask],cost)
        np.testing.assert_allclose(chain_reference.costs(terms,3),expected)
        self.assertEqual(chain_reference.allowed(np.array([0.,1.,0.,1.]),1,1),{0,1})
        with self.assertRaises(ValueError): chain_reference.allowed(np.array([np.inf,np.inf]),0,0)

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
