"""Run the deposited TQD terminal curve and Table I timing workload."""

from fractions import Fraction
from functools import partial
import math
from pathlib import Path
from statistics import median
from tempfile import TemporaryDirectory

from cultivation_benchmark import atomic, digest_json, sha, valid_seconds
import decoder_campaign
from decoder_campaign import stage_path
import decoder_workloads_reference as historical
import reference
from remaining_contracts import read_json, require
import tqd_fault_map
from tqd_paper_statistics import RNG_PROTOCOL, WARMUP, fault_stream, historical_networks, paired_rows, summarize, verify_readout_witness


HERE = Path(__file__).resolve().parent
SCHEMA = 'rmagna.paper-tqd-plan.v1'
CAPTURE = 'rmagna.paper-tqd-capture.v1'
MAP_NAME = 'tqd-fault-map.json'
FILES = dict(physical_sha256='physical.magna', task_sha256='branch-0.task.json', target_sha256='target.magna',
             controllers_sha256='controllers.json')
DECODER = ('decoder/__init__.py','decoder/author.py','decoder/decoder.py','decoder/author_source/provenance.json')
MODULES = ('geometry.py','lattice.py','decoder.py','twisted.py','runner.py')
RUNNER_FILES = ('benchmarks/magna_reproduction/tqd_paper_benchmark.py',
                'benchmarks/magna_reproduction/tqd_paper_statistics.py',
                'benchmarks/magna_reproduction/tqd_fault_map.py',
                'benchmarks/magna_reproduction/tqd_author_approximation.py')


def manifest(pinned, pilot=False):
    families = {}
    for size, depth in historical.TQD_SCALES:
        rates = []
        for index,(rate, draws) in enumerate(zip(historical.TQD_RATES,historical.TQD_DRAWS,strict=True)):
            if pilot and index != 0 and (size,index) != (3,4):
                continue
            seed = 202610080000 + size*100000 + index*10000
            pilot_draws = 48 if index == 4 else (8 if size == 3 else 2)
            rates.append(dict(p=str(Fraction(rate)),seed=seed,draws=pilot_draws if pilot else draws,point=index))
        families[f'tqd{size}'] = dict(size=size,depth=depth,outputs=4*size**2,
                                     physical_fault_categories=6*size**2*(3*depth-1),
                                     grid=rates)
    return dict(schema=SCHEMA, scope='pilot' if pilot else 'terminal-curve-table1',
                reference_sha256=reference.digest(HERE/'reference.json'), archive_sha256=pinned['paper']['archive_sha256'],
                paper=pinned['paper']['arxiv'], upstream_commit=pinned['upstream']['commit'],
                historical_kernel=pinned['paper']['kernel_commit'], families=families,
                chunk_size=64, timeout_seconds=1800, rng_protocol=RNG_PROTOCOL, nested_draws=32,
                timing=dict(seed=historical.TIMING_SEED,p='1/100',shots=2 if pilot else 256,
                            families=['tqd3'] if pilot else list(families),repetitions=1 if pilot else 3,
                            timeout_seconds=3600,excluded_warmup_draws=1,workers=1))


def validate_manifest(plan,pinned):
    require(digest_json(plan) == digest_json(manifest(pinned,plan['scope']=='pilot')),
            'TQD source pins, rate grid, or draw schedule changed')


def cases(plan):
    timing=plan['timing']
    for family in timing['families']:
        for i in range(timing['repetitions']):
            yield dict(id=f'{family}-timing-{i}',kind='timing',family=family,p=timing['p'],
                       seed=timing['seed']+i,start=0,shots=timing['shots'],timeout_seconds=timing['timeout_seconds'])
    for family, config in plan['families'].items():
        for point in config['grid']:
            for start in range(0,point['draws'],plan['chunk_size']):
                yield dict(id=f"{family}-p{point['point']}-{start:05d}",kind='precision',family=family,
                           p=point['p'],seed=point['seed'],start=start,shots=min(plan['chunk_size'],point['draws']-start),
                           point=point['point'],timeout_seconds=plan['timeout_seconds'])


def configuration(case,plan):
    return dict(schema='rmagna.paper-tqd-configuration.v1',size=plan['families'][case['family']]['size'],
                **{key:case[key] for key in ('p','seed','start','shots')})


def input_pins(pinned):
    public=pinned['upstream']['files']
    author=pinned['upstream']['decoder_files']
    files={}
    for size,_ in historical.TQD_SCALES:
        root=f'examples/tqd{size}'
        for name in (*FILES.values(),'layout.json',*DECODER):
            files[f'{root}/{name}']=public[f'{root}/{name}']
        for name in MODULES:
            path=f'{root}/decoder/author_source/{name}'
            files[path]=author[path]
    return files


