import hashlib
from pathlib import Path
import tempfile
import unittest
import warnings
import zipfile

from wheel_bindings import bind_wheel


class WheelBindingsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.wheel = self.root/'peer.whl'
        self.installed = self.root/'snapshot'
        self.code = {'peer/__init__.py': b'import peer._native\n', 'peer/_native.so': b'actual native bytes'}
        self.original = '/original/venv/lib/python3.12/site-packages'
        self.metadata = {'version': '1', 'files': {self.original+'/'+n: hashlib.sha256(b).hexdigest() for n, b in self.code.items()}}
        for name, data in self.code.items():
            path = self.installed/name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.write_wheel(self.code.items())

    def write_wheel(self, members):
        with warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            with zipfile.ZipFile(self.wheel, 'w') as archive:
                for name, data in members:
                    archive.writestr(name, data)

    def check(self):
        return bind_wheel(self.wheel, self.metadata, original_site_packages=self.original, installed_root=self.installed)

    def test_relocated_snapshot_binds_all_code(self):
        result = self.check()
        self.assertEqual(result['code_files'], {n: hashlib.sha256(b).hexdigest() for n, b in self.code.items()})
        self.assertEqual(result['sha256'], hashlib.sha256(self.wheel.read_bytes()).hexdigest())

    def test_installed_mutation_is_rejected(self):
        (self.installed/'peer/_native.so').write_bytes(b'changed native bytes')
        with self.assertRaisesRegex(ValueError, 'installed code bytes'):
            self.check()

    def test_installed_digest_mutation_is_rejected(self):
        self.metadata['files'][self.original+'/peer/_native.so'] = '0'*64
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            self.check()

    def test_missing_or_extra_code_cannot_hide_in_an_inventory(self):
        for change in ['missing', 'extra']:
            with self.subTest(change=change):
                metadata = {**self.metadata, 'files': dict(self.metadata['files'])}
                if change == 'missing':
                    metadata['files'].pop(self.original+'/peer/__init__.py')
                else:
                    metadata['files'][self.original+'/peer/extra.py'] = '0'*64
                with self.assertRaisesRegex(ValueError, 'inventory mismatch'):
                    bind_wheel(self.wheel, metadata, original_site_packages=self.original, installed_root=self.installed)

    def test_duplicate_wheel_members_are_rejected(self):
        self.write_wheel(list(self.code.items())+[('peer/__init__.py', self.code['peer/__init__.py'])])
        with self.assertRaisesRegex(ValueError, 'duplicate wheel'):
            self.check()

    def test_unsafe_paths_are_rejected_even_when_not_code(self):
        for name in ['../escape.txt', '/absolute.txt', 'peer/../escape.txt', 'peer\\escape.txt']:
            with self.subTest(name=name):
                self.write_wheel(list(self.code.items())+[(name, b'escape')])
                with self.assertRaisesRegex(ValueError, 'unsafe wheel'):
                    self.check()

    def test_foreign_installed_paths_are_rejected(self):
        self.metadata['files']['/another/venv/peer/extra.py'] = '0'*64
        with self.assertRaisesRegex(ValueError, 'outside original'):
            self.check()


if __name__ == '__main__':
    unittest.main()
