"""Capture and verify the paper code-switching precision and Table I trials."""

from collections import Counter, defaultdict
from fractions import Fraction
from functools import partial
from pathlib import Path
from statistics import median

from cultivation_benchmark import digest_json, sha, valid_seconds
import decoder_campaign
from decoder_campaign import stage_path
import decoder_workloads_reference as historical
from decoder_workloads_statistics import code_switch_histogram, code_switch_moments, verify_code_switch_decoder
import reference
from remaining_contracts import read_json, require


HERE = Path(__file__).resolve().parent
SCHEMA = 'rmagna.paper-code-switch-plan.v1'
CAPTURE = 'rmagna.paper-code-switch-capture.v1'
CONTRACT = 'unit_noise_history_normalized_masses_with_external_occurrence_ratio'
PROTOCOL = 'splitmix64-addressed-u32-seed-u32-index-v1'
FAMILIES = tuple(historical.CODE_SWITCHING)
TIMED_FAMILIES = ('dk', 'wg-T')
INPUT_FILES = dict(physical_sha256='physical.magna', task_sha256='task.json', ideal_sha256='ideal.magna',
                   recovery_sha256='recovery.magna', controllers_sha256='controllers.json')
RUNNER_FILES = (
    'benchmarks/magna_reproduction/code_switching_benchmark.py',
    'benchmarks/magna_reproduction/decoder_workloads_statistics.py',
)


def manifest(pinned, pilot=False):
    families = {}
    for family, (sites, base, counts) in historical.CODE_SWITCHING.items():
        grid = [dict(p=str(p), q=str(q), draws=8192 if pilot else count, noise_seed=base+index,
                     born_seed=base+historical.BORN_SEED_OFFSET+index)
                for index, (p, q, count) in enumerate(zip(
                    historical.PHYSICAL_RATES, historical.PROPOSAL_RATES, counts, strict=True))]
        families[family] = dict(active_sites=sites, excluded_ideal_sites=0 if family == 'dk' else 8, grid=grid)
    return dict(schema=SCHEMA, scope='pilot' if pilot else 'paper-code-switching',
                reference_sha256=reference.digest(HERE/'reference.json'),
                archive_sha256=pinned['paper']['archive_sha256'], paper=pinned['paper']['arxiv'],
                upstream_commit=pinned['upstream']['commit'], historical_kernel=pinned['paper']['kernel_commit'],
                families=families, chunk_size=4096 if pilot else 1 << 18, timeout_seconds=300,
                max_histogram_entries=1 << 16, rng_protocol=PROTOCOL, output_contract=CONTRACT,
                timing=dict(families=list(TIMED_FAMILIES), p='1/1000', shots=1024, repetitions=3,
                            noise_seed=historical.TIMING_SEED, born_seed=historical.TIMING_SEED+historical.BORN_SEED_OFFSET,
                            excluded_warmup_draws=1, workers=1))


def validate_manifest(plan, pinned):
    require(digest_json(plan) == digest_json(manifest(pinned, plan['scope'] == 'pilot')),
            'code-switching source pins, draw grid, or timing plan changed')


def cases(plan):
    timing = plan['timing']
    for family in timing['families']:
        for index in range(timing['repetitions']):
            yield dict(id=f'{family}-time-{index}', family=family, kind='timing', point=index,
                       p=timing['p'], q=timing['p'], timeout_seconds=plan['timeout_seconds'], noise_seed=timing['noise_seed']+index,
                       born_seed=timing['born_seed']+index, start=0, shots=timing['shots'])
    for family in FAMILIES:
        for index, point in enumerate(plan['families'][family]['grid']):
            for start in range(0, point['draws'], plan['chunk_size']):
                yield dict(id=f'{family}-p{index}-{start:08d}', family=family, kind='precision', point=index,
                           p=point['p'], q=point['q'], timeout_seconds=plan['timeout_seconds'], noise_seed=point['noise_seed'],
                           born_seed=point['born_seed'], start=start, shots=min(plan['chunk_size'], point['draws']-start))


