from fractions import Fraction as F
import math
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import cultivation_benchmark as benchmark
import cultivation_peers as peers
import cultivation_statistics as stats


class CultivationBenchmarkTests(unittest.TestCase):
    def test_prescribed_paper_budget_disjoint_ranges_and_seeds(self):
        pinned = benchmark.reference.load_manifest()
        plan = benchmark.manifest(pinned)
        cases = list(benchmark.cases(plan))
        precision = [row for row in cases if row['kind'] != 'timing']
        streams = {}
        for row in precision:
            identifies = (row['family'], row['point'], row['kind'])
            ranges = streams.setdefault(row['seed'], (identifies, []))
            self.assertEqual(ranges[0], identifies)
            ranges[1].append((row['start'], row['start']+row['shots']))
        self.assertEqual(len(streams), 40)
        self.assertEqual(sum(row['shots'] for row in precision), 1384120320)
        self.assertEqual(len(cases)-len(precision), 12)
        for _, ranges in streams.values():
            self.assertEqual(ranges[0][0], 0)
            self.assertEqual([r[1] for r in ranges[:-1]], [r[0] for r in ranges[1:]])
        self.assertEqual(plan['families']['rp2-d5-T']['grid'][-1]['failure_seed'],2026101542)

    def test_exact_moments_and_variance_use_different_acceptance_and_failure_batches(self):
        no_closing = (0, 0, 0, False)
        acceptance = stats.moments({(no_closing,stats.ONE,stats.ZERO):3,
                                    ((1,0,0,False),stats.ZERO,stats.ZERO):1}, F(1,8),F(1,8),4,'acceptance')
        # Independent direct witness: exactly one of four observations equals
        # 1/2 with a P/Q of (1/8)/(7/8) * (7/8)^3/(1/8)^3 / (59/40).
        accepted_error = (F(1,2),F(0))
        key = (1, 3, 2, True)
        failure = stats.moments({(key,stats.ONE,accepted_error):1,
                                 ((3,3,0,False),stats.ZERO,stats.ZERO):3}, F(1,8),F(7,8),4,'failure')
        weighted = F(1,2)*49*F(40,59)
        self.assertEqual(failure['mean'],(weighted/4,0))
        self.assertEqual(failure['variance'],(weighted**2/16,0))
        self.assertEqual(acceptance['variance'],(F(1,16),0))
        combined = stats.two_estimator(acceptance,failure)
        estimate = (weighted/4)/F(3,4)
        expected_variance = (weighted**2/16 + estimate**2/16) / F(3,4)**2
        self.assertAlmostEqual(combined['conditional_error'],float(estimate))
        self.assertAlmostEqual(combined['conditional_error_variance'],float(expected_variance))
        self.assertAlmostEqual(combined['conditional_error_se'],math.sqrt(expected_variance))
        self.assertGreater(combined['conditional_error'],combined['accepted_failure'])

    def test_real_cyclotomic_values_and_impossible_closing_observations(self):
        exact = stats.real_field(['1/2','-1/4','0','1/4'])
        self.assertAlmostEqual(stats.as_float(exact),0.5-math.sqrt(2)/4)
        self.assertEqual(stats.sign(exact),1)
        self.assertEqual(stats.multiply(exact,(F(1,2),F(1,4))),(F(1,8),0))
        with self.assertRaisesRegex(ValueError,'not real'):
            stats.real_field(['1/2','1/4','0','1/4'])
        with self.assertRaisesRegex(ValueError,'closing normalizer'):
            stats.weight_key(dict(occurrences=3,tail_candidates=15,passing_candidates=3,passes=True),
                             (15,1,1,1),2,[])
        with self.assertRaisesRegex(ValueError,'offset parity'):
            stats.weight_key(dict(occurrences=0,tail_candidates=1,passing_candidates=0,passes=False),
                             (1,1,1,1),1,[])

    def test_sampled_peer_readout_preserves_signed_absolute_detector_rows(self):
        source = '''RX 0
T 0
MX(0.001) 0
DETECTOR(1, 0, 0) rec[-1]
CX rec[-1] 1
MPP X0*X1 Z0*Z1
DETECTOR rec[-3] rec[-1]
OBSERVABLE_INCLUDE(4) rec[-1]
POSTSELECT det[1] !det[0]
'''
        text, expected = peers.peer_circuit(source,dict(outputs=[1],ideal_state='target.magna'),'H 0\nT 0')
        self.assertIn('T 0\nMX(0.001) 0\nDETECTOR(1, 0, 0) rec[-1]\nCX rec[-1] 1\n',text)
        self.assertTrue(text.endswith('T_DAG 1\nH 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]\n'))
        self.assertNotIn('OBSERVABLE_INCLUDE(4)',text)
        self.assertEqual(peers.scoring_layout(text,expected),
                         dict(detectors=[[0],[0,2]],expected=[True,False],target_record=3))
        with self.assertRaisesRegex(ValueError,'every selected detector'):
            peers.peer_circuit(source.replace(' !det[0]',''),dict(outputs=[1]),'')
        with self.assertRaisesRegex(ValueError,'unsupported cultivation target'):
            peers.peer_circuit(source,dict(outputs=[1]),'H 0')

    def test_ppvm_inversion_lowering_restores_the_projected_qubit_and_record_position(self):
        source = 'M(0.01) 0 !1 2\nMX !3\nMY !4\nCX rec[-1] 5\n'
        self.assertEqual(peers.lower_inverted_measurements(source),
                         'M(0.01) 0\nX 1\nM(0.01) 1\nX 1\nM(0.01) 2\n'
                         'Z 3\nMX 3\nZ 3\nX 4\nMY 4\nX 4\nCX rec[-1] 5\n')

    def test_peer_failure_inventory_requires_every_raw_attempt(self):
        case = dict(id='physical-0', family='example')
        expected = {(case['id'],'ppvm'):case}
        with TemporaryDirectory() as scratch:
            root = Path(scratch).resolve()
            previous = root/'previous-attempts'
            previous.mkdir()
            file = previous/'attempt.json'
            stage = dict(case=case, backend='ppvm', status='failed', returncode=1)
            benchmark.atomic(file,stage)
            row = dict(stage,path=file.name,sha256=benchmark.reference.digest(file))
            receipt = dict(previous_failures=[row])
            self.assertEqual(peers.previous_failure_inventory(root,receipt,expected),[row])
            with self.assertRaisesRegex(ValueError,'missing previous timing attempts'):
                peers.previous_failure_inventory(root,{},expected)
            with self.assertRaisesRegex(ValueError,'inventory differs'):
                peers.previous_failure_inventory(root,dict(previous_failures=[]),expected)
            with self.assertRaisesRegex(ValueError,'inventory differs'):
                peers.previous_failure_inventory(root,dict(previous_failures=[row,row]),expected)
            file.unlink()
            with self.assertRaisesRegex(ValueError,'inventory differs'):
                peers.previous_failure_inventory(root,receipt,expected)

    def test_peer_reuse_checks_active_failed_batches_before_retry(self):
        case = dict(id='physical-0', family='example')
        wanted = {(case['id'],backend):case for backend in ('rust','ppvm')}
        with TemporaryDirectory() as scratch:
            root = Path(scratch).resolve()
            batches = root/'batches'
            batches.mkdir()
            rows = []
            for backend,status in [('rust','invalid'),('ppvm','failed')]:
                path = batches/f'{backend}.json'
                stage = dict(case=case,backend=backend,status=status)
                benchmark.atomic(path,stage)
                rows.append(dict(stage,path=path.name,sha256=benchmark.reference.digest(path)))
            receipt = dict(schema=peers.SCHEMA,backends=['ppvm'],cells=rows)
            self.assertEqual(set(peers.active_batch_inventory(root,receipt,wanted)),set(wanted))
            with self.assertRaisesRegex(ValueError,'active matched batch inventory differs'):
                peers.active_batch_inventory(root,dict(receipt,cells=rows[:1]),wanted)
            (batches/'ppvm.json').unlink()
            with self.assertRaisesRegex(ValueError,'inventory dropped a declared cell'):
                peers.active_batch_inventory(root,dict(receipt,cells=rows[:1]),wanted)

    def test_peer_reuse_history_requires_failed_attempts_from_every_parent(self):
        case = dict(id='physical-0',family='example')
        failed = dict(case=case,backend='ppvm',status='failed',sha256='a'*64)
        identity = dict(host=dict(system='Linux-test',machine='x86_64',fingerprint='b'*64),
                        output_contract=peers.OUTPUT_CONTRACT)
        with TemporaryDirectory() as scratch:
            root = Path(scratch).resolve()
            history = root/'reuse-history'
            history.mkdir()
            parent = dict(identity,schema=peers.SCHEMA,reuse_parent_sha256=None,
                          cells=[failed],previous_failures=[])
            path = history/'parent.json'
            benchmark.atomic(path,parent)
            digest = benchmark.reference.digest(path)
            saved = path.rename(history/f'{digest}.json')
            child = dict(parent,reuse_parent_sha256=digest,cells=[],previous_failures=[failed])
            self.assertEqual(peers.reuse_history(root,child),{digest:saved})
            with self.assertRaisesRegex(ValueError,'dropped an inherited failed attempt'):
                peers.reuse_history(root,dict(child,previous_failures=[]))
            with self.assertRaisesRegex(ValueError,'host or output contract changed'):
                peers.reuse_history(root,dict(child,host=dict(identity['host'],fingerprint='c'*64)))
            with self.assertRaisesRegex(ValueError,'history inventory differs'):
                peers.reuse_history(root,dict(child,reuse_parent_sha256=None))

    def test_native_peer_compares_counts_and_integrated_scores(self):
        case = dict(seed=3,shots=8)
        times = dict(sampling_seconds=0.25,decoder_seconds=0)
        estimates = dict(acceptance=0.625,error=0.125,conditional_error=0.2)
        direct = dict(status='complete',requested=8,completed=8,seed=3,not_attempted=0,
                      failed=0,failures=[],timings=times,estimates=estimates)
        payload = dict(direct=direct,completed=8,sampling_seconds=0.25)
        self.assertEqual(peers.native_acceptance(payload,case),0.625)
        for changes in (dict(status='failed'),dict(completed=0),dict(seed=0),dict(timings=dict(times,sampling_seconds=0.5))):
            with self.assertRaisesRegex(ValueError,'counts, seed or timing'):
                peers.native_acceptance(dict(payload,direct=dict(direct,**changes)),case)
        for changes in (dict(error=-1),dict(conditional_error=0.125),dict(error=0.75)):
            with self.assertRaisesRegex(ValueError,'joint or conditional score'):
                peers.native_acceptance(dict(payload,direct=dict(direct,estimates=dict(estimates,**changes))),case)

    def test_symft_direct_counts_and_stage_identity_must_match(self):
        case = dict(shots=8)
        direct = dict(shots=8,accepted=5,discarded=3,logical_errors=1,active_threads=1)
        payload = dict(direct=direct,completed=8,accepted=5,accepted_failure=1)
        peers.check_symft_counts(payload,case)
        for changes in (dict(accepted=4),dict(logical_errors=0),dict(discarded=4),dict(active_threads=2)):
            with self.assertRaisesRegex(ValueError,'SymFT direct counts changed'):
                peers.check_symft_counts(dict(payload,direct=dict(direct,**changes)),case)
        identity = dict(host=dict(system='Linux-test',machine='x86_64'),output_contract=peers.OUTPUT_CONTRACT)
        peers.check_timing_identity(identity,identity)
        for key in identity:
            with self.assertRaisesRegex(ValueError,'host or output contract changed'):
                peers.check_timing_identity({k:v for k,v in identity.items() if k!=key},identity)


if __name__ == '__main__':
    unittest.main()