def historical_pins(pinned):
    names=['data/tqd-comparison.json','raw/tqd/terminal-summary.json']
    for size,_ in historical.TQD_SCALES:
        timing=f'raw/tqd/timing/tqd{size}'
        names.extend([f'{timing}/result.json',*(f'{timing}/draw-batch-{i}.json' for i in range(3))])
        for rate in historical.TQD_RATES:
            folder=f'raw/tqd/terminal/tqd-L{size}-p{rate}'
            names.extend(f'{folder}/{name}' for name in ('manifest.json','result.json'))
    return {name:pinned['paper']['files'][name] for name in names}


def command(case,out,executable,python,config):
    root=out/'inputs'/'examples'/case['family']
    return [executable,root/FILES['physical_sha256'],root/FILES['task_sha256'],config,
            root/FILES['controllers_sha256'],python,out/'inputs'/MAP_NAME]


def author_stage(stage,case,out,python,run_process):
    if case['kind'] == 'timing':
        return
    with TemporaryDirectory(dir=out/'config') as directory:
        physical=Path(directory)/'physical.json'
        atomic(physical,stage['payload'])
        argv=[python,'-E','-s','-B',HERE/'tqd_author_approximation.py','--upstream',out/'inputs',
              '--fault-map',out/'inputs'/MAP_NAME,'--physical-result',physical]
        stage['author']=run_process(list(map(str,argv)),case['timeout_seconds'],out/'config')
    require(stage['author']['status'] == 'complete','TQD author approximation failed; raw results retained')


def validate_stage(stage,case,plan,receipt,pinned):
    require(digest_json(stage['case']) == digest_json(case) and stage['status'] == 'complete'
            and type(stage['returncode']) is int and stage['returncode'] == 0 and valid_seconds(stage['wall_seconds']),
            'failed, missing, or mismatched TQD batch: ' + case['id'])
    raw=stage['payload']
    family=plan['families'][case['family']]
    prefix=f"examples/{case['family']}"
    files=input_pins(pinned)
    require(digest_json(raw['configuration']) == digest_json(configuration(case,plan))
            and raw['rust_source_sha256'] == receipt['rust_source_sha256']
            and raw['executable_sha256'] == receipt['rust_binary_sha256']
            and raw['fault_map_sha256'] == receipt['derived_input_sha256'][MAP_NAME]
            and raw['inputs'] == {key:files[f'{prefix}/{filename}'] for key,filename in FILES.items()}
            and raw['build']['debug_assertions'] is False,
            'TQD parameter, source, derived map, or full target changed')
    n=case['shots']
    require(raw['schema'] == 'rmagna.paper-tqd-physical-run.v1' and raw['status'] == 'complete' and raw['failure'] is None
            and type(raw['completed']) is int and raw['completed'] == len(raw['rows']) == n
            and raw['outputs'] == family['outputs'] and raw['charge_depth'] == family['depth']+1
            and raw['active_sites'] == family['physical_fault_categories'] and raw['branch_target_norm'] == '5/8'
            and raw['rng_protocol'] == RNG_PROTOCOL and raw['excluded_warmup'] == dict(draws=1,index=WARMUP),
            'incomplete TQD draws or changed terminal branch')
    clock=raw['timing']
    require(all(valid_seconds(clock[key]) for key in ('compile_seconds','preparation_seconds','sampling_seconds','decoder_seconds'))
            and 0 < clock['compile_seconds'] <= clock['preparation_seconds'] <= stage['wall_seconds']
            and 0 < clock['decoder_seconds'] < clock['sampling_seconds'] <= stage['wall_seconds']
            and all(type(clock[key]) is int and clock[key] == n*(3*family['depth']+4)
                    for key in ('decoder_attempted','decoder_completed')), 'invalid TQD timing or decoder count')
    decoder=raw['decoder']
    expected_sources={name:files[f'{prefix}/decoder/author_source/{name}'] for name in MODULES}
    modules={name.removeprefix(prefix+'/'):digest for name,digest in files.items()
             if name.startswith(prefix+'/decoder/') and name.endswith('.py')}
    depth=family['depth']
    names={f'jit_{color}_{step}' for color in tqd_fault_map.COLORS for step in range(depth)}
    names.update(['exit', 'charge_b', 'charge_g', 'charge_r'])
    require(decoder['controllers_sha256'] == raw['inputs']['controllers_sha256']
            and decoder['module_sources'] == modules and set(decoder['bindings']) == names
            and sha(raw['decoder_transcript_sha256']), 'TQD decoder sources or binding coverage changed')
    calls=raw['first_decoder_calls']
    require(len(calls) == len(names) and set(call['name'] for call in calls) == names
            and all(call['call'] == index and all(type(bit) is bool for key in ('inputs','outputs') for bit in call[key])
                    for index,call in enumerate(calls)), 'incomplete TQD first decoder transcript')
    resources={(tuple(row['resources']['acceptance']),tuple(row['resources']['target'])) for row in raw['rows']}
    require(resources <= {((5,32),(10,4)),((0,0),(0,0))}
            and ((5,32),(10,4)) in resources, 'TQD Rust phase coordinates or terms differ')
    if case['kind'] == 'timing':
        require('author' not in stage, 'TQD timing includes the author approximation')
        for index,row in enumerate(raw['rows'], start=case['start']):
            require(row['index'] == index and row['faults'] == fault_stream(case['seed'],index,Fraction(case['p']),raw['active_sites']),
                    'TQD timed physical RNG stream changed')
        return None
    author=stage['author']
    require(author['status'] == 'complete' and author['returncode'] == 0 and valid_seconds(author['wall_seconds']),
            'TQD author approximation is incomplete')
    return paired_rows(raw,author['payload'],expected_sources)


