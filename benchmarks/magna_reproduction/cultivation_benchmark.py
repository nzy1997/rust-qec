"""Run and verify addressed, source-bound cultivation benchmark batches."""
import concurrent.futures
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import time

from bh_benchmark import validate_inventory
import cultivation_reference as historical
import cultivation_statistics as exact
from paper_probe_identity import source_identity, source_inputs
import reference
from remaining_contracts import read_json, require


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CONTRACT = 'unit_prescribed_full_T_joint_acceptance_error_with_external_P_over_Q'
PROTOCOL = 'splitmix64-addressed-u32-seed-u32-index-v1'
FAMILIES = {
    'color3': ('color3', 518, 16, 4, 2444, 1, 'readout-ideal.magna', 2026100900, 2026100902, 22),
    'color5': ('color5', 3564, 93, 14, 15994, 1, 'readout-ideal.magna', 2026101200, 2026101202, 27),
    'rp2-d3-T': ('rp2-d3', 678, 34, 10, 3624, 1, 'target.magna', 2026101000, 2026101002, 22),
    'rp2-d5-T': ('rp2-d5', 2405, 102, 30, 12973, 2, 'target.magna', 2026101300, 2026101502, 26),
}
RUNNER_FILES = ('cultivation_benchmark.py', 'cultivation_statistics.py', 'cultivation_reference.py', 'paper_benchmark.py',
                'paper_probe_identity.py', 'reference.py', 'bh_benchmark.py')


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def digest_json(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def sha(value):
    return isinstance(value, str) and re.fullmatch(r'[0-9a-f]{64}', value) is not None


def atomic(path, value):
    temporary = path.with_suffix(path.suffix + '.next')
    with temporary.open('w', encoding='utf-8') as stream:
        stream.write(canonical(value)+'\n')
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def manifest(pinned, pilot=False):
    families = {}
    for family, (task, n, safe, unsafe, channels, closing, target, a_seed, e_seed, power) in FAMILIES.items():
        rates = (['1/2000', '3/4000', '1/1000', '3/2000', '1/500'] if power > 22
                 else ['1/2000', '1/1000', '1/500', '3/1000', '1/200'])
        grid = [dict(p=p, acceptance_seed=a_seed+10*i, failure_seed=e_seed+10*i,
                     acceptance_shots=1024 if pilot else 1 << 24, failure_shots=1024 if pilot else 1 << power)
                for i, p in enumerate(rates) if not pilot or p == '1/1000']
        families[family] = dict(task=task, original_sites=n, safe=safe, unsafe_t=unsafe, channels=channels,
                                closing_sites=closing, target=target, grid=grid)
    return dict(schema='rmagna.paper-cultivation-plan.v1', scope='pilot' if pilot else 'figure4-table2',
                reference_sha256=reference.digest(HERE/'reference.json'), upstream_commit=pinned['upstream']['commit'],
                paper=pinned['paper']['arxiv'], archive_sha256=pinned['paper']['archive_sha256'],
                historical_kernel=pinned['paper']['kernel_commit'], families=families, output_contract=CONTRACT,
                rng_protocol=PROTOCOL, chunk_size=1 << 20, timeout_seconds=1800,
                max_signature_work=1 << 34, max_entropy_words=1 << 20, max_histogram_entries=1 << 15,
                timing=dict(p='1/1000', seed_base=2026100101, shots=1024, repetitions=0 if pilot else 3,
                            threads=1, excluded_warmup_draws=1),
                proposal='uniform total p at every original site; q=p+3/N; tail mixture alpha=19/20',
                uncertainty='independent physical acceptance and closing accepted-failure streams')


def validate_manifest(value, pinned):
    require(digest_json(value) == digest_json(manifest(pinned, pilot=value['scope'] == 'pilot')),
            'cultivation source pins, sample sizes, or design changed')


def cases(plan):
    timing = plan['timing']
    for family in FAMILIES:
        for repetition in range(timing['repetitions']):
            yield dict(id=f'{family}-timing-{repetition}', family=family, point='timing',
                       p=timing['p'], kind='timing', seed=timing['seed_base']+repetition,
                       start=0, shots=timing['shots'])
    for family in FAMILIES:
        for point, data in enumerate(plan['families'][family]['grid']):
            for kind in ('acceptance', 'failure'):
                shots = data[f'{kind}_shots']
                step = plan['chunk_size']
                for start in range(0, shots, step):
                    yield dict(id=f'{family}-p{point}-{kind}-{start:09d}', family=family, point=point,
                               p=data['p'], kind=kind, seed=data[f'{kind}_seed'], start=start,
                               shots=min(step, shots-start))


def configuration(case, plan):
    closing = case['kind'] == 'failure'
    return dict(schema='rmagna.paper-cultivation-configuration.v1', physical_rate=case['p'],
                mode='closing' if closing else 'physical',
                closing_sites=plan['families'][case['family']]['closing_sites'] if closing else 0,
                seed=case['seed'], start=case['start'], shots=case['shots'],
                **{key:plan[key] for key in ('max_signature_work','max_entropy_words','max_histogram_entries')})


def input_paths(plan, family):
    info = plan['families'][family]
    return {key: Path(info['task'])/name for key, name in (
        ('physical_sha256','physical.magna'), ('task_sha256','task.json'), ('ideal_sha256',info['target']))}


def input_identity(plan, root, pinned, copy_from=None):
    result = {}
    for family in FAMILIES:
        result[family] = {}
        for key, path in input_paths(plan, family).items():
            expected = pinned['upstream']['files'][(Path('examples')/path).as_posix()]
            if copy_from is not None:
                source = reference.relative_file(copy_from, (Path('examples')/path).as_posix())
                destination = root/path
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source,destination)
            actual = reference.digest(reference.relative_file(root, path.as_posix()))
            require(actual == expected, 'cultivation full-T source/target changed: '+str(path))
            result[family][key] = actual
    return result


