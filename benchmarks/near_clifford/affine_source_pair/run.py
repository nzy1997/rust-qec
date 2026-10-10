"""Bounded exploratory source-pair screen; preserves every child result/failure."""
import hashlib
import importlib.util
import json
import math
import os
import platform
from pathlib import Path
import statistics
import signal
import subprocess
import sys
import time

PREP = Path(__file__).resolve().parent
PROTOCOL_ROOT = PREP.parents[2]
common_spec=importlib.util.spec_from_file_location('counts_semantics',PREP.parent/'application_counts/common.py')
semantics=importlib.util.module_from_spec(common_spec);common_spec.loader.exec_module(semantics)
ROOTS={};HEADS={};PROBE_ROOTS={};BINARIES={};ROOT=None;ZERO=None;BASE_ROOT=None

def configure(preparation):
    global ROOTS,HEADS,PROBE_ROOTS,BINARIES,ROOT,ZERO,BASE_ROOT
    data=json.loads((preparation/'preparation.json').read_text())
    ROOTS={role:Path(path) for role,path in data['roots'].items()}
    HEADS=data['heads'];ROOT=ROOTS['candidate'];ZERO=preparation/'candidate';BASE_ROOT=preparation/'baseline'
    PROBE_ROOTS={role:(ZERO if role=='candidate' else BASE_ROOT)/'native-probes' for role in ROOTS}
    BINARIES={role:{kind:(ZERO if role=='candidate' else BASE_ROOT)/(binary+'.bin') for kind,binary in [('structural','near-clifford-diagnostics'),('counts','near-clifford-application-counts')]} for role in ROOTS}
    return data

def verify_preparation(preparation):
    spec = importlib.util.spec_from_file_location('cdf_evidence', PREP/'evidence.py')
    evidence = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(evidence)
    return evidence.verify_preparation(preparation, live=True)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root, text=True).strip()

def identity(route):
    root = ROOTS[route]
    require(git(root, 'rev-parse', 'HEAD') == HEADS[route], 'unexpected head')
    require(not git(root, 'status', '--porcelain'), 'dirty tracked/untracked checkout')
    paths = [p for p in git(root, 'ls-files').splitlines()
             if p.endswith('.rs') or Path(p).name in ('Cargo.toml', 'Cargo.lock')]
    sources = {p: sha(root / p) for p in paths}
    for p in paths:
        data = subprocess.check_output(['git', 'show', HEADS[route] + ':' + p], cwd=root)
        require(hashlib.sha256(data).hexdigest() == sources[p], 'Git source mismatch: ' + p)
    probes = {}
    for kind, binary in [('structural', 'near-clifford-diagnostics'),
                         ('counts', 'near-clifford-application-counts')]:
        probes[kind] = {p: sha(PROBE_ROOTS[route] / kind / p)
                        for p in ('main.rs', 'Cargo.toml', 'Cargo.lock')}
        probes[kind]['binary'] = sha(BINARIES[route][kind])
    return dict(head=HEADS[route], sources=sources, probes=probes)

