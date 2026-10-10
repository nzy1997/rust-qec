"""Seal a closed CDF bundle, then independently replay its complete fixed protocol.

Sealing does not read performance summaries. Verification is read-only and requires
that original seal. Historical PIDs are not probed after artifact relocation.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import statistics
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


evidence = load_module('cdf_evidence', HERE / 'evidence.py')
driver = load_module('cdf_driver', HERE / 'run.py')
prepare_driver = load_module('cdf_prepare', HERE / 'prepare.py')


def read(path):
    return json.loads(path.read_text())


def require(value, message):
    if not value:
        raise ValueError(message)


def require_candidate_tests(log, *, layout=False):
    expected = {'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage'} if layout else {
        'near_clifford::coherent_packet::diagonal_projection_offset_tests::large_diagonal_projection_preserves_frozen_plane_bits_for_masks_and_pivots',
        'near_clifford::coherent_packet::diagonal_projection_offset_tests::diagonal_projection_preserves_first_error_and_partial_scratch_bits'
    }
    count = len(expected)
    records, summaries = [], []
    for line in log.splitlines():
        if not line.startswith('test '):
            continue
        if line.startswith('test result:'):
            summaries.append(line)
        else:
            record = re.fullmatch(r'test (\S+) \.\.\. (.+)', line)
            if record is None:
                raise ValueError('malformed diagonal-projection candidate test result record')
            records.append(record.groups())
    valid_summary = len(summaries) == 1 and re.fullmatch(
        rf'test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?',
        summaries[0],
    )
    if (len(records) != count or {name for name, _ in records} != expected
            or any(status != 'ok' for _, status in records) or not valid_summary):
        raise ValueError('every named diagonal-projection candidate test must execute successfully')

def receipt_pids(receipt):
    pids = [receipt['controller_pid'], receipt['child_pid']]
    require(all(type(pid) is int and pid > 0 for pid in pids), 'invalid actual process PID')
    return pids


def verify_absence(receipt, expected, *, waited):
    pids = sorted(set(expected))
    command = ['ps', '-p', ','.join(map(str, pids)), '-o', 'pid=,comm=']
    require(receipt['pids'] == pids and receipt['command'] == command and
            type(receipt['exit_code']) is int and receipt['exit_code'] == 1 and
            receipt['stdout'] == '' and receipt['stderr'] == '', 'process absence PID/command coverage differs')
    if waited:
        require(receipt['child_waited'] is True, 'process absence child was not waited')


def verify_worker_command(event, original_prep, original_protocol, original_output, output):
    """Bind the role to its actual native probe and exact retained input config."""
    kind, action = event['kind'], event['action']
    name, shots, policy = event['case']
    role = 'candidate' if event['route'] == 'candidate' else 'baseline'
    binary = 'near-clifford-diagnostics' if kind == 'structural' else 'near-clifford-application-counts'
    executable = str(original_prep / role / (binary + '.bin'))
    circuit = str(original_protocol / 'fixtures' / (name + '.stim'))
    if kind == 'structural':
        call = event['call_kind']
        config = dict(circuit=circuit, arithmetic=policy, cache_bytes=64 << 20,
                      shots=(1 if call == 'structured' else 129) if action == 'dump' else shots,
                      repetitions=7, action=action, call_kind=call, total=129 if action == 'dump' else shots)
        filename = f"{event['index']:05d}.config.json"
        require(read(output / filename) == config, 'retained structural command config differs')
        expected = [executable, str(original_output / filename)]
        if action == 'bench':
            require(event['result']['config'] == config, 'returned structural config differs from executed config')
    else:
        expected = [executable, circuit, str(shots), '1' if action == 'validate' else '7',
                    policy, action, 'native']
    require(event['command'] == expected, 'worker executable/command/native route differs')


def files(root):
    result = evidence.inventory(root, excluded=evidence.EXCLUDED | {'analysis'})
    result.pop('original-seal.json', None)
    return result


def seal_bundle(root):
    require(not (root / 'original-seal.json').exists(), 'original seal already exists')
    preparation = root / 'preparation'
    evidence.verify_preparation(preparation)
    outer = read(root / 'control/closure.json')
    require(outer['child_waited'] and outer['exit_code'] == 0 and
            not outer['timed_out'] and outer['cancellation'] is None and
            outer['post_run_seal_error'] is None, 'producer did not close successfully')
    pids = [outer['controller_pid'], outer['child_pid']]
    for name in read(preparation / 'seal.json')['files']:
        if name.endswith('.receipt.json'):
            receipt = read(preparation / name)
            require(receipt['child_waited'] and receipt['exit_code'] == 0,
                    'preparation child did not close successfully')
            pids.extend([receipt['controller_pid'], receipt['child_pid']])
    for line in (root / 'output/events.jsonl').read_text().splitlines():
        event = json.loads(line)
        require(event['child_waited'], 'worker was not actually waited')
        pids.extend([event['controller_pid'], event['child_pid']])
    command = ['ps', '-p', ','.join(map(str, sorted(set(pids)))), '-o', 'pid=,comm=']
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout, stderr = child.communicate(timeout=30)
    absence = dict(command=command, controller_pid=os.getpid(), child_pid=child.pid,
                   child_waited=True, exit_code=child.returncode,
                   stdout=stdout.decode(), stderr=stderr.decode(), pids=sorted(set(pids)))
    (root / 'process-absence.json').write_text(json.dumps(absence, indent=2) + '\n')
    require(child.returncode == 1 and not stdout, 'collection/preparation process still active')
    seal = dict(created=time.time(), scope='original bytes sealed before external summary interpretation',
                files=files(root))
    (root / 'original-seal.json').write_text(json.dumps(seal, indent=2) + '\n')
    print('original bundle sealed: ' + evidence.sha(root / 'original-seal.json'))


def schedule(manifest):
    cases = [('structural', tuple(case)) for case in manifest['structural_cases']]
    cases += [('counts', tuple(case)) for case in manifest['application_counts_cases']]
    witnesses = list(dict.fromkeys((case[0], case[2]) for case in manifest['structural_cases']))
    result = []
    for name, policy in witnesses:
        for route in ['baseline', 'candidate']:
            for call in ['structured', 'flat']:
                result.append((route, 'structural', (name, 1, policy), 'dump', None, call))
    for case in manifest['application_counts_cases']:
        for route in ['baseline', 'candidate']:
            result.append((route, 'counts', tuple(case), 'validate', None, 'flat'))
    for pair in range(6):
        order = cases[pair:] + cases[:pair]
        if pair % 2:
            order.reverse()
        for kind, case in order:
            ordinal = cases.index((kind, case))
            routes = ['baseline', 'candidate', 'control']
            shift = (pair + ordinal) % 3
            routes = routes[shift:] + routes[:shift]
            if (pair + ordinal) % 2:
                routes.reverse()
            for route in routes:
                result.append((route, kind, case, 'bench', pair, 'flat'))
    return cases, result


def verify_bundle(root, git_sources=False):
    seal = read(root / 'original-seal.json')
    evidence.verify_files(root, seal['files'])
    require(files(root) == seal['files'], 'original bundle inventory changed')
    prep, output = root / 'preparation', root / 'output'
    prep_digest = evidence.verify_preparation(prep)
    metadata = read(prep / 'preparation.json')
    original_prep = Path(metadata['preparation_directory'])
    original_protocol = Path(metadata['protocol_directory'])
    require(original_prep.is_absolute() and original_protocol.is_absolute(), 'original execution directories missing')
    require(metadata['roots'] == {role: str(original_prep / 'source' / ('baseline' if role == 'control' else role))
                                for role in ['baseline', 'candidate', 'control']}, 'source checkout directories differ')
    protocol = prep / 'protocol/benchmarks/near_clifford/cdf_source_pair'
    manifest = read(protocol / 'manifest.json')
    require(manifest == read(HERE / 'manifest.json'), 'published fixed matrix differs')
    require(set(metadata['heads']) == {'baseline', 'candidate', 'control'} and
            all(re.fullmatch('[0-9a-f]{40}', ref) for ref in metadata['heads'].values()) and
            metadata['heads']['baseline'] == metadata['heads']['control'], 'exact source refs differ')
    if git_sources:
        for name, item in read(prep / 'seal.json')['files'].items():
            if name.startswith('protocol/'):
                path = str(Path(name).relative_to('protocol'))
                data = subprocess.check_output(['git', 'show', metadata['protocol_revision'] + ':' + path], cwd=ROOT)
                require(hashlib.sha256(data).hexdigest() == item['sha256'], 'Git/protocol bytes differ: ' + path)
    expected_manifest = dict(manifest, source_pair=metadata['heads'])
    header, closure = read(output / 'header.json'), read(output / 'closure.json')
    outer = read(root / 'control/closure.json')
    require(len(outer['command']) == 5, 'outer producer command differs')
    original_output = Path(outer['command'][-1])
    require(original_output.is_absolute() and outer['command'][1:] ==
            ['-I', str(original_protocol / 'run.py'), str(original_prep), str(original_output)],
            'outer producer protocol/preparation/output command differs')
    require(header['rustc'] == metadata['compiler'] and
            metadata['compiler'].startswith('rustc 1.93.1 ('), 'pinned compiler identity differs')
    require(header['manifest'] == expected_manifest and header['inputs'] == manifest['inputs'],
            'fixed matrix/input inventory differs')
    require(header['protocol_revision'] == metadata['protocol_revision'] and
            header['driver_sha256'] == closure['driver_sha256'] == evidence.sha(protocol / 'run.py') and
            header['cases_sha256'] == evidence.sha(protocol / 'manifest.json'), 'protocol binding differs')
    require(header['preparation_seal_sha256'] == closure['preparation_seal_after'] ==
            outer['preparation_seal_before'] == outer['preparation_seal_after'] == prep_digest,
            'preparation seal binding differs')
    require(outer['child_waited'] and outer['exit_code'] == 0 and not outer['timed_out'] and
            outer['cancellation'] is None and outer['post_run_seal_error'] is None and
            outer['producer_log_sha256'] == evidence.sha(root / 'control/producer.log'),
            'outer producer closure differs')
    identities = header['identities']
    require(set(identities) == {'baseline', 'candidate', 'control'} and
            identities == closure['identities_after'] and identities['baseline'] == identities['control'],
            'before/after source or identical-binary control differs')
    require(closure['inputs_after'] == manifest['inputs'], 'consumed fixture identities changed')
    for name, digest in manifest['inputs'].items():
        require(evidence.sha(protocol / 'fixtures' / name) == digest, 'fixture hash differs')
    preparation_pids = []
    for role in ['baseline', 'candidate']:
        identity = identities[role]
        require(identity['head'] == metadata['heads'][role], 'source ref differs')
        snapshots = prep / 'production-sources' / role
        snapshot_hashes = {name: item['sha256'] for name, item in evidence.inventory(snapshots).items()}
        require(snapshot_hashes == identity['sources'], 'production source snapshot differs')
        if git_sources:
            for name, digest in identity['sources'].items():
                data = subprocess.check_output(['git', 'show', identity['head'] + ':' + name], cwd=ROOT)
                require(hashlib.sha256(data).hexdigest() == digest, 'Git/source binding differs: ' + name)
        for kind, binary in [('counts', 'near-clifford-application-counts'),
                             ('structural', 'near-clifford-diagnostics')]:
            receipt = read(prep / role / ('native-build-' + kind + '.receipt.json'))
            preparation_pids.extend(receipt_pids(receipt))
            original_probe = original_prep / role / 'native-probes' / kind
            require(receipt['command'] == ['rustup', 'run', '1.93.1', 'cargo', 'build', '--release', '--locked',
                                            '--manifest-path', str(original_probe / 'Cargo.toml')],
                    'exact native build command differs')
            package = 'application_counts' if kind == 'counts' else 'diagnostics'
            canonical = prep / 'protocol/benchmarks/near_clifford' / package
            probe = prep / role / 'native-probes' / kind
            require(all((probe / name).read_bytes() == (canonical / name).read_bytes()
                        for name in ['main.rs', 'Cargo.lock']) and
                    (probe / 'Cargo.toml').read_text() == prepare_driver.probe_manifest(
                        (canonical / 'Cargo.toml').read_text(), Path(metadata['roots'][role]) / 'rstim'),
                    'canonical public probe or path-only dependency differs')
            hashes = {name: evidence.sha(prep / role / 'native-probes' / kind / name)
                      for name in ['main.rs', 'Cargo.toml', 'Cargo.lock']}
            require(receipt['child_waited'] and receipt['exit_code'] == 0 and not receipt['timed_out'] and
                    receipt['cancellation'] is None and receipt['head'] == identity['head'] and
                    receipt['sources'] == receipt['sources_after'] == identity['sources'] and
                    receipt['probe'] == hashes and dict(hashes, binary=receipt['binary']['sha256']) ==
                    identity['probes'][kind], 'native build/source/probe receipt differs')
            retained = prep / role / (binary + '.bin')
            require(retained.stat().st_size == receipt['binary']['bytes'] and
                    evidence.sha(retained) == receipt['binary']['sha256'], 'actual retained binary differs')
            require(receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native', 'native flags differ')
            require(receipt['environment']['CARGO_TARGET_DIR'] == str(original_prep / role / 'native-target') and
                    receipt['environment']['CARGO_ENCODED_RUSTFLAGS'] is None and
                    receipt['environment']['CARGO_PROFILE_RELEASE_OPT_LEVEL'] is None and
                    receipt['binary']['path'] == str(original_prep / role / (binary + '.bin')) and
                    receipt['binary']['mode'] == '0o755', 'native target/flags/executable binding differs')
            require(evidence.sha(prep / role / ('native-build-' + kind + '.log')) ==
                    receipt['log_sha256'], 'build log differs')
    for index in [0, 1, 2, 3]:
        receipt = read(prep / ('native-check-' + str(index) + '.receipt.json'))
        preparation_pids.extend(receipt_pids(receipt))
        selection = (['--lib', 'phase_specialized_cdf_tests'] if index == 0 else
                     ['--test', 'near_clifford_compiled',
                      'compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails', '--', '--exact'] if index == 1 else
                     ['--lib', 'diagonal_projection_offset_tests'] if index == 2 else
                     ['--lib', 'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage', '--', '--exact'])
        require(receipt['command'] == ['rustup', 'run', '1.93.1', 'cargo', 'test', '--release', '--locked',
                                        '-p', 'rstim', '--no-default-features', *selection] and
                receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and
                receipt['timed_out'] is False and receipt['cancellation'] is None,
                'exact native arithmetic/RNG test selection/status differs')
        log = prep / ('native-check-' + str(index) + '.log')
        require(receipt['head'] == metadata['heads']['candidate'] and receipt['child_waited'] and
                receipt['exit_code'] == 0 and evidence.sha(log) == receipt['log_sha256'],
                'native arithmetic/public RNG check differs')
        evidence.require_executed_tests(log.read_text())
        if index >= 2:require_candidate_tests(log.read_text(), layout=index == 3)
    verify_absence(read(root / 'control/preparation-process-absence.json'), preparation_pids, waited=False)
    require(evidence.sha(output / 'events.jsonl') == closure['events_sha256'], 'ledger hash differs')
    events = [json.loads(line) for line in (output / 'events.jsonl').read_text().splitlines()]
    cases, expected = schedule(manifest)
    require(len(events) == len(expected) == closure['events'] == 720, '720-child inventory differs')
    values = {(kind, case): {route: [] for route in identities} for kind, case in cases}
    observations_count = 0
    all_pids = preparation_pids + receipt_pids(outer)
    for index, (event, context) in enumerate(zip(events, expected)):
        route, kind, case, action, pair, call = context
        require((event['index'], event['route'], event['kind'], tuple(event['case']), event['action'],
                 event['pair'], event['call_kind']) == (index, *context), 'fixed execution order differs')
        raw, stderr = output / f'{index:05d}.stdout', output / f'{index:05d}.stderr'
        require(evidence.sha(raw) == event['stdout_sha256'] and evidence.sha(stderr) ==
                event['stderr_sha256'] and read(raw) == event['result'], 'raw/ledger result differs')
        require(event['child_waited'] and event['exit_code'] == 0 and not event['timed_out'] and
                event['cancellation'] is None and event['controller_pid'] == outer['child_pid'],
                'worker transport/lifecycle differs')
        all_pids.extend(receipt_pids(event))
        verify_worker_command(event, original_prep, original_protocol, original_output, output)
        digest = manifest['inputs'][case[0] + '.stim']
        if kind == 'counts':
            driver.validate_counts(event, case, digest, (protocol / 'fixtures' / (case[0] + '.stim')).read_text(),
                                   finite=action == 'validate')
        elif action == 'dump' and call == 'flat':
            driver.validate_structural_pair(events[index - 1], event, case, digest)
        elif action == 'bench':
            relocated = copy.deepcopy(event)
            relocated['result']['config']['circuit'] = str(protocol / 'fixtures' / (case[0] + '.stim'))
            driver.validate_structural_timing(relocated, case, digest)
        if action == 'bench':
            observations = event['result']['warm' if kind == 'structural' else 'observations']
            driver.validate_observations(observations)
            observations_count += len(observations)
            values[kind, case][route].append(statistics.median(o['ns_per_call'] for o in observations))
    require(observations_count == 4536, '4536-observation inventory differs')
    summaries = []
    for kind, case in cases:
        v = values[kind, case]
        baseline, candidate, control = (statistics.median(v[role]) for role in ['baseline', 'candidate', 'control'])
        summaries.append(dict(kind=kind, case=list(case), complete=True, process_medians_ns=v,
                              baseline_ns=baseline, candidate_ns=candidate, median_ratio=baseline / candidate,
                              paired_ratios=[a / b for a, b in zip(v['baseline'], v['candidate'])],
                              null_ratio=baseline / control,
                              null_paired_ratios=[a / b for a, b in zip(v['baseline'], v['control'])]))
    require(read(output / 'summary.json') == summaries, 'independently derived summary differs')
    absence = read(root / 'process-absence.json')
    verify_absence(absence, all_pids, waited=True)
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--seal', action='store_true')
    parser.add_argument('--git-sources', action='store_true')
    args = parser.parse_args()
    root = args.bundle.resolve()
    if args.seal:
        seal_bundle(root)
    else:
        verify_bundle(root, args.git_sources)
        print('verified: 36 cells, 72 finite workers, 648 timing workers, 4536 observations; source-only')


if __name__ == '__main__':
    main()
