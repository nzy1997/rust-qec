"""Independently re-export an immutable Linux perf artifact; never time a probe."""
import argparse
import collections
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import signal
import stat
import sys
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from evidence_common import meta, read, strict
from profile_processes import Recorder

RUN = 38082672814
ARTIFACT = 11680987670
HEAD = '2ec05f5f7528b0cac618850598650bc6a078d4d9'
TAG = 'benchmark-source/near-clifford-replay-profile-protocol-2026-10-11'
ZIP_SHA = 'c3e0d934c77b565bb22deb198568fdf94cd22ec72eb5f34aa7884fb26002c9b3'
ZIP_BYTES = 24363122
WORKFLOW = 377359128
ORIGINAL_ROOT = Path('/home/runner/work/rust-qec/rust-qec')
EXPORTS = {
    'script': ['script', '--header', '-F', 'comm,pid,tid,cpu,time,event,ip,sym,dso'],
    'buildids': ['buildid-list'], 'events': ['evlist'],
    'header': ['report', '--header-only'],
    'report': ['report', '--stdio', '--no-children', '--percent-limit', '0'],
}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def restore_zip(archive, destination, digest=ZIP_SHA, size=ZIP_BYTES):
    require(meta(archive) == dict(bytes=size, sha256=digest), 'original raw ZIP differs')
    with zipfile.ZipFile(archive) as bundle:
        entries = bundle.infolist()
        filenames = {e.filename for e in entries}
        require(len(filenames) == len(entries), 'duplicate ZIP names')
        names = {e.filename.rstrip('/') for e in entries}
        require(len(names) == len(entries), 'overlapping ZIP file/directory names')
        # Validate all names and existing parents before writing any member.
        for entry in entries:
            name = PurePosixPath(entry.filename)
            require(name.parts and not name.is_absolute() and '..' not in name.parts
                    and '\\' not in entry.filename, 'unsafe ZIP name')
            require(name.as_posix() == entry.filename.rstrip('/'), 'noncanonical ZIP name')
            require(name.parts[0] in ['cdf-profile-protocol', 'profile-driver.log'], 'unexpected ZIP root')
            kind = stat.S_IFMT(entry.external_attr >> 16)
            require(kind in [0, stat.S_IFDIR if entry.is_dir() else stat.S_IFREG], 'nonregular ZIP member')
            require(name.parts[0] != 'profile-driver.log' or (len(name.parts) == 1 and not entry.is_dir()), 'invalid driver log member')
            for parent in name.parents:
                if parent.as_posix() in names:
                    require(parent.as_posix() + '/' in filenames, 'ZIP file used as directory')
            target = destination.joinpath(*name.parts)
            require(not target.exists() and not target.is_symlink(), 'restore target already exists')
            for parent in target.parents:
                if parent == destination.parent:
                    break
                require(not parent.is_symlink() and (not parent.exists() or parent.is_dir()), 'unsafe restore parent')
        for entry in entries:
            target = destination.joinpath(*PurePosixPath(entry.filename).parts)
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with bundle.open(entry) as src, target.open('xb') as dst:
                while chunk := src.read(1 << 20):
                    dst.write(chunk)


def expected_cells():
    cases = [('structural', 'full-rank-12', 1), ('structural', 'projected-rank-16', 1)]
    cases += [('counts', 'msc_d5_inject_cultivate_p1e-3', n) for n in [1, 64, 1024]]
    cases += [('counts', 'msc_d3_inject_cultivate_p1e-3', 64), ('structural', 'depth-32', 32)]
    return [(*case, policy) for case in cases for policy in ['strict', 'fused']]


