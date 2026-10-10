import hashlib, json, os, signal, stat, subprocess, time
from pathlib import Path

def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key: ' + key)
        result[key] = value
    return result

def read(path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique)

def meta(path):
    data = path.read_bytes()
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

def strict(folder, name, expected):
    seal = folder / name
    assert meta(seal)['sha256'] == expected
    actual = {}
    for path in folder.rglob('*'):
        mode = path.lstat().st_mode
        assert stat.S_ISREG(mode) or stat.S_ISDIR(mode)
        if not path.is_file():
            continue
        if path != seal:
            actual[path.relative_to(folder).as_posix()] = meta(path)
        if path.suffix == '.json':
            read(path)
        elif path.suffix == '.jsonl':
            for line in path.read_bytes().splitlines():
                if line.strip():
                    json.loads(line, object_pairs_hook=unique)
    assert actual == read(seal)['files']
    return len(actual)

class Recorder:
    def __init__(self, root, output, source):
        self.root = root
        self.output = output
        output.mkdir(parents=True)
        (output / 'executed-controller.py').write_bytes(source.read_bytes())
        (output / 'executed-common.py').write_bytes(Path(__file__).read_bytes())
        self.children = []

    def run(self, label, argv, timeout=300):
        start = time.time()
        cancellation = None
        with (self.output / (label + '.stdout')).open('xb') as out, (self.output / (label + '.stderr')).open('xb') as err:
            child = subprocess.Popen(argv, cwd=self.root, stdout=out, stderr=err, start_new_session=True)
            self.children.append(child.pid)
            (self.output / (label + '.start.json')).write_text(json.dumps(dict(controller_pid=os.getpid(), child_pid=child.pid, argv=argv, started=start), indent=2)+'\n')
            print(label, 'START', os.getpid(), child.pid, flush=True)
            try:
                code = child.wait(timeout=timeout)
            except BaseException as exc:
                cancellation = type(exc).__name__
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                code = child.wait(timeout=20)
        receipt = dict(controller_pid=os.getpid(), child_pid=child.pid, child_waited=True, argv=argv, started=start, closed=time.time(), exit_code=code, cancellation=cancellation, stdout=meta(self.output / (label+'.stdout')), stderr=meta(self.output / (label+'.stderr')))
        (self.output / (label+'.receipt.json')).write_text(json.dumps(receipt, indent=2)+'\n')
        print(label, 'CLOSED', code, cancellation, flush=True)
        return receipt

    def close(self, failure, context):
        (self.output / 'controller-closed.json').write_text(json.dumps(dict(controller_pid=os.getpid(), children=self.children, all_children_reaped=True, failure=failure, closed=time.time(), context=context), indent=2)+'\n')
        files = {p.relative_to(self.output).as_posix():meta(p) for p in self.output.rglob('*') if p.is_file()}
        (self.output / 'seal.json').write_text(json.dumps(dict(files=files), indent=2)+'\n')
        print('SEALED', str(self.output), os.getpid(), self.children, len(files), meta(self.output / 'seal.json')['sha256'], 'failure', failure, flush=True)
