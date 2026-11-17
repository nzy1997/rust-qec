#!/usr/bin/env python3
"""Replay cultivation timing cells on a common host with pinned peer runtimes."""
import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import importlib.metadata
import json
import math
import os
from pathlib import Path
import platform
import re
import resource
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

import cultivation_benchmark as benchmark
import cultivation_statistics as exact
import reference
from remaining_contracts import read_json, require


THREADS = {name:'1' for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS')}
EXPECTED_NATIVE = '8764804ce11a1ef0ce4ab7df71d72811d1ad6d693b55c564e3c777a65bfcc9e2'
SYMFT_COMMIT = 'c89b98514a919240b8afa53a271e08d926d3c987'
SYMFT_ARCHIVE = '30e9b25cfcbb4f04dce204bb9c3d72f5b924de56225f5dd5f47e606dd4e3fe01'
PPVM_COMMIT = '7b2e1bcbca06bcf28a57ef600b80273d6a337842'
BACKENDS = ('magna','clifft','symft','ppvm')
FAILURE_STATUSES = ('failed','timeout','invalid')
LEGACY_SCHEMA = 'rmagna.cultivation-matched-peer-timing.v1'
SCHEMA = 'rmagna.cultivation-matched-peer-timing.v2'
OUTPUT_CONTRACT = 'joint acceptance and target failure per attempted draw'
RETAINED_CAPTURE = Path(__file__).parent/'paper/cultivation-peer-capture-v1.json'


def host_fingerprint():
    if sys.platform=='darwin':
        details = subprocess.check_output(
            ['/usr/sbin/ioreg','-rd1','-c','IOPlatformExpertDevice'], text=True)
        value = re.search(r'"IOPlatformUUID"\s*=\s*"([0-9a-fA-F-]+)"',details)
        require(value is not None, 'missing macOS platform UUID for matched timing')
        identity = value[1].lower()
    else:
        path = next((path for path in (Path('/etc/machine-id'),Path('/var/lib/dbus/machine-id'))
                     if path.is_file()),None)
        require(path is not None, 'matched timing requires a machine identifier')
        identity = path.read_text().strip()
        require(re.fullmatch(r'[0-9a-f]{32}',identity) is not None, 'invalid machine identifier for matched timing')
        identity += ':'+platform.node()
    return hashlib.sha256(f'{sys.platform}:{identity}'.encode()).hexdigest()


def timing_identity():
    return dict(host=dict(system=f'{platform.system()}-{platform.release()}', machine=platform.machine(),
                          fingerprint=host_fingerprint()),
                output_contract=OUTPUT_CONTRACT)


def check_timing_identity(actual, expected):
    require(all(actual.get(key) == expected[key] for key in ('host','output_contract')),
            'matched timing host or output contract changed')


def legacy_capture(receipt_path, receipt):
    if receipt.get('schema') != LEGACY_SCHEMA:
        require(receipt.get('schema') == SCHEMA, 'unsupported matched timing receipt')
        return None
    pinned = read_json(RETAINED_CAPTURE)
    require(pinned['schema']=='rmagna.cultivation-matched-peer-provenance.v1'
            and reference.digest(receipt_path) in (pinned['receipt_sha256'],pinned['prior_receipt_sha256']),
            'retained legacy matched timing receipt changed')
    return pinned


def timing_cases(plan):
    return {(case['id'],backend):timing_case(case,backend)
            for case in benchmark.cases(plan) if case['kind']=='timing'
            for backend in ('rust', *BACKENDS)}


def previous_failure_inventory(root, receipt, wanted, *, legacy=False):
    directory = root/'previous-attempts'
    require(not directory.is_symlink(), 'invalid previous timing attempts directory')
    rows = receipt.get('previous_failures', [] if legacy else None)
    require(isinstance(rows,list), 'missing previous timing attempts')
    paths = [row['path'] for row in rows]
    actual = {path.name for path in directory.iterdir()} if directory.exists() else set()
    require(len(paths)==len(set(paths)) and set(paths)==actual, 'previous timing attempt inventory differs')
    for row in rows:
        key = row['case']['id'], row['backend']
        path = reference.relative_file(directory,row['path'])
        stage = read_json(path)
        require(reference.digest(path)==row['sha256'] and key in wanted
                and row['case']==stage['case']==wanted[key] and row['backend']==stage.get('backend','rust')
                and stage['status']==row['status'] and row['status'] in FAILURE_STATUSES,
                'prior peer failure was changed or promoted')
    return rows


def active_batch_inventory(root, receipt, wanted):
    directory = root/'batches'
    require(directory.is_dir() and not directory.is_symlink(), 'invalid matched batch directory')
    rows = receipt['cells']
    paths = [row['path'] for row in rows]
    actual = {path.name for path in directory.iterdir()}
    require(len(paths)==len(set(paths)) and set(paths)==actual, 'active matched batch inventory differs')
    cells = {}
    for row in rows:
        key = row['case']['id'],row['backend']
        require(key in wanted and key not in cells and row['case']==wanted[key],
                'wrong or duplicate matched timing cell')
        path = reference.relative_file(directory,row['path'])
        require(reference.digest(path)==row['sha256'], 'matched cell bytes changed')
        stage = read_json(path)
        require(stage['case']==row['case'] and stage.get('backend','rust')==row['backend']
                and stage['status']==row['status'] and row['status'] in ('complete', *FAILURE_STATUSES),
                'matched timing cell status or address changed')
        cells[key] = (row,path,stage)
    if receipt.get('schema')==SCHEMA:
        backends = receipt['backends']
        require(isinstance(backends,list) and len(backends)==len(set(backends))
                and set(backends)<=set(BACKENDS), 'invalid matched backend coverage')
        expected = {key for key in wanted if key[1] in {'rust', *backends}}
        require(set(cells)==expected, 'matched batch inventory dropped a declared cell')
    return cells


def failure_counts(receipt, *, legacy=False):
    previous = receipt.get('previous_failures', [] if legacy else None)
    return Counter((row['case']['id'],row['backend'],row['status'],row['sha256'])
                   for row in (*previous,*receipt['cells'])
                   if row['status']!='complete')


def reuse_history(root, receipt):
    directory = root/'reuse-history'
    require(not directory.is_symlink(), 'invalid matched reuse history directory')
    digest = receipt['reuse_parent_sha256']
    paths = {}
    retained = failure_counts(receipt)
    while digest is not None:
        require(benchmark.sha(digest) and digest not in paths, 'invalid or cyclic matched reuse parent')
        path = reference.relative_file(directory,digest+'.json')
        require(reference.digest(path)==digest, 'matched reuse parent receipt changed')
        parent = read_json(path)
        legacy = legacy_capture(path,parent)
        require(retained >= failure_counts(parent,legacy=legacy is not None),
                'matched reuse dropped an inherited failed attempt')
        if legacy:
            require(receipt['host']['fingerprint']==legacy['host_fingerprint'],
                    'legacy matched reuse host changed')
        else:
            check_timing_identity(parent,receipt)
        paths[digest] = path
        digest = None if legacy else parent['reuse_parent_sha256']
    actual = {path.name for path in directory.iterdir()} if directory.exists() else set()
    require(actual=={path.name for path in paths.values()}, 'matched reuse history inventory differs')
    return paths


def native_acceptance(payload, case):
    direct = payload['direct']
    estimates = direct['estimates']
    a, e, conditional = (estimates[key] for key in ('acceptance','error','conditional_error'))
    require(direct['status']=='complete' and direct['seed']==case['seed']
            and direct['requested']==direct['completed']==payload['completed']==case['shots']
            and direct['failed']==direct['not_attempted']==0 and direct['failures']==[]
            and direct['timings']['sampling_seconds']==payload['sampling_seconds']
            and direct['timings']['decoder_seconds']==0, 'Magna native counts, seed or timing changed')
    require(0 <= e <= a <= 1
            and ((conditional is None and a==0)
                 or (a>0 and isinstance(conditional,(float,int))
                     and math.isclose(conditional,e/a,rel_tol=1e-12,abs_tol=1e-15))),
            'Magna native joint or conditional score changed')
    return a


def check_symft_counts(payload, case):
    direct = payload['direct']
    require(direct['shots']==payload['completed']==case['shots'] and direct['active_threads']==1
            and direct['accepted']==payload['accepted'] and direct['logical_errors']==payload['accepted_failure']
            and direct['accepted']+direct['discarded']==case['shots'], 'SymFT direct counts changed')


def peer_circuit(physical, task, target):
    """Lower a pinned one-output task to sampled detectors and target readout."""
    require(set(task) <= {'outputs','schema_version','ideal_state'} and len(task['outputs']) == 1,
            'peer adapter needs a single output and a unitary target')
    wire = task['outputs'][0]
    circuit = []
    detectors, selection = 0, {}
    for raw in physical.splitlines():
        line = raw.partition('#')[0].strip()
        if not line:
            continue
        tokens = line.split()
        if tokens[0] == 'POSTSELECT':
            for token in tokens[1:]:
                match = re.fullmatch(r'(!?)det\[([0-9]+)\]', token)
                require(match is not None, 'peer adapter needs signed detector postselection')
                index = int(match[2])
                require(index not in selection, 'repeated peer detector selection')
                selection[index] = int(bool(match[1]))
        elif not tokens[0].startswith('OBSERVABLE_INCLUDE('):
            if tokens[0] == 'DETECTOR' or tokens[0].startswith('DETECTOR('):
                detectors += 1
            circuit.append(line)
    require(set(selection) == set(range(detectors)), 'peer adapter needs every selected detector')
    target_gates = [line.partition('#')[0].strip() for line in target.splitlines()]
    target_gates = [line for line in target_gates if line]
    if target_gates == ['H 0','T 0']:
        circuit.extend([f'T_DAG {wire}',f'H {wire}'])
    else:
        require(not target_gates, 'unsupported cultivation target for sampled readout')
    circuit.extend([f'M {wire}', 'OBSERVABLE_INCLUDE(0) rec[-1]'])
    return '\n'.join(circuit)+'\n', [selection[index] for index in range(detectors)]


def sampled_input(paths):
    return peer_circuit(paths['physical_sha256'].read_text(),read_json(paths['task_sha256']),
                        paths['ideal_sha256'].read_text())


def timing_case(case, backend):
    return dict(case,shots=128) if backend == 'ppvm' and case['family'] == 'rp2-d5-T' else case


def lower_inverted_measurements(text):
    """PPVM lacks !q. Conjugation flips the report and restores the projected state."""
    output = []
    flips = {'M':'X','MX':'Z','MY':'X'}
    for line in text.splitlines():
        gate, _, tail = line.partition(' ')
        basis = gate.split('(',1)[0]
        if basis not in flips or '!' not in tail:
            output.append(line)
            continue
        for target in tail.split():
            require(re.fullmatch(r'!?[0-9]+',target) is not None, 'unsupported PPVM measurement target')
            if target.startswith('!'):
                qubit = target[1:]
                flip = f'{flips[basis]} {qubit}'
                output.extend([flip,f'{gate} {qubit}',flip])
            else:
                output.append(f'{gate} {target}')
    return '\n'.join(output)+'\n'


def scoring_layout(text, expected):
    """Absolute measurement rows for peers whose executor returns raw records."""
    records, target = 0, None
    detectors = []
    for line in text.splitlines():
        # Coordinate arguments contain whitespace and do not count as targets.
        instruction, _, args = re.sub(r'\([^)]*\)', '', line, count=1).partition(' ')
        row = args.split()
        if instruction in ('M','MX','MY','MPP'):
            records += len(row)
        if instruction == 'DETECTOR':
            terms = []
            for token in row:
                match = re.fullmatch(r'rec\[-([1-9][0-9]*)\]',token)
                require(match is not None and int(match[1]) <= records, 'invalid peer detector lookback')
                terms.append(records-int(match[1]))
            detectors.append(terms)
        if instruction == 'OBSERVABLE_INCLUDE':
            require(line == 'OBSERVABLE_INCLUDE(0) rec[-1]' and target is None,
                    'peer sampled readout must be a single terminal measurement')
            target = records-1
        require(instruction not in ('REPEAT','MR','MRX','MRY','MPAD','MXX','MYY','MZZ'),
                'unsupported peer measurement layout')
    require(len(detectors)==len(expected) and target==records-1 and records>0, 'incomplete peer scoring layout')
    return dict(detectors=detectors,expected=list(map(bool,expected)),target_record=target)


def native_cell(upstream, paths, case):
    sys.path.insert(0,str(upstream/'core'))
    from magna import _native, compile_circuit, run

    extension = Path(_native.__file__).resolve()
    require(extension.is_relative_to(upstream/'core/magna'), 'Magna extension escaped its pinned source')
    binary_hash = reference.digest(extension)
    started = time.perf_counter()
    compiled = compile_circuit(paths['physical_sha256'], task=paths['task_sha256'], options={
        'history_sampler':'affine_support','max_path_bits':65536,'max_doubled_path_bits':131072,
        'max_quotient_rank':12,'max_terms':4096}, sampling={'method':'physical'})
    preparation = time.perf_counter()-started
    require(compiled.route == 'linear' and compiled._compilation['sampled_history'] is False,
            'Magna peer used sampled history')
    rates = set(sum(Fraction(*p) for p in row) for row in compiled.noise_schedule.probabilities)
    maximum_deviation = max(abs(rate-Fraction(case['p'])) for rate in rates)
    require(maximum_deviation <= Fraction(1,10**18), 'Magna peer uses different physical probabilities')
    warmup = run(compiled,shots=1,seed=0)
    require(warmup['status'] == 'complete' and warmup['completed'] == 1, 'Magna warmup failed')
    output = run(compiled,shots=case['shots'],seed=case['seed'])
    require(reference.digest(extension) == binary_hash and output['status'] == 'complete'
            and output['completed'] == case['shots'] and output['timings']['decoder_seconds'] == 0,
            'Magna timing failed or used different callback scope')
    return dict(build=dict(upstream_commit=reference.load_manifest()['upstream']['commit'],
                           extension_sha256=binary_hash, protected_author_extension=binary_hash == EXPECTED_NATIVE),
                preparation_seconds=preparation, sampling_seconds=output['timings']['sampling_seconds'],
                completed=output['completed'], physical_rate_deviation=str(maximum_deviation),
                direct=output)


def clifft_cell(paths, case):
    import clifft
    from clifft import _clifft_core
    import numpy as np

    require(clifft.__version__ == importlib.metadata.version('clifft') == '0.11.0', 'expected Clifft 0.11.0')
    text, expected = sampled_input(paths)
    started = time.perf_counter()
    program = clifft.compile(text)
    preparation = time.perf_counter()-started

    def draw(shots, seed):
        result = clifft.sample(program,shots=shots,seed=seed,threads=1)
        require(result.detectors.shape == (shots,len(expected)) and result.observables.shape == (shots,1),
                'Clifft did not return the complete selected and target parities')
        accepted = (result.detectors == expected).all(axis=1)
        return int(np.count_nonzero(accepted)), int(np.count_nonzero(accepted & (result.observables[:,0] != 0)))

    draw(1,0)
    started = time.perf_counter()
    accepted, failed = draw(case['shots'],case['seed'])
    seconds = time.perf_counter()-started
    return dict(build=dict(version=clifft.__version__, numpy=np.__version__,
                           extension_sha256=reference.digest(Path(_clifft_core.__file__))),
                preparation_seconds=preparation,sampling_seconds=seconds,completed=case['shots'],
                adapted_circuit_sha256=hashlib.sha256(text.encode()).hexdigest(),
                expected_detectors=expected,accepted=accepted,accepted_failure=failed)


def symft_cell(paths, case, expected_extension):
    import symft
    from symft import _native

    extension = Path(_native.__file__)
    require(benchmark.sha(expected_extension) and reference.digest(extension)==expected_extension,
            'loaded SymFT extension differs from the declared pinned-source build')
    text, expected = sampled_input(paths)
    started = time.perf_counter()
    sampler = symft.Circuit(text).compile_counts_sampler(batch=True,observable=0,
        postselect_detectors=True,reference_sample=False,expected_detectors=list(map(bool,expected)),
        expected_observables=[False],threads=1,cuda=False)
    preparation_seconds = time.perf_counter()-started
    require(sampler.info['num_detectors'] == len(expected) and sampler.info['threads'] == 1,
            'SymFT sampler changed detector count or thread scope')
    warmup = sampler.sample(shots=1,stream_id=0)
    require(warmup['shots']==1, 'SymFT warmup failed')
    started = time.perf_counter()
    result = sampler.sample(shots=case['shots'],stream_id=case['seed'])
    sampling_seconds = time.perf_counter()-started
    require(result['active_threads']==1 and result['shots']==case['shots']
            and result['accepted']+result['discarded']==case['shots']
            and reference.digest(extension)==expected_extension, 'SymFT changed binary, timing or draw scope')
    return dict(build=dict(upstream_commit=SYMFT_COMMIT,archive_sha256=SYMFT_ARCHIVE,
                           extension_sha256=expected_extension),
                preparation_seconds=preparation_seconds,sampling_seconds=sampling_seconds,completed=result['shots'],
                adapted_circuit_sha256=hashlib.sha256(text.encode()).hexdigest(),expected_detectors=expected,
                accepted=result['accepted'],accepted_failure=result['logical_errors'],
                direct=result, sampler_info=sampler.info)


def ppvm_cell(paths, case, binary):
    text, expected = sampled_input(paths)
    text = lower_inverted_measurements(text)
    binary_hash = reference.digest(binary)
    with tempfile.TemporaryDirectory(prefix='cultivation-ppvm-') as directory:
        program = Path(directory)/'full-t.stim'
        scoring = Path(directory)/'scoring.json'
        program.write_text(text)
        benchmark.atomic(scoring,scoring_layout(text,expected))
        result = json.loads(subprocess.check_output([binary,program,scoring,str(case['shots']),str(case['seed'])]))
    require(result['schema']=='rmagna.cultivation-ppvm-timing.v1' and result['upstream_commit']==PPVM_COMMIT
            and result['threads']==1 and result['coefficient_threshold']==1e-10
            and reference.digest(binary)==binary_hash, 'PPVM revision/thread/threshold or executable changed')
    result.update(build=dict(upstream_commit=PPVM_COMMIT,executable_sha256=binary_hash),
                  adapted_circuit_sha256=hashlib.sha256(text.encode()).hexdigest(),expected_detectors=expected)
    return result


def bounded_worker(command, env, seconds):
    with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          text=True, env=env, start_new_session=True) as process:
        try:
            stdout, stderr = process.communicate(timeout=seconds)
        except subprocess.TimeoutExpired:
            # The PPVM adapter runs inside this subprocess group as well.
            try:
                os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError:
                pass
            stdout, stderr = process.communicate()
            return dict(status='timeout',reason=f'exceeded {seconds} seconds',stdout=stdout,stderr=stderr)
        return dict(status='failed',returncode=process.returncode,stdout=stdout,stderr=stderr)