def results_from_batches(plan,out,receipt,pinned):
    planned=list(cases(plan))
    require({p.name for p in (out/'batches').iterdir()} == {stage_path(out,case).name for case in planned},
            'TQD batch inventory is incomplete or has extra files')
    read=lambda name:read_json(reference.relative_file(out/'historical',name))
    deposit=historical.terminal_curves(read,plan['historical_kernel'])
    references={f'tqd{size}':historical_networks(read,size) for size,_ in historical.TQD_SCALES}
    historic_timing={family:historical.timing(read,family,historical.TIMING[family]) for family in plan['families']}
    map_=read_json(reference.relative_file(out/'inputs',MAP_NAME))
    precision={}
    timing={}
    hashes={}
    for case in planned:
        path=reference.relative_file(out,'batches/'+stage_path(out,case).name)
        stage=read_json(path)
        selected=validate_stage(stage,case,plan,receipt,pinned)
        family=case['family']
        verify_readout_witness(stage['payload'],map_['families'][family]['entries'])
        hashes[case['id']]=reference.digest(path)
        if selected is not None:
            precision.setdefault((family,case['point']),[]).extend(selected)
        else:
            n=case['shots']
            clock=stage['payload']['timing']
            timing.setdefault(family,[]).append(dict(seed=case['seed'],draws=n,timing=clock,
                sampling_seconds_per_attempt=clock['sampling_seconds']/n,
                decoder_subtracted_seconds_per_attempt=(clock['sampling_seconds']-clock['decoder_seconds'])/n))
    deposited={(f"tqd{point['distance']}",point['p']):point for point in deposit['points']}
    curves={}
    for family, config in plan['families'].items():
        curves[family]=[]
        for point in config['grid']:
            measured=summarize(precision[(family,point['point'])])
            require(measured['draws'] == point['draws'],'TQD paper precision draw count differs')
            old=deposited[family,point['p']]
            estimates={}
            for label in old['estimates']:
                current=measured[label]
                combined=math.hypot(current['standard_error'],old['estimates'][label]['se']) if current['standard_error'] is not None else 0
                estimates[label]=(current['mean']-old['estimates'][label]['mean'])/combined if combined else None
            curves[family].append(dict(p=point['p'],measured=measured,historical=old,pulls=estimates))
    return dict(schema='rmagna.paper-tqd-report.v1',scope=plan['scope'],curves=curves,
                timing={family:dict(trials=trials,historical=historic_timing[family],
                    median_sampling_seconds_per_attempt=median(t['sampling_seconds_per_attempt'] for t in trials),
                    median_decoder_subtracted_seconds_per_attempt=median(t['decoder_subtracted_seconds_per_attempt'] for t in trials))
                    for family,trials in timing.items()},
                declared_reference_networks=references,
                rust_terminal_resources=dict(acceptance=[5,32],target=[10,4]),
                raw_batch_sha256=hashes,completed_batches=len(planned))


def summarize_report(report):
    return dict(status='PASS',scope=report['scope'],completed_batches=report['completed_batches'],
                precision_draws=sum(point['measured']['draws'] for points in report['curves'].values() for point in points),
                timing_batches=sum(len(family['trials']) for family in report['timing'].values()),
                paired_flux_draws=sum(point['measured']['flux_success'] for points in report['curves'].values() for point in points))


def campaign(pinned):
    return decoder_campaign.Campaign(
        schema=CAPTURE,label='TQD',runner_files=RUNNER_FILES,input_pins=input_pins(pinned),
        historical_pins=historical_pins(pinned),cases=cases,configuration=configuration,command=command,
        validate_stage=partial(validate_stage,pinned=pinned),results_from_batches=partial(results_from_batches,pinned=pinned),
        summarize=summarize_report,derived_inputs=lambda root:{MAP_NAME:tqd_fault_map.inspect(root,pinned)},after_worker=author_stage)


def capture(args,plan,run_process):
    pinned=reference.load_manifest()
    validate_manifest(plan,pinned)
    return decoder_campaign.capture(campaign(pinned),args,plan,run_process)


def check_capture(plan,out):
    pinned=reference.load_manifest()
    validate_manifest(plan,pinned)
    return decoder_campaign.check_capture(campaign(pinned),plan,out)