def samples(text, task, cpu):
    pattern = re.compile(r'(?m)^\s*(\S+)\s+(\d+)/\s*(\d+)\s+\[(\d+)\]\s+(\d+\.\d+):\s+(\S+):')
    frame = re.compile(r'^\s*([0-9a-fA-F]+)\s+(.+?)\s+\(([^\n]+)\)\s*$')
    headers = list(pattern.finditer(text))
    require(headers, 'no decoded sample headers')
    rows = []
    counts = collections.Counter()
    mapped = native = missing = 0
    for index, header in enumerate(headers):
        comm, pid, tid, cp, timestamp, event = header.groups()
        require((int(pid), int(tid), int(cp), event) == (task['pid'], task['pid'], cpu, 'cpu-clock:u'), 'decoded task/CPU/event differs')
        block = text[header.end():headers[index + 1].start() if index + 1 < len(headers) else len(text)]
        frames = [m.groups() for line in block.splitlines() if (m := frame.fullmatch(line))]
        require(comm in [Path(task['executable']).name[:15], 'python3', 'taskset'], 'decoded command differs')
        is_native = comm == Path(task['executable']).name[:15]
        # Preserve missing frames rather than silently dropping samples.
        leaf = frames[0] if frames else None
        rows.append((comm, int(pid), int(tid), int(cp), timestamp, event, leaf if is_native else None))
        if is_native:
            native += 1
            mapped += any(f[2] in [task['executable'], Path(task['executable']).name] for f in frames)
            missing += leaf is None
            counts[tuple(leaf[1:]) if leaf else ('[no decoded leaf]', '')] += 1
    require(native >= 100 and mapped >= 100, 'insufficient native/executable-mapped samples')
    return dict(rows=rows, native=native, mapped=mapped, total=len(headers), missing=missing,
                leaf=[dict(symbol=symbol, dso=dso, count=count) for (symbol, dso), count in sorted(counts.items())])


def compare_samples(original, replay, task, cpu):
    a, b = samples(original, task, cpu), samples(replay, task, cpu)
    require(a['rows'] == b['rows'], 'raw replay sample order/time/native first-frame IP/symbol/DSO differs')
    require(a['leaf'] == b['leaf'] and a['missing'] == b['missing'], 'raw replay native leaf population differs')
    return {key: b[key] for key in ['native', 'mapped', 'total', 'missing', 'leaf']}


def build_id(notes, buildids, executable):
    ids = re.findall(r'Build ID: ([0-9a-f]+)', notes)
    matched = [line.split()[0] for line in buildids.splitlines()
               if re.fullmatch(r'[0-9a-f]+\s+' + re.escape(executable), line)]
    require(len(ids) == 1 and matched == ids, 'actual ELF / raw build-id mismatch')
    return ids[0]


def run(rec, label, argv, timeout=180, accepted=(0,)):
    result = rec.run(label, argv, timeout=timeout)
    require(result['child_waited'] and result['cancellation'] is None and result['exit_code'] in accepted, 'command failed: ' + label)
    return (rec.output / (label + '.stdout')).read_bytes()


