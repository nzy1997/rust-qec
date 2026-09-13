"""Semantic controls for the benchmark/reference harness, without timing assertions."""
import unittest
import hashlib
import csv
import io
import json
from pathlib import Path
import shutil
import tempfile
import zipfile
import pymatching
from unittest.mock import patch
import numpy as np
from . import reference
from .verify import require_complete_sweep, verify
from .run import ROOT, build_matching, logical_x, score, wilson, measure_python, python_decode
from . import decoder_reference, chain_reference, correctness, noise_controls, channel_probes, low_probability
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
        for circuit in ['R 0\nT 0\nM 0','R 0\nML(0.1) 0','REPEAT 2 {\nR 0']:
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
            phases=[timing[k] for k in ['topology_seconds','preprocess_seconds','graph_build_seconds','matching_seconds','output_seconds','adapter_overhead_seconds']]
            self.assertGreaterEqual(min(phases),0)
            self.assertAlmostEqual(sum(phases),timing['decode_seconds'])
            loop,_=python_decode(graph,conditioned,batch=False)
            np.testing.assert_array_equal(batch,loop)
        graph['syndromes'][0].append(1)
        for row in graph['syndromes'][1:]: row.append(0)
        with self.assertRaisesRegex(ValueError,'Unreachable'):
            python_decode(graph,False)

    def test_bulk_graph_preserves_parallel_boundary_weights_and_predictions(self):
        # Parallel edges have identical logical labels, as required by the compiler.
        # Conditioning changes which parallel edge survives smallest-weight merge.
        graph = {'edges': [
            {'u':0,'v':None,'observables':[0],'weight':4.,'loss_factor':.25},
            {'u':0,'v':None,'observables':[0],'weight':2.,'loss_factor':.5},
            {'u':0,'v':1,'observables':[],'weight':3.,'loss_factor':.25},
            {'u':0,'v':1,'observables':[],'weight':1.,'loss_factor':.5},
            {'u':1,'v':None,'observables':[],'weight':2.,'loss_factor':.5}],
            'mean_weight':2.4, 'num_observables':1,
            'loss_edges': [[0,2], [1,3], [4]], 'syndromes':[[0,0]], 'losses':[[]]}
        for mask in range(8):
            losses = [i for i in range(3) if mask & (1 << i)]
            active = {i for loss in losses for i in graph['loss_edges'][loss]}
            legacy = pymatching.Matching()
            for i, edge in enumerate(graph['edges']):
                weight = (edge['loss_factor']*graph['mean_weight'] if i in active else edge['weight'])/4.
                kwargs = dict(weight=weight, fault_ids=set(edge['observables']), merge_strategy='smallest-weight')
                if edge['v'] is None: legacy.add_boundary_edge(edge['u'], **kwargs)
                else: legacy.add_edge(edge['u'], edge['v'], **kwargs)
            bulk = build_matching(graph, losses)
            self.assertEqual(bulk.edges(), legacy.edges())
            rows = np.array(list(itertools.product([0,1], repeat=2)), dtype=np.uint8)
            np.testing.assert_array_equal(bulk.decode_batch(rows), legacy.decode_batch(rows))

    def test_bundle_rejects_resealed_native_time_and_missing_checksums(self):
        source = ROOT/'site/static/data/atom-loss'
        self.assertEqual(verify(source), 'PASS')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)/'bundle'
            def reset():
                shutil.copytree(source, root, dirs_exist_ok=True)
            def reseal(name):
                manifest = json.loads((root/'bundle.json').read_text())
                manifest['sha256'][name] = hashlib.sha256((root/name).read_bytes()).hexdigest()
                (root/'bundle.json').write_text(json.dumps(manifest))
            for defect in ['total', 'missing_stats', 'negative', 'nan', 'failed']:
                reset()
                data = json.loads((root/'tradeoff.json').read_text())
                native = data['decoders']['envelope-matching']
                if defect == 'total': native['total_seconds'] = [v/100 for v in native['total_seconds']]
                elif defect == 'missing_stats': del native['runs'][0]['stats']
                elif defect in ['negative','nan']: native['runs'][0]['stats']['compile_seconds'] = -1. if defect == 'negative' else float('nan')
                else: native['runs'][0]['exit_code'] = 1
                (root/'tradeoff.json').write_text(json.dumps(data))
                reseal('tradeoff.json')
                with self.subTest(defect=defect), self.assertRaisesRegex(ValueError, 'native|Native'):
                    verify(root)
            for name in ['accuracy-time.svg', 'source-snapshot-timing.json', 'shot-data-v1.zip', 'timing-sweep.svg', 'sampling-reference-cost.svg', 'provenance-correctness.json']:
                reset()
                manifest = json.loads((root/'bundle.json').read_text())
                del manifest['sha256'][name]
                (root/name).write_text('replaced but unlisted')
                (root/'bundle.json').write_text(json.dumps(manifest))
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'Missing required'):
                    verify(root)
            reset()
            manifest = json.loads((root/'bundle.json').read_text())
            for name in ['provenance-timing.json', 'source-snapshot-timing.json']:
                del manifest['sha256'][name]
                (root/name).unlink()
            (root/'bundle.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'Missing required'):
                verify(root)
            reset()
            snapshot = json.loads((root/'source-snapshot-timing.json').read_text())
            snapshot['files'][next(iter(snapshot['files']))] += '# changed'
            (root/'source-snapshot-timing.json').write_text(json.dumps(snapshot))
            reseal('source-snapshot-timing.json')
            with self.assertRaisesRegex(ValueError, 'Source snapshot mismatch'):
                verify(root)

    def test_timing_table_corruption_is_rejected_after_resealing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)/'bundle'
            shutil.copytree(ROOT/'site/static/data/atom-loss',root)
            path = root/'timing-sweep.csv'
            lines = path.read_text().splitlines()
            cells = lines[1].split(',')
            cells[5] = str(float(cells[5])/100)
            lines[1] = ','.join(cells)
            path.write_text('\n'.join(lines)+'\n')
            manifest = json.loads((root/'bundle.json').read_text())
            manifest['sha256'][path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
            (root/'bundle.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'Timing sweep CSV'):
                verify(root)

    def test_summary_fields_and_completeness_reject_resealed_corruption(self):
        source = ROOT/'site/static/data/atom-loss'
        with (source/'summary.csv').open() as stream:
            reader = csv.DictReader(stream)
            fields, original = reader.fieldnames, list(reader)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)/'bundle'
            shutil.copytree(source,root)
            for defect in fields + ['missing','duplicate','extra','header']:
                rows = [row.copy() for row in original]
                header = fields[:]
                if defect == 'missing': rows.pop()
                elif defect == 'duplicate': rows[1] = rows[0].copy()
                elif defect == 'extra': rows.append(rows[0].copy())
                elif defect == 'header': header[0] = 'wrong_experiment'
                elif defect == 'errors': rows[0][defect] = '4999'
                elif defect == 'median_microseconds_per_shot': rows[0][defect] = '0.000001'
                else: rows[0][defect] = 'wrong'
                stream = io.StringIO()
                writer = csv.DictWriter(stream,fieldnames=header,extrasaction='ignore')
                writer.writeheader(); writer.writerows(rows)
                path = root/'summary.csv'
                path.write_text(stream.getvalue())
                manifest = json.loads((root/'bundle.json').read_text())
                manifest['sha256']['summary.csv'] = hashlib.sha256(path.read_bytes()).hexdigest()
                (root/'bundle.json').write_text(json.dumps(manifest))
                with self.subTest(defect=defect), self.assertRaisesRegex(ValueError,'Summary CSV'):
                    verify(root)

    def test_low_probability_cutoff_fails_overall_sampling_report(self):
        original = correctness.rust_rows
        affected = []
        def defective(binary,text,shots,seed,work):
            changed,count = low_probability.remove_low_noise(text,'both')
            affected.append(count)
            return original(binary,changed,shots,seed,work)
        with patch('benchmarks.atom_loss.correctness.rust_rows',side_effect=defective):
            report = correctness.run(ROOT/'target/release/rustqec')
        self.assertGreater(sum(affected),0)
        self.assertEqual(report['status'],'FAIL')
        self.assertEqual(report['low_probability_controls']['analytic']['status'],'FAIL')
        rejected = {r['case'] for r in report['low_probability_controls']['analytic']['cases'] if r['status']=='FAIL'}
        self.assertEqual(rejected,{'X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2',
                                  *[f'LOSS_{p}' for p in low_probability.LOSS_RATES if p < .01]})

    def test_export_only_low_pauli_cutoff_is_detected_independently(self):
        # Ordinary circuit sample can be correct while the benchmark export path
        # drops noise. Ensure the real-circuit reference catches that alone.
        def defective(binary,text,shots,seed,work):
            changed,_ = low_probability.remove_low_noise(text,'pauli')
            return low_probability.export_rows(binary,changed,shots,seed,work)
        report = low_probability.real_circuit(ROOT/'target/release/rustqec',65536,exporter=defective)
        self.assertEqual(report['status'],'FAIL')
        self.assertTrue(any('/no_visible_loss/' in e for e in report['comparison']['failed_events']))

    def test_archive_rejects_resealed_wrong_predictions_and_missing_rows(self):
        from .shot_data import rescore, ARCHIVE
        source = ROOT/'site/static/data/atom-loss'/ARCHIVE
        with zipfile.ZipFile(source) as archive:
            original = {name:archive.read(name) for name in archive.namelist()}
        member = 'tradeoff/envelope-matching-0.b8'
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp)/ARCHIVE
            for defect in ['prediction', 'missing']:
                payload = original.copy()
                index = json.loads(payload['index.json'])
                if defect == 'prediction':
                    rows = bytearray(payload[member]); rows[0] ^= 1
                    payload[member] = bytes(rows)
                    index['sha256'][member] = hashlib.sha256(rows).hexdigest()
                else:
                    del payload[member]; del index['sha256'][member]
                payload['index.json'] = json.dumps(index).encode()
                with zipfile.ZipFile(target, 'w') as archive:
                    for name, data in payload.items(): archive.writestr(name, data)
                with self.subTest(defect=defect), self.assertRaisesRegex(ValueError, 'rescore mismatch|Incomplete'):
                    rescore(target)

    def test_bell_readout_resolves_every_two_qubit_pauli(self):
        # Hand-derived Bell syndromes, phase bit then bit-flip bit, per wire.
        syndromes = {'I':[0,0], 'X':[0,1], 'Y':[1,1], 'Z':[1,0]}
        with tempfile.TemporaryDirectory() as tmp:
            for first, second in itertools.product('IXYZ', repeat=2):
                text = channel_probes.bell_text(f'{first} 0\n{second} 1'.replace('I 0\n','').replace('I 1',''), 2)
                expected = np.tile(syndromes[first]+syndromes[second], (16,1))
                with self.subTest(pauli=first+second):
                    np.testing.assert_array_equal(reference.sample(text,16), expected)
                    np.testing.assert_array_equal(correctness.rust_rows(ROOT/'target/release/rustqec', text,16,7,Path(tmp)), expected)

    def test_ix_only_channel_fails_overall_sampling_report(self):
        original = correctness.rust_rows
        def defective(binary,text,shots,seed,work):
            return original(binary, channel_probes.replace_channel(text,'DEPOLARIZE2_ix_only'), shots,seed,work)
        with patch('benchmarks.atom_loss.correctness.rust_rows', side_effect=defective):
            report = correctness.run(ROOT/'target/release/rustqec')
        self.assertEqual(report['status'], 'FAIL')
        probes = report['analytic_noise_controls']['distribution_probes']
        self.assertEqual(probes['status'], 'FAIL')
        product = next(c for c in probes['cases'] if c['case']=='DEPOLARIZE2_product_ZZ')
        self.assertEqual(product['status'], 'FAIL')
        self.assertEqual(product['rust']['marginals'][0], 0.)
        self.assertAlmostEqual(product['expected_marginals'][0],8*.17/15)
        # The old single parity signature agreed; the first marginal exposes it.
        bell = next(c for c in probes['cases'] if c['case']=='DEPOLARIZE2_bell_p0.6')
        self.assertEqual(bell['status'], 'FAIL')
        self.assertEqual(len(bell['expected_joint']),16)

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