def worker(args):
    pinned = reference.load_manifest()
    plan = benchmark.manifest(pinned)
    case = next(row for row in benchmark.cases(plan) if row['id'] == args.cell)
    case = timing_case(case,args.backend)
    paths = {key:args.upstream/'examples'/path for key,path in benchmark.input_paths(plan,case['family']).items()}
    identities = {key:reference.digest(path) for key,path in paths.items()}
    if args.backend == 'magna':
        result = native_cell(args.upstream,paths,case)
    elif args.backend == 'ppvm':
        result = ppvm_cell(paths,case,args.ppvm_executable)
    elif args.backend == 'symft':
        result = symft_cell(paths,case,args.symft_extension_sha256)
    else:
        result = clifft_cell(paths,case)
    # ru_maxrss is bytes on Darwin and KiB on Linux.
    rss = max(resource.getrusage(who).ru_maxrss for who in (resource.RUSAGE_SELF,resource.RUSAGE_CHILDREN))
    rss *= 1 if sys.platform=='darwin' else 1024
    print(json.dumps(dict(case=case,inputs=identities,peer=args.backend,result=result,peak_rss_bytes=rss,
                          **timing_identity()),allow_nan=False))


def run(args):
    pinned = reference.load_manifest()
    plan = benchmark.manifest(pinned)
    binary = args.rust_example.resolve(strict=True)
    upstream = args.upstream.resolve(strict=True)
    result_root = args.out.resolve()
    result_root.mkdir(exist_ok=False)
    (result_root/'batches').mkdir()
    reference.verify_source(upstream,pinned)
    identity = timing_identity()
    receipt = dict(schema=SCHEMA, threads=1, reuse_parent_sha256=None, backends=args.backends, **identity,
                   rust_source_sha256=benchmark.source_identity(benchmark.ROOT),rust_binary_sha256=reference.digest(binary),
                   inputs=benchmark.input_identity(plan,result_root/'inputs',pinned,copy_from=upstream),
                   bounds=dict(timeout_seconds=args.timeout_seconds, rss_limit_bytes=64 << 30),
                   timing_protocol=dict(threads=1,excluded_warmup_draws=1,shots=1024,repetitions=3),
                   variants=dict(rust='integrated exact full-T masses',magna='integrated native full-T masses',
                                 clifft='full-T Born records with equivalent signed detectors and target readout',
                                 symft='full-T sampled counts with signed detector postselection',
                                 ppvm='full-T Born records with the paper coefficient threshold'))
    wanted = timing_cases(plan)
    old_results, previous_failures = {}, []
    retained_legacy = saved_symft_build = None
    if args.reuse is not None:
        existing_path = args.reuse/'receipt.json'
        existing = read_json(existing_path)
        retained_legacy = legacy_capture(existing_path,existing)
        for key in ('threads','rust_source_sha256','rust_binary_sha256','inputs','timing_protocol','output_contract'):
            require(existing[key]==receipt[key], 'timing reuse identity changed: '+key)
        # The archived v1 host used Python's presentation string. CPython 3.14
        # includes the executable format on macOS, unlike the capture runtime.
        expected_host = (dict(system=platform.platform().removesuffix('-Mach-O'),machine=platform.machine())
                         if retained_legacy else identity['host'])
        require(existing['host']==expected_host, 'timing reuse identity changed: host')
        if retained_legacy:
            require(identity['host']['fingerprint']==retained_legacy['host_fingerprint'],
                    'legacy matched reuse host changed')
        old_results = active_batch_inventory(args.reuse,existing,wanted)
        ancestors = {} if retained_legacy else reuse_history(args.reuse,existing)
        for row in previous_failure_inventory(args.reuse,existing,wanted,legacy=retained_legacy is not None):
            previous = result_root/'previous-attempts'
            previous.mkdir(exist_ok=True)
            shutil.copyfile(args.reuse/'previous-attempts'/row['path'],previous/row['path'])
            previous_failures.append(row)
        parent = reference.digest(existing_path)
        receipt['reuse_parent_sha256'] = parent
        history = result_root/'reuse-history'
        history.mkdir()
        for digest,path in {**ancestors,parent:existing_path}.items():
            shutil.copyfile(path,history/(digest+'.json'))
        saved_symft_build = retained_legacy['symft_build'] if retained_legacy else existing.get('symft_build')
    if 'symft' in args.backends:
        if args.symft_archive is not None or args.symft_extension_sha256 is not None:
            require(args.symft_archive is not None and reference.digest(args.symft_archive)==SYMFT_ARCHIVE
                    and benchmark.sha(args.symft_extension_sha256),
                    'SymFT timing requires the pinned source archive and the built extension SHA-256')
            declared = dict(upstream_commit=SYMFT_COMMIT,archive_sha256=SYMFT_ARCHIVE,
                            extension_sha256=args.symft_extension_sha256)
            require(saved_symft_build is None or saved_symft_build==declared, 'reused SymFT build changed')
            saved_symft_build = declared
        require(saved_symft_build is not None, 'SymFT timing requires a pinned-source build identity')
        receipt['symft_build'] = saved_symft_build
    if 'ppvm' in args.backends and any(
        key not in old_results or (args.retry_failed and old_results[key][0]['status']!='complete')
        for key in wanted if key[1]=='ppvm'
    ):
        require(args.ppvm_executable is not None, 'PPVM timing requires its pinned executable')
    env = dict(os.environ, **THREADS, PYTHONDONTWRITEBYTECODE='1')
    results = []
    stage_identity = dict(identity,reuse_parent_sha256=receipt['reuse_parent_sha256'])

    def preserve_failure(row,path):
        previous = result_root/'previous-attempts'
        previous.mkdir(exist_ok=True)
        name = f'{path.stem}-attempt-{len(previous_failures)}.json'
        require(not (previous/name).exists(), 'duplicate previous attempt path')
        shutil.copyfile(path,previous/name)
        previous_failures.append(dict(row,path=name))

    selected = {'rust', *args.backends}
    for (_,backend),(row,path,_) in old_results.items():
        if backend not in selected and row['status']!='complete':
            preserve_failure(row,path)

    def retained(case, backend):
        entry = old_results.get((case['id'],backend))
        if entry is None:
            return None
        row,path,stage = entry
        if args.retry_failed and row['status'] != 'complete':
            preserve_failure(row,path)
            return None
        destination = result_root/'batches'/path.name
        if stage['status']=='complete':
            if not retained_legacy:
                check_timing_identity(stage,identity)
                require(stage['reuse_parent_sha256']==existing['reuse_parent_sha256'],
                        'reused timing stage lost its source receipt')
            stage.update(stage_identity)
            if retained_legacy and backend!='rust':
                stage['output'].update(identity)
            benchmark.atomic(destination,stage)
            return dict(row,sha256=reference.digest(destination))
        shutil.copyfile(path,destination)
        return row

    for case in benchmark.cases(plan):
        if case['kind'] != 'timing':
            break
        row = retained(case,'rust')
        if row is None:
            status = benchmark.run_stage(case,plan,result_root,binary,receipt)
            batch = benchmark.read_stage(result_root,case)
            batch.update(stage_identity)
            benchmark.atomic(benchmark.stage_path(result_root,case),batch)
            row = dict(case=case,backend='rust',status=status,path=benchmark.stage_path(result_root,case).name,
                       sha256=reference.digest(benchmark.stage_path(result_root,case)))
            if status == 'complete':
                payload = batch['payload']
                values = exact.histogram(payload)
                a = exact.moments(values,Fraction(case['p']),Fraction(case['p']),payload['law']['original_sites'],'acceptance')
                row.update(seconds_per_draw=payload['sampling_seconds']/case['shots'],
                           acceptance=exact.as_float(a['mean']),preparation_seconds=payload['preparation']['seconds'])
        results.append(row)
        for backend in args.backends:
            peer_case = timing_case(case,backend)
            old = retained(peer_case,backend)
            if old is not None:
                results.append(old)
                continue
            command = [args.python,str(Path(__file__).resolve()),'worker','--backend',backend,
                       '--upstream',upstream,'--cell',case['id']]
            if backend == 'ppvm':
                require(args.ppvm_executable is not None, 'PPVM timing requires its pinned executable')
                command.extend(['--ppvm-executable',args.ppvm_executable.resolve(strict=True)])
            if backend == 'symft':
                command.extend(['--symft-extension-sha256',saved_symft_build['extension_sha256']])
            stage = dict(case=peer_case,backend=backend,status='failed',**stage_identity)
            try:
                stage.update(bounded_worker(command,env,args.timeout_seconds))
                if stage.get('returncode') == 0:
                    output = json.loads(stage['stdout'])
                    check_timing_identity(output,identity)
                    require(output['inputs'] == receipt['inputs'][case['family']] and output['case'] == peer_case,
                            'peer did not use the matched full T input')
                    payload = output['result']
                    require(payload['completed'] == peer_case['shots'] and benchmark.valid_seconds(payload['sampling_seconds'])
                            and output['peak_rss_bytes'] <= receipt['bounds']['rss_limit_bytes'],
                            'peer timing output failed its bounds')
                    stage.update(status='complete',output=output,seconds_per_draw=payload['sampling_seconds']/peer_case['shots'])
                    del stage['stdout']
            except (OSError,ValueError,KeyError) as error:
                stage['reason'] = str(error)
            path = result_root/'batches'/(case['id']+f'-{backend}.json')
            benchmark.atomic(path,stage)
            results.append(dict(case=peer_case,backend=backend,status=stage['status'],path=path.name,sha256=reference.digest(path),
                                seconds_per_draw=stage.get('seconds_per_draw')))
            print(case['id']+' '+backend+' '+stage['status'],flush=True)
    reference.verify_source(upstream,pinned)
    require(reference.digest(binary) == receipt['rust_binary_sha256'], 'matched timing executable changed')
    receipt['cells'] = results
    receipt['previous_failures'] = previous_failures
    active_batch_inventory(result_root,receipt,wanted)
    previous_failure_inventory(result_root,receipt,wanted)
    reuse_history(result_root,receipt)
    summary = {}
    for family in benchmark.FAMILIES:
        summary[family] = {}
        for backend in ['rust', *args.backends]:
            rows = [row for row in results if row['case']['family']==family and row['backend']==backend]
            require(len(rows) == 3, 'missing matched timing cell')
            summary[family][backend] = dict(statuses=[row['status'] for row in rows],
                median_seconds_per_draw=statistics.median(row['seconds_per_draw'] for row in rows)
                if all(row['status']=='complete' for row in rows) else None)
    receipt['summary'] = summary
    benchmark.atomic(result_root/'receipt.json',receipt)
    print(json.dumps(summary,sort_keys=True))


