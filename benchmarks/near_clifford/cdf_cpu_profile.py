"""Sample the exact full36 probes; all sampled timings are diagnostic only."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import signal
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from evidence_common import meta, read
from profile_processes import Recorder

BASE = '6e079197ce9ba62079744417f3f701d4562fa149'
CANDIDATE = 'f2b1a474093728d0ec31c32faf5a533c65a370ac'
PROTOCOL = '02ad5e9993a032b73f64f4950ae96e5a4bd0b8de'
EXPORTS = {'perf-script.txt': ['script', '--header', '-F', 'comm,pid,tid,cpu,time,event,ip,sym,dso'],
           'perf-report.txt': ['report', '--stdio', '--no-children', '--percent-limit', '0'],
           'perf-buildids.txt': ['buildid-list'], 'perf-header.txt': ['report', '--header-only'],
           'perf-events.txt': ['evlist']}
THREADS = ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'RAYON_NUM_THREADS']
PHASE_TESTS = {
    'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::' + name for name in [
        'i_pow_modulo_literals_keep_exact_components_for_all_u8_values',
        'scalar_cdf_preserves_all_unsigned_modulo_phase_aliases',
        'phase_specialized_scalar_and_packet_cdfs_match_independent_frozen_bits']}
WIDE_TESTS = {'compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails'}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def fixed_cells():
    specs = [('structural', 'full-rank-12', 1), ('structural', 'projected-rank-16', 1)]
    specs += [('counts', 'msc_d5_inject_cultivate_p1e-3', shots) for shots in [1, 64, 1024]]
    specs += [('counts', 'msc_d3_inject_cultivate_p1e-3', 64), ('structural', 'depth-32', 32)]
    return [(kind, name, shots, policy) for kind, name, shots in specs for policy in ['strict', 'fused']]


def structural_config(circuit, shots, policy, action, call_kind='flat'):
    require(policy in ['strict', 'fused'] and action in ['dump', 'bench'], 'invalid structural task')
    require(type(shots) is int and shots > 0, 'invalid shots')
    require(call_kind in ['structured', 'flat'], 'invalid call kind')
    return dict(circuit=str(circuit), arithmetic=policy, cache_bytes=64 << 20,
                shots=(1 if call_kind == 'structured' else 129) if action == 'dump' else shots,
                total=129 if action == 'dump' else shots, repetitions=32, action=action, call_kind=call_kind)


def verify_samples(text, cpu, pid, executable):
    headers = list(re.finditer(r'(?m)^\s*(\S+)\s+(\d+)/\s*(\d+)\s+\[(\d+)\]\s+\d+\.\d+:\s+(\S+):', text))
    require(headers, 'actual sample headers required')
    native = mapped = 0
    for index, match in enumerate(headers):
        comm, sample_pid, tid, sample_cpu, event = match.groups()
        require(event == 'cpu-clock:u', 'unexpected sampled event')
        require(int(sample_cpu) == cpu and int(sample_pid) == pid and int(tid) == pid, 'sample CPU/process/thread differs')
        block = text[match.start():headers[index + 1].start() if index + 1 < len(headers) else len(text)]
        if comm == Path(executable).name[:15]:
            native += 1
            mapped += any(d in [executable, Path(executable).name] for d in re.findall(r'\(([^()\n]+)\)', block))
        else:
            require(comm in ['taskset', 'python3'], 'unexpected sampled command')
    require(native >= 100 and mapped >= 100, 'at least100 native executable-mapped samples required')
    return dict(native=native, mapped=mapped, total=len(headers))


def require_build_id(note_text, perf_lines, executable):
    ids = re.findall(r'Build ID: ([0-9a-f]+)', note_text)
    require(len(ids) == 1, 'one ELF GNU build-id required')
    matches = [line.split()[0] for line in perf_lines.splitlines()
               if re.fullmatch(r'[0-9a-f]+\s+' + re.escape(executable), line)]
    require(matches == ids, 'raw perf build-id differs from actual ELF')
    return ids[0]


def exec_probe(task_path, argv):
    path = Path(task_path)
    require(path.is_file() and not path.is_symlink() and path.stat().st_size == 0, 'fresh task receipt required')
    require(argv and Path(argv[0]).is_absolute() and Path(argv[0]).is_file(), 'actual executable required')
    path.write_text(json.dumps(dict(pid=os.getpid(), executable=argv[0], binary=meta(Path(argv[0])),
                                   argv=argv, affinity=sorted(os.sched_getaffinity(0)))) + '\n')
    os.execv(argv[0], argv)


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def archive_members(path):
    with tarfile.open(path) as archive:
        return {member.name: archive.extractfile(member).read() for member in archive.getmembers() if member.isfile()}


def command(rec, label, argv, timeout=180, accepted=(0,)):
    result = rec.run(label, argv, timeout=timeout)
    require(result['cancellation'] is None and result['child_waited'] and result['exit_code'] in accepted,
            'command failed; retained receipts: ' + label)
    return result


def require_exact_tests(log, expected):
    records = [re.fullmatch(r'test (\S+) \.\.\. (.+)', line) for line in log.splitlines()
               if line.startswith('test ') and not line.startswith('test result:')]
    summaries = [line for line in log.splitlines() if line.startswith('test result:')]
    require(all(record is not None for record in records) and len(records) == len(expected) and
            {record[1] for record in records} == expected and all(record[2] == 'ok' for record in records), 'exact preflight test outcomes differ')
    require(len(summaries) == 1 and re.fullmatch(
        r'test result: ok\. ' + str(len(expected)) + r' passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?', summaries[0]),
        'exact preflight summary differs')


def profile(args):
    root = args.protocol_root.resolve()
    out = args.out.resolve()
    require(out.is_relative_to(root / 'drafts'), 'fresh ignored output required')
    rec = Recorder(root, out, Path(__file__))
    print('PROFILE CONTROLLER', os.getpid(), flush=True)
    failure = None
    context = dict(performance_valid=False, profiling_completed=False)
    try:
        require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'native Linux x86 required')
        require(os.environ.get('RUSTFLAGS') == '-C target-cpu=native' and
                all(os.environ.get(key) == '1' for key in THREADS) and
                os.environ.get('CARGO_ENCODED_RUSTFLAGS') is None and
                os.environ.get('CARGO_PROFILE_RELEASE_OPT_LEVEL') is None, 'fixed native/thread flags required')
        prep = args.preparation.resolve()
        here = root / 'benchmarks/near_clifford/cdf_source_pair'
        command(rec, 'protocol-head', ['git', 'rev-parse', 'HEAD'])
        command(rec, 'protocol-status', ['git', 'status', '--porcelain'])
        require((out / 'protocol-head.stdout').read_text().strip() == PROTOCOL and
                (out / 'protocol-status.stdout').read_bytes() == b'', 'exact clean frozen protocol required')
        protocol_paths = ['benchmarks/near_clifford/cdf_source_pair']
        protocol_paths += ['benchmarks/near_clifford/' + package + '/' + name
                           for package in ['application_counts', 'diagnostics'] for name in ['main.rs', 'Cargo.toml', 'Cargo.lock']]
        protocol_paths += ['benchmarks/near_clifford/' + name for name in
                           ['application_counts/common.py', 'application_counts/manifest.json', 'diagnostics/corpus.py']]
        command(rec, 'protocol-archive', ['git', 'archive', PROTOCOL, *protocol_paths], timeout=60)
        protocol_bytes = archive_members(out / 'protocol-archive.stdout')
        prepared = read(prep / 'preparation.json')
        require(prepared['protocol_revision'] == PROTOCOL and
                prepared['heads'] == dict(baseline=BASE, candidate=CANDIDATE, control=BASE), 'exact source-pair preparation required')
        # Bind all retained protocol imports before executing their Python modules.
        preparation_seal = read(prep / 'seal.json')
        for name, expected in preparation_seal['files'].items():
            path = prep / name
            require(not path.is_symlink() and path.is_file() and meta(path) == expected, 'preparation member changed: ' + name)
            if name.startswith('protocol/'):
                relative = Path(name).relative_to('protocol').as_posix()
                require(protocol_bytes[relative] == path.read_bytes() == (root / relative).read_bytes(), 'protocol Git binding differs')
        evidence = load_module('profile_cdf_evidence', here / 'evidence.py')
        before_seal = evidence.verify_preparation(prep, live=False)
        frozen = load_module('profile_cdf_validation', here / 'run.py')
        (out / 'original-preparation-seal.json').write_bytes((prep / 'seal.json').read_bytes())
        manifest = read(here / 'manifest.json')
        for kind, name, shots, policy in fixed_cells():
            allowed = manifest['structural_cases'] if kind == 'structural' else manifest['application_counts_cases']
            require([name, shots, policy] in allowed, 'task outside frozen full36')
        roots = {role: Path(prepared['roots'][role]) for role in ['baseline', 'candidate']}
        binaries = {}
        identities = {}
        for role, head in [('baseline', BASE), ('candidate', CANDIDATE)]:
            source = roots[role]
            command(rec, role + '-head', ['git', '-C', str(source), 'rev-parse', 'HEAD'])
            command(rec, role + '-status', ['git', '-C', str(source), 'status', '--porcelain'])
            require((out / (role + '-head.stdout')).read_text().strip() == head and
                    (out / (role + '-status.stdout')).read_bytes() == b'', 'source checkout differs')
            command(rec, role + '-source-paths', ['git', '-C', str(source), 'ls-tree', '-r', '--name-only', head])
            source_paths = [name for name in (out / (role + '-source-paths.stdout')).read_text().splitlines()
                            if name.endswith('.rs') or Path(name).name in ['Cargo.toml', 'Cargo.lock']]
            require(source_paths, 'nonempty tracked source inventory required')
            command(rec, role + '-archive', ['git', '-C', str(source), 'archive', head, *source_paths], timeout=60)
            source_bytes = archive_members(out / (role + '-archive.stdout'))
            expected_sources = {name: dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest()) for name, data in source_bytes.items()
                                if name.endswith('.rs') or Path(name).name in ['Cargo.toml', 'Cargo.lock']}
            require(expected_sources and all(meta(source / name) == info for name, info in expected_sources.items()), 'live production source differs')
            identities[role] = dict(head=head, sources=expected_sources, probes={})
            binaries[role] = {}
            for kind, binary, package in [('structural', 'near-clifford-diagnostics', 'diagnostics'),
                                           ('counts', 'near-clifford-application-counts', 'application_counts')]:
                receipt = read(prep / role / ('native-build-' + kind + '.receipt.json'))
                executable = prep / role / (binary + '.bin')
                require(receipt['exit_code'] == 0 and receipt['child_waited'] and receipt['head'] == head and
                        receipt['sources'] == receipt['sources_after'] == {name: info['sha256'] for name, info in expected_sources.items()}, 'native build binding differs')
                require(receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and
                        receipt['binary']['sha256'] == meta(executable)['sha256'], 'native executable/flags differ')
                probe = prep / role / 'native-probes' / kind
                for name in ['main.rs', 'Cargo.toml', 'Cargo.lock']:
                    require(meta(probe / name)['sha256'] == receipt['probe'][name], 'build probe changed')
                    expected = protocol_bytes['benchmarks/near_clifford/' + package + '/' + name]
                    if name == 'Cargo.toml':
                        text = expected.decode()
                        require(text.count('path = "../../../rstim"') == 1, 'probe path ambiguous')
                        expected = text.replace('path = "../../../rstim"', 'path = ' + json.dumps(str(source / 'rstim'))).encode()
                    require((probe / name).read_bytes() == expected, 'public probe differs')
                binaries[role][kind] = executable
                identities[role]['probes'][kind] = dict(binary=meta(executable), probe=receipt['probe'])
                command(rec, role + '-' + kind + '-elf-notes', ['readelf', '-n', str(executable)])
        # Frozen preparation verifier checks inventory; exact preflight tests are
        # independently audited in their retained source-qualified logs/receipts.
        selections = [['--lib', 'phase_specialized_cdf_tests'], ['--test', 'near_clifford_compiled',
                       'compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails', '--', '--exact'],
                      ['--lib', 'near_clifford::compiled::probability_replay_tests']]
        for index, selection in enumerate(selections):
            receipt = read(prep / ('native-check-' + str(index) + '.receipt.json'))
            log = prep / ('native-check-' + str(index) + '.log')
            require(receipt['exit_code'] == 0 and receipt['child_waited'] and receipt['head'] == CANDIDATE and
                    receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and
                    receipt['log_sha256'] == meta(log)['sha256'], 'native preflight differs')
            require(receipt['command'] == ['rustup','run','1.93.1','cargo','test','--release','--locked','-p','rstim',
                                           '--no-default-features', *selection], 'native preflight selection differs')
            evidence.require_executed_tests(log.read_text())
            if index < 2:
                require_exact_tests(log.read_text(), PHASE_TESTS if index == 0 else WIDE_TESTS)
        prepare_module = load_module('profile_cdf_prepare', here / 'prepare.py')
        prepare_module.require_replay_tests((prep / 'native-check-2.log').read_text())
        command(rec, 'cpu', ['lscpu', '--json'])
        command(rec, 'compiler', ['rustup', 'run', '1.93.1', 'rustc', '-Vv'])
        require((out / 'compiler.stdout').read_text() == prepared['compiler'], 'compiler differs from native build')
        affinity = sorted(os.sched_getaffinity(0))
        require(affinity, 'CPU required')
        cpu = min(affinity)
        candidates = [Path('/usr/bin/perf'), *sorted(Path('/usr/lib/linux-tools').glob('*/perf'))]
        perf = None
        for index, path in enumerate(candidates):
            if not path.is_file():
                continue
            receipt = command(rec, 'perf-version-' + str(index), [str(path), '--version'], accepted=(0,1,255))
            if receipt['exit_code'] == 0:
                perf = path.resolve()
                break
        require(perf is not None, 'no working perf; do not claim profile')
        context.update(candidate=CANDIDATE, baseline=BASE, protocol=PROTOCOL, preparation=str(prep),
                       preparation_seal=before_seal, identities=identities, cells=fixed_cells(),
                       cpu=cpu, available_affinity=affinity, perf=dict(path=str(perf), identity=meta(perf)),
                       sample_scope='whole process includes setup/cold/warmup/teardown',
                       process_provenance_scope='Recorder actual command children plus exec-task PID; not universal descendant provenance',
                       witnesses=[], profiles=[])
        counter = 0
        def task(role, kind, name, shots, policy, action, call_kind='flat'):
            nonlocal counter
            circuit = here / 'fixtures' / (name + '.stim')
            require(meta(circuit)['sha256'] == manifest['inputs'][name + '.stim'], 'input differs')
            if kind == 'structural':
                path = out / ('config-' + str(counter) + '.json')
                path.write_text(json.dumps(structural_config(circuit, shots, policy, action, call_kind)) + '\n')
                argv = [str(binaries[role][kind]), str(path)]
            else:
                argv = [str(binaries[role][kind]), str(circuit), str(shots), str(1 if action == 'validate' else 32), policy, action, 'native']
            counter += 1
            return circuit, argv
        # Literal structural scalar/flat record+carry witnesses stay within each
        # source. Counts validate all8192 rows using original native self-check
        # plus independent annotation replay, with each actual requested size.
        seen = set()
        for kind, name, shots, policy in fixed_cells():
            for role in roots:
                key = (role, kind, name, policy) if kind == 'structural' else (role, kind, name, shots, policy)
                if key in seen:
                    continue
                seen.add(key)
                events = []
                for call_kind in (['structured', 'flat'] if kind == 'structural' else ['flat']):
                    action = 'dump' if kind == 'structural' else 'validate'
                    circuit, argv = task(role, kind, name, shots, policy, action, call_kind)
                    label = 'witness-' + str(counter)
                    command(rec, label, argv)
                    events.append(dict(exit_code=0, timed_out=False, result=read(out / (label + '.stdout'))))
                if kind == 'structural':
                    frozen.validate_structural_pair(*events, [name, 1, policy], manifest['inputs'][name + '.stim'])
                else:
                    frozen.validate_counts(events[0], [name, shots, policy], manifest['inputs'][name + '.stim'], circuit.read_text(), finite=True)
                context['witnesses'].append(dict(role=role, kind=kind, name=name, shots=shots, policy=policy, result='PASS'))
        for index, (kind, name, shots, policy) in enumerate(fixed_cells()):
            for role in (['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline']):
                circuit, argv = task(role, kind, name, shots, policy, 'bench')
                label = 'profile-' + str(index) + '-' + role
                directory = out / label
                directory.mkdir()
                pid_path = directory / 'task.json'
                pid_path.touch(mode=0o600)
                data_path = directory / 'perf.data'
                record = ['sudo', '-n', 'env', 'RUSTFLAGS=-C target-cpu=native', *[key + '=1' for key in THREADS],
                          'timeout', '--signal=KILL', '150s', str(perf), 'record', '-e', 'cpu-clock:u', '--sample-cpu',
                          '-F', '499', '--call-graph', 'dwarf,8192', '-o', str(data_path), '--', 'taskset', '-c', str(cpu),
                          sys.executable, '-I', '-B', str(Path(__file__).resolve()), '--exec-probe', str(pid_path), '--', *argv]
                try:
                    command(rec, label, record)
                finally:
                    if data_path.exists():
                        command(rec, label + '-ownership', ['sudo', '-n', 'chown', '--', str(os.getuid()) + ':' + str(os.getgid()), str(data_path)])
                result = read(out / (label + '.stdout'))
                require(result['status'] == 'ok' and result['input_sha256'] == manifest['inputs'][name + '.stim'], 'sampled probe result differs')
                observations = result['warm'] if kind == 'structural' else result['observations']
                frozen.validate_observations(observations, 32)
                if kind == 'structural':
                    require(result['config'] == read(Path(argv[1])) and result['arithmetic'] == policy and result['shots'] == shots, 'sampled structural task differs')
                else:
                    # Counts semantics accepts any declared observation count.
                    frozen.semantics.check_result(result, 'rstim', shots, 32, validate=False, policy=policy)
                for name_export, subcommand in EXPORTS.items():
                    command(rec, label + '-' + name_export, [str(perf), *subcommand, '-i', str(data_path)])
                task_identity = read(pid_path)
                require(task_identity['argv'] == argv and task_identity['executable'] == argv[0] and
                        task_identity['binary'] == identities[role]['probes'][kind]['binary'] and
                        task_identity['affinity'] == [cpu], 'actual exec task binding differs')
                samples = verify_samples((out / (label + '-perf-script.txt.stdout')).read_text(), cpu, task_identity['pid'], argv[0])
                require(data_path.read_bytes()[:8] == b'PERFILE2', 'raw perf magic differs')
                build_ids = (out / (label + '-perf-buildids.txt.stdout')).read_text().splitlines()
                build_id = require_build_id((out / (role + '-' + kind + '-elf-notes.stdout')).read_text(), '\n'.join(build_ids), argv[0])
                require((out / (label + '-perf-events.txt.stdout')).read_text().splitlines() == ['cpu-clock:u'], 'raw perf event inventory differs')
                context['profiles'].append(dict(role=role, kind=kind, name=name, shots=shots, policy=policy,
                    record_label=label, task=task_identity, samples=samples, raw_data=meta(data_path), build_id=build_id, performance_valid=False))
        require(len(context['profiles']) == 28, 'all14cells/bothsources required')
        require(evidence.verify_preparation(prep, live=False) == before_seal, 'preparation changed')
        for role, identity in identities.items():
            require(all(meta(roots[role] / name) == expected for name, expected in identity['sources'].items()), 'source changed during profile')
            require(all(meta(binaries[role][kind]) == expected['binary'] for kind, expected in identity['probes'].items()), 'binary changed during profile')
        context['profiling_completed'] = True
        (out / 'profile.json').write_text(json.dumps(context, indent=2) + '\n')
    except BaseException as exc:
        failure = repr(exc)
        raise
    finally:
        rec.close(failure, context)


def cancel(signum, frame):
    raise RuntimeError('profile cancelled by signal ' + str(signum))


def main():
    if len(sys.argv) > 2 and sys.argv[1] == '--exec-probe':
        separator = sys.argv.index('--')
        require(separator == 3, 'exact exec task arguments required')
        exec_probe(sys.argv[2], sys.argv[separator + 1:])
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--protocol-root', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    profile(args)


if __name__ == '__main__':
    main()