def valid_seconds(value):
    return type(value) in (float, int) and 0 <= value < 1e10 and math.isfinite(value)


def validate_score_work(payload, histogram):
    completed = payload['completed']
    if 'score_work' not in payload:
        validate_inventory(payload['inventory'], completed)
        return
    counts = payload['score_work']
    require(isinstance(counts, dict) and set(counts) ==
            {'rejected_by_detectors', 'reused_no_fault', 'reused_signature', 'evaluated_full_t'}
            and all(type(count) is int and count >= 0 for count in counts.values())
            and sum(counts.values()) == completed, 'invalid cultivation score work')
    empty = sum(count for (key, _, _), count in histogram.items() if key[0] == 0)
    rejected = sum(count for (key, acceptance, _), count in histogram.items()
                   if key[0] != 0 and acceptance == exact.ZERO)
    require(counts['reused_no_fault'] == empty and counts['rejected_by_detectors'] <= rejected,
            'cultivation reuse or rejection does not match the exact histogram')
    if payload['configuration']['closing_sites']:
        failed_constraints = sum(count for (key, _, _), count in histogram.items()
                                 if key[0] != 0 and not key[-1])
        require(counts['rejected_by_detectors'] == failed_constraints,
                'cultivation closing rejection does not match the exact constraints')
    skipped = payload.get('skipped_zero_targets', 0)
    require(type(skipped) is int and 0 <= skipped <= min(
        counts['evaluated_full_t'], rejected - counts['rejected_by_detectors']),
        'invalid cultivation skipped zero-target work')
    validate_inventory(payload['inventory'], counts['evaluated_full_t'], preparation_assignments=1,
                       skipped_terminal_queries=skipped)