def replay(output):
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'Linux x86 decoder required')
    root = Path.cwd().resolve()
    require(root == ORIGINAL_ROOT, 'restore root must equal original native DSO paths')
    rec = Recorder(root, output, Path(__file__))
    failure = None
    context = dict(original_run=RUN, original_head=HEAD, artifact=ARTIFACT, performance_valid=False,
                   scope='Independent symbolic raw-file replay only; no native probe execution, new timings or caller-stack completeness claim.')
    try:
        record = json.loads(run(rec, 'original-run', ['gh', 'api', f'repos/nzy1997/rust-qec/actions/runs/{RUN}']))
        require(record['id'] == RUN and record['head_sha'] == HEAD and record['head_branch'] == TAG
                and record['workflow_id'] == WORKFLOW and record['run_attempt'] == 1
                and record['status'] == 'completed' and record['conclusion'] == 'success', 'original run binding differs')
        api = json.loads(run(rec, 'original-artifact', ['gh', 'api', f'repos/nzy1997/rust-qec/actions/artifacts/{ARTIFACT}']))
        require(api['id'] == ARTIFACT and not api['expired'] and api['size_in_bytes'] == ZIP_BYTES
                and api['digest'] == 'sha256:' + ZIP_SHA and api['workflow_run']['id'] == RUN
                and api['workflow_run']['head_sha'] == HEAD, 'original artifact binding differs')
        run(rec, 'original-zip', ['gh', 'api', f'repos/nzy1997/rust-qec/actions/artifacts/{ARTIFACT}/zip'], timeout=600)
        restore_zip(output / 'original-zip.stdout', root / 'drafts')
        original = root / 'drafts/cdf-profile-protocol/drafts/profile-output'
        prep = root / 'drafts/cdf-profile-protocol/drafts/profile-preparation'
        require(strict(original, 'seal.json', meta(original / 'seal.json')['sha256']) == 1114, 'original profile inventory differs')
        data = read(original / 'profile.json')
        require(strict(prep, 'seal.json', data['preparation_seal']) == 1407, 'original preparation inventory differs')
        require(data['profiling_completed'] and not data['performance_valid'] and data['cpu'] == 0
                and data['cells'] == [list(c) for c in expected_cells()] and len(data['profiles']) == 28, 'original task manifest differs')
        require((data['baseline'], data['candidate'], data['protocol']) ==
                ('6e079197ce9ba62079744417f3f701d4562fa149', 'f2b1a474093728d0ec31c32faf5a533c65a370ac', '02ad5e9993a032b73f64f4950ae96e5a4bd0b8de'), 'source pair differs')
        # Pick a real root-owned packaged ELF, not a launcher wrapper, and keep
        # its bytes/version. Original underlying ELF hash was not retained.
        first = data['profiles'][0]['record_label']
        header = (original / (first + '-perf-header.txt.stdout')).read_text()
        paths = re.findall(r'^# cmdline : (/usr/lib/[^\n ]+/perf)\s*$', header, re.M)
        candidates = [Path(p) for p in paths] + sorted(Path('/usr/lib/linux-tools').glob('*/perf'))
        old_version = (original / 'perf-version-0.stdout').read_text().strip()
        perf = None
        for index, candidate in enumerate(dict.fromkeys(candidates)):
            if not candidate.is_file():
                continue
            actual = candidate.resolve()
            require(actual.is_relative_to('/usr/lib') and actual.name == 'perf', 'unexpected decoder location')
            info = actual.stat()
            require(info.st_uid == 0 and not (info.st_mode & 0o022), 'decoder ownership differs')
            if actual.read_bytes()[:4] != b'\x7fELF':
                continue
            version = run(rec, 'decoder-version-' + str(index), [str(actual), '--version'], accepted=(0, 1, 255))
            version_receipt = read(output / ('decoder-version-' + str(index) + '.receipt.json'))
            if version_receipt['exit_code'] == 0 and version.decode().strip() == old_version:
                perf = actual
                break
        require(perf is not None, 'matching actual ELF perf decoder unavailable')
        context['decoder'] = dict(path=str(perf), binary=meta(perf), version=old_version,
                                  original_underlying_elf_hash_bound=False)
        results = []
        for index, task in enumerate(data['profiles']):
            cell = expected_cells()[index // 2]
            roles = ['baseline', 'candidate'] if (index // 2) % 2 == 0 else ['candidate', 'baseline']
            require((task['kind'], task['name'], task['shots'], task['policy']) == cell and task['role'] == roles[index % 2], 'task order differs')
            label = task['record_label']
            directory = original / label
            raw = directory / 'perf.data'
            executable = Path(task['task']['executable'])
            require(executable == prep / task['role'] / ('near-clifford-diagnostics.bin' if task['kind'] == 'structural' else 'near-clifford-application-counts.bin'), 'executable path differs')
            require(meta(raw) == task['raw_data'] and raw.read_bytes()[:8] == b'PERFILE2'
                    and meta(executable) == task['task']['binary'] and read(directory / 'task.json') == task['task'], 'raw file/task/binary differs')
            notes = run(rec, label + '-actual-elf', ['readelf', '-n', str(executable)]).decode()
            exports = {}
            for name, args in EXPORTS.items():
                exports[name] = run(rec, label + '-' + name, [str(perf), *args, '-i', str(raw)]).decode()
            require(exports['events'].splitlines() == ['cpu-clock:u'], 'replayed event inventory differs')
            require(build_id(notes, exports['buildids'], str(executable)) == task['build_id'], 'replayed actual ELF build-id differs')
            counts = compare_samples((original / (label + '-perf-script.txt.stdout')).read_text(), exports['script'], task['task'], 0)
            require({k: counts[k] for k in ['native', 'mapped', 'total']} == task['samples'], 'replayed sample totals differ')
            results.append(dict(label=label, cell=list(cell), role=task['role'], samples=counts,
                                script_byte_equal=meta(output / (label + '-script.stdout')) == meta(original / (label + '-perf-script.txt.stdout'))))
        require(len(results) == 28, 'all raw profiles required')
        context.update(completed=True, results=results, original_zip=meta(output / 'original-zip.stdout'))
        (output / 'replay.json').write_text(json.dumps(context, indent=2) + '\n')
    except BaseException as exc:
        failure = repr(exc)
        raise
    finally:
        rec.close(failure, context)


def cancel(signum, frame):
    raise RuntimeError('replay cancelled by signal ' + str(signum))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    replay(args.out.resolve())
