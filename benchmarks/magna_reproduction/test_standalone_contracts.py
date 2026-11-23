import copy
import json
from pathlib import Path
import unittest
from tempfile import TemporaryDirectory
import shutil

import standalone_contracts as contracts
from standalone_consumer import consumer_lock

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


class StandaloneContractsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalogue = json.loads((HERE/'standalone-contracts.json').read_text())
        cls.ledger = json.loads((HERE/'coverage.json').read_text())
        cls.manifest = json.loads((HERE/'reference.json').read_text())

    def check(self, catalogue=None, ledger=None):
        return contracts.check_inventory(
            self.catalogue if catalogue is None else catalogue,
            self.ledger if ledger is None else ledger, self.manifest,
            self.catalogue['reference_sha256'], ROOT)

    def test_every_helper_assertion_and_loop_is_catalogued_without_aggregate_promotion(self):
        result = self.check()
        self.assertEqual((result['helpers'], result['groups'], result['assertions'], result['loop_headers']),
                         (22, 66, 217, 226))
        self.assertEqual((result['covered_groups'], result['covered_assertions']), (0, 0))
        self.assertEqual((result['unresolved_groups'], result['unresolved_assertions']), (56, 187))
        self.assertEqual((result['unexecuted_groups'], result['unexecuted_assertions']), (10, 30))
        self.assertEqual(result['aggregate_statuses'], dict(causal='requires_execution', foundation='unmapped'))
        self.assertEqual(result['assessments'], dict(validation_gap=47, helper_context=5,
                                                   source_bound_witness=10, representation_difference=4))

    def test_missing_group_helper_loop_and_forged_coverage_all_fail(self):
        changes = [
            lambda c: c['groups'].pop(5),
            lambda c: c['sources']['foundation']['helpers'].pop(2),
            lambda c: c['sources']['causal']['helpers'].append(dict(name='missing_helper')),
            lambda c: next(g for g in c['groups'] if g['loops'])['loops'].pop(),
            lambda c: next(g for g in c['groups'] if g['assertions'])['assertions'].pop(),
            lambda c: c['groups'][3].update(status='verified'),
            lambda c: c['groups'][3].update(reason=''),
            lambda c: next(g for g in c['groups'] if g['candidate_owners'])['candidate_owners'][0].update(anchor='missing_owner'),
            lambda c: next(g for g in c['groups'] if g['candidate_owners'])['candidate_owners'][0].update(path='../outside.rs'),
        ]
        for change in changes:
            with self.subTest(change=change):
                altered = copy.deepcopy(self.catalogue)
                change(altered)
                with self.assertRaises(ValueError):
                    self.check(catalogue=altered)
        for name, method in contracts.AGGREGATES.items():
            invalid = ['architecture_specific', 'partial', 'unmapped' if name == 'causal' else 'verified']
            for status in invalid:
                altered = copy.deepcopy(self.ledger)
                next(r for r in altered['cases'] if r['id'] == method)['status'] = status
                with self.assertRaisesRegex(ValueError, 'aggregate cannot be promoted'):
                    self.check(ledger=altered)
        with self.assertRaisesRegex(ValueError, 'pinned source changed'):
            contracts.check_catalogue(
                dict(causal='int main() {}', foundation='int main() {}'),
                self.catalogue, self.ledger, self.manifest,
                self.catalogue['reference_sha256'], ROOT)

    def test_qualified_groups_require_exact_fixtures_and_explicit_adaptations(self):
        for mutation in ('missing', 'adjacent', 'adaptation', 'stale_source'):
            changed = copy.deepcopy(self.catalogue)
            group = next(group for group in changed['groups'] if group['status'] == 'bound')
            if mutation == 'missing':
                del group['qualification']
            elif mutation == 'adjacent':
                group['qualification']['witnesses'][0]['test'] = 'unknown_high_pivots_stop_reduction_and_word_admission_is_atomic'
            elif mutation == 'adaptation':
                group['qualification']['adaptation'] = 'Use a rank-only basis instead of affine values.'
            else:
                group['qualification']['witnesses'][0]['source_sha256'] = '0'*64
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.check(catalogue=changed)

    def test_reused_input_generator_is_bound_beyond_the_root_test_file(self):
        group = next(group for group in self.catalogue['groups'] if group['id'] == 'causal/check_finite_assignments')
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            fixture = root/'rmagna/tests/standalone_causal_knowledge.rs'
            helper = root/'rmagna/tests/support/original_gf2.rs'
            helper.parent.mkdir(parents=True)
            shutil.copyfile(ROOT/fixture.relative_to(root), fixture)
            shutil.copyfile(ROOT/helper.relative_to(root), helper)
            contracts.qualification(root, group)
            helper.write_text(helper.read_text().replace('6364136223846793005', '6364136223846793007'))
            with self.assertRaisesRegex(ValueError, 'fixture support source changed'):
                contracts.qualification(root, group)


    def test_discovery_ignores_literals_and_preserves_real_nested_loops_and_helpers(self):
        import standalone_discovery as lexer
        source = '\n'.join([
            'void require(bool b, const char* message) {}',
            'int helper(int value) {',
            '// require(false,"ignored");',
            'const char* s = R"tag(require(false,"ignored"); while(1){})tag";',
            'for(int j=0;j<3;j++) { require(value==j,"real"); }',
            'return value; }',
            'int main() { helper(1); require(true,"other"); }',
        ])
        found = lexer.discover(source)
        self.assertEqual(found['counts'], dict(functions=3, assertions=2, loop_headers=1))
        self.assertEqual(found['functions'][-1]['helper_calls'], ['helper', 'require'])
        self.assertEqual(found['assertions'][0]['line'], 5)
        self.assertIn('j<3', found['functions'][1]['loops'][0]['header'])
        for malformed in ['namespace x { void helper() {} }',
                          'int f(){} int f(int x){}',
                          'int f(){require(true,"broken);}',
                          'int f(){ /* missing end }']:
            with self.subTest(source=malformed), self.assertRaises(ValueError):
                lexer.discover(malformed)

    def test_locked_consumer_projects_exact_transitive_versions_without_resolving(self):
        text = (ROOT/'Cargo.lock').read_text()
        locked, count = consumer_lock(text)
        self.assertEqual(count, 28)
        import tomllib
        self.assertEqual(len(tomllib.loads(locked)['package']), count)
        self.assertEqual(consumer_lock(text), (locked, count))
        minimal = '''version=4
[[package]]
name="rmagna"
version="0.1.0"
dependencies=["dep 2.0.0"]
[[package]]
name="dep"
version="1.0.0"
[[package]]
name="dep"
version="2.0.0"
'''
        projected, size = consumer_lock(minimal)
        self.assertEqual(size, 3)
        self.assertNotIn('version = "1.0.0"', projected)
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            consumer_lock(minimal.replace('dep 2.0.0', 'dep'))
        with self.assertRaisesRegex(ValueError, 'missing'):
            consumer_lock(minimal.replace('dep 2.0.0', 'absent'))
        with self.assertRaisesRegex(ValueError, 'Python dependency'):
            consumer_lock(minimal.replace('\"dep', '\"pyo3'))


if __name__ == '__main__':
    unittest.main()
