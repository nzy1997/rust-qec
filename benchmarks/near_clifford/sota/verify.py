"""Fail closed on incomplete, inconsistent or stale near-Clifford evidence."""
import argparse
from datetime import datetime
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import subprocess
from evidence import expand, parity_counts
from lowering import records_only, lower, physical_width
from run import BACKENDS, BATCHES, HERE, ROOT, harness_inventory, masks, report, sha


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def digest(value):
    return isinstance(value, str) and re.fullmatch('[a-f0-9]{64}', value) is not None


def close(a, b):
    return type(a) in (int, float) and math.isfinite(a) and math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12)


def timing(raw, backend, shots, width, repetitions, batch=None, tuning=False):
    require(raw['backend'] == backend and raw['shots'] == shots and raw['width'] == width, 'timing identity')
    require(integer(raw['compile_ns'], 1), 'compile duration')
    if backend == 'rstim':
        require(raw['rng'] == 'SmallRng/rand-0.8.7' and integer(raw['prepare_ns'], 1), 'rstim configuration')
    else:
        require(raw['version'] == ('0.11.0' if backend.startswith('clifft') else '0.1.1'), 'peer version')
        require(raw['threads'] == 1 and raw['batch'] == batch, 'peer configuration')
        require(integer(raw['peak_active_width']), 'planned width')
        if backend == 'symft':
            require(isinstance(raw['simd_backend'], str) and bool(raw['simd_backend']), 'SIMD metadata')
    for key in ['warm_ns', 'warm_totals_ns', 'warm_calls']:
        require(len(raw[key]) == repetitions, 'warm observation cardinality')
    for ns, total, calls in zip(raw['warm_ns'], raw['warm_totals_ns'], raw['warm_calls']):
        require(integer(total, 50_000_000) and integer(calls, 1) and calls < 1_000_000, 'timing interval')
        require(close(ns, total/calls), 'normalized observation')
    require(len(raw['first_ns']) == (0 if tuning else repetitions), 'first-call cardinality')
    require(all(integer(ns, 1) for ns in raw['first_ns']), 'first-call duration')


def transcript(encoded, backend, shots, width, call_shots, expected_hash, counts, selected, batch='auto'):
    payload=expand(encoded)
    require(payload['shots']==shots and payload['width']==width and payload['call_shots']==call_shots, 'transcript dimensions')
    if backend!='rstim':
        require(payload['backend']==backend and payload['batch']==batch and payload['threads']==1 and
            payload['version']==('0.11.0' if backend.startswith('clifft') else '0.1.1'), 'transcript configuration')
    require(hashlib.sha256((json.dumps(payload)+'\n').encode()).hexdigest()==expected_hash, 'transcript content digest')
    require(parity_counts(payload,selected)==counts, 'counts differ from transcript')


