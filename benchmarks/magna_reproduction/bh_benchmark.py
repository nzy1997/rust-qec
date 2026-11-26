"""Paper BH campaigns: literal inputs, independent exact equations, raw totals."""
from fractions import Fraction
import hashlib
import json
import math
import platform
from pathlib import Path
import re
import statistics
import subprocess
import sys

import bh_analytic
import remaining_contracts as contracts
from paper_probe_identity import source_identity, source_inputs

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
reference = contracts.reference
require = contracts.require
read_json = contracts.read_json
KS = [2, 4, 6, 10, 20, 40, 60, 80, 100]
CONTRACT = 'integrated_joint_acceptance_error_masses'
RUNNER_FILES = ('bh_benchmark.py', 'bh_analytic.py', 'paper_benchmark.py',
                'paper_probe_identity.py', 'reference.py', 'remaining_contracts.py',
                'coverage_ledger.py', 'coverage_rust.py')


def json_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def expected_manifest(pinned):
    return dict(schema='rmagna.paper-bh-plan.v1',
        reference_sha256=reference.digest(HERE/'reference.json'),
        upstream_commit=pinned['upstream']['commit'], paper='2610.22315v1',
        archive_sha256=pinned['paper']['archive_sha256'],
        historical_kernel=pinned['paper']['kernel_commit'], output_contract=CONTRACT,
        max_signature_work=17179869184, timeout_seconds=600,
        timing=dict(k=list(KS), variants=['T', 'S'], p='1/1000', shots=2048,
                    repetitions=3, overrides={'40':10, '60':10},
                    seed_base=2026100100, excluded_warmup_draws=1),
        precision=dict(k=[2, 6, 10], p=['1/10000', '3/10000', '1/1000'],
                       shots=65536, seed_base=2026101000, excluded_warmup_draws=1),
        timing_pairing='T/S share each k/repetition seed; paired timing observations are never pooled for precision',
        proposal='q=2/(3*k+8); exact common rate scale q/p',
        uncertainty='IID physical draws; weighted joint means and paired delta covariance; pointwise 5-SE correctness gate',
        serial_execution='one process/thread at a time; no Native campaign rerun')


def validate_manifest(manifest, pinned):
    require(json_hash(manifest) == json_hash(expected_manifest(pinned)), 'BH paper design/pins/sample sizes changed')


def cases(manifest):
    result = []
    timing = manifest['timing']
    for k in timing['k']:
        for variant in timing['variants']:
            for repetition in range(timing['overrides'].get(str(k), timing['repetitions'])):
                result.append(dict(id=f'timing-bh{k}-{variant}-r{repetition:02}',
                    campaign='timing', k=k, variant=variant, p=timing['p'],
                    shots=timing['shots'], seed=timing['seed_base']+100*k+repetition))
    precision = manifest['precision']
    for k in precision['k']:
        for index, p in enumerate(precision['p']):
            result.append(dict(id=f'precision-bh{k}-T-p{index}', campaign='precision',
                k=k, variant='T', p=p, shots=precision['shots'],
                seed=precision['seed_base']+100*k+index))
    return result


def configuration(case, manifest):
    q = Fraction(2, 3*case['k']+8)
    return dict(schema='rmagna.paper-bh-configuration.v1', outputs=case['k'],
        shots=case['shots'], seed=case['seed'], proposal_scale=str(q/Fraction(case['p'])),
        max_signature_work=manifest['max_signature_work'])


def rational_field(value):
    require(isinstance(value, list) and len(value) == 4, 'invalid BH exact field')
    coefficients = []
    for pair in value:
        require(isinstance(pair, list) and len(pair) == 2
                and all(isinstance(s, str) and re.fullmatch(r'-?[0-9]{1,4096}', s) for s in pair),
                'invalid BH exact rational coefficient')
        require(int(pair[1]) > 0, 'BH exact denominator must be positive')
        coefficients.append(Fraction(int(pair[0]), int(pair[1])))
    require(coefficients[1:] == [0, 0, 0], 'BH joint contributions must be rational')
    return coefficients[0]


