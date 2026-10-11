"""Sample only the acknowledged warm loop; preserve raw samples and finite witnesses."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import signal
import shutil
import stat
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from evidence_common import meta, read
from profile_processes import Recorder
from warm_profile_contract import finite_payload, perf_clock, record_perf_path, select_samples

BASE = '6e079197ce9ba62079744417f3f701d4562fa149'
CANDIDATE = 'cc13770aded27b434cdedd8a07c2a7adae3f5ec4'
PROTOCOL = '73107400ee67376cfa549f1d1b9c4e20e7c0f7d5'
EXPORTS = {'perf-script.txt': ['script', '--header', '--ns', '-F', 'comm,pid,tid,cpu,time,event,ip,sym,dso'],
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
REAL_TESTS = {'near_clifford::compiled::real_coherent_packet::tests::' + name for name in [
    'real_projection_matches_complex_for_all_small_gauges_and_mixed_branches',
    'real_rotations_match_complex_policies_expansion_and_wide_lanes',
    'real_packet_limits_and_tiny_positive_branches_match_complex',
    'eligible_counts_preserve_raw_records_rng_and_positive_logical_errors',
    'msc_real_packets_preserve_both_counts_policies_and_rng']}
LAYOUT_TESTS = {'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage'}
NATIVE_CHECKS = [(['--lib', 'phase_specialized_cdf_tests'], PHASE_TESTS),
                (['--test', 'near_clifford_compiled',
                  'compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails', '--', '--exact'], WIDE_TESTS),
                (['--lib', 'near_clifford::compiled::real_coherent_packet::tests::'], REAL_TESTS),
                (['--lib', 'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage', '--', '--exact'], LAYOUT_TESTS)]



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
                                   argv=argv, affinity=sorted(os.sched_getaffinity(0)),
                                   phase_paths={key: os.environ.get(key) for key in
                                       ['RSTIM_PHASE_CONTROL', 'RSTIM_PHASE_ACK', 'RSTIM_PHASE_REPORT']})) + '\n')
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


def validate_native_preflight(prep, evidence):
    for index, (selection, expected) in enumerate(NATIVE_CHECKS):
        receipt = read(prep / ('native-check-' + str(index) + '.receipt.json'))
        log = prep / ('native-check-' + str(index) + '.log')
        require(receipt['exit_code'] == 0 and receipt['child_waited'] and receipt['head'] == CANDIDATE and
                receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and
                receipt['log_sha256'] == meta(log)['sha256'], 'native preflight differs')
        require(receipt['command'] == ['rustup', 'run', '1.93.1', 'cargo', 'test', '--release', '--locked', '-p', 'rstim',
                                      '--no-default-features', *selection], 'native preflight selection differs')
        evidence.require_executed_tests(log.read_text())
        require_exact_tests(log.read_text(), expected)


def profile(args):
    root = args.protocol_root.resolve()
    warm_root = args.warm_root.resolve()
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
        require(not any(key.startswith('RSTIM_PHASE_') for key in os.environ), 'phase environment must start empty')
        command(rec, 'warm-head', ['git', '-C', str(warm_root), 'rev-parse', 'HEAD'])
        command(rec, 'warm-status', ['git', '-C', str(warm_root), 'status', '--porcelain'])
        warm_head = (out / 'warm-head.stdout').read_text().strip()
        require(re.fullmatch(r'[0-9a-f]{40}', args.warm_revision) and warm_head == args.warm_revision and
                (out / 'warm-status.stdout').read_bytes() == b'', 'clean exact warm adapter required')
        warm_paths = ['benchmarks/near_clifford/' + name for name in
                      ['cdf_warm_profile.py', 'warm_profile_contract.py', 'evidence_common.py',
                       'profile_processes.py', 'test_warm_profile_contract.py', 'phase_gate.rs', 'warm_probes/diagnostics.rs', 'warm_probes/application_counts.rs']]
        warm_paths += ['.github/workflows/near-clifford-x86-application-counts.yml']
        command(rec, 'warm-archive', ['git', '-C', str(warm_root), 'archive', warm_head, *warm_paths])
        warm_bytes = archive_members(out / 'warm-archive.stdout')
        require(set(warm_bytes) == set(warm_paths) and all((warm_root / name).read_bytes() == data
                for name, data in warm_bytes.items()), 'warm adapter differs from archived Git source')
        require(Path(__file__).resolve() == warm_root / 'benchmarks/near_clifford/cdf_warm_profile.py',
                'actual warm controller location differs')
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
        originals = {role: dict(routes) for role, routes in binaries.items()}
        for role, source in roots.items():
            identities[role]['original_probes'] = identities[role]['probes']
            identities[role]['probes'] = {}
            probe_parent = out / 'warm-probes' / role
            probe_parent.mkdir(parents=True)
            helper = probe_parent / 'phase_gate.rs'
            helper.write_bytes(warm_bytes['benchmarks/near_clifford/phase_gate.rs'])
            for kind, binary, package in [('structural', 'near-clifford-diagnostics', 'diagnostics'),
                                           ('counts', 'near-clifford-application-counts', 'application_counts')]:
                probe = probe_parent / kind
                probe.mkdir()
                for name in ['main.rs', 'Cargo.toml', 'Cargo.lock']:
                    relative = 'benchmarks/near_clifford/' + package + '/' + name
                    data = warm_bytes['benchmarks/near_clifford/warm_probes/' + package + '.rs'] if name == 'main.rs' else protocol_bytes[relative]
                    if name == 'Cargo.toml':
                        text = data.decode()
                        require(text.count('path = "../../../rstim"') == 1, 'warm dependency path ambiguous')
                        data = text.replace('path = "../../../rstim"', 'path = ' + json.dumps(str(source / 'rstim'))).encode()
                    (probe / name).write_bytes(data)
                probe_identity = {name: meta(probe / name) for name in ['main.rs', 'Cargo.toml', 'Cargo.lock']}
                probe_identity['../phase_gate.rs'] = meta(helper)
                # Build targets stay outside retained evidence and sealed preparation.
                target = warm_root / 'drafts' / 'warm-native-target' / role / kind
                build_args = ['env', 'CARGO_TARGET_DIR=' + str(target), 'rustup', 'run', '1.93.1', 'cargo',
                              'build', '--release', '--locked', '--manifest-path', str(probe / 'Cargo.toml')]
                command(rec, role + '-warm-build-' + kind, build_args, timeout=600)
                executable = probe_parent / (binary + '.bin')
                shutil.copyfile(target / 'release' / binary, executable)
                executable.chmod(0o755)
                require(all(meta(probe / name) == identity for name, identity in probe_identity.items()),
                        'warm probe changed during build')
                identities[role]['probes'][kind] = dict(binary=meta(executable), probe=probe_identity,
                                                       build_label=role + '-warm-build-' + kind)
                binaries[role][kind] = executable
                command(rec, role + '-warm-' + kind + '-elf-notes', ['readelf', '-n', str(executable)])
                if role == 'candidate' and kind == 'structural':
                    command(rec, 'warm-gate-tests', ['env', 'CARGO_TARGET_DIR=' + str(target),
                            'rustup', 'run', '1.93.1', 'cargo', 'test', '--release', '--locked',
                            '--manifest-path', str(probe / 'Cargo.toml'), 'phase_gate::'], timeout=600)
                    require_exact_tests((out / 'warm-gate-tests.stdout').read_text(), {
                        'phase_gate::unix::tests::' + name for name in [
                        'phase_command_waits_for_fragmented_acknowledgement',
                        'phase_command_rejects_wrong_acknowledgement',
                        'partial_acknowledgement_has_a_total_deadline',
                        'fifo_gate_retains_a_completed_monotonic_warm_span']})
        # Frozen preparation verifier checks inventory; exact preflight tests are
        # independently audited in their retained source-qualified logs/receipts.
        validate_native_preflight(prep, evidence)
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
                       environment={key: os.environ.get(key) for key in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', *THREADS]},
                       cpu=cpu, available_affinity=affinity, perf=dict(path=str(perf), identity=meta(perf)),
                       warm_adapter=dict(head=warm_head, paths={name: dict(bytes=len(data),
                           sha256=hashlib.sha256(data).hexdigest()) for name, data in warm_bytes.items()}),
                       sample_scope='acknowledged warm observation loop; raw boundary samples retained separately',
                       process_provenance_scope='Recorder actual command children plus exec-task PID; not universal descendant provenance',
                       witnesses=[], profiles=[])
        counter = 0
        def task(role, kind, name, shots, policy, action, call_kind='flat', original=False):
            nonlocal counter
            circuit = here / 'fixtures' / (name + '.stim')
            require(meta(circuit)['sha256'] == manifest['inputs'][name + '.stim'], 'input differs')
            if kind == 'structural':
                path = out / ('config-' + str(counter) + '.json')
                path.write_text(json.dumps(structural_config(circuit, shots, policy, action, call_kind)) + '\n')
                argv = [str((originals if original else binaries)[role][kind]), str(path)]
            else:
                argv = [str((originals if original else binaries)[role][kind]), str(circuit), str(shots), str(1 if action == 'validate' else 32), policy, action, 'native']
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
                    result = read(out / (label + '.stdout'))
                    _, original_argv = task(role, kind, name, shots, policy, action, call_kind, original=True)
                    original_label = 'original-witness-' + str(counter)
                    command(rec, original_label, original_argv)
                    original_result = read(out / (original_label + '.stdout'))
                    require(finite_payload(result, kind) == finite_payload(original_result, kind),
                            'warm and original finite outputs differ within source')
                    events.append(dict(exit_code=0, timed_out=False, result=result))
                if kind == 'structural':
                    frozen.validate_structural_pair(*events, [name, 1, policy], manifest['inputs'][name + '.stim'])
                else:
                    frozen.validate_counts(events[0], [name, shots, policy], manifest['inputs'][name + '.stim'], circuit.read_text(), finite=True)
                context['witnesses'].append(dict(role=role, kind=kind, name=name, shots=shots, policy=policy, result='PASS', original_probe_equal=True))
        for index, (kind, name, shots, policy) in enumerate(fixed_cells()):
            for role in (['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline']):
                circuit, argv = task(role, kind, name, shots, policy, 'bench')
                label = 'profile-' + str(index) + '-' + role
                directory = out / label
                directory.mkdir()
                pid_path = directory / 'task.json'
                pid_path.touch(mode=0o600)
                data_path = directory / 'perf.data'
                control, ack = directory / 'control.fifo', directory / 'ack.fifo'
                phase_path = directory / 'phase.json'
                try:
                    os.mkfifo(control, 0o600)
                    os.mkfifo(ack, 0o600)
                    (directory / 'fifo-identities.json').write_text(json.dumps({path.name: dict(
                        path=str(path), mode=stat.S_IMODE(path.lstat().st_mode), inode=path.lstat().st_ino,
                        fifo=stat.S_ISFIFO(path.lstat().st_mode)) for path in [control, ack]}, indent=2) + '\n')
                    record = ['sudo', '-n', 'env', 'RUSTFLAGS=-C target-cpu=native', *[key + '=1' for key in THREADS],
                              'RSTIM_PHASE_CONTROL=' + str(control), 'RSTIM_PHASE_ACK=' + str(ack),
                              'RSTIM_PHASE_REPORT=' + str(phase_path),
                              'timeout', '--signal=KILL', '150s', str(perf), 'record', '--delay=-1',
                              '--control=fifo:' + str(control) + ',' + str(ack), '--clockid=mono', '-e', 'cpu-clock:u', '--sample-cpu',
                              '-F', '499', '--call-graph', 'dwarf,8192', '-o', str(data_path), '--', 'taskset', '-c', str(cpu),
                              sys.executable, '-I', '-B', str(Path(__file__).resolve()), '--exec-probe', str(pid_path), '--', *argv]
                    command(rec, label, record)
                finally:
                    # Only the two controller-owned FIFOs are removed after actual child wait.
                    control.unlink(missing_ok=True)
                    ack.unlink(missing_ok=True)
                    owned = [str(path) for path in [data_path, phase_path, pid_path] if path.exists()]
                    if owned:
                        command(rec, label + '-ownership', ['sudo', '-n', 'chown', '--',
                                str(os.getuid()) + ':' + str(os.getgid()), *owned])
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
                header = (out / (label + '-perf-header.txt.stdout')).read_text()
                actual_perf = record_perf_path(header)
                require(actual_perf.is_file() and actual_perf.read_bytes()[:4] == b'\x7fELF', 'actual perf ELF required')
                tool_identity = dict(path=str(actual_perf), identity=meta(actual_perf))
                if 'actual_perf' not in context:
                    command(rec, 'actual-perf-version', [str(actual_perf), '--version'])
                    shutil.copyfile(actual_perf, out / 'record-perf.bin')
                    (out / 'record-perf.bin').chmod(0o755)
                    context['actual_perf'] = tool_identity
                require(context['actual_perf'] == tool_identity, 'actual perf tool changed between cells')
                task_identity = read(pid_path)
                require(task_identity['argv'] == argv and task_identity['executable'] == argv[0] and
                        task_identity['binary'] == identities[role]['probes'][kind]['binary'] and
                        task_identity['affinity'] == [cpu] and task_identity['phase_paths'] == dict(
                            RSTIM_PHASE_CONTROL=str(control), RSTIM_PHASE_ACK=str(ack),
                            RSTIM_PHASE_REPORT=str(phase_path)), 'actual exec task binding differs')
                clock = perf_clock(data_path.read_bytes())
                phase = read(phase_path)
                selected, selection = select_samples((out / (label + '-perf-script.txt.stdout')).read_text(),
                                                      phase, cpu, task_identity['pid'], argv[0])
                (directory / 'warm-script.txt').write_text(selected)
                (directory / 'selection.json').write_text(json.dumps(selection, indent=2) + '\n')
                samples = selection['counts']
                build_ids = (out / (label + '-perf-buildids.txt.stdout')).read_text().splitlines()
                build_id = require_build_id((out / (role + '-warm-' + kind + '-elf-notes.stdout')).read_text(), '\n'.join(build_ids), argv[0])
                events = (out / (label + '-perf-events.txt.stdout')).read_text().splitlines()
                require(events.count('cpu-clock:u') == 1 and len(events) == len(clock['attributes']) and
                        all(event in ['cpu-clock:u', 'dummy', 'dummy:u'] for event in events),
                        'raw sample/tracking event inventory differs')
                context['profiles'].append(dict(role=role, kind=kind, name=name, shots=shots, policy=policy,
                    record_label=label, task=task_identity, phase=phase, clock=clock, samples=samples,
                    selection=meta(directory / 'selection.json'), selected_script=meta(directory / 'warm-script.txt'),
                    raw_data=meta(data_path), build_id=build_id, performance_valid=False))
        require(len(context['profiles']) == 28, 'all14cells/bothsources required')
        require(evidence.verify_preparation(prep, live=False) == before_seal, 'preparation changed')
        for role, identity in identities.items():
            require(all(meta(roots[role] / name) == expected for name, expected in identity['sources'].items()), 'source changed during profile')
            require(all(meta(binaries[role][kind]) == expected['binary'] for kind, expected in identity['probes'].items()), 'binary changed during profile')
        require(meta(Path(context['actual_perf']['path'])) == context['actual_perf']['identity'] and
                meta(out / 'record-perf.bin') == context['actual_perf']['identity'], 'retained actual perf tool differs')
        require(all((warm_root / name).read_bytes() == data for name, data in warm_bytes.items()),
                'warm adapter changed during profile')
        for role in roots:
            for kind in binaries[role]:
                probe = out / 'warm-probes' / role / kind
                require(all(meta(probe / name) == identity for name, identity in identities[role]['probes'][kind]['probe'].items()),
                        'warm build probe changed during profile')
                require(meta(originals[role][kind]) == identities[role]['original_probes'][kind]['binary'],
                        'original binary changed during profile')
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
    parser.add_argument('--warm-root', type=Path, required=True)
    parser.add_argument('--warm-revision', required=True)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--protocol-root', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    profile(args)


if __name__ == '__main__':
    main()
