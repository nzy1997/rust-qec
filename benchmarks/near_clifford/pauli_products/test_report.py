"""Profile summary-boundary and zero-duration regression-screen controls."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

from report import profile_counts, screen, table


class ReportContract(unittest.TestCase):
    def test_profile_counts_only_main_graph_and_deduplicates_ancestors(self):
        text='''Analysis prelude
    99 rstim::near_clifford::ActiveState::project_active_measurement::fake
Call graph:
    20 Thread_1 DispatchQueue_1: com.apple.main-thread (serial)
      18 start
      + 14 rstim::near_clifford::ActiveState::project_active_measurement::outer
      +   2 rstim::near_clifford::ActiveState::project_active_measurement::inner
      + 3 rstim::near_clifford::ActiveState::project_active_measurement::other
Total number in stack (recursive counted multiple, when >=5):
    99 rstim::near_clifford::ActiveState::project_active_measurement::aggregate
Sort by top of stack:
    99 rstim::near_clifford::ActiveState::project_active_measurement::aggregate
'''
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'fixture.sample.txt';path.write_text(text)
            result=profile_counts(path)
        self.assertEqual(result['main_samples'],20)
        self.assertEqual(result['inclusive_counts']['project_active_measurement'],17)

    def test_profile_ignores_other_threads_and_requires_main_graph(self):
        text='''Call graph:
    20 Thread_1 DispatchQueue_1: com.apple.main-thread (serial)
      17 rstim::near_clifford::ActiveState::single_qubit_pauli::main
    90 Thread_2 worker
      90 rstim::near_clifford::ActiveState::single_qubit_pauli::worker
'''
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'fixture.sample.txt';path.write_text(text)
            self.assertEqual(profile_counts(path)['inclusive_counts']['single_qubit_pauli'],17)
            path.write_text(text.replace('com.apple.main-thread','other-thread'))
            with self.assertRaises(ValueError): profile_counts(path)

    def test_zero_duration_queries_remain_visible_without_undefined_ratios(self):
        source=Path(__file__).resolve().parent.parent/'results/apple-m4-pauli-scale-2026-10-03.json'
        result=json.loads(source.read_text())
        result['cases']=[next(c for c in result['cases'] if c['shots']==0)]
        for pair in result['cases'][0]['runs']:
            for label in ['baseline','candidate']:
                for value in pair[label]['measurements'][0].values():
                    if isinstance(value,dict) and 'median_ns' in value:
                        value.update(median_ns=0,raw_ns=[0]*3)
        self.assertIn('n/a (zero duration)',table(result))
        control=screen(result)
        self.assertEqual(control['hits'],[])
        self.assertEqual(len(control['zero_duration_exclusions']),6)

    def test_screen_requires_all_pairs_above_the_second_threshold(self):
        source=Path(__file__).resolve().parent.parent/'results/apple-m4-pauli-scale-2026-10-03.json'
        result=json.loads(source.read_text());result['cases']=result['cases'][:1]
        for pair in result['cases'][0]['runs']:
            pair['candidate']['measurements'][0]['warm_prepared_flat']['median_ns']=int(pair['baseline']['measurements'][0]['warm_prepared_flat']['median_ns']*1.2)
        self.assertEqual(len(screen(result)['hits']),1)
        changed=copy.deepcopy(result)
        pair=changed['cases'][0]['runs'][0]
        pair['candidate']['measurements'][0]['warm_prepared_flat']['median_ns']=int(pair['baseline']['measurements'][0]['warm_prepared_flat']['median_ns']*1.09)
        self.assertEqual(screen(changed)['hits'],[])


if __name__=='__main__': unittest.main()