def regenerate_statistics(checkpoint):
    """Use retained exact sums/products, without executing any simulator."""
    require(checkpoint['schema_version'] == 1 and checkpoint['status'] == 'draws_complete',
            'incomplete BH stream checkpoint')
    state = contracts.ledger_tools.unique_object
    raw = json.loads(checkpoint['statistics'], object_pairs_hook=state,
                     parse_constant=contracts.ledger_tools.invalid_constant)
    require(raw['schema_version'] == 1 and raw['kind'] == 'exact_statistics',
            'unknown BH exact statistics checkpoint')
    require(isinstance(raw['count'], str) and raw['count'].isdigit(), 'invalid BH draw count')
    n = int(raw['count'])
    require(n >= 2 and str(n) == checkpoint['committed'] == checkpoint['requested'],
            'BH exact draw counts differ')
    require(len(raw['sums']) == 2 and len(raw['products']) == 3, 'BH exact quantities changed')
    a, e = map(rational_field, raw['sums'])
    aa, ae, ee = map(rational_field, raw['products'])
    w, ww = rational_field(raw['weight']), rational_field(raw['weight_squared'])
    require(a > 0 and 0 <= e <= a and w > 0 and ww > 0 and w*w <= n*ww,
            'BH complete joint totals are invalid or unresolved')
    covariance = [(aa-a*a/n)/(n*(n-1)), (ae-a*e/n)/(n*(n-1)), (ee-e*e/n)/(n*(n-1))]
    require(covariance[0] >= 0 and covariance[2] >= 0, 'negative BH exact variance')
    require(covariance[1]**2 <= covariance[0]*covariance[2], 'BH covariance matrix is not positive semidefinite')
    ratio = e/a
    ratio_variance = (covariance[2]+ratio*ratio*covariance[0]-2*ratio*covariance[1])/(a/n)**2
    require(ratio_variance >= 0, 'negative BH paired ratio variance')
    return dict(draws=n, acceptance=a/n, error=e/n, conditional_error=ratio,
                acceptance_variance=covariance[0], error_variance=covariance[2],
                conditional_error_variance=ratio_variance, mean_weight=w/n, ess=w*w/ww)


def close(actual, expected, label):
    require(type(actual) in (int, float) and math.isfinite(actual)
            and math.isclose(actual, float(expected), rel_tol=2e-11, abs_tol=1e-20),
            f'BH regenerated {label} differs from exported metrics')


def validate_inventory(inventory, shots, *, preparation_assignments=0, skipped_terminal_queries=0):
    require(set(inventory) == {'after_preparation', 'after_warmup', 'after_sampling'},
            'missing BH inventory stage')
    first = inventory['after_preparation']
    for name, assignments in [('after_preparation', preparation_assignments),
                              ('after_warmup', preparation_assignments+1),
                              ('after_sampling', preparation_assignments+1+shots)]:
        stage = inventory[name]
        require(type(stage['source_assignments']) is int and stage['source_assignments'] == assignments,
                'BH source assignment duplicated or warmup pooled')
        require(stage['storage'] == first['storage'], 'BH retained program/geometry inventory changed')
        for owner in ['source', 'target']:
            baseline, actual = first['work'][owner], stage['work'][owner]
            require(isinstance(actual, dict) and actual['saturated'] is False, 'missing/saturated BH factory counts')
            require(all(type(v) is int and v >= 0 for k,v in actual.items() if k != 'saturated'),
                    'invalid BH factory counter type')
            for key, value in baseline.items():
                if key not in {'closure_evaluations', 'fixed_evaluations'}:
                    require(actual[key] == value, 'BH per-draw construction detected: '+key)
            expected = 2*assignments if owner == 'source' else 0
            if owner == 'source' and name == 'after_sampling':
                expected -= skipped_terminal_queries
            require(actual['closure_evaluations'] == actual['fixed_evaluations'] == expected,
                    'BH terminal family evaluation count changed')


def input_files(case):
    p = Fraction(case['p'])
    directory = f"inputs/bh{case['k']}-{case['variant']}-p{p.numerator}_{p.denominator}"
    return {name:f'{directory}/{name}' for name in ['physical.magna', 'task.json', 'ideal.magna']}


def validate_inputs(case, results, pinned):
    files = input_files(case)
    contents = {name:reference.relative_file(results,path).read_bytes() for name,path in files.items()}
    source_prefix = f"examples/bh{case['k']}-{case['variant']}/"
    for name in ['task.json', 'ideal.magna']:
        require(hashlib.sha256(contents[name]).hexdigest() == pinned['upstream']['files'][source_prefix+name],
                'BH complete task/target input changed: '+name)
    text = contents['physical.magna'].decode()
    original, count = re.subn(r'Z_ERROR\('+re.escape(case['p'])+r'\)', 'Z_ERROR(0.001)', text)
    require(count == 3*case['k']+8, 'BH physical noise rate/site count changed')
    require(hashlib.sha256(original.encode()).hexdigest() == pinned['upstream']['files'][source_prefix+'physical.magna'],
            'BH literal T/S circuit changed or proxy substituted')
    return contents, original


