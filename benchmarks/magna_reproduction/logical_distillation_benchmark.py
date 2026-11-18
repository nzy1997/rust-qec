"""Capture the full encoded-input precision curve and serial timing run."""

from collections import Counter
from fractions import Fraction
from functools import partial
import math
from pathlib import Path

from cultivation_benchmark import digest_json, sha, valid_seconds
from cultivation_statistics import add, as_float, divide, multiply, scale, sign
import decoder_campaign
from decoder_campaign import stage_path
import decoder_workloads_reference as historical
from logical_distillation_statistics import NONIDENTITY, P0, RATES, ResidualWeights, histogram, moments
import reference
from remaining_contracts import read_json, require


HERE = Path(__file__).resolve().parent
ROOT = 'examples/logical-distillation'
SCHEMA = 'rmagna.paper-logical-distillation-plan.v1'
CAPTURE = 'rmagna.paper-logical-distillation-capture.v1'
PROTOCOL = 'splitmix64-addressed-u32-seed-u32-index-v1'
INPUTS = dict(physical_sha256='physical.magna', ideal_sha256='ideal.magna',
              task_sha256='task.json', controllers_sha256='controllers.json')
DECODER_SOURCES = ('decoder/__init__.py', 'decoder/decoder.py', 'decoder/generate.py', 'decoder/patch.py')
DECODER_CALLS = dict(gauge=(5, 4, 9), ideal_input_recovery=(15, 8, 18), readout_X=(4, 9, 1), readout_Z=(15, 9, 1))
CALLS_PER_DRAW = sum(shape[0] for shape in DECODER_CALLS.values())
RUNNER_FILES = ('benchmarks/magna_reproduction/logical_distillation_benchmark.py',
                'benchmarks/magna_reproduction/logical_distillation_statistics.py',
                'benchmarks/magna_reproduction/paper/logical-residuals.json')


def manifest(pinned, pilot=False):
    return dict(schema=SCHEMA, scope='pilot' if pilot else 'figure5-serial-table1',
                reference_sha256=reference.digest(HERE/'reference.json'),
                archive_sha256=pinned['paper']['archive_sha256'], paper=pinned['paper']['arxiv'],
                upstream_commit=pinned['upstream']['commit'], historical_kernel=pinned['paper']['kernel_commit'],
                rates=list(map(str,RATES)), proposal_rate=str(P0), nonidentity_class_mass=str(NONIDENTITY),
                precision=dict(seed=historical.LOGICAL_SCOPES['encoded-input'], draws=32 if pilot else 16384,
                               chunk_size=16 if pilot else 256, timeout_seconds=900),
                serial_timing=dict(seed=historical.LOGICAL_SCOPES['serial-matched'], draws=16 if pilot else 16384,
                                   timeout_seconds=14400, excluded_warmup_draws=1, workers=1),
                rng_protocol=PROTOCOL, born_seed_offset=100000,
                target=dict(physical_qubits=102, physical_records=451, controls=334, outputs=9,
                            input_recovery_stages=15, decoder_calls_per_draw=CALLS_PER_DRAW))


def validate_manifest(plan, pinned):
    require(digest_json(plan) == digest_json(manifest(pinned, plan['scope'] == 'pilot')),
            'logical distillation source pins, proposal, or draw grid changed')


def cases(plan):
    timing = plan['serial_timing']
    yield dict(id='serial-timing', kind='timing', seed=timing['seed'], start=0, shots=timing['draws'],
               timeout_seconds=timing['timeout_seconds'])
    precision = plan['precision']
    for start in range(0, precision['draws'], precision['chunk_size']):
        yield dict(id=f'precision-{start:05d}', kind='precision', seed=precision['seed'], start=start,
                   shots=min(precision['chunk_size'], precision['draws']-start), timeout_seconds=precision['timeout_seconds'])


def configuration(case, _plan):
    return dict(schema='rmagna.paper-logical-distillation-configuration.v1',
                scope='serial_timing' if case['kind'] == 'timing' else 'precision',
                **{key:case[key] for key in ('seed','start','shots')})


def input_pins(pinned):
    files = pinned['upstream']['files']
    return {f'{ROOT}/{name}': files[f'{ROOT}/{name}'] for name in (*INPUTS.values(), *DECODER_SOURCES)}


def historical_pins(pinned):
    files = pinned['paper']['files']
    names = [f'raw/logical-distillation/{scope}/{name}' for scope in historical.LOGICAL_SCOPES
             for name in ('metadata.json', 'result.json')]
    names.append('raw/logical-distillation/serial-matched/draws.npz')
    return {name:files[name] for name in names}


