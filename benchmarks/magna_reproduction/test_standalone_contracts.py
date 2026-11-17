import copy
import json
from pathlib import Path
import unittest

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
        self.assertEqual((result['unresolved_groups'], result['unresolved_assertions']), (66, 217))
        self.assertEqual(result['aggregate_statuses'], dict(causal='unmapped', foundation='unmapped'))
        self.assertEqual(result['assessments'], dict(validation_gap=47, helper_context=9,
                                                   implementation_gap=6, representation_difference=4))

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
        for method in contracts.AGGREGATES.values():
            for status in ['verified', 'architecture_specific', 'partial']:
                altered = copy.deepcopy(self.ledger)
                next(r for r in altered['cases'] if r['id'] == method)['status'] = status
                with self.assertRaisesRegex(ValueError, 'aggregate cannot be promoted'):
                    self.check(ledger=altered)
        with self.assertRaisesRegex(ValueError, 'pinned source changed'):
            contracts.check_catalogue(
                dict(causal='int main() {}', foundation='int main() {}'),
                self.catalogue, self.ledger, self.manifest,
                self.catalogue['reference_sha256'], ROOT)


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