def validate_cell(case, stage, results, manifest, receipt, pinned):
    require(json_hash(stage['case']) == json_hash(case) and stage['status'] == 'complete'
            and type(stage['returncode']) is int and stage['returncode'] == 0,
            'BH failed/missing cell cannot be dropped or promoted')
    payload = stage['payload']
    require(payload['schema'] == 'rmagna.paper-bh-run.v1' and payload['status'] == 'complete'
            and payload['output_contract'] == CONTRACT, 'mixed or failed BH output contract')
    require(json_hash(payload['configuration']) == json_hash(configuration(case, manifest)),
            'BH seed/law/budget/sample configuration changed')
    require(payload['rust_source_sha256'] == receipt['rust_source_sha256']
            and payload['executable_sha256'] == receipt['rust_binary_sha256'], 'mixed BH source/executable identity')
    require(payload['build']['debug_assertions'] is False, 'BH timings require the optimized production build')
    contents, original = validate_inputs(case, results, pinned)
    require(payload['inputs']['physical_sha256'] == hashlib.sha256(contents['physical.magna']).hexdigest()
            and payload['inputs']['task_sha256'] == hashlib.sha256(contents['task.json']).hexdigest(),
            'BH executed input identities differ')
    task = json.loads(contents['task.json'])
    require(task['outputs'] == list(range(3*case['k']+8,4*case['k']+8)), 'BH joint output mapping changed')
    require(payload['widths']['physical'] == 7*case['k']+16, 'BH physical width is not the original circuit width')
    require(type(payload['widths']['compiled']) is int and payload['widths']['compiled'] >= payload['widths']['physical'],
            'BH compiled width missing')
    require(payload['policy']['max_quotient_rank'] == 0 and payload['policy']['max_terms'] == 1
            and payload['policy']['compiler'] == 'SharedCommutingLayer', 'BH rank/compiler policy changed')
    require(payload['excluded_warmup']['draws'] == 1 and payload['excluded_warmup']['seed'] == 0
            and payload['excluded_warmup']['metrics']['completed'] == 1, 'missing/pooled BH excluded warmup')
    for family in ['acceptance', 'target']:
        require(all(type(v) is int for v in payload['resources'][family].values())
                and payload['resources'][family] == dict(queries=1, resources=1, maximum_quotient_rank=0, maximum_terms=1),
                'BH reference rank-one contract changed: '+family)
    validate_inventory(payload['inventory'],case['shots'])
    metrics, checkpoint = payload['metrics'], payload['checkpoint']
    require(all(type(metrics[k]) is int and metrics[k] >= 0
                for k in ['requested','completed','failed','not_attempted','seed','random_state']),
            'invalid BH execution counter type')
    for key in ['circuit','task','ideal_state','recovery','controllers','options','sampling']:
        require(checkpoint['identity'][key] == payload['inputs']['fingerprints'][key],
                'BH checkpoint content identity changed: '+key)
    require(checkpoint['identity']['implementation'] == receipt['rust_binary_sha256'],
            'BH checkpoint implementation identity changed')
    require(metrics['status'] == 'complete' and metrics['estimation_status'] == 'final'
            and metrics['requested'] == metrics['completed'] == case['shots']
            and metrics['failed'] == metrics['not_attempted'] == 0 and metrics['failure'] is None
            and metrics['seed'] == case['seed'], 'BH sampling/diagnostic run is incomplete')
    require(checkpoint['failed'] == checkpoint['not_attempted'] == '0'
            and checkpoint['failure'] is None and checkpoint['records'] is None
            and int(checkpoint['original_seed']) == case['seed']
            and str(metrics['random_state']) == checkpoint['random_state'], 'BH checkpoint/RNG/cursors changed')
    values = regenerate_statistics(checkpoint)
    require(values['draws'] == case['shots'], 'BH exact sufficient-statistic count changed')
    for key in ['acceptance', 'error', 'conditional_error']:
        close(metrics['estimates'][key],values[key],key)
        close(metrics['standard_errors'][key],math.sqrt(float(values[key+'_variance'])),key+' standard error')
    close(metrics['weights']['mean'],values['mean_weight'],'mean weight')
    close(metrics['weights']['ess'],values['ess'],'ESS')
    times = metrics['timings']
    require(all(type(times[k]) in (int,float) and math.isfinite(times[k]) and times[k] >= 0
                for k in ['sampling_seconds','total_seconds']), 'invalid BH sampling time')
    require(times['sampling_seconds'] <= times['total_seconds'], 'BH sampling time exceeds total')
    prepare = payload['preparation']
    require(0 <= prepare['compile_seconds'] <= prepare['seconds'], 'BH preparation time changed')
    reference_value = bh_analytic.exact(case['k'], original, case['p'])
    pulls = {}
    for key in ['acceptance', 'conditional_error']:
        sigma = metrics['standard_errors'][key]
        delta = float(values[key]-reference_value[key])
        pulls[key] = delta/sigma if sigma else None
        if case['campaign'] == 'precision':
            require(sigma > 0 and abs(delta) <= 5*sigma+1e-15, 'BH precision point disagrees with exact joint equations')
    return dict(case=case, estimates={k:float(values[k]) for k in ['acceptance','error','conditional_error']},
        standard_errors=metrics['standard_errors'], exact={k:str(v) for k,v in reference_value.items()},
        pulls=pulls, widths=payload['widths'], resources=payload['resources'],
        sufficient_statistics_sha256=hashlib.sha256(checkpoint['statistics'].encode()).hexdigest(),
        preparation_seconds=prepare['seconds'], sampling_seconds=times['sampling_seconds'],
        sampling_seconds_per_draw=times['sampling_seconds']/case['shots'])