def command(_case, out, executable, python, config):
    root = out/'inputs'/ROOT
    return [executable, root/'physical.magna', root/'task.json', config, root/'controllers.json', python]


def verify_decoder(payload, expected_inputs, pinned):
    decoder = payload['decoder']
    paths = pinned['upstream']['files']
    require(decoder['controllers_sha256'] == expected_inputs['controllers_sha256']
            and decoder['module_sources'] == {name:paths[f'{ROOT}/{name}'] for name in DECODER_SOURCES}
            and set(decoder['bindings']) == set(DECODER_CALLS), 'logical distillation decoder identity changed')
    expected = dict(gauge=dict(distance=3, mode='gauge'),
                    ideal_input_recovery=dict(distance=3, mode='ideal_recovery'),
                    readout_X=dict(distance=3, mode='readout', basis='X'),
                    readout_Z=dict(distance=3, mode='readout', basis='Z'))
    for name, config in expected.items():
        require(decoder['bindings'][name] == dict(import_name='decoder.decoder:decode', version='1',
                                                 config_sha256=digest_json(config)),
                'logical distillation decoder binding changed: ' + name)
    calls = payload['first_decoder_calls']
    require(isinstance(calls, list) and len(calls) == CALLS_PER_DRAW, 'missing logical distillation decoder transcript')
    counts = Counter()
    for index, call in enumerate(calls):
        require(type(call['call']) is int and call['call'] == index and call['name'] in DECODER_CALLS,
                'invalid logical distillation decoder call order')
        name = call['name']
        _, inputs, outputs = DECODER_CALLS[name]
        for field, size in (('inputs', inputs), ('outputs', outputs)):
            require(isinstance(call[field], list) and len(call[field]) == size
                    and all(type(bit) is bool for bit in call[field]), 'logical distillation decoder wires changed')
        counts[name] += 1
    require(counts == {name:shape[0] for name,shape in DECODER_CALLS.items()}
            and sha(payload['decoder_transcript_sha256']), 'logical distillation omitted decoder stages')


def validate_stage(stage, case, plan, receipt, pinned, model):
    require(digest_json(stage['case']) == digest_json(case) and stage['status'] == 'complete'
            and type(stage['returncode']) is int and stage['returncode'] == 0 and valid_seconds(stage['wall_seconds']),
            'failed, missing, or mismatched logical distillation batch: ' + case['id'])
    payload = stage['payload']
    expected_inputs = {key:pinned['upstream']['files'][f'{ROOT}/{name}'] for key,name in INPUTS.items()}
    require(digest_json(payload['configuration']) == digest_json(configuration(case,plan))
            and payload['rng_protocol'] == PROTOCOL and type(payload['born_seed']) is int
            and payload['born_seed'] == case['seed']+plan['born_seed_offset']
            and digest_json(payload['excluded_warmup']) == digest_json(dict(draws=1,index=(1 << 32)-1)),
            'logical distillation seed, range, or warmup changed')
    require(payload['rust_source_sha256'] == receipt['rust_source_sha256']
            and payload['executable_sha256'] == receipt['rust_binary_sha256']
            and payload['inputs'] == expected_inputs and payload['build']['debug_assertions'] is False,
            'logical distillation source, executable or encoded target changed')
    verify_decoder(payload, expected_inputs, pinned)
    elapsed = payload['timing']
    require(all(valid_seconds(elapsed[key]) for key in ('preparation_seconds','compile_seconds','sampling_seconds','decoder_seconds'))
            and 0 < elapsed['compile_seconds'] <= elapsed['preparation_seconds']
            and 0 < elapsed['decoder_seconds'] < elapsed['sampling_seconds'] <= stage['wall_seconds'],
            'invalid logical distillation timing')
    require(all(type(elapsed[key]) is int and elapsed[key] == CALLS_PER_DRAW*case['shots']
                for key in ('decoder_attempted','decoder_completed')), 'logical distillation decoder count differs')
    observations = histogram(payload,model)
    for row in payload['rows']:
        require(set(row['resources']) == {'acceptance','target'} and all(
                type(pair) is list and len(pair) == 2 and all(type(item) is int for item in pair)
                and pair[0] == 0 and pair[1] in (0,1) for pair in row['resources'].values()),
                'logical distillation reference rank or term count differs')
    return observations


def exact_field(encoded):
    return tuple(Fraction(int(num,16),int(den,16)) for num,den in encoded)


