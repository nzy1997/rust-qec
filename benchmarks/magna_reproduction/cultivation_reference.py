"""Verify deposited Figure 4 and Table II evidence and compare to Rust draws."""
from fractions import Fraction
import hashlib
import math
from pathlib import Path
import zipfile

import reference
from remaining_contracts import read_json, require


FIGURE = 'data/cultivation-two-estimator.json'
TABLE = 'data/cultivation-comparison.json'
PUBLISHED = 'raw/published/cultivation.json'
TABLE_CASES = {'msc3':'color3', 'msc5':'color5', 'rp2-d3':'rp2-d3-T', 'rp2-d5':'rp2-d5-T'}
BACKENDS = {'global':'magna', 'clifft':'clifft', 'symft':'symft', 'ppvm':'ppvm', 'wan':'tsim'}


def summary_path(family, kind, p):
    rate = str(float(Fraction(p)))
    return f'raw/cultivation/{family}/{kind}-p{rate}/summary.json'


def members(plan, pinned):
    files = {FIGURE,TABLE,PUBLISHED}
    for family, info in plan['families'].items():
        for point in info['grid']:
            for kind in ('acceptance','failure'):
                files.add(summary_path(family,kind,point['p']))
    files.update(name for name in pinned['paper']['files']
                 if name.startswith('raw/cultivation-comparators/'))
    return sorted(files)


def capture(archive, out, plan, pinned):
    require(reference.digest(archive) == pinned['paper']['archive_sha256'], 'cultivation paper archive changed')
    out.mkdir(exist_ok=False)
    with zipfile.ZipFile(archive) as source:
        for name in members(plan,pinned):
            data = source.read('magna-data-deposit/'+name)
            require(hashlib.sha256(data).hexdigest() == pinned['paper']['files'][name], 'deposited cultivation member changed: '+name)
            path = out/name
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes(data)


def finite(value, name):
    require(type(value) in (int,float) and 0 <= value < 1e12 and math.isfinite(value), 'invalid deposited '+name)
    return value


def match(a,b,name):
    finite(a,name)
    finite(b,name)
    require(math.isclose(a,b,rel_tol=2e-12,abs_tol=1e-25), 'deposited cultivation '+name+' differs')


def compare_curves(plan, root, rust_report, pinned):
    data = read_json(root/FIGURE)
    provenance = data['_provenance']
    require(provenance['kernel_commit'] == pinned['paper']['kernel_commit'], 'cultivation historical kernel changed')
    points = {}
    for family, info in plan['families'].items():
        points[family] = []
        origin = {Fraction(str(row['p'])):row for row in data[family]['rows']}
        for want, rust in zip(info['grid'],rust_report['precision'][family],strict=True):
            p = Fraction(want['p'])
            old = origin[p]
            acceptance = read_json(root/summary_path(family,'acceptance',str(p)))
            failure = read_json(root/summary_path(family,'failure',str(p)))
            n = info['original_sites']
            for kind, raw in [('acceptance',acceptance),('failure',failure)]:
                require(raw['mode'] == kind and raw.get('status','complete') == 'complete'
                        and raw['rate'] == old['p'] and raw['noise_categories'] == n
                        and raw['seed'] == old[kind+'_seed'] == want[kind+'_seed']
                        and raw['shots'] == old[kind+'_shots'] == want[kind+'_shots'],
                        'deposited cultivation draw design differs')
                match(raw['seconds'],old[kind+'_seconds'],kind+'_seconds')
            proposal = failure['proposal']
            require(acceptance['sampling'] == 'physical' and proposal['extra_faults'] == 3
                    and proposal['closure_mixture'] == 0.95 and proposal['closing_channels'] == info['closing_sites']
                    and proposal['metadata']['proposal_only'] is True
                    and proposal['metadata']['retained_constraints'] == info['safe']
                    and proposal['metadata']['unsafe_T_constraints'] == info['unsafe_t'],
                    'deposited cultivation proposal differs')
            match(proposal['q'],float(p+Fraction(3,n)),'proposal q')
            for key in ('acceptance','acceptance_se'):
                match(acceptance[key],old[key],key)
            for key in ('accepted_failure','accepted_failure_se'):
                match(failure[key],old[key],key)
            match(old['conditional_error'],old['accepted_failure']/old['acceptance'],'conditional error')
            ratio_var = (old['accepted_failure_se']/old['acceptance'])**2 + (
                old['conditional_error']*old['acceptance_se']/old['acceptance'])**2
            match(old['conditional_error_se'],math.sqrt(ratio_var),'independent conditional error SE')
            historical_input = provenance['input_hashes'][family]
            rust_input = rust_report['inputs'][family]
            a_pull = (rust['acceptance']-old['acceptance'])/math.sqrt(
                rust['acceptance_variance']+old['acceptance_se']**2)
            error_pull = None
            if rust['precision_resolved']:
                error_pull = (rust['conditional_error']-old['conditional_error'])/math.sqrt(
                    rust['conditional_error_variance']+old['conditional_error_se']**2)
            points[family].append(dict(p=str(p), historical_acceptance=old['acceptance'],
                historical_conditional_error=old['conditional_error'], historical_conditional_error_se=old['conditional_error_se'],
                historical_acceptance_seconds=old['acceptance_seconds'], historical_failure_seconds=old['failure_seconds'],
                acceptance_pull=a_pull, conditional_error_pull=error_pull,
                physical_hash_matches=historical_input['physical_sha256']==rust_input['physical_sha256'],
                task_hash_matches=historical_input['task_sha256']==rust_input['task_sha256'],
                historical_input=historical_input))
    return points


