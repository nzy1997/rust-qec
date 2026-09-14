"""Retained negative controls must support their rejection claims from observations."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from . import correctness, low_probability
from .report_contract import verify_sampler

ROOT = Path(__file__).resolve().parents[2]


class SamplerMutationEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # One executed experiment supplies every test; individual report mutations
        # never rerun sampling or substitute fabricated positive observations.
        cls.report = correctness.run(ROOT/'target/release/rustqec')
        if cls.report['status'] != 'PASS':
            raise ValueError('Fresh sampling experiment failed')

    def test_producer_retains_complete_negative_observations(self):
        report = self.report
        analytic = report['analytic_noise_controls']
        for family in [analytic['channel_deletion_mutations'],
                       analytic['distribution_probes']['channel_replacement_mutations']]:
            for control in family.values():
                self.assertEqual(control['failed_cases'],
                                 [r['case'] for r in control['observations'] if r['status']=='FAIL'])
                self.assertTrue(control['rejected'])
        unit = report['low_probability_controls']['analytic']
        intervals = {r['case']:r['accepted_counts'] for r in unit['cases']}
        for kind,control in unit['low_probability_deletion_mutations'].items():
            expected = {name:removed for name,text,_,_ in low_probability.specifications()
                        if (removed:=low_probability.remove_low_noise(text,kind)[1])}
            self.assertEqual({r['case']:r['removed_instructions'] for r in control['observations']}, expected)
            self.assertEqual(set(control['failed_cases']),set(expected))
            for observation in control['observations']:
                self.assertTrue(any(k<lo or k>hi for k,(lo,hi) in
                                    zip(observation['counts'],intervals[observation['case']],strict=True)))
        for control in report['low_probability_controls']['real_circuit']['low_probability_deletion_mutations'].values():
            self.assertEqual(control['failed_events'],control['comparison']['failed_events'])
            self.assertEqual(len(control['comparison']['events']),196)
        verify_sampler(report)

    def corrupted_reports(self):
        cases = []
        def mutate(label, action):
            report = copy.deepcopy(self.report)
            action(report)
            cases.append((label, report))
        def low(report):
            return report['low_probability_controls']['analytic']['low_probability_deletion_mutations']['pauli']
        def noise(report):
            return report['analytic_noise_controls']['channel_deletion_mutations']['X_ERROR']
        def distribution(report):
            return report['analytic_noise_controls']['distribution_probes']['channel_replacement_mutations']['DEPOLARIZE2_ix_only']
        def real(report):
            return report['low_probability_controls']['real_circuit']['low_probability_deletion_mutations']['pauli']

        mutate('original truncated failure list',lambda r:low(r).update(failed_cases=low(r)['failed_cases'][:1]))
        mutate('unaffected LOSS_0.01 witness',lambda r:low(r).update(failed_cases=['LOSS_0.01']))
        mutate('missing negative observations',lambda r:low(r).pop('observations'))
        mutate('missing affected probe',lambda r:low(r)['observations'].pop())
        mutate('duplicate affected probe',lambda r:low(r)['observations'].append(copy.deepcopy(low(r)['observations'][0])))
        mutate('unaffected probe raw record',lambda r:low(r)['observations'][0].update(case='LOSS_0.01'))
        mutate('false mutation verdict',lambda r:low(r).update(rejected=False))
        mutate('altered deleted instruction count',lambda r:low(r)['observations'][0].update(removed_instructions=2))
        mutate('incomplete raw counts',lambda r:low(r)['observations'][0].update(counts=[]))
        def healthy_low(report):
            healthy = {r['case']:r['counts']['rust'] for r in report['low_probability_controls']['analytic']['cases']}
            for observation in low(report)['observations']:
                observation['counts'] = healthy[observation['case']]
        mutate('healthy observations cannot claim channel deletion rejected',healthy_low)
        mutate('analytic healthy observations cannot claim rejection',
               lambda r:noise(r).update(observations=copy.deepcopy(r['analytic_noise_controls']['cases'])))
        mutate('analytic unrelated witness',lambda r:noise(r).update(failed_cases=['Z_ERROR_alive']))
        mutate('distribution missing observations',lambda r:distribution(r).update(observations=[]))
        mutate('distribution altered raw marginal',
               lambda r:distribution(r)['observations'][0]['rust'].update(marginals=[]))
        mutate('distribution healthy observations cannot claim rejection',lambda r:distribution(r).update(
            observations=copy.deepcopy(r['analytic_noise_controls']['distribution_probes']['cases'])))
        mutate('real circuit missing raw events',lambda r:real(r)['comparison'].update(events=[]))
        mutate('real circuit false event count',lambda r:real(r)['comparison']['events'][0].update(rust_events=999999))
        mutate('real circuit stale pvalue',lambda r:real(r)['comparison']['events'][0].update(pvalue=-1.))
        mutate('real circuit false rejection list',lambda r:real(r).update(failed_events=[]))
        mutate('real circuit wrong removed instructions',lambda r:real(r).update(removed_instructions=1))
        return cases

    def test_negative_claims_are_recomputed_from_raw_observations(self):
        for label,report in self.corrupted_reports():
            with self.subTest(mutation=label), self.assertRaises((ValueError,KeyError)):
                verify_sampler(report)

    def test_optimized_python_rejects_the_same_negative_claims(self):
        script = '''import json,sys
from benchmarks.atom_loss.report_contract import verify_sampler
healthy,cases=json.load(open(sys.argv[1]))
verify_sampler(healthy)
for label,report in cases:
    try:
        verify_sampler(report)
    except (ValueError,KeyError):
        continue
    raise RuntimeError('Accepted corrupted negative evidence: '+label)
print('PASS: optimized negative-observation checks')
'''
        with tempfile.TemporaryDirectory(prefix='sampler-mutations-') as tmp:
            path = Path(tmp)/'reports.json'
            path.write_text(json.dumps([self.report,self.corrupted_reports()]))
            result = subprocess.run([sys.executable,'-O','-c',script,str(path)],cwd=ROOT,
                                    capture_output=True,text=True,timeout=120)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)


if __name__ == '__main__':
    unittest.main()