def validate_stage(stage, case, plan, receipt):
    require(digest_json(stage['case']) == digest_json(case) and stage['status'] == 'complete'
            and type(stage['returncode']) is int and stage['returncode'] == 0,
            'cultivation failed, missing, or mismatched batch: '+case['id'])
    require(valid_seconds(stage['wall_seconds']), 'invalid cultivation wall time')
    payload = stage['payload']
    require(payload['schema'] == 'rmagna.paper-cultivation-run.v1' and payload['status'] == 'complete'
            and payload['failure'] is None and payload['output_contract'] == CONTRACT
            and payload['rng_protocol'] == PROTOCOL, 'mixed or failed cultivation output contract')
    require(digest_json(payload['configuration']) == digest_json(configuration(case, plan)),
            'cultivation seed, range, or proposal configuration changed')
    require(payload['rust_source_sha256'] == receipt['rust_source_sha256']
            and payload['executable_sha256'] == receipt['rust_binary_sha256']
            and payload['inputs'] == receipt['inputs'][case['family']]
            and payload['build']['debug_assertions'] is False, 'mixed cultivation source, input, or executable')
    n = case['shots']
    require(type(payload['completed']) is int and payload['completed'] == n
            and type(payload['entropy_words']) is int and payload['entropy_words'] > 0,
            'cultivation completed counts or entropy missing')
    require(digest_json(payload['excluded_warmup']) == digest_json(dict(draws=1, seed=case['seed'], index=(1 << 32)-1)),
            'missing or pooled cultivation warmup')
    law = payload['law']
    family = plan['families'][case['family']]
    channels, flow = law['conditional_channel_counts'], law['flow']
    sites = family['original_sites']
    require(type(law['original_sites']) is int and law['original_sites'] == len(channels) == sites
            and all(type(value) is int and value in (1,3,15) for value in channels)
            and sum(channels) == family['channels'], 'cultivation original channel inventory changed')
    d = family['safe'] + family['unsafe_t']
    require(flow['offset'] == [] and len(flow['retained']) == family['safe'] and len(flow['unsafe_t']) == family['unsafe_t']
            and all(type(value) is int for value in flow['retained']+flow['unsafe_t'])
            and sorted(flow['retained']+flow['unsafe_t']) == list(range(d))
            and flow['signature_words'] == sum(channels)*((d+63)//64), 'cultivation constraint inventory changed')
    p = Fraction(case['p'])
    q = p+Fraction(3,sites) if case['kind'] == 'failure' else p
    require(law['physical_rate'] == str(p) and law['proposal_rate'] == str(q)
            and law['mixture_alpha'] == '19/20', 'cultivation physical or closing law changed')
    preparation = payload['preparation']
    require(valid_seconds(payload['sampling_seconds']) and payload['sampling_seconds'] > 0
            and valid_seconds(preparation['compile_seconds']) and valid_seconds(preparation['seconds'])
            and preparation['compile_seconds'] <= preparation['seconds'], 'invalid cultivation preparation/sampling time')
    insertions = preparation['insertions']
    require(type(insertions['catalog_entries']) is int and insertions['catalog_entries'] >= sum(channels)
            and type(insertions['estimated_peak_bytes']) is int
            and 0 < insertions['estimated_peak_bytes'] <= 128 << 20, 'cultivation prepared insertion budget changed')
    for key in ('acceptance','target'):
        family_resources = payload['resources'][key]
        require(digest_json(family_resources) == digest_json(dict(queries=1, resources=1, maximum_quotient_rank=0, maximum_terms=1)),
                'cultivation prepared resource changed: '+key)
    histogram = exact.histogram(payload)
    validate_score_work(payload, histogram)
    return histogram


def stage_path(root, case):
    return root/'batches'/(case['id']+'.json')


def read_stage(root, case):
    path = stage_path(root, case)
    require(path.is_file() and not path.is_symlink(), 'missing cultivation batch: '+case['id'])
    return read_json(path)


def run_stage(case, plan, out, executable, receipt):
    path = stage_path(out, case)
    config = path.with_suffix('.config.json')
    atomic(config, configuration(case,plan))
    inputs = input_paths(plan,case['family'])
    command = [executable, out/'inputs'/inputs['physical_sha256'], out/'inputs'/inputs['task_sha256'], config]
    started = time.monotonic()
    stage = dict(case=case, status='failed')
    try:
        result = subprocess.run(command, capture_output=True, timeout=plan['timeout_seconds'], check=False)
        stage.update(returncode=result.returncode, stderr=result.stderr.decode(errors='replace'))
        try:
            stage['payload'] = json.loads(result.stdout)
        except (ValueError, UnicodeError):
            stage['stdout'] = result.stdout.decode(errors='replace')
        if result.returncode == 0:
            stage['status'] = 'complete'
    except subprocess.TimeoutExpired as error:
        stage.update(status='timeout', timeout_seconds=plan['timeout_seconds'],
                     stdout=(error.stdout or b'').decode(errors='replace'),
                     stderr=(error.stderr or b'').decode(errors='replace'))
    except OSError as error:
        stage.update(status='failed', launch_error=str(error))
    stage['wall_seconds'] = time.monotonic()-started
    if stage['status'] == 'complete':
        try:
            validate_stage(stage, case, plan, receipt)
        except (ValueError, KeyError, TypeError) as error:
            stage.update(status='invalid', validation_error=str(error))
    atomic(path,stage)
    config.unlink()
    print(case['id']+' '+stage['status'],flush=True)
    return stage['status']


def results_from_batches(plan, out, receipt):
    expected = list(cases(plan))
    actual = {path.name for path in (out/'batches').glob('*.json')}
    require(actual == {stage_path(out, case).name for case in expected}, 'cultivation batch inventory is incomplete or has extra files')
    total = 0
    hashes = {}
    precision, laws, timing, work = {}, {}, {}, {}
    for case in expected:
        stage = read_stage(out, case)
        entries = validate_stage(stage, case, plan, receipt)
        hashes[case['id']] = reference.digest(stage_path(out,case))
        payload = stage['payload']
        if case['kind'] == 'timing':
            timing.setdefault(case['family'],[]).append(dict(
                sampling_seconds=payload['sampling_seconds'],
                seconds_per_draw=payload['sampling_seconds']/case['shots'],
                preparation_seconds=payload['preparation']['seconds'], wall_seconds=stage['wall_seconds']))
            continue
        total += case['shots']
        key = (case['family'],case['point'],case['kind'])
        law = payload['law']
        require(laws.setdefault(key, law) == law, 'cultivation proposal law changed across chunks')
        work.setdefault(key,[]).append(stage['wall_seconds'])
        aggregate = precision.setdefault(key,{})
        for observation, count in entries.items():
            aggregate[observation] = aggregate.get(observation,0)+count
    points = {}
    for family in FAMILIES:
        points[family] = []
        info = plan['families'][family]
        for index, data in enumerate(info['grid']):
            p = Fraction(data['p'])
            keys = [(family,index,kind) for kind in ('acceptance','failure')]
            a, e = [exact.moments(precision[key],p,Fraction(laws[key]['proposal_rate']),info['original_sites'],key[2])
                    for key in keys]
            result = exact.two_estimator(a,e)
            result['p'] = data['p']
            result['time_to_observed_precision'] = dict(
                acceptance_worker_seconds=math.fsum(work[keys[0]]),
                failure_worker_seconds=math.fsum(work[keys[1]]),
                relative_conditional_error_se=result['conditional_error_se']/result['conditional_error']
                if result['precision_resolved'] else None)
            points[family].append(result)
    result = dict(schema='rmagna.paper-cultivation-report.v1', batch_sha256=hashes, executed_batches=len(expected),
                inputs=receipt['inputs'],
                independent_precision_draws=total, precision=points,
                timing={family:dict(batches=rows, median_seconds_per_draw=statistics.median(
                    row['seconds_per_draw'] for row in rows)) for family,rows in timing.items()})
    if plan['scope'] != 'pilot':
        result['historical_comparison'] = historical.compare(plan,out/'historical',result,reference.load_manifest())
    return result


def runner_identity():
    return {name:reference.digest(HERE/name) for name in RUNNER_FILES}


def verify_identity(plan, out, receipt, pinned):
    old_source = read_json(HERE/'paper/cultivation-capture-v1.json')
    known_runner = (receipt['runner_sha256'] == runner_identity()
                    or (old_source['schema'] == 'rmagna.cultivation-capture-source.v1'
                        and receipt['rust_source_sha256'] == old_source['rust_source_sha256']
                        and receipt['runner_sha256'] == old_source['runner_sha256']))
    require(receipt['schema'] == 'rmagna.paper-cultivation-capture.v1'
            and receipt['manifest_sha256'] == digest_json(plan)
            and receipt['inputs'] == input_identity(plan,out/'inputs',pinned)
            and known_runner
            and receipt['rust_source_sha256'] == source_identity(ROOT)
            and receipt['rust_source_inputs'] == source_inputs(ROOT)
            and sha(receipt['rust_binary_sha256']), 'cultivation capture identity changed')
    require(type(receipt['precision_workers']) is int and 1 <= receipt['precision_workers'] <= 64
            and receipt['timing_workers'] == 1, 'cultivation timing must use one worker')


def capture(args, plan):
    pinned = reference.load_manifest()
    validate_manifest(plan,pinned)
    executable = args.rust_example.resolve(strict=True)
    upstream = args.upstream.resolve(strict=True)
    out = args.out.resolve()
    require(1 <= args.jobs <= 64, 'cultivation --jobs must be 1..64')
    reference.verify_source(upstream,pinned)
    if args.resume:
        receipt = read_json(out/'receipt.json')
        verify_identity(plan,out,receipt,pinned)
        require(receipt['runner_sha256'] == runner_identity()
                and receipt['rust_binary_sha256'] == reference.digest(executable)
                and receipt['precision_workers'] == args.jobs, 'cultivation resume executable or workers changed')
    else:
        out.mkdir(parents=True,exist_ok=False)
        (out/'batches').mkdir()
        receipt = dict(schema='rmagna.paper-cultivation-capture.v1', status='recording', manifest_sha256=digest_json(plan),
                       inputs=input_identity(plan,out/'inputs',pinned,copy_from=upstream), runner_sha256=runner_identity(),
                       rust_source_sha256=source_identity(ROOT), rust_source_inputs=source_inputs(ROOT),
                       rust_binary_sha256=reference.digest(executable), precision_workers=args.jobs, timing_workers=1,
                       host=dict(os=platform.system(),architecture=platform.machine(),release=platform.release(),
                                 logical_cpus=os.cpu_count()))
        atomic(out/'receipt.json',receipt)
    # Lock ownership is the open OS file descriptor, which releases at process exit.
    import fcntl
    with (out/'capture.lock').open('a') as lock:
        try:
            fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError('cultivation capture already running') from error
        if plan['scope'] != 'pilot':
            if not (out/'historical').is_dir():
                require(args.archive is not None, 'full cultivation capture requires --archive with the pinned paper deposit')
                historical.capture(args.archive.resolve(strict=True),out/'historical',plan,pinned)
            reference.verify_files(out/'historical', {name:pinned['paper']['files'][name]
                                                    for name in historical.members(plan,pinned)})
        remaining = []
        for case in cases(plan):
            path = stage_path(out,case)
            if path.exists():
                validate_stage(read_stage(out,case),case,plan,receipt)
            else:
                remaining.append(case)
        for case in remaining:
            if case['kind'] == 'timing':
                require(run_stage(case,plan,out,executable,receipt) == 'complete', 'cultivation timing failed; receipt retained')
        pending = iter(case for case in remaining if case['kind'] != 'timing')
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as workers:
            def submit(case):
                return workers.submit(run_stage,case,plan,out,executable,receipt)
            live = {submit(case) for case in (next(pending,None) for _ in range(args.jobs)) if case is not None}
            while live:
                done, live = concurrent.futures.wait(live,return_when=concurrent.futures.FIRST_COMPLETED)
                require(all(future.result() == 'complete' for future in done), 'cultivation execution failed; receipt retained')
                for _ in done:
                    case = next(pending,None)
                    if case is not None:
                        live.add(submit(case))
        reference.verify_source(upstream,pinned)
        verify_identity(plan,out,receipt,pinned)
        require(receipt['rust_binary_sha256'] == reference.digest(executable),'cultivation executable changed during capture')
        report = results_from_batches(plan,out,receipt)
        atomic(out/'report.json',report)
        receipt.update(status='recorded',report_sha256=reference.digest(out/'report.json'))
        atomic(out/'receipt.json',receipt)
    return summarize(plan,report)


def summarize(plan,report):
    return dict(status='PASS', scope=plan['scope'], executed_batches=report['executed_batches'],
                independent_precision_draws=report['independent_precision_draws'],
                precision_points=sum(map(len,report['precision'].values())),
                rust_timing_batches=sum(len(row['batches']) for row in report['timing'].values()))


def check_capture(plan,out):
    pinned = reference.load_manifest()
    validate_manifest(plan,pinned)
    out = out.resolve(strict=True)
    receipt = read_json(out/'receipt.json')
    verify_identity(plan,out,receipt,pinned)
    actual = results_from_batches(plan,out,receipt)
    report = read_json(out/'report.json')
    require(digest_json(report) == digest_json(actual), 'cultivation independent denominator, variance, or raw batch identity differs')
    require(receipt['status'] == 'recorded' and receipt['report_sha256'] == reference.digest(out/'report.json'),
            'cultivation capture report is incomplete or changed')
    return summarize(plan,actual)