def compare_timings(root, pinned):
    rows = read_json(root/TABLE)['rows']
    expected = {(family, backend) for family in TABLE_CASES.values() for backend in BACKENDS.values()}
    results = {}
    for row in rows:
        family, backend = TABLE_CASES[row['case']], BACKENDS[row['mode']]
        key = family, backend
        require(key in expected and key not in results, 'invalid deposited cultivation timing matrix')
        completed = row['status'] == 'completed'
        require(row['status'] in ('completed','failed') and row['timed_batches'] == 3
                and row['shots_per_batch'] == (128 if key == ('rp2-d5-T','ppvm') else 1024),
                'deposited timing batch scope changed')
        if completed:
            require(finite(row['sampling_ms_per_trial'],'sampling_ms_per_trial') > 0, 'missing deposited throughput')
        else:
            require(backend == 'tsim' and row['sampling_ms_per_trial'] is None and row['failure_reason'],
                    'deposited failure lost its reason')
        if backend == 'magna':
            require(row['kernel_commit'] == pinned['paper']['kernel_commit'], 'deposited Magna timing kernel changed')
        if backend == 'clifft':
            require(row['version']['clifft'] == '0.11.0', 'deposited Clifft version changed')
        if backend == 'ppvm':
            require(row['upstream_commit'].startswith('7b2e1bcb') and row['threads'] == 1
                    and row['coefficient_threshold'] == 1e-10 and row['records_match'] is True,
                    'deposited PPVM revision, single-thread or coefficient contract changed')
        results[key] = row
    require(set(results) == expected, 'deposited timing matrix has a dropped timeout or unsupported cell')
    return [dict(family=family,backend=backend,raw=results[(family,backend)]) for family,backend in sorted(results)]


def compare(plan, root, rust_report, pinned):
    files = {name:pinned['paper']['files'][name] for name in members(plan,pinned)}
    reference.verify_files(root,files)
    return dict(schema='rmagna.cultivation-historical-comparison.v1', paper=plan['paper'],
                archive_sha256=plan['archive_sha256'], kernel=plan['historical_kernel'],
                input_relation='deposited historical input hashes and pinned public inputs are reported separately',
                timing_relation='deposited Mac Studio timings preserve each backend input and warmup; no cross-host speedup',
                precision=compare_curves(plan,root,rust_report,pinned), timings=compare_timings(root,pinned),
                published_reference=read_json(root/PUBLISHED), input_files=files)
