"""Explicit current-production contract, separate from frozen v1 evidence."""
import hashlib
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
SCHEMA = 'rstim.near-clifford-diagnostics.v2'
CONTRACT = 'measured-git-production.v1'


def require(value, message):
    if not value:
        raise ValueError(message)


def production_inventory(revision, current=False):
    """Bind every tracked Rust source and Cargo manifest/lock at an exact commit."""
    require(isinstance(revision, str) and re.fullmatch(r'[0-9a-f]{40}', revision) is not None,
            'production revision must be an exact Git commit')
    try:
        kind = subprocess.check_output(['git', 'cat-file', '-t', revision], cwd=ROOT,
                                       text=True, stderr=subprocess.PIPE).strip()
        require(kind == 'commit', 'production revision is not a Git commit')
        names = subprocess.check_output(
            ['git', 'ls-tree', '-r', '--name-only', revision], cwd=ROOT, text=True).splitlines()
    except subprocess.CalledProcessError as error:
        raise ValueError('production Git commit is unavailable') from error
    paths = [name for name in names if name.endswith('.rs')
             or Path(name).name in ('Cargo.toml', 'Cargo.lock')]
    require(paths and 'Cargo.lock' in paths and 'rstim/Cargo.toml' in paths,
            'production source inventory is incomplete')
    result = {}
    for name in paths:
        data = subprocess.check_output(['git', 'show', revision + ':' + name], cwd=ROOT)
        if current:
            require((ROOT / name).read_bytes() == data,
                    'dirty production source: ' + name)
        result[name] = hashlib.sha256(data).hexdigest()
    return result


def validate_contract(header, closure):
    contract = header.get('production_contract')
    require(isinstance(contract, dict) and set(contract) == {'kind', 'revision', 'sources'},
            'missing or malformed current-production contract')
    require(contract['kind'] == CONTRACT and contract['revision'] == header['source_revision'],
            'current-production revision differs from measured source')
    require(contract == closure.get('production_contract_after'),
            'current-production closure differs')
    require(contract['sources'] == production_inventory(contract['revision']),
            'current-production Git inventory differs')
    try:
        producer = subprocess.check_output(
            ['git', 'show', header['source_revision'] +
             ':benchmarks/near_clifford/diagnostics/run.py'], cwd=ROOT, stderr=subprocess.PIPE)
        helper = subprocess.check_output(
            ['git', 'show', header['source_revision'] +
             ':benchmarks/near_clifford/diagnostics/source_contract.py'], cwd=ROOT, stderr=subprocess.PIPE)
    except subprocess.CalledProcessError as error:
        raise ValueError('current-production producer/helper is unavailable') from error
    require(hashlib.sha256(producer).hexdigest() == header['sources'].get(
        'benchmarks/near_clifford/diagnostics/run.py'), 'current producer source differs')
    require(hashlib.sha256(helper).hexdigest() == header['sources'].get(
        'benchmarks/near_clifford/diagnostics/source_contract.py'),
        'current-production contract helper differs')
    require(b'from source_contract import' in producer and SCHEMA.encode() in helper,
            'current schema requires a source-bound current-production producer')