class CampaignCancelled(RuntimeError):pass
def cancel(signum, frame):raise CampaignCancelled("producer cancelled by signal "+str(signum))

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
        event = dict(index=index, kind=kind, command=command, start=start, end=time.time(),
                     controller_pid=os.getpid(), child_pid=child.pid, child_waited=True,
                     exit_code=child.returncode, timed_out=timed_out,
                     process_status='timeout-group-killed-and-waited' if timed_out else 'closed',
                     stdout_sha256=sha(stdout_path), stderr_sha256=sha(stderr_path),
                     result=result,
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


def timing_transport_ok(event):
    return event["exit_code"] == 0 and not event["timed_out"] and isinstance(event["result"], dict) and event["result"].get("status") == "ok"

def validate_counts(event, case, input_hash, text, finite=False):
    require(timing_transport_ok(event), 'counts child failed/status/timeout')
    data=event['result']
    require(data['input_sha256']==input_hash,'counts input differs')
    repetitions=1 if finite else 7
    semantics.check_result(data,'rstim',case[1],repetitions,validate=finite,policy=case[2])
    if finite:
        require(data['call_shots']==case[1], 'counts call_shots differs')
        require(data['exact_native_counts_rng'] is True, 'counts native chronological self-check failed')
        replay=semantics.raw_counts(data,semantics.annotations(text))
        require(replay==semantics.counts(data),'native counts differ from independent raw annotation replay')
        require(data['shots']==8192,'counts finite population differs')
    return data

def validate_structural_pair(a,b,case,input_hash):
    for event,kind,call_shots in [(a,'structured',1),(b,'flat',129)]:
        require(timing_transport_ok(event),'structural finite child failed/status/timeout')
        data=event['result']
        require(data['input_sha256']==input_hash and data['arithmetic']==case[2],'structural input/policy differs')
        require(data['call_kind']==kind and data['call_shots']==call_shots and data['shots']==129,'structural finite call/population differs')
        require(type(data['width']) is int and data['width']>0 and len(data['measurements'])==129*data['width'],'structural dimensions differ')
        require(all(type(bit) is int and bit in (0,1) for bit in data['measurements']),'invalid structural record bit')
        require(len(data['continuation'])==16 and all(type(word) is int and 0<=word<1<<64 for word in data['continuation']),'structural literal RNG carry differs')
    require(a['result']['width']==b['result']['width'] and a['result']['measurements']==b['result']['measurements'] and a['result']['continuation']==b['result']['continuation'],'structural records/RNG streams differ')

def validate_structural_timing(event,case,input_hash):
    require(timing_transport_ok(event),'structural timing child failed/status/timeout')
    data=event['result'];config=data['config']
    require(data['arithmetic']==case[2] and data['shots']==case[1] and data['input_sha256']==input_hash,'structural timing input/call/policy differs')
    require(config['arithmetic']==case[2] and config['call_kind']=='flat' and config['shots']==case[1] and config['total']==case[1] and config['repetitions']==7 and config['cache_bytes']==64<<20 and config['action']=='bench','structural timing config differs')
    require(sha(Path(config['circuit']))==input_hash,'structural timing config input differs')
    return data

def validate_observations(observations, repetitions=7):
    require(len(observations) == repetitions, 'incomplete observations')
    for observation in observations:
        elapsed, calls, rate = (observation[key] for key in ['elapsed_ns', 'calls', 'ns_per_call'])
        require(type(elapsed) is int and elapsed >= 50_000_000 and
                type(calls) is int and calls > 0 and type(rate) in [int, float] and
                math.isfinite(rate) and rate == elapsed / calls, 'invalid observation')

def main():
    require(len(sys.argv) == 3, 'usage: run.py PREPARATION NEW_OUTPUT_DIR')
    preparation=Path(sys.argv[1]).resolve();preparation_seal_before=verify_preparation(preparation);prepared=configure(preparation)
    out = Path(sys.argv[2]).resolve()
    out.mkdir(exist_ok=False)
    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)
    recorder = Recorder(out)
    manifest = json.loads((PREP / 'manifest.json').read_text())
    manifest['source_pair']=HEADS
    require(manifest['source_pair'] == HEADS, 'manifest source_pair differs from executed fixed heads')
    inputs = {p.name: sha(p) for p in (PREP / 'fixtures').glob('*.stim')}
    require(inputs == manifest['inputs'], 'fixture mutation')
    before = {route: identity(route) for route in ROOTS}
    require(before['baseline'] == before['control'], 'AA control differs from baseline')
    for route in ['baseline','candidate']:
        directory=preparation/route
        for kind in ['structural','counts']:
            receipt=json.loads((directory/('native-build-'+kind+'.receipt.json')).read_text())
            require(receipt['exit_code']==0 and receipt['child_waited'] and receipt['head']==HEADS[route] and receipt['sources']==receipt['sources_after']==before[route]['sources'],'build/source differs')
            require(receipt['probe']=={k:v for k,v in before[route]['probes'][kind].items() if k!='binary'} and receipt['binary']['sha256']==before[route]['probes'][kind]['binary'],'build/probe/binary differs')
            require(receipt['environment']['RUSTFLAGS']=='-C target-cpu=native','native build flags differ')
    for kind in ['structural','counts']:
        require(before['baseline']['probes'][kind]['main.rs']==before['candidate']['probes'][kind]['main.rs'] and before['baseline']['probes'][kind]['Cargo.lock']==before['candidate']['probes'][kind]['Cargo.lock'],'public probe implementation/dependencies differ')

    header = dict(schema=manifest['schema'], started=time.time(), identities=before,
                  manifest=manifest, inputs=inputs, driver_sha256=sha(Path(__file__)),
                  protocol_revision=prepared['protocol_revision'],preparation_seal_sha256=preparation_seal_before,
                  cases_sha256=sha(PREP/'manifest.json'),
                  rustc=subprocess.check_output(['rustup', 'run', '1.93.1', 'rustc', '-Vv'], text=True),
                  host=subprocess.check_output(['uname', '-a'], text=True),
                  cpu=(subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip() if sys.platform=='darwin' else subprocess.check_output(['lscpu','--json'],text=True)),
                  machine=platform.machine(),affinity=sorted(os.sched_getaffinity(0)) if hasattr(os,'sched_getaffinity') else None,
                  environment={k: os.environ.get(k) for k in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
                    'CARGO_PROFILE_RELEASE_OPT_LEVEL', 'OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'RAYON_NUM_THREADS']},
                  scope=manifest['scope'], aa_control='same baseline source/probe/executable, no duplicated finite witness calls; adaptive sampling horizons can differ')
    (out/'header.json').write_text(json.dumps(header, indent=2)+'\n')
    events = []
    def execute(route, kind, case, action, rep=None, call_kind='flat'):
        name, shots, policy = case
        circuit = PREP/'fixtures'/(name+'.stim')
        number = len(events)
        if kind == 'structural':
            config = dict(circuit=str(circuit), arithmetic=policy, cache_bytes=64<<20,
                          shots=shots, repetitions=manifest['repetitions'], action=action,
                          call_kind=call_kind, total=129 if action == 'dump' else shots)
            if action == 'dump':
                config['shots'] = 1 if call_kind == 'structured' else 129
            file = out/f'{number:05d}.config.json'
            file.write_text(json.dumps(config)+'\n')
            cmd = [str(BINARIES[route][kind]), str(file)]
        else:
            cmd = [str(BINARIES[route][kind]),
                   str(circuit), str(shots), str(1 if action == 'validate' else manifest['repetitions']),
                   policy, action, 'native']
        event, data = recorder.invoke(cmd, kind, route=route, case=case,
                                     action=action, pair=rep, call_kind=call_kind)
        events.append(event)
        return event
    all_cases = [('structural', c) for c in manifest['structural_cases']] + [('counts', c) for c in manifest['application_counts_cases']]
    # Existing public probe gives exact native-vs-structured counts/RNG witnesses.
    # Structural validation compares scalar and flat rows within each fixed plan,
    # never compares seeded rows across different compiler revisions.
    witness_cases = {(kind, case[0], case[2]): (kind, (case[0], 1, case[2])) for kind, case in all_cases if kind == 'structural'}
    witness_cases.update({('counts', *case): ('counts', case) for case in manifest['application_counts_cases']})
    for kind, case in witness_cases.values():
        for route in ['baseline','candidate']:
            if kind == 'counts':
                event = execute(route, kind, case, 'validate')
                validate_counts(event,case,inputs[case[0]+'.stim'],(PREP/'fixtures'/(case[0]+'.stim')).read_text(),finite=True)
            elif case[1] == 1:
                a = execute(route, kind, case, 'dump', call_kind='structured')
                b = execute(route, kind, case, 'dump', call_kind='flat')
                validate_structural_pair(a,b,case,inputs[case[0]+'.stim'])
    print('validation closed', flush=True)
    for pair in range(manifest['pairs']):
        order = all_cases[pair:] + all_cases[:pair]
        if pair % 2:
            order.reverse()
        for index, (kind, case) in enumerate(order):
            routes = list(ROOTS)
            ordinal = all_cases.index((kind, case))
            shift = (pair + ordinal) % len(routes)
            routes = routes[shift:] + routes[:shift]
            if (pair + ordinal) % 2:
                routes.reverse()
            for route in routes:
                execute(route, kind, case, 'bench', rep=pair)
        print('paired round', pair+1, 'closed', flush=True)
    preparation_seal_after=verify_preparation(preparation)
    require(preparation_seal_after==preparation_seal_before,'preparation seal changed')
    after = {route: identity(route) for route in ROOTS}
    require(before == after, 'source/probe/binary mutation')
    require(inputs == {p.name: sha(p) for p in (PREP/'fixtures').glob('*.stim')}, 'input mutation')
    summaries = []
    for kind, case in all_cases:
        medians = {route: [] for route in ROOTS}
        complete = True
        for pair in range(manifest['pairs']):
            for route in ROOTS:
                matching = [e for e in events if e['route'] == route and e['kind'] == kind and e['case'] == case and e['action'] == 'bench' and e['pair'] == pair]
                require(len(matching) == 1, 'missing timing process')
                e = matching[0]
                if not timing_transport_ok(e):
                    complete = False
                    continue
                data = e['result']
                if kind=='counts':validate_counts(e,case,inputs[case[0]+'.stim'],(PREP/'fixtures'/(case[0]+'.stim')).read_text())
                else:validate_structural_timing(e,case,inputs[case[0]+'.stim'])
                require(data['input_sha256'] == inputs[case[0]+'.stim'], 'consumed input mismatch')
                observations = data['warm' if kind == 'structural' else 'observations']
                validate_observations(observations, manifest['repetitions'])
                medians[route].append(statistics.median(o['ns_per_call'] for o in observations))
        row = dict(kind=kind, case=case, complete=complete, process_medians_ns=medians)
        if complete:
            ratios = [a/b for a,b in zip(medians['baseline'],medians['candidate'])]
            row.update(baseline_ns=statistics.median(medians['baseline']),
                       candidate_ns=statistics.median(medians['candidate']),
                       median_ratio=statistics.median(medians['baseline'])/statistics.median(medians['candidate']),
                       paired_ratios=ratios,
                       null_ratio=statistics.median(medians['baseline'])/statistics.median(medians['control']),
                       null_paired_ratios=[a/b for a,b in zip(medians['baseline'],medians['control'])])
        summaries.append(row)
    (out/'summary.json').write_text(json.dumps(summaries,indent=2)+'\n')
    require(len(events) == manifest['expected_events'], 'unexpected child event count')
    (out/'closure.json').write_text(json.dumps(dict(finished=time.time(),events=len(events),
        events_sha256=sha(out/'events.jsonl'),identities_after=after,preparation_seal_after=preparation_seal_after,
        driver_sha256=sha(Path(__file__)),inputs_after=inputs),indent=2)+'\n')
    print("source pair actual closed", flush=True)

if __name__ == '__main__':
    main()