def check(root):
    """Check raw matched cells, including actual quotas, adapter and peer revisions."""
    pinned = reference.load_manifest()
    plan = benchmark.manifest(pinned)
    receipt = read_json(root/'receipt.json')
    legacy = legacy_capture(root/'receipt.json',receipt)
    host_fields = {'system','machine'} if legacy else {'system','machine','fingerprint'}
    require(isinstance(receipt.get('host'),dict) and set(receipt['host'])==host_fields
            and all(isinstance(value,str) and value.strip() for value in receipt['host'].values())
            and (legacy or benchmark.sha(receipt['host']['fingerprint']))
            and receipt.get('output_contract')==OUTPUT_CONTRACT
            and receipt['inputs'] == benchmark.input_identity(plan,root/'inputs',pinned)
            and receipt['rust_source_sha256'] == benchmark.source_identity(benchmark.ROOT)
            and receipt['threads'] == 1 and receipt['timing_protocol'] == dict(
                threads=1,excluded_warmup_draws=1,shots=1024,repetitions=3), 'matched timing identity or protocol changed')
    symft_build = legacy['symft_build'] if legacy else receipt['symft_build']
    wanted = timing_cases(plan)
    previous_failure_inventory(root,receipt,wanted,legacy=legacy is not None)
    cells = active_batch_inventory(root,receipt,wanted)
    require(set(cells)==set(wanted), 'matched timing report dropped a failed or missing cell')
    if not legacy:
        reuse_history(root,receipt)
    data, builds = {}, {}
    for key,(row,_,stage) in cells.items():
        if not legacy:
            check_timing_identity(stage,receipt)
            require(stage['reuse_parent_sha256']==receipt['reuse_parent_sha256'],
                    'matched timing stage lost its source receipt')
        require(row['status']==stage['status']=='complete' and stage['returncode']==0,
                'matched backend failed or missed a timing cell: '+str(key))
        case = wanted[key]
        backend = key[1]
        if backend == 'rust':
            observations = benchmark.validate_stage(stage,case,plan,receipt)
            payload = stage['payload']
            rate=Fraction(case['p'])
            moments=exact.moments(observations,rate,rate,payload['law']['original_sites'],'acceptance')
            acceptance=exact.as_float(moments['mean'])
        else:
            output = stage['output']
            if not legacy:
                check_timing_identity(output,receipt)
            require(output['case']==case and output['peer']==backend and output['inputs']==receipt['inputs'][case['family']]
                    and 0 < output['peak_rss_bytes'] <= 64 << 30, 'matched peer source, backend or memory bound changed')
            payload = output['result']
            build = payload['build']
            require(builds.setdefault(backend,build)==build, 'peer build changed across matched batches')
            require(payload['completed']==case['shots'], 'matched peer completed count changed')
            if backend == 'magna':
                require(build['upstream_commit']==pinned['upstream']['commit'] and benchmark.sha(build['extension_sha256'])
                        and Fraction(payload['physical_rate_deviation']) <= Fraction(1,10**18), 'Magna peer law or binary changed')
                acceptance=native_acceptance(payload,case)
            else:
                paths = {k:root/'inputs'/name for k,name in benchmark.input_paths(plan,case['family']).items()}
                text, expected = sampled_input(paths)
                if backend == 'ppvm':
                    text = lower_inverted_measurements(text)
                require(payload['adapted_circuit_sha256']==hashlib.sha256(text.encode()).hexdigest()
                        and payload['expected_detectors']==expected
                        and 0 <= payload['accepted_failure'] <= payload['accepted'] <= case['shots'],
                        'matched peer readout or selected signs changed')
                acceptance=payload['accepted']/case['shots']
                if backend=='clifft':
                    require(build['version']=='0.11.0' and benchmark.sha(build['extension_sha256']), 'Clifft version changed')
                elif backend=='symft':
                    require(build==symft_build and build['upstream_commit']==SYMFT_COMMIT
                            and build['archive_sha256']==SYMFT_ARCHIVE and benchmark.sha(build['extension_sha256']),
                            'SymFT revision or thread count changed')
                    check_symft_counts(payload,case)
                else:
                    require(build['upstream_commit']==PPVM_COMMIT and benchmark.sha(build['executable_sha256'])
                            and payload['threads']==1 and payload['coefficient_threshold']==1e-10,
                            'PPVM revision, threads or coefficient threshold changed')
        seconds=payload['sampling_seconds']
        require(0 <= acceptance <= 1 and benchmark.valid_seconds(seconds) and seconds > 0
                and row['seconds_per_draw']==seconds/case['shots'], 'matched timing/acceptance changed')
        data.setdefault((case['family'],backend),[]).append((acceptance,case['shots'],seconds/case['shots']))
    summary = {}
    for family in benchmark.FAMILIES:
        summary[family] = {}
        for backend in ('rust', *BACKENDS):
            batches=data[(family,backend)]
            samples=sum(n for _,n,_ in batches)
            acceptance=math.fsum(a*n for a,n,_ in batches)/samples
            summary[family][backend] = dict(shots=samples, acceptance=acceptance,
                median_seconds_per_draw=statistics.median(t for _,_,t in batches))
        baseline=summary[family]['rust']
        for backend in BACKENDS:
            peer=summary[family][backend]
            # Every contribution belongs to [0,1], so Bernoulli variance is
            # conservative even for the integrated exact Rust/Magna draws.
            a, b = baseline['acceptance'],peer['acceptance']
            variance=a*(1-a)/baseline['shots']+b*(1-b)/peer['shots']
            require(abs(a-b) <= 6*math.sqrt(variance)+0.005, 'peer acceptance differs on matched input: '+family+' '+backend)
    return dict(status='PASS', executed_batches=len(cells), builds=builds, summary=summary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation',required=True)
    execute = sub.add_parser('run')
    execute.add_argument('--upstream',type=Path,required=True)
    execute.add_argument('--out',type=Path,required=True)
    execute.add_argument('--python',type=Path,required=True)
    execute.add_argument('--rust-example',type=Path,required=True)
    execute.add_argument('--backends',nargs='+',choices=BACKENDS,default=['magna','clifft'])
    execute.add_argument('--timeout-seconds',type=int,default=120)
    execute.add_argument('--reuse',type=Path)
    execute.add_argument('--retry-failed',action='store_true')
    execute.add_argument('--symft-archive',type=Path)
    execute.add_argument('--symft-extension-sha256')
    execute.add_argument('--ppvm-executable',type=Path)
    verify = sub.add_parser('check')
    verify.add_argument('--results',type=Path,required=True)
    child = sub.add_parser('worker')
    child.add_argument('--upstream',type=Path,required=True)
    child.add_argument('--backend',choices=BACKENDS,required=True)
    child.add_argument('--cell',required=True)
    child.add_argument('--ppvm-executable',type=Path)
    child.add_argument('--symft-extension-sha256')
    args = parser.parse_args()
    if args.operation == 'check':
        print(json.dumps(check(args.results.resolve(strict=True)),sort_keys=True))
    elif args.operation == 'run':
        require(1 <= args.timeout_seconds <= 1800 and len(set(args.backends))==len(args.backends), 'invalid peer run limits')
        run(args)
    else:
        worker(args)


if __name__ == '__main__':
    main()