def validate(result, allow_subset=False, git_sources=False, artifacts=None):
    manifest = json.loads((HERE/'manifest.json').read_text())
    require(result['schema'] == 'rstim.near-clifford-sota-results.v1', 'schema')
    require(type(result['subset']) is bool and (allow_subset or not result['subset']), 'subset is not publication evidence')
    require(datetime.fromisoformat(result['finished']) > datetime.fromisoformat(result['started']), 'unfinished campaign')
    for key in ['pairs', 'repetitions', 'validation_shots']:
        require(integer(result[key], 1), 'positive protocol counts')
    if not result['subset']:
        require(result['pairs'] >= 3 and result['repetitions'] >= 7 and result['validation_shots'] >= 8192,
            'publication sampling floor')
    require(result['manifest_sha256'] == sha(HERE/'manifest.json'), 'manifest identity')
    require(result['symft_source_revision'] == 'c89b98514a919240b8afa53a271e08d926d3c987', 'SymFT revision')
    require(digest(result['binary_sha256']), 'binary digest')
    require(result['harness'] == harness_inventory(), 'harness/fixture inventory')
    for environment, distributions in [(result['packages'], ['symft','numpy']),
            (result['packages']['clifft_environment'], ['clifft','numpy'])]:
        for name in distributions:
            package = environment[name]
            require(package['version'] == manifest['baseline_versions'][name], 'package version')
            require(bool(package['files']) and all(digest(d) for d in package['files'].values()), 'package inventory')
    require(bool(result['symft_source_files']) and all(digest(d) for d in result['symft_source_files'].values()), 'SymFT sources')
    if git_sources:
        revision = result['source_revision']
        require(re.fullmatch('[a-f0-9]{40}', revision) is not None, 'source revision')
        tracked = subprocess.check_output(['git','ls-tree','-r','--name-only',revision], cwd=ROOT, text=True).splitlines()
        names = {name for name in tracked if name.startswith('rstim/src/') and name.endswith('.rs')}
        names.update(['Cargo.toml','Cargo.lock','rstim/Cargo.toml'])
        require(set(result['source_files']) == names, 'production inventory cardinality')
        for name, expected in result['source_files'].items():
            content = subprocess.check_output(['git','show',f'{revision}:{name}'], cwd=ROOT)
            require(hashlib.sha256(content).hexdigest() == expected, 'production source hash')
    else:
        require(bool(result['source_files']) and all(digest(d) for d in result['source_files'].values()), 'source inventory')
    cases = {c['id']:c for c in manifest['cases']}
    validations = result['validation']
    ids = [v['id'] for v in validations]
    require(len(set(ids)) == len(ids) and set(ids) <= cases.keys() and bool(ids), 'validation identities')
    if not result['subset']:
        require(ids == list(cases), 'missing or reordered fixture')
    n = result['validation_shots']
    by_id = {}
    for validation in validations:
        case = cases[validation['id']]
        fixture=HERE/case['file']
        require(sha(fixture)==case['sha256'], 'frozen fixture digest')
        original=fixture.read_text()
        native=records_only(original)
        physical=physical_width(original)
        require(validation['physical_width']==physical, 'physical width')
        adapted=lower(native,physical)
        require(hashlib.sha256(native.encode()).hexdigest()==validation['native_sha256'] and
            hashlib.sha256(adapted.encode()).hexdigest()==validation['adapted_sha256'], 'timed circuit lowering identity')
        require(validation['passed'] is True and 'error' not in validation, 'validation failure')
        width = validation['width']; require(integer(width, 1), 'record width')
        selected = masks((HERE/case['file']).read_text(), width)
        require(validation['masks'] == [list(m) for m in selected], 'validation masks')
        require(set(validation['parity_counts']) == set(BACKENDS), 'validation backends')
        for counts in validation['parity_counts'].values():
            require(len(counts) == len(selected) and all(integer(c) and c <= n for c in counts), 'parity counts')
        threshold = 2*math.sqrt(math.log(4*24*len(selected)*len(ids)/0.001)/(2*n))
        require(close(validation['threshold'], threshold), 'validation threshold')
        difference = max(abs(validation['parity_counts'][a][i]-validation['parity_counts'][b][i])/n
            for i in range(len(selected)) for a in BACKENDS for b in BACKENDS)
        require(close(validation['max_difference'], difference) and difference <= threshold, 'parity agreement')
        require(digest(validation['native_sha256']) and digest(validation['adapted_sha256']), 'input digests')
        expected_payloads = {case['id']+'-'+b+'-validation.json' for b in BACKENDS}
        require(set(validation['payload_sha256']) == expected_payloads and
            all(digest(d) for d in validation['payload_sha256'].values()), 'validation payload inventory')
        require(set(validation['transcripts'])==set(BACKENDS), 'default transcript inventory')
        for backend in BACKENDS:
            transcript(validation['transcripts'][backend],backend,n,width,n,
                validation['payload_sha256'][case['id']+'-'+backend+'-validation.json'],
                validation['parity_counts'][backend],selected)
        by_id[case['id']] = validation
        if artifacts:
            for name, expected in validation['payload_sha256'].items():
                require(sha(artifacts/name) == expected, 'validation payload hash')
            require(sha(artifacts/(case['id']+'-native.stim')) == validation['native_sha256'], 'native input hash')
            require(sha(artifacts/(case['id']+'-rstim.stim')) == validation['adapted_sha256'], 'adapted input hash')
    expected_keys = [(c['id'], shots) for c in manifest['cases'] for shots in c['shots']]
    keys = [(c['id'], c['shots']) for c in result['cases']]
    require(len(set(keys)) == len(keys) and bool(keys) and all(k in expected_keys for k in keys), 'timed identities')
    require(keys == (expected_keys if not result['subset'] else [k for k in expected_keys if k in keys]), 'missing/reordered timed case')
    require({k[0] for k in keys} == set(ids), 'validated/timed fixture mismatch')
    require([(t['id'], t['shots']) for t in result['tuning']] == keys, 'tuning identities')
    frozen = (json.dumps(result['tuning'], indent=2)+'\n').encode()
    require(hashlib.sha256(frozen).hexdigest() == result['frozen_tuning_sha256'], 'frozen tuning hash')
    if artifacts:
        require((artifacts/'frozen-tuning.json').read_bytes() == frozen, 'frozen tuning artifact')
    for index, (case, tuning) in enumerate(zip(result['cases'], result['tuning'])):
        require('error' not in case, 'timing failure')
        validation = by_id[case['id']]; width = validation['width']
        require(set(tuning['backends']) == set(BACKENDS[1:]), 'tuning backends')
        reference=tuning['rstim_parity_counts']
        require(len(reference)==len(validation['masks']) and all(integer(c) and c<=n for c in reference), 'selected rstim counts')
        expected_payloads={f"{case['id']}-{case['shots']}-{b}-selected.json" for b in BACKENDS}
        require(set(tuning['selected_payload_sha256'])==expected_payloads and
            all(digest(d) for d in tuning['selected_payload_sha256'].values()), 'selected payload inventory')
        require(set(tuning['transcripts'])==set(BACKENDS), 'selected transcript inventory')
        if artifacts:
            for name, expected in tuning['selected_payload_sha256'].items():
                require(sha(artifacts/name)==expected, 'selected payload hash')
                payload=json.loads((artifacts/name).read_text())
                require(payload['shots']==n and payload['call_shots']==case['shots'] and payload['width']==width,
                    'selected invocation shape')
        for backend, choice in tuning['backends'].items():
            require([t['batch'] for t in choice['trials']] == BATCHES, 'tuning grid')
            valid = []
            for trial in choice['trials']:
                if 'error' in trial:
                    require(isinstance(trial['error'], str) and bool(trial['error']), 'invalid tuning failure')
                    continue
                timing(trial['raw'], backend, case['shots'], width, 3, trial['batch'], True)
                median = statistics.median(trial['raw']['warm_ns'])
                require(close(trial['median_ns'], median), 'tuning median')
                valid.append(trial)
            require(bool(valid) and choice['selected_batch'] == min(valid,key=lambda t:t['median_ns'])['batch'], 'tuning decision')
            chosen = choice['selected_parity_counts']
            require(len(chosen) == len(validation['masks']) and all(integer(c) and c <= n for c in chosen), 'selected parity counts')
            difference = max(abs(a-b)/n for a,b in zip(chosen,reference))
            require(close(choice['selected_max_difference'], difference) and difference <= validation['threshold'], 'selected batch correctness')
        counts=[reference]+[tuning['backends'][b]['selected_parity_counts'] for b in BACKENDS[1:]]
        require(max(abs(a[i]-b[i])/n for a in counts for b in counts for i in range(len(reference)))
            <= validation['threshold'], 'all selected pairs agree')
        for backend, chosen_counts in zip(BACKENDS,counts):
            transcript(tuning['transcripts'][backend],backend,n,width,case['shots'],
                tuning['selected_payload_sha256'][f"{case['id']}-{case['shots']}-{backend}-selected.json"],
                chosen_counts,validation['masks'], 'auto' if backend=='rstim' else tuning['backends'][backend]['selected_batch'])
        require(len(case['runs']) == result['pairs'], 'paired-process cardinality')
        for pair, run in enumerate(case['runs']):
            offset = (index+pair)%4; order = BACKENDS[offset:]+BACKENDS[:offset]
            if pair%2: order = list(reversed(order))
            require(run['order'] == order and set(run) == set(BACKENDS+['order']), 'process rotation')
            for backend in BACKENDS:
                timing(run[backend], backend, case['shots'], width, result['repetitions'],
                    None if backend=='rstim' else tuning['backends'][backend]['selected_batch'])
    return report(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('result', type=Path)
    parser.add_argument('--allow-subset', action='store_true')
    parser.add_argument('--git-sources', action='store_true')
    parser.add_argument('--artifacts', type=Path)
    args = parser.parse_args()
    try:
        text = validate(json.loads(args.result.read_text()), args.allow_subset, args.git_sources, args.artifacts)
        require(args.result.with_name('report.md').read_text() == text, 'report differs from raw observations')
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f'evidence rejected: {error}\n')
    print('evidence verified')


if __name__ == '__main__':
    main()