def configuration(case, plan):
    return dict(schema='rmagna.paper-code-switch-configuration.v1', family=case['family'],
                physical_rate=case['p'], proposal_rate=case['q'], noise_seed=case['noise_seed'],
                born_seed=case['born_seed'], start=case['start'], shots=case['shots'],
                max_histogram_entries=plan['max_histogram_entries'])


def input_pins(pinned):
    public = pinned['upstream']['files']
    files = {}
    for family in FAMILIES:
        required = ['physical.magna', 'task.json', 'ideal.magna']
        if family == 'dk':
            required += ['recovery.magna', 'controllers.json', 'decoder/__init__.py', 'decoder/decoder.py']
        for name in required:
            path = f'examples/{family}/{name}'
            files[path] = public[path]
    return files


def historical_pins(pinned):
    names = [f'raw/code-switching/precision-{family}/summary.json' for family in FAMILIES]
    for family in TIMED_FAMILIES:
        folder = historical.TIMING[family][0]
        names.extend([f'{folder}/result.json', *(f'{folder}/draw-batch-{i}.json' for i in range(3))])
    return {name: pinned['paper']['files'][name] for name in names}


def validate_stage(stage, case, plan, receipt, pinned):
    require(digest_json(case) == digest_json(stage['case']) and stage['status'] == 'complete'
            and type(stage['returncode']) is int and stage['returncode'] == 0 and valid_seconds(stage['wall_seconds']),
            'failed, missing, or mismatched code-switching batch: ' + case['id'])
    payload = stage['payload']
    require(digest_json(payload['configuration']) == digest_json(configuration(case, plan))
            and payload['output_contract'] == CONTRACT and payload['rng_protocol'] == PROTOCOL
            and payload['execution_route'] == ('causal_fixed_template' if case['family'] == 'dk' else 'linear_affine'),
            'code-switching seed, range, proposal, or execution route changed')
    family = case['family']
    task_pins = input_pins(pinned)
    expected_inputs = {key: task_pins.get(f'examples/{family}/{name}') for key, name in INPUT_FILES.items()}
    require(payload['executable_sha256'] == receipt['rust_binary_sha256']
            and payload['rust_source_sha256'] == receipt['rust_source_sha256']
            and payload['inputs'] == expected_inputs and payload['build']['debug_assertions'] is False,
            'unexpected code-switching executable, source, target, or recovery')
    require(digest_json(payload['excluded_warmup']) == digest_json(dict(draws=1, index=(1 << 32)-1)),
            'code-switching warmup missing or pooled')
    expected = plan['families'][family]
    for key, plan_key in (('original_sites', 'active_sites'), ('excluded_ideal_sites', 'excluded_ideal_sites')):
        require(type(payload[key]) is int and payload[key] == expected[plan_key], 'code-switching fault sites changed')
    timing = payload['timings']
    require(all(valid_seconds(timing[name]) for name in ('outer_seconds', 'preparation_seconds', 'compile_seconds',
                                                         'sampling_seconds', 'decoder_seconds'))
            and 0 < timing['compile_seconds'] <= timing['preparation_seconds'] <= timing['outer_seconds']
            and 0 < timing['sampling_seconds'] <= timing['outer_seconds'] <= stage['wall_seconds']
            and 0 <= timing['decoder_seconds'] < timing['sampling_seconds'],
            'invalid code-switching timing')
    require(all(type(timing[name]) is int for name in ('decoder_attempted', 'decoder_completed'))
            and (family != 'dk' or sha(payload['decoder_transcript_sha256'])), 'invalid decoder clock or transcript')
    decoder = verify_code_switch_decoder(payload, pinned)
    return code_switch_histogram(payload), decoder


def command(case, out, executable, python, config):
    root = out/'inputs'/'examples'/case['family']
    argv = [executable, root/'physical.magna', root/'task.json', config]
    if case['family'] == 'dk':
        argv += [root/'controllers.json', python]
    return argv


