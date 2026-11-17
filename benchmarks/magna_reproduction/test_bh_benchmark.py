import copy
from fractions import Fraction
import json
from pathlib import Path
import tempfile
import unittest
import hashlib

import bh_benchmark as bh


def field(value):
    value = Fraction(value)
    return [[str(value.numerator), str(value.denominator)]] + [['0','1'] for _ in range(3)]


def checkpoint():
    # Three already weighted observations: (A,E)=(1/2,0),(0,0),(3/2,1/2).
    raw = dict(schema_version=1,kind='exact_statistics',count='3',
               sums=[field(2),field('1/2')],
               products=[field('5/2'),field('3/4'),field('1/4')],
               weight=field(3),weight_squared=field('7/2'))
    return dict(schema_version=1,status='draws_complete',committed='3',requested='3',
                statistics=json.dumps(raw))


class BhCampaignTests(unittest.TestCase):
    def setUp(self):
        self.pinned = bh.reference.load_manifest()
        self.plan = bh.read_json(bh.HERE/'paper/bh.json')

    def test_fixed_design_keeps_timing_precision_samples_targets_and_seeds_separate(self):
        bh.validate_manifest(self.plan,self.pinned)
        rows = bh.cases(self.plan)
        self.assertEqual(len(rows),91)
        self.assertEqual(sum(c['campaign']=='timing' for c in rows),82)
        self.assertEqual(sum(c['shots'] for c in rows),757760)
        self.assertEqual(len({c['id'] for c in rows}),91)
        precision = [c for c in rows if c['campaign']=='precision']
        self.assertEqual([c['seed'] for c in precision],
                         [2026101200,2026101201,2026101202,2026101600,2026101601,
                          2026101602,2026102000,2026102001,2026102002])
        self.assertTrue(all(c['variant']=='T' and c['shots']==65536 for c in precision))
        self.assertEqual(bh.configuration(precision[0],self.plan)['proposal_scale'],'10000/7')
        returned=bh.expected_manifest(self.pinned)
        returned['timing']['k'].pop()
        self.assertEqual(bh.KS[-1],100)
        with self.assertRaisesRegex(ValueError,'design/pins/sample'):
            bh.validate_manifest(returned,self.pinned)
        for mutation in [lambda p:p['precision'].update(shots=64),
                         lambda p:p['timing'].update(variants=['S']),
                         lambda p:p.update(paper='2610.22315v2'),
                         lambda p:p['timing']['overrides'].update({'40':3})]:
            altered=copy.deepcopy(self.plan)
            mutation(altered)
            with self.assertRaisesRegex(ValueError,'design/pins/sample'):
                bh.validate_manifest(altered,self.pinned)

    def test_exact_joint_totals_reconstruct_paired_covariance_without_simulation(self):
        result = bh.regenerate_statistics(checkpoint())
        self.assertEqual(result['draws'],3)
        self.assertEqual(result['acceptance'],Fraction(2,3))
        self.assertEqual(result['error'],Fraction(1,6))
        self.assertEqual(result['conditional_error'],Fraction(1,4))
        self.assertEqual(result['acceptance_variance'],Fraction(7,36))
        self.assertEqual(result['error_variance'],Fraction(1,36))
        self.assertEqual(result['conditional_error_variance'],Fraction(3,256))
        self.assertEqual(result['mean_weight'],1)
        self.assertEqual(result['ess'],Fraction(18,7))
        altered=checkpoint()
        altered['committed']='2'
        with self.assertRaisesRegex(ValueError,'counts differ'):
            bh.regenerate_statistics(altered)
        altered=checkpoint()
        raw=json.loads(altered['statistics'])
        raw['products'][1]=field(100)
        altered['statistics']=json.dumps(raw)
        with self.assertRaisesRegex(ValueError,'positive semidefinite'):
            bh.regenerate_statistics(altered)
        with self.assertRaisesRegex(ValueError,'differs'):
            bh.close(.5,result['conditional_error'],'wrong conditional denominator')

    def test_missing_entire_production_cells_cannot_be_hidden_in_a_complete_receipt(self):
        receipt=dict(schema='rmagna.paper-bh-capture.v1',status='recorded',
                     manifest_sha256=bh.json_hash(self.plan),cells=[])
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/'receipt.json').write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError,'cell inventory'):
                bh.check_capture(self.plan,root,self.pinned)

    def test_actual_rust_execution_rejects_false_target_proxy_variance_and_warmup_claims(self):
        fixture=bh.read_json(bh.HERE/'paper/bh-checker-reference.json')
        original=fixture['stage']
        case=original['case']
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            files=bh.input_files(case)
            for name,text in fixture['inputs'].items():
                path=root/files[name]
                path.parent.mkdir(parents=True,exist_ok=True)
                path.write_text(text)
            def validate(stage):
                return bh.validate_cell(case,stage,root,self.plan,fixture['identity'],self.pinned)
            result=validate(original)
            self.assertEqual(result['case']['shots'],2048)
            self.assertEqual(result['resources']['target']['maximum_terms'],1)
            mutations=[
                lambda p:p.update(output_contract='single_output_fidelity'),
                lambda p:p['configuration'].update(seed=1),
                lambda p:p['metrics']['standard_errors'].update(conditional_error=1),
                lambda p:p['excluded_warmup'].update(draws=0),
                lambda p:p['inventory']['after_sampling']['work']['source'].update(phase_maps_prepared=999),
                lambda p:p['inventory']['after_warmup'].update(source_assignments=True),
                lambda p:p['checkpoint']['identity'].update(ideal_state='0'*64),
                lambda p:p['build'].update(debug_assertions=True),
            ]
            for mutation in mutations:
                altered=copy.deepcopy(original)
                mutation(altered['payload'])
                with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                    validate(altered)
            ideal=root/files['ideal.magna']
            ideal.write_text('H 0\nT 0\n')
            with self.assertRaisesRegex(ValueError,'complete task/target'):
                validate(original)
            ideal.write_text(fixture['inputs']['ideal.magna'])
            native=bh.read_json(bh.ROOT/'rmagna/tests/fixtures/bh-native-inputs.json')
            physical=root/files['physical.magna']
            physical.write_text(native['cases']['S']['inputs']['physical.magna'].replace('Z_ERROR(0.001)','Z_ERROR(1/1000)'))
            altered=copy.deepcopy(original)
            altered['payload']['inputs']['physical_sha256']=hashlib.sha256(physical.read_bytes()).hexdigest()
            with self.assertRaisesRegex(ValueError,'proxy substituted'):
                validate(altered)


if __name__ == '__main__':
    unittest.main()