def check_capture(manifest, results, pinned=None):
    pinned = reference.load_manifest() if pinned is None else pinned
    validate_manifest(manifest,pinned)
    results = results.resolve()
    receipt = read_json(reference.relative_file(results,'receipt.json'))
    require(receipt['schema'] == 'rmagna.paper-bh-capture.v1' and receipt['status'] == 'recorded'
            and receipt['manifest_sha256'] == json_hash(manifest), 'BH capture is incomplete or design changed')
    expected = cases(manifest)
    require([r['id'] for r in receipt['cells']] == [r['id'] for r in expected], 'BH complete timing/precision cell inventory differs')
    require(receipt['before'] == receipt['after'] and receipt['before']['source_files'] == 475,
            'BH pinned source verification changed')
    source_digest = hashlib.sha256()
    for name, digest in sorted(receipt['rust_source_inputs'].items()):
        require(re.fullmatch(r'[0-9a-f]{64}',digest) is not None, 'invalid BH source-input digest')
        source_digest.update(name.encode()+b'\0'+digest.encode()+b'\n')
    require(source_digest.hexdigest() == receipt['rust_source_sha256'], 'BH compiled source manifest digest changed')
    points = []
    for case, row in zip(expected, receipt['cells']):
        require(row['file'] == f"cells/{case['id']}.json", 'BH cell path changed')
        path = reference.relative_file(results,row['file'])
        require(reference.digest(path) == row['sha256'], 'BH raw cell checksum mismatch')
        points.append(validate_cell(case,read_json(path),results,manifest,receipt,pinned))
    timing = []
    for k in KS:
        for repetition in range(manifest['timing']['overrides'].get(str(k),manifest['timing']['repetitions'])):
            paired=[p for p in points if p['case']['campaign']=='timing' and p['case']['k']==k
                    and p['case']['id'].endswith(f'r{repetition:02}')]
            require(len(paired)==2 and paired[0]['sufficient_statistics_sha256']==paired[1]['sufficient_statistics_sha256'],
                    'BH matched T/S exact joint observations differ')
        for variant in ['T','S']:
            rows = [p for p in points if p['case']['campaign'] == 'timing' and p['case']['k'] == k and p['case']['variant'] == variant]
            sampling = [p['sampling_seconds_per_draw'] for p in rows]
            preparation = [p['preparation_seconds'] for p in rows]
            timing.append(dict(k=k,variant=variant,repetitions=len(rows),
                sampling_seconds_median=statistics.median(sampling),sampling_seconds_range=[min(sampling),max(sampling)],
                preparation_seconds_median=statistics.median(preparation),preparation_seconds_range=[min(preparation),max(preparation)],
                physical_width=rows[0]['widths']['physical'],compiled_width=rows[0]['widths']['compiled']))
    return dict(status='PASS',scope='independent_Rust_BH_paper_campaign_not_full_Magna_parity',
                timing_batches=82,timing_pairs=41,precision_points=9,
                production_draw_evaluations=sum(c['shots'] for c in expected),
                independent_precision_draws=sum(c['shots'] for c in expected if c['campaign']=='precision'),
                timing=timing,precision=[p for p in points if p['case']['campaign']=='precision'],
                provenance='Each raw cell and exact checkpoint is hash-bound; no simulation during validation')


