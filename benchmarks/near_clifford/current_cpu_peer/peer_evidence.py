"""Portable byte seals with explicit preparation and worker lifecycle coverage."""
import hashlib
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def inventory(root, *, bundle=False):
    result = {}
    for path in sorted(root.rglob('*')):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        parts = relative.parts
        offset = 1 if bundle and parts[0] == 'preparation' else 0
        prepared = parts[offset:]
        if (bundle and parts[0] == 'analysis') or any(p in {'native-target', 'test-target', '__pycache__', '.git'} for p in parts):
            continue
        if prepared[0] in {'source', 'symft', 'clifft'} or prepared[:2] == ('rust', 'source'):
            continue
        require(not path.is_symlink(), 'sealed evidence must contain literal files')
        result[str(relative)] = dict(bytes=path.stat().st_size, sha256=sha(path))
    return result


def verify_files(root, files):
    for name, meta in files.items():
        relative = Path(name)
        require(not relative.is_absolute() and '..' not in relative.parts and name == str(relative), 'unsafe seal member')
        path = root/relative
        require(not path.is_symlink() and path.stat().st_size == meta['bytes'] and sha(path) == meta['sha256'], 'sealed bytes changed '+name)


def seal_preparation(root, revision):
    require(not (root/'seal.json').exists(), 'preparation seal already exists')
    (root/'seal.json').write_text(json.dumps(dict(created=time.time(), protocol_revision=revision, files=inventory(root)), indent=2)+'\n')


def verify_preparation(root, *, live=False):
    seal = read(root/'seal.json')
    verify_files(root, seal['files'])
    current = inventory(root)
    current.pop('seal.json', None)
    require(current == seal['files'], 'preparation inventory changed')
    if live:
        require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == seal['protocol_revision']
                and not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT), 'protocol changed or dirty')
        for name, meta in seal['files'].items():
            if name.startswith('protocol/'):
                require(sha(ROOT/Path(name).relative_to('protocol')) == meta['sha256'], 'running protocol changed')
    return sha(root/'seal.json')


def receipt_pids(receipt):
    values = [receipt['controller_pid'], receipt['child_pid']]
    require(all(type(v) is int and v > 0 for v in values), 'invalid retained PID')
    return values


def preparation_pids(root):
    pids = []
    for name in read(root/'seal.json')['files']:
        if name.endswith('.receipt.json'):
            receipt = read(root/name)
            require(receipt['child_waited'] is True and type(receipt['exit_code']) is int and receipt['exit_code'] == 0
                    and receipt.get('cancellation') is None and not receipt.get('timed_out', False), 'preparation child not successfully closed')
            pids.extend(receipt_pids(receipt))
    require(bool(pids), 'preparation receipts missing')
    return pids


def verify_absence(receipt, pids):
    values = sorted(set(pids))
    require(receipt['pids'] == values and receipt['command'] == ['ps', '-p', ','.join(map(str, values)), '-o', 'pid=,comm=']
            and type(receipt['exit_code']) is int and receipt['exit_code'] == 1
            and receipt['stdout'] == receipt['stderr'] == '', 'historical complete PID absence differs')


def seal_bundle(root):
    require(not (root/'original-seal.json').exists(), 'original seal already exists')
    preparation = root/'preparation'
    preparation_sha = verify_preparation(preparation)
    outer = read(root/'control/closure.json')
    require(outer['child_waited'] is True and type(outer['exit_code']) is int and outer['exit_code'] == 0
            and outer['timed_out'] is False and outer['cancellation'] is None and outer['post_run_seal_error'] is None
            and outer['preparation_seal_before'] == outer['preparation_seal_after'] == preparation_sha, 'producer not successfully closed')
    pids = preparation_pids(preparation)+receipt_pids(outer)
    for line in (root/'output/events.jsonl').read_text().splitlines():
        event = json.loads(line)
        require(event['child_waited'] is True, 'worker not waited')
        pids.extend(receipt_pids(event))
    command = ['ps', '-p', ','.join(map(str, sorted(set(pids)))), '-o', 'pid=,comm=']
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        stdout, stderr = child.communicate(timeout=30)
    except BaseException:
        child.kill()
        child.communicate(timeout=10)
        raise
    receipt = dict(command=command, pids=sorted(set(pids)), controller_pid=__import__('os').getpid(), child_pid=child.pid,
                   child_waited=True, exit_code=child.returncode, stdout=stdout.decode(), stderr=stderr.decode())
    (root/'process-absence.json').write_text(json.dumps(receipt, indent=2)+'\n')
    verify_absence(receipt, pids)
    (root/'original-seal.json').write_text(json.dumps(dict(created=time.time(), scope='All native originals sealed before external performance interpretation', files=inventory(root, bundle=True)), indent=2)+'\n')
