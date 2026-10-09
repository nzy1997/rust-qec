"""Relocatable byte seals and bounded native-test checks for the CDF scout."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
EXCLUDED = {'source', 'native-target', 'test-target', '__pycache__'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory(root, *, excluded=EXCLUDED):
    return {
        str(path.relative_to(root)): {'bytes': path.stat().st_size, 'sha256': sha(path)}
        for path in sorted(root.rglob('*'))
        if path.is_file() and not set(path.relative_to(root).parts) & excluded
    }


def verify_files(root, files):
    for name, expected in files.items():
        relative = Path(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('seal paths must stay within the artifact')
        path = root / relative
        if path.stat().st_size != expected['bytes'] or sha(path) != expected['sha256']:
            raise ValueError('sealed bytes changed: ' + name)


def require_executed_tests(log):
    results = re.findall(r'test result: ok\. ([0-9]+) passed', log)
    if not results or sum(map(int, results)) < 1:
        raise ValueError('required native arithmetic/RNG filter ran zero tests')


def seal_preparation(root, revision):
    files = inventory(root)
    (root / 'seal.json').write_text(json.dumps(
        dict(created=time.time(), files=files, protocol_revision=revision), indent=2) + '\n')


def verify_preparation(root, *, live=False):
    seal = json.loads((root / 'seal.json').read_text())
    verify_files(root, seal['files'])
    current = inventory(root)
    current.pop('seal.json', None)
    if current != seal['files']:
        raise ValueError('preparation inventory changed')
    if live:
        head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        dirty = subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
        if head != seal['protocol_revision'] or dirty:
            raise ValueError('protocol checkout changed or dirty')
        for name in seal['files']:
            if name.startswith('protocol/'):
                original = ROOT / Path(name).relative_to('protocol')
                if sha(original) != seal['files'][name]['sha256']:
                    raise ValueError('running protocol differs from retained protocol: ' + name)
    return sha(root / 'seal.json')