def prepare_inputs(case, out, upstream, pinned):
    files = input_files(case)
    source = upstream/f"examples/bh{case['k']}-{case['variant']}"
    for name, relative in files.items():
        contents = (source/name).read_bytes()
        if name == 'physical.magna':
            text, count = re.subn(r'Z_ERROR\(0\.001\)',f"Z_ERROR({case['p']})",contents.decode())
            require(count == 3*case['k']+8, 'BH original input noise count differs')
            contents = text.encode()
        path = out/relative
        path.parent.mkdir(parents=True,exist_ok=True)
        if path.exists():
            require(path.read_bytes() == contents, 'BH input collision or changed resume source')
        else:
            path.write_bytes(contents)
    validate_inputs(case,out,pinned)
    return files


def capture(args,manifest):
    from paper_benchmark import run_process, save
    pinned = reference.load_manifest()
    validate_manifest(manifest,pinned)
    upstream, binary, out = args.upstream.resolve(), args.rust_example.resolve(strict=True), args.out.resolve()
    before = reference.verify_source(upstream,pinned)
    digest, build_inputs = source_identity(ROOT), source_inputs(ROOT)
    binary_digest = reference.digest(binary)
    if out.exists():
        require(args.resume, 'BH output directory already exists; use --resume to validate and reuse recorded cells')
        receipt = read_json(reference.relative_file(out,'receipt.json'))
        require(receipt['schema'] == 'rmagna.paper-bh-capture.v1'
                and receipt['status'] in {'running','recorded'}, 'invalid BH resume receipt')
        require(receipt['manifest_sha256'] == json_hash(manifest)
                and receipt['rust_binary_sha256'] == binary_digest and receipt['rust_source_sha256'] == digest,
                'BH resume source/executable/design changed; preserve the old campaign')
        require(receipt['runner_files'] == {name:reference.digest(HERE/name) for name in RUNNER_FILES},
                'BH resume runner revision changed; preserve its provenance')
    else:
        require(not args.resume, 'BH resume directory does not exist')
        out.mkdir(parents=True)
        cpu=platform.processor()
        if sys.platform=='darwin':
            cpu=subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip()
        receipt = dict(schema='rmagna.paper-bh-capture.v1',status='running',manifest_sha256=json_hash(manifest),
            before=before,rust_binary_sha256=binary_digest,rust_source_sha256=digest,rust_source_inputs=build_inputs,
            runner_files={name:reference.digest(HERE/name) for name in RUNNER_FILES},
            source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            source_dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT)),
            host=dict(platform=platform.platform(),machine=platform.machine(),cpu=cpu,python=sys.version),
            rustc=subprocess.check_output(['rustc','--version'],text=True).strip(),
            cargo=subprocess.check_output(['cargo','--version'],text=True).strip(),
            serial_processes=1,cells=[])
        save(out/'receipt.json',receipt)
    (out/'cells').mkdir(exist_ok=True)
    (out/'configurations').mkdir(exist_ok=True)
    expected = cases(manifest)
    require([r['id'] for r in receipt['cells']] == [c['id'] for c in expected[:len(receipt['cells'])]],
            'BH resume inventory is not an ordered prefix')
    for case in expected:
        prior = next((r for r in receipt['cells'] if r['id']==case['id']),None)
        if prior:
            path = reference.relative_file(out,prior['file'])
            require(reference.digest(path) == prior['sha256'], 'BH resume raw cell checksum mismatch')
            validate_cell(case,read_json(path),out,manifest,receipt,pinned)
            continue
        files = prepare_inputs(case,out,upstream,pinned)
        config = out/'configurations'/f"{case['id']}.json"
        save(config,configuration(case,manifest))
        command = [str(binary),str(out/files['physical.magna']),str(out/files['task.json']),str(config)]
        stage = run_process(command,manifest['timeout_seconds'],out)
        stage['case'] = case
        path = out/'cells'/f"{case['id']}.json"
        require(not path.exists(), 'BH unindexed cell exists; inspect it before resuming')
        save(path,stage)
        receipt['cells'].append(dict(id=case['id'],file=path.relative_to(out).as_posix(),sha256=reference.digest(path)))
        save(out/'receipt.json',receipt)
        validate_cell(case,stage,out,manifest,receipt,pinned)
        print(f"{case['id']}: complete ({stage['wall_seconds']:.3f}s)",flush=True)
    receipt['after'] = reference.verify_source(upstream,pinned)
    require(reference.digest(binary) == binary_digest and source_identity(ROOT) == digest,
            'BH source/executable changed during campaign')
    receipt['status'] = 'recorded'
    save(out/'receipt.json',receipt)
    result = check_capture(manifest,out,pinned)
    save(out/'regenerated.json',result)
    return result
