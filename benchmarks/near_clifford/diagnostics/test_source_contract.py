"""Git-backed adversarial tests for the v2 production contract."""
import copy
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

import source_contract as contract


class ProductionContractTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.previous = contract.ROOT
        contract.ROOT = self.root
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(setattr, contract, 'ROOT', self.previous)
        self.git('init', '-q')
        self.git('config', 'user.email', 'benchmark-test@example.invalid')
        self.git('config', 'user.name', 'Benchmark contract test')
        self.put('Cargo.toml', '[workspace]\n')
        self.put('Cargo.lock', 'version = 4\n')
        self.put('rstim/Cargo.toml', '[package]\nname = "test"\n')
        self.put('rstim/src/lib.rs', 'pub fn sample() {}\n')
        self.put('renvelope/src/lib.rs', 'pub fn decode() {}\n')
        self.put('benchmarks/near_clifford/diagnostics/run.py',
                 'from source_contract import SCHEMA, CONTRACT, production_inventory\n')
        self.put('benchmarks/near_clifford/diagnostics/source_contract.py',
                 "SCHEMA = 'rstim.near-clifford-diagnostics.v2'\n")
        self.commit()

    def git(self, *arguments):
        return subprocess.check_output(['git', *arguments], cwd=self.root,
                                       stderr=subprocess.STDOUT, text=True).strip()

    def put(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-qm', 'Freeze test source')
        self.revision = self.git('rev-parse', 'HEAD')

    def receipt(self):
        sources = contract.production_inventory(self.revision, current=True)
        bound = dict(kind=contract.CONTRACT, revision=self.revision, sources=sources)
        harness = {}
        for name in ['run.py', 'source_contract.py']:
            path = 'benchmarks/near_clifford/diagnostics/' + name
            harness[path] = hashlib.sha256((self.root / path).read_bytes()).hexdigest()
        return (dict(source_revision=self.revision, sources=harness,
                     production_contract=bound),
                dict(production_contract_after=copy.deepcopy(bound)))

    def test_clean_commit_includes_dependency_and_locks(self):
        header, closure = self.receipt()
        contract.validate_contract(header, closure)
        self.assertIn('renvelope/src/lib.rs', header['production_contract']['sources'])
        self.assertIn('Cargo.lock', header['production_contract']['sources'])

    def test_dirty_rust_and_manifest_rejected_before_measurement(self):
        for name in ['renvelope/src/lib.rs', 'rstim/Cargo.toml', 'Cargo.lock']:
            with self.subTest(name=name):
                original = (self.root / name).read_bytes()
                (self.root / name).write_bytes(original + b'# changed\n')
                with self.assertRaisesRegex(ValueError, 'dirty production source'):
                    contract.production_inventory(self.revision, current=True)
                (self.root / name).write_bytes(original)

    def test_noncommit_revision_rejected(self):
        for value in ['HEAD', self.revision[:8], None, 'g' * 40]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                contract.production_inventory(value)
        for value in [self.git('rev-parse', 'HEAD^{tree}'),
                      self.git('hash-object', 'rstim/src/lib.rs')]:
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'not a Git commit'):
                contract.production_inventory(value)

    def test_resealed_incomplete_extra_or_wrong_sources_rejected(self):
        for mutation in ['missing', 'extra', 'changed']:
            header, closure = self.receipt()
            sources = header['production_contract']['sources']
            if mutation == 'missing':
                del sources['renvelope/src/lib.rs']
            elif mutation == 'extra':
                sources['invented.rs'] = '0' * 64
            else:
                sources['Cargo.lock'] = '0' * 64
            closure['production_contract_after'] = copy.deepcopy(header['production_contract'])
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, 'Git inventory differs'):
                contract.validate_contract(header, closure)

    def test_revision_and_closure_cannot_be_resealed_independently(self):
        header, closure = self.receipt()
        header['production_contract']['revision'] = '0' * 40
        closure['production_contract_after'] = copy.deepcopy(header['production_contract'])
        with self.assertRaisesRegex(ValueError, 'revision differs'):
            contract.validate_contract(header, closure)
        header, closure = self.receipt()
        closure['production_contract_after']['sources']['Cargo.lock'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'closure differs'):
            contract.validate_contract(header, closure)

    def test_producer_and_helper_hash_cannot_be_resealed(self):
        for name in ['run.py', 'source_contract.py']:
            header, closure = self.receipt()
            header['sources']['benchmarks/near_clifford/diagnostics/' + name] = '0' * 64
            with self.subTest(name=name), self.assertRaises(ValueError):
                contract.validate_contract(header, closure)

    def test_legacy_producer_cannot_be_retagged_as_current(self):
        self.put('benchmarks/near_clifford/diagnostics/run.py', "SCHEMA = 'rstim.near-clifford-diagnostics.v1'\n")
        self.commit()
        header, closure = self.receipt()
        with self.assertRaisesRegex(ValueError, 'source-bound current-production producer'):
            contract.validate_contract(header, closure)


if __name__ == '__main__':
    unittest.main()
