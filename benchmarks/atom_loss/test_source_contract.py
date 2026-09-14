"""Adversarial source-binding and actual decoder replay regression tests."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile
from .source_contract import clean_source, verify_source, ROOT


class SourceContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git('init', '-q')
        self.git('config', 'user.email', 'test@example.invalid')
        self.git('config', 'user.name', 'Source test')
        self.write('Cargo.toml', '[workspace]\nmembers = ["decoder"]\n')
        self.write('decoder/src/lib.rs', '// measured decoder\n')
        self.write('decoder/build.rs', '// measured build\n')
        self.write('benchmarks/atom_loss/run.py', '# measured harness\n')
        self.commit()
        self.record = clean_source(self.repo)

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.STDOUT)

    def write(self, name, text):
        path = self.repo/name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(text)

    def commit(self):
        self.git('add', '.'); self.git('commit', '-qm', 'test source')

    def test_later_artifact_commit_valid_but_changed_decoder_fails(self):
        self.write('site/static/data/result.json', '{}'); self.commit()
        self.assertEqual(verify_source(self.record, self.repo), self.record['source_commit'])
        self.write('decoder/src/lib.rs', '// different decoder\n'); self.commit()
        with self.assertRaisesRegex(ValueError, 'Current source/build inputs differ'):
            verify_source(self.record, self.repo)

    def test_missing_inventory_and_new_build_input_rejected(self):
        record = copy.deepcopy(self.record); del record['inputs']['decoder/build.rs']
        with self.assertRaisesRegex(ValueError, 'source inventory'):
            verify_source(record, self.repo)
        self.write('.cargo/config.toml', '[build]\nrustflags = ["-C", "opt-level=0"]\n'); self.commit()
        with self.assertRaisesRegex(ValueError, 'Current source/build inputs differ'):
            verify_source(self.record, self.repo)

    def test_dirty_generation_and_dirty_verification_rejected(self):
        self.write('benchmarks/atom_loss/run.py', '# changed harness\n')
        with self.assertRaisesRegex(ValueError, 'clean checkout'):
            clean_source(self.repo)
        with self.assertRaisesRegex(ValueError, 'Dirty source input'):
            verify_source(self.record, self.repo)

    def test_new_untracked_build_input_is_rejected(self):
        self.write('.cargo/config.toml', '[build]\nrustflags = []\n')
        with self.assertRaisesRegex(ValueError, 'Uncommitted source input'):
            verify_source(self.record, self.repo)

    def test_optimized_python_rejects_old_results_after_source_change(self):
        record_path = self.repo/'record.json'; record_path.write_text(json.dumps(self.record))
        self.write('decoder/src/lib.rs', '// changed\n'); self.commit()
        program = ('from pathlib import Path; import json; '
                   'from benchmarks.atom_loss.source_contract import verify_source; '
                   'verify_source(json.loads(Path(__import__("sys").argv[1]).read_text()), Path(__import__("sys").argv[2]))')
        for flags in [[], ['-O']]:
            result = subprocess.run([sys.executable, *flags, '-c', program, str(record_path), str(self.repo)], capture_output=True, text=True, cwd=ROOT)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Current source/build inputs differ', result.stderr)


class DecoderReplayTests(unittest.TestCase):
    def test_actual_wrong_decoder_with_unchanged_archive_is_rejected(self):
        from .decoder_replay import replay_case
        from .shot_data import cases_from
        binary = ROOT/'target/release/rustqec'
        exporter = ROOT/'target/release/examples/export_matching_benchmark'
        with tempfile.TemporaryDirectory() as tmp, zipfile.ZipFile(ROOT/'site/static/data/atom-loss/shot-data-v1.zip') as z:
            path = Path(tmp); label, case = cases_from(z.read)[0]
            # Positive control exercises real current compiler + all four backends.
            self.assertEqual(replay_case(z, label, case, path/'good', binary, exporter, False), 4)
            wrapper = path/'wrong-decoder'
            wrapper.write_text('#!'+sys.executable+'\nimport pathlib, subprocess, sys\n'
                +'subprocess.run(['+repr(str(binary))+', *sys.argv[1:]], check=True)\n'
                +'p=pathlib.Path(sys.argv[sys.argv.index("--out")+1])\n'
                +'b=bytearray(p.read_bytes()); b[0]^=1; p.write_bytes(b)\n')
            wrapper.chmod(0o755)
            with self.assertRaisesRegex(ValueError, 'Current decoder differs from archive'):
                replay_case(z, label, case, path/'bad', wrapper, exporter, False)


if __name__ == '__main__':
    unittest.main()
