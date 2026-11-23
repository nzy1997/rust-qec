"""Compiler/process receipt grammar and corruption controls; fixture data is synthetic."""
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
import json
import unittest

import coverage_rust as rust
import standalone_execution as execution


class StandaloneExecutionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root/'rmagna/tests').mkdir(parents=True)
        (self.root/'Cargo.toml').write_text('[workspace]\nmembers=["rmagna"]\n')
        (self.root/'Cargo.lock').write_text('version=4\n')
        (self.root/'rmagna/Cargo.toml').write_text('[package]\nname="rmagna"\nversion="0.1.0"\n')
        self.source = self.root/'rmagna/tests/fixture.rs'
        self.source.write_text('#[test]\nfn original_case() { assert_eq!(2 + 2, 4); }\n')
        self.binary = self.root/'fixture-binary'
        self.binary.write_bytes(b'synthetic receipt validation fixture')
        self.witness = dict(package='rmagna', target='fixture', test='original_case', source_sha256=rust.sha(self.source))
        artifact = dict(reason='compiler-artifact', manifest_path=str(self.root/'rmagna/Cargo.toml'),
                        target=dict(name='fixture', kind=['test'], src_path=str(self.source)), executable=str(self.binary))
        self.receipt = dict(schema=execution.SCHEMA,
            compiler=dict(argv=execution.command({'fixture'}), exit_code=0, stderr='', stdout=json.dumps(artifact)+'\n'),
            tests=dict(schema='rstim.magna-rust-tests.v1', source_files=rust.source_identity(self.root), targets=[
                dict(package='rmagna', target='fixture', binary=dict(path=str(self.binary), sha256=rust.sha(self.binary)),
                     list=dict(exit_code=0, stderr='', stdout='original_case: test\n\n1 test, 0 benchmarks\n'),
                     run=dict(exit_code=0, stderr='', stdout='running 1 test\ntest original_case ... ok\n\n'
                              'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s\n'))]))

    def test_matching_source_compiler_binary_and_complete_process_observations(self):
        observed = execution.validate(self.root, self.receipt, [self.witness])
        self.assertEqual(observed, dict(status='PASS', targets=['fixture'], tests=1,
                                       source_bound=True, compiler_bound=True))

    def test_missing_compiler_failed_or_fabricated_runs_and_stale_inputs_fail(self):
        changes = [
            lambda receipt: receipt['compiler'].update(stdout=''),
            lambda receipt: receipt['compiler'].update(exit_code=1),
            lambda receipt: receipt['compiler']['argv'].remove('--locked'),
            lambda receipt: receipt['tests']['source_files'].update({'Cargo.lock': '0'*64}),
            lambda receipt: receipt['tests']['targets'][0]['binary'].update(sha256='0'*64),
            lambda receipt: receipt['tests']['targets'][0]['run'].update(stdout='test result: ok. 1 passed\n'),
            lambda receipt: receipt['tests']['targets'][0]['run'].update(exit_code=1),
            lambda receipt: receipt['tests']['targets'][0]['run'].update(stdout=
                'test original_case ... ignored\n'
                'test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.0s\n'),
        ]
        for change in changes:
            receipt = deepcopy(self.receipt)
            change(receipt)
            with self.subTest(change=change), self.assertRaises(ValueError):
                execution.validate(self.root, receipt, [self.witness])

    def test_compiler_artifact_cannot_substitute_another_source_or_binary(self):
        for key, value in [('manifest_path', str(self.root/'other/Cargo.toml')),
                           ('executable', str(self.root/'other-binary')),
                           ('target', dict(name='fixture', kind=['test'], src_path=str(self.root/'other.rs')))]:
            receipt = deepcopy(self.receipt)
            artifact = json.loads(receipt['compiler']['stdout'])
            artifact[key] = value
            receipt['compiler']['stdout'] = json.dumps(artifact)+'\n'
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'does not bind the fixture'):
                execution.validate(self.root, receipt, [self.witness])


if __name__ == '__main__':
    unittest.main()
