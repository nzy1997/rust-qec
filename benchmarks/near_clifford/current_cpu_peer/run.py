"""Prespecified October SymFT counts screen with retained child lifecycles."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = ROOT / 'benchmarks/near_clifford/current_cpu_peer'
PREP = OUT = SOURCE = PYTHON = CLIFFT_PYTHON = BIN = None
COUNTS = ROOT / 'benchmarks/near_clifford/application_counts'
SOTA = ROOT / 'benchmarks/near_clifford/compiled_sota'
sys.path.insert(0, str(Path(__file__).resolve().parent))
from peer_evidence import verify_preparation
from commands import HOST_SCRIPT, package_script
sys.path.insert(0, str(COUNTS))
from common import annotations, check_result, compare, counts, raw_counts, require
sys.path.insert(0, str(SOTA))
from projection import records_only
from evidence import compact
spec = importlib.util.spec_from_file_location('peer_contract', SOTA / 'run.py')
peer_contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(peer_contract)
sys.path.insert(0, str(ROOT / 'benchmarks/near_clifford/diagnostics'))
from source_contract import production_inventory


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def role_order(roles, pair):
    offset = pair % len(roles)
    ordered = roles[offset:] + roles[:offset]
    return ordered[::-1] if pair % 2 else ordered


def git(*args, root=ROOT):
    return subprocess.check_output(['git', *args], cwd=root, text=True).strip()


def source_state(manifest):
    require(git('rev-parse', 'HEAD') == manifest['protocol_revision'], 'protocol HEAD changed')
    require(not git('status', '--porcelain'), 'root Git candidate changed')
    require(git('rev-parse', 'HEAD', root=SOURCE) == manifest['symft_revision'], 'peer HEAD changed')
    require(not git('diff', '--name-only', 'HEAD', root=SOURCE), 'peer tracked source changed')
    files = git('ls-files', root=SOURCE).splitlines()
    peer_sources = {name: sha(SOURCE / name) for name in files if (SOURCE / name).is_file()}
    checkout = json.loads((PREP / 'checkout-receipt.json').read_text())['source_inventory']
    require(peer_sources == {name: data['sha256'] for name, data in checkout.items()}, 'peer checkout byte mutation')
    rust = production_inventory(manifest['rust_source_head'])
    receipt = json.loads((PREP / 'rust/baseline/native-build-counts.receipt.json').read_text())
    require(receipt['exit_code'] == 0 and receipt['head'] == manifest['rust_source_head'], 'wrong Rust build')
    require(rust == receipt['sources'], 'merged Rust differs from retained original build')
    require(sha(BIN) == receipt['binary']['sha256'], 'Rust executable changed')
    require({name: sha(PREP / 'rust/baseline/native-probes/counts' / name) for name in receipt['probe']} == receipt['probe'], 'Rust probe inputs changed')
    require(sha(COUNTS / 'main.rs') == receipt['probe']['main.rs'], 'retained probe differs from public probe')
    inputs = {name: sha(COUNTS / 'fixtures' / (name + '.stim')) for name in manifest['names']}
    require(inputs == manifest['inputs'], 'original circuit changed')
    harness_paths = [*sorted(HERE.glob('*.py')), HERE / 'manifest.json', PREP / 'manifest.json', PREP / 'expected-environment.json']
    harness_paths += [ROOT / 'benchmarks/near_clifford/diagnostics/source_contract.py', COUNTS / 'common.py'] + [SOTA / name for name in ['run.py', 'worker.py', 'projection.py', 'evidence.py']]
    return dict(protocol_revision=manifest['protocol_revision'], rust_build_head=receipt['head'],
                rust_sources=rust, peer_sources=peer_sources, inputs=inputs,
                rust_binary_sha256=sha(BIN), harness={str(path): sha(path) for path in harness_paths})


class Recorder:
    def __init__(self, output):
        self.output = output
        self.index = 0

    def invoke(self, command, kind, *, timeout=180, **context):
        index = self.index
        self.index += 1
        start = time.time()
        child = None
        timed_out = False
        cancellation = None
        active_path = self.output / 'active-child.json'
        try:
            previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGTERM, signal.SIGINT})
            try:
                child = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE, start_new_session=True,
                                         # Single-thread controller restores the child's inherited mask.
                                         preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask))
                temporary = active_path.with_suffix('.tmp')
                temporary.write_text(json.dumps(dict(child_pid=child.pid, controller_pid=os.getpid(), command=command)))
                temporary.replace(active_path)
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
            stdout, stderr = child.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(child.pid, signal.SIGKILL)
            stdout, stderr = child.communicate(timeout=10)
        except BaseException as error:
            if child is None:
                raise
            cancellation = error
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            stdout, stderr = child.communicate(timeout=10)
        stdout_path = self.output / f'{index:05d}.stdout'
        stderr_path = self.output / f'{index:05d}.stderr'
        stdout_path.write_bytes(stdout)
        stderr_path.write_bytes(stderr)
        try:
            result = json.loads(stdout)
        except (ValueError, UnicodeDecodeError):
            result = None
        compaction_error = None
        retained_result = result
        if isinstance(result, dict) and 'measurements' in result:
            try:
                retained_result = compact(result)
            except ValueError as error:
                # Keep the child lifecycle even when its transcript is invalid.
                # Original stdout remains the authoritative unaltered payload.
                compaction_error = str(error)
                retained_result = {key: value for key, value in result.items() if key != 'measurements'}
        event = dict(index=index, kind=kind, command=command, start=start, end=time.time(),
                     controller_pid=os.getpid(), child_pid=child.pid, child_waited=True,
                     exit_code=child.returncode, timed_out=timed_out,
                     process_status='timeout-group-killed-and-waited' if timed_out else 'closed',
                     stdout_sha256=sha(stdout_path), stderr_sha256=sha(stderr_path),
                     result=retained_result, result_compaction_error=compaction_error,
                     cancellation=type(cancellation).__name__ if cancellation else None,
                     **context)
        with (self.output / 'events.jsonl').open('a') as file:
            file.write(json.dumps(event, separators=(',', ':')) + '\n')
            file.flush()
            os.fsync(file.fileno())
        active_path.unlink(missing_ok=True)
        if cancellation is not None:
            raise cancellation
        return event, result


def capture_environment(recorder):
    packages = {}
    identities = {}
    for backend, python in [('clifft', CLIFFT_PYTHON), ('symft', PYTHON)]:
        event, result = recorder.invoke([str(python), '-I', '-c', package_script([backend, 'numpy'])],
                                        'package-inspection', backend=backend)
        require(event['exit_code'] == 0 and isinstance(result, dict), 'package capture failed')
        packages[backend] = result
        event, result = recorder.invoke([str(python), '-I', str(SOTA / 'worker.py'), backend,
                                        str(SOTA / 'manifest.json'), '1', '--mode', 'identity'],
                                       'import-inspection', backend=backend)
        require(event['exit_code'] == 0 and result.get('isolated') is True, 'import capture failed')
        identities[backend] = result['loaded_files']
    actual = dict(packages=dict(symft=packages['symft']['symft'], numpy=packages['symft']['numpy'],
                                clifft_environment=packages['clifft']), identities=identities)
    expected = json.loads((PREP / 'expected-environment.json').read_text())
    require(actual == expected, 'live package/import differs from sealed wheel-bound environment')
    return actual['packages'], actual['identities']


def bind_peer(event, result, backend, input_hash, packages, identities):
    require(event['exit_code'] == 0 and not event['timed_out'] and isinstance(result, dict), 'peer child failed')
    require(result['input_sha256'] == input_hash, 'peer consumed wrong input')
    peer_contract.bind_peer(result, backend, packages, identities)


def validate_peer_result(event, result, backend, input_hash, packages, identities,
                         shots, batch, cpu, repetitions, *, validate, required):
    successful = (event['exit_code'] == 0 and not event['timed_out']
                  and isinstance(result, dict) and result.get('status') == 'ok')
    require(not required or successful, 'required peer child failed or timed out')
    if successful:
        bind_peer(event, result, backend, input_hash, packages, identities)
        check_result(result, backend, shots, repetitions, validate=validate, batch=batch)
        if backend == 'symft':
            require(result['cpu_backend'] == cpu, 'requested CPU backend changed')
            info = result['sampler_info']
            require(info['threads'] == 1 and info['detector_postselection'] and not info['reference_normalized'], 'counts semantics/thread mismatch')
            require(info['cpu_compiled'] == (cpu == 'compiled'), 'actual CPU executor differs from request')
    return successful


class CampaignCancelled(RuntimeError):
    pass


def cancel(signum, frame):
    raise CampaignCancelled('campaign cancelled by signal ' + str(signum))


def main():
    global PREP, OUT, SOURCE, PYTHON, CLIFFT_PYTHON, BIN
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    PREP=args.preparation.resolve(); OUT=args.out.resolve()
    SOURCE=PREP/'source'; PYTHON=PREP/'symft/bin/python'; CLIFFT_PYTHON=PREP/'clifft/bin/python'
    BIN=PREP/'rust/baseline/near-clifford-application-counts.bin'
    for key in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS','VECLIB_MAXIMUM_THREADS','NUMEXPR_NUM_THREADS']:os.environ[key]='1'
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    preparation_seal = verify_preparation(PREP)
    manifest = json.loads((PREP / 'manifest.json').read_text())
    before = source_state(manifest)
    OUT.mkdir(exist_ok=False)
    recorder = Recorder(OUT)
    packages, identities = capture_environment(recorder)
    host_event, host = recorder.invoke([sys.executable, '-I', '-c',
        HOST_SCRIPT], 'host-inspection')
    require(host_event['exit_code']==0 and host['uname'][0]=='Linux' and host['uname'][4]=='x86_64', 'Linux x86_64 host required')
    require(len(host['affinity'])==1, 'taskset exactly one available logical CPU before the controller')
    require(all(value=='1' for value in host['thread_environment'].values()), 'thread environment must be one')
    require(packages['symft']['version'] == manifest['versions']['symft'], 'wrong new SymFT version')
    require(packages['clifft_environment']['clifft']['version'] == manifest['versions']['clifft'], 'wrong Clifft version')
    require(packages['numpy']['version'] == packages['clifft_environment']['numpy']['version'] == manifest['versions']['numpy'], 'NumPy mismatch')
    header = dict(schema=manifest['schema'], manifest=manifest, before=before, preparation_seal_sha256=preparation_seal,
                  packages=packages, identities=identities, host=host, controller_pid=os.getpid(), started=time.time(),
                  scope='Prespecified first12 Linux x86_64 counts cells. NewSymFT performance branch, not new main release. No general SOTA claim.',
                  counts_rng_scope='Retained Rust executable self-attests carry; literal RNG words absent in counts output.')
    (OUT / 'header.json').write_text(json.dumps(header, indent=2) + '\n')

    def peer(backend, name, shots, selection, *, validate=False, pair=None, policy=None):
        batch = selection['batch']
        cpu = selection.get('cpu_backend', 'legacy')
        python = PYTHON if backend == 'symft' else CLIFFT_PYTHON
        cmd = [str(python), '-I', str(HERE / 'worker.py'), backend,
               str(COUNTS / 'fixtures' / (name + '.stim')), str(shots), '--batch', str(batch),
               '--cpu-backend', cpu, '--repetitions', str(1 if validate else manifest['repetitions'])]
        if validate:
            cmd.append('--validate')
        event, result = recorder.invoke(cmd, 'counts-validation' if validate else ('timing' if pair is not None else 'tuning'),
                                       backend=backend, name=name, shots=shots, selection=selection, pair=pair, policy=policy)
        validate_peer_result(event, result, backend, manifest['inputs'][name], packages, identities,
                             shots, batch, cpu, 1 if validate else manifest['repetitions'],
                             validate=validate, required=validate or pair is not None)
        return event, result

    def rust(name, shots, policy, *, validate=False, pair=None, role='rstim'):
        cmd = [str(BIN), str(COUNTS / 'fixtures' / (name + '.stim')), str(shots),
               str(1 if validate else manifest['repetitions']), policy, 'validate' if validate else 'bench', 'native']
        event, result = recorder.invoke(cmd, 'counts-validation' if validate else 'timing',
                                       backend=role, name=name, shots=shots, policy=policy, pair=pair)
        require(event['exit_code'] == 0 and not event['timed_out'] and result['input_sha256'] == manifest['inputs'][name], 'Rust child/input failed')
        check_result(result, 'rstim', shots, 1 if validate else manifest['repetitions'], validate=validate, policy=policy)
        if validate:
            require(result['exact_native_counts_rng'], 'Rust counts/RNG executable self-check failed')
        return event, result

    selections = {}
    cases = [(name, shots, policy) for name in manifest['names'] for shots in manifest['shots'] for policy in manifest['policies']]
    # Selection and every finite validation close before any comparison timing.
    for name in manifest['names']:
        masks = annotations((COUNTS / 'fixtures' / (name + '.stim')).read_text())
        native_input = OUT / (name + '.records.stim')
        native_input.write_text(records_only((COUNTS / 'fixtures' / (name + '.stim')).read_text()))
        for shots in manifest['shots']:
            selections[(name, shots)] = {}
            references = {}
            for policy in manifest['policies']:
                _, data = rust(name, shots, policy, validate=True)
                raw_reference = raw_counts(data, masks)
                require(raw_reference == counts(data), 'Rust raw detector/observable parity disagrees')
                references[policy] = raw_reference
            for backend in ['clifft', 'clifft-scheduled', 'symft']:
                options = [{'batch': batch} for batch in [1, 64, 256, 1024, 'auto']]
                if backend == 'symft':
                    options = [{'cpu_backend': 'legacy', 'batch': batch} for batch in ['scalar', 1, 64, 256, 1024, 'auto']]
                    options.append({'cpu_backend': 'compiled', 'batch': 'auto'})
                trials = []
                for selection in options:
                    event, data = peer(backend, name, shots, selection)
                    if event['exit_code'] == 0 and not event['timed_out'] and isinstance(data, dict) and data.get('status') == 'ok':
                        trials.append((statistics.median(o['ns_per_call'] for o in data['observations']), selection))
                require(trials, 'no valid candidate for ' + backend)
                selected = min(trials, key=lambda value: value[0])[1]
                selections[(name, shots)][backend] = selected
                _, data = peer(backend, name, shots, selected, validate=True)
                native_counts = counts(data)
                python = PYTHON if backend == 'symft' else CLIFFT_PYTHON
                raw_total = ((8192 + shots - 1) // shots) * shots
                cmd = [str(python), '-I', str(SOTA / 'worker.py'), backend, str(native_input), str(shots),
                       '--batch', str(selected['batch']), '--mode', 'dump', '--dump-total', str(raw_total)]
                event, raw = recorder.invoke(cmd, 'raw-validation', timeout=manifest['raw_timeout_s'],
                                            backend=backend, name=name, shots=shots, selection=selected)
                bind_peer(event, raw, backend, sha(native_input), packages, identities)
                own_records = raw_counts(raw, masks)
                # Conservative union budget: three comparisons, two Bernoulli
                # quantities, and both sampled populations for each case/peer.
                alpha = 0.001 / (len(cases) * 3 * 12)
                checks = dict(own_records=compare(native_counts, own_records, alpha),
                              against_rust={policy: compare(native_counts, ref, alpha) for policy, ref in references.items()})
                require(checks['own_records']['passed'] and all(check['passed'] for check in checks['against_rust'].values()), 'finite counts disagreement')
                with (OUT / 'validation-checks.jsonl').open('a') as file:
                    file.write(json.dumps(dict(name=name, shots=shots, backend=backend, selection=selected,
                                               counts=native_counts, own_records=own_records, rust=references,
                                               alpha_per_population=alpha, checks=checks)) + '\n')
    frozen = [{'name': name, 'shots': shots, 'backends': value} for (name, shots), value in selections.items()]
    selection_path = OUT / 'frozen-selections.json'
    selection_path.write_text(json.dumps(frozen, indent=2) + '\n')
    print('selection and validation closed', flush=True)
    for pair in range(manifest['pairs']):
        ordered_cases = cases[pair % len(cases):] + cases[:pair % len(cases)]
        if pair % 2:
            ordered_cases.reverse()
        for name, shots, policy in ordered_cases:
            for role in role_order(manifest['roles'], pair):
                if role in ['rstim', 'control']:
                    rust(name, shots, policy, pair=pair, role=role)
                else:
                    _, result = peer(role, name, shots, selections[(name, shots)][role], pair=pair, policy=policy)
                    require(result and result.get('status') == 'ok', 'selected timing candidate failed')
        print('comparison round ' + str(pair + 1) + ' closed', flush=True)
    require(source_state(manifest) == before, 'source/harness changed during campaign')
    packages_after, identities_after = capture_environment(recorder)
    require(packages_after == packages and identities_after == identities, 'package/import changed during campaign')
    require(sha(selection_path) == hashlib.sha256((json.dumps(frozen, indent=2) + '\n').encode()).hexdigest(), 'selection mutated')
    closure = dict(controller_pid=os.getpid(), all_children_waited=True, events=recorder.index,
                   preparation_seal_after=verify_preparation(PREP),
                   events_sha256=sha(OUT / 'events.jsonl'), before=before, after=source_state(manifest),
                   packages_after=packages_after, identities_after=identities_after, finished=time.time(),
                   timing_children=len(cases) * manifest['pairs'] * len(manifest['roles']))
    (OUT / 'closure.json').write_text(json.dumps(closure, indent=2) + '\n')
    print('campaign closed', flush=True)


if __name__ == '__main__':
    main()