def comparison(measured, source):
    points = []
    for index, (row, old) in enumerate(zip(measured['rows'],source['points'],strict=True)):
        require(row['p'] == old['p'] and row['q_eff'] == old['q_eff'], 'logical distillation rate or effective q changed')
        sd_a = math.sqrt(as_float(exact_field(measured['covariance']['acceptance'][index][index])))
        sd_e = math.sqrt(as_float(exact_field(measured['covariance']['conditional_error'][index][index])))
        a, error = row['acceptance_float'], row['conditional_error_float']
        points.append(dict(p=row['p'], q_eff=row['q_eff'], acceptance_standard_error=sd_a,
                           conditional_standard_error=sd_e if measured['precision_resolved'] else None,
                           acceptance_pull=(a-old['acceptance'])/math.hypot(sd_a,old['acceptance_se']),
                           conditional_error_pull=(error-old['conditional_error'])/math.hypot(sd_e,old['conditional_error_se'])
                           if measured['precision_resolved'] else None))
    endpoints = None
    if measured['precision_resolved']:
        values = measured['rows']
        lo, hi = (exact_field(values[index]['conditional_error']) for index in (0,-1))
        covariance = measured['covariance']['conditional_error']
        parts = [divide(exact_field(covariance[i][j]),multiply(x,y))
                 for i,j,x,y in ((0,0,lo,lo),(-1,-1,hi,hi),(0,-1,lo,hi))]
        total = add(add(parts[0],parts[1]),scale(parts[2],-2))
        require(sign(total) >= 0, 'invalid logical distillation endpoint variance')
        log_base = math.log(float(RATES[-1]/RATES[0]))
        endpoints = dict(slope=(math.log(as_float(hi))-math.log(as_float(lo)))/log_base,
                         standard_error=math.sqrt(as_float(total))/log_base)
    return dict(points=points, endpoint=endpoints)


def results_from_batches(plan, out, receipt, pinned, model):
    planned = list(cases(plan))
    require({path.name for path in (out/'batches').iterdir()} == {stage_path(out,case).name for case in planned},
            'logical distillation batch inventory is incomplete or has extra files')
    deposited = historical.logical_distillation(
        lambda name: read_json(reference.relative_file(out/'historical', name)), plan['historical_kernel'])
    precision = Counter()
    raw_hashes = {}
    serial = None
    for case in planned:
        path = reference.relative_file(out, 'batches/' + stage_path(out,case).name)
        stage = read_json(path)
        observations = validate_stage(stage,case,plan,receipt,pinned,model)
        raw_hashes[case['id']] = reference.digest(path)
        if case['kind'] == 'precision':
            precision.update(observations)
        else:
            elapsed = stage['payload']['timing']
            count = case['shots']
            serial = dict(attempted=count, seed=case['seed'], timing=elapsed,
                          sampling_seconds_per_attempt=elapsed['sampling_seconds']/count,
                          decoder_subtracted_seconds_per_attempt=(elapsed['sampling_seconds']-elapsed['decoder_seconds'])/count)
    measured = moments(precision, model)
    require(measured['completed'] == plan['precision']['draws'] and serial is not None,
            'logical distillation precision or serial timing draws are incomplete')
    if measured['minimum_error_weight'] is not None:
        require(measured['minimum_error_weight'] >= 6, 'logical distillation observed subleading fault weight below six')
    return dict(schema='rmagna.paper-logical-distillation-report.v1', scope=plan['scope'],
                measured=measured, comparison=comparison(measured,deposited['encoded-input']),
                serial_timing=serial, historical=deposited, raw_batch_sha256=raw_hashes, completed_batches=len(planned))


def summarize(report):
    return dict(status='PASS', scope=report['scope'], completed_batches=report['completed_batches'],
                precision_draws=report['measured']['completed'], timing_draws=report['serial_timing']['attempted'],
                physical_rates=len(report['measured']['rows']), positive_error_draws=report['measured']['positive_error_draws'])


def campaign(pinned):
    model = ResidualWeights()
    return decoder_campaign.Campaign(
        schema=CAPTURE, label='logical distillation', runner_files=RUNNER_FILES,
        input_pins=input_pins(pinned), historical_pins=historical_pins(pinned),
        cases=cases, configuration=configuration, command=command,
        validate_stage=partial(validate_stage, pinned=pinned, model=model),
        results_from_batches=partial(results_from_batches, pinned=pinned, model=model), summarize=summarize)


def capture(args, plan, run_process):
    pinned = reference.load_manifest()
    validate_manifest(plan,pinned)
    return decoder_campaign.capture(campaign(pinned),args,plan,run_process)


def check_capture(plan, out):
    pinned = reference.load_manifest()
    validate_manifest(plan,pinned)
    return decoder_campaign.check_capture(campaign(pinned),plan,out)