def results_from_batches(plan, out, receipt, pinned):
    expected = list(cases(plan))
    require({path.name for path in (out/'batches').iterdir()} == {stage_path(out, c).name for c in expected},
            'code-switching batch inventory is incomplete or has extra files')
    read = lambda name: read_json(reference.relative_file(out/'historical', name))
    precision = {family: historical.precision(read, family, historical.CODE_SWITCHING[family]) for family in FAMILIES}
    timing = {family: historical.timing(read, family, historical.TIMING[family]) for family in TIMED_FAMILIES}
    timings, entries, draw_hashes = defaultdict(list), defaultdict(Counter), {}
    decoder_calls = Counter()
    for case in expected:
        path = reference.relative_file(out, 'batches/' + stage_path(out, case).name)
        stage = read_json(path)
        histogram, decoder = validate_stage(stage, case, plan, receipt, pinned)
        draw_hashes[case['id']] = reference.digest(path)
        family, point = case['family'], case['point']
        if case['kind'] == 'timing':
            elapsed = stage['payload']['timings']
            count = case['shots']
            timings[family].append(dict(noise_seed=case['noise_seed'], born_seed=case['born_seed'], attempts=count,
                                        decoder_calls=decoder['calls'], timings=elapsed,
                                        sampling_seconds_per_attempt=elapsed['sampling_seconds']/count,
                                        decoder_subtracted_seconds_per_attempt=(elapsed['sampling_seconds']-elapsed['decoder_seconds'])/count))
        else:
            entries[(family,point)].update(histogram)
            decoder_calls.update(decoder)
    require(decoder_calls['nonzero_input_calls'] > 0, 'code-switching observed no nonzero DK syndromes')
    curves = {}
    for family, design in plan['families'].items():
        curves[family] = []
        for index, point in enumerate(design['grid']):
            actual = code_switch_moments(entries[(family,index)], Fraction(point['p']), Fraction(point['q']), design['active_sites'])
            require(actual['completed'] == point['draws'], 'code-switching precision draw count incomplete')
            old = precision[family]['points'][index]
            result = {}
            for field in ('acceptance', 'conditional_error'):
                variance = 'variance_' + field
                denominator = (actual['estimates'][variance] + old[variance])**0.5
                result[field + '_pull'] = (
                    (actual['estimates'][field] - old[field])/denominator
                    if denominator and (field == 'acceptance' or actual['precision_resolved']) else None
                )
            curves[family].append(dict(p=point['p'], q=point['q'], observed=actual, historical=old, comparison=result))
    timing_report = {
        family: dict(trials=trials, historical=timing[family],
                     median_sampling_seconds_per_attempt=median(trial['sampling_seconds_per_attempt'] for trial in trials),
                     median_decoder_subtracted_seconds_per_attempt=median(
                         trial['decoder_subtracted_seconds_per_attempt'] for trial in trials))
        for family, trials in timings.items()
    }
    return dict(schema='rmagna.paper-code-switch-report.v1', scope=plan['scope'], curves=curves, timing=timing_report,
                raw_batch_sha256=draw_hashes, decoder_precision_calls=dict(decoder_calls), completed_batches=len(expected))


def summarize(report):
    return dict(status='PASS', scope=report['scope'], completed_batches=report['completed_batches'],
                precision_draws=sum(point['observed']['completed'] for points in report['curves'].values() for point in points),
                timing_batches=sum(len(row['trials']) for row in report['timing'].values()))


def campaign(pinned):
    return decoder_campaign.Campaign(
        schema=CAPTURE, label='code-switching', runner_files=RUNNER_FILES,
        input_pins=input_pins(pinned), historical_pins=historical_pins(pinned),
        cases=cases, configuration=configuration, command=command,
        validate_stage=partial(validate_stage, pinned=pinned),
        results_from_batches=partial(results_from_batches, pinned=pinned), summarize=summarize)


def capture(args, plan, run_process):
    pinned = reference.load_manifest()
    validate_manifest(plan, pinned)
    return decoder_campaign.capture(campaign(pinned), args, plan, run_process)


def check_capture(plan, out):
    pinned = reference.load_manifest()
    validate_manifest(plan, pinned)
    return decoder_campaign.check_capture(campaign(pinned), plan, out)
