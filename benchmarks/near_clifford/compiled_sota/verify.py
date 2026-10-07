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
from projection import records_only, physical_width
from run import BACKENDS, BATCHES, HERE, ROOT, RSTIM_API, RSTIM_ROTATION_ARITHMETIC, batches, bind_arithmetic, bind_peer, default_batch, environment_summary, harness_inventory, masks, report, sha


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def digest(value):
    return isinstance(value, str) and re.fullmatch('[a-f0-9]{64}', value) is not None


def close(a, b):
    return type(a) in (int, float) and math.isfinite(a) and math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12)


def compiled_configuration(raw):
    require(raw['backend'] == 'rstim' and raw['api'] == RSTIM_API, 'compiled API identity')
    bind_arithmetic(raw)
    require(raw['rng'] == 'SmallRng/rand-0.8.7', 'compiled RNG identity')
    require(integer(raw['peak_active_rank']) and raw['peak_active_rank'] <= 16, 'compiled rank')
    require(integer(raw['cache_reserved_bytes']) and raw['cache_reserved_bytes'] <= 64*1024*1024,
        'compiled cache reservation (not RSS)')


def peer_configuration(raw, backend, batch, packages, identities):
    bind_peer(raw, backend, packages, identities)
    require(raw['version'] == ('0.11.0' if backend.startswith('clifft') else '0.1.1'), 'peer version')
    require(type(raw['threads']) is int and raw['threads'] == 1 and raw['batch'] == batch, 'peer configuration')
    if backend == 'symft':
        require(type(raw['batch_enabled']) is bool and raw['batch_enabled'] == (batch != 'scalar'),
            'SymFT scalar/batch executor configuration')


def timing(raw, backend, shots, width, repetitions, input_hash, batch=None, tuning=False,
        packages=None, identities=None):
    require(raw['backend'] == backend and raw['shots'] == shots and raw['width'] == width, 'timing identity')
    require(raw['input_sha256'] == input_hash, 'timing consumed-input digest')
    require(integer(raw['compile_ns'], 1), 'compile duration')
    if backend == 'rstim':
        compiled_configuration(raw)
        require(raw['rng'] == 'SmallRng/rand-0.8.7' and integer(raw['prepare_ns'], 1), 'rstim configuration')
    else:
        peer_configuration(raw, backend, batch, packages, identities)
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


def transcript(encoded, backend, shots, width, call_shots, expected_hash, input_hash, counts, selected,
        batch='auto', packages=None, identities=None):
    payload=expand(encoded)
    require(integer(payload['shots'], 1) and integer(payload['width'], 1) and
        integer(payload['call_shots'], 1) and payload['shots'] % payload['call_shots'] == 0,
        'transcript invocation dimensions')
    require(payload['shots']==shots and payload['width']==width and payload['call_shots']==call_shots, 'transcript dimensions')
    require(payload['input_sha256'] == input_hash, 'transcript consumed-input digest')
    if backend == 'rstim':
        compiled_configuration(payload)
        require(integer(payload['continuation']) and payload['continuation'] < 2**64, 'compiled RNG continuation')
    else:
        require(payload['backend']==backend, 'transcript backend')
        peer_configuration(payload, backend, batch, packages, identities)
    require(hashlib.sha256((json.dumps(payload)+'\n').encode()).hexdigest()==expected_hash, 'transcript content digest')
    require(parity_counts(payload,selected)==counts, 'counts differ from transcript')


def validate(result, allow_subset=False, git_sources=False, artifacts=None):
    manifest = json.loads((HERE/'manifest.json').read_text())
    require(manifest['schema'] == 'rstim.near-clifford-compiled-sota.v2' and
        manifest['rstim_api'] == RSTIM_API, 'compiled manifest')
    require(manifest['rotation_arithmetic'] == RSTIM_ROTATION_ARITHMETIC, 'compiled manifest arithmetic')
    require(result['schema'] == 'rstim.near-clifford-compiled-sota-results.v2', 'schema')
    require(result['rotation_arithmetic'] == RSTIM_ROTATION_ARITHMETIC, 'compiled result arithmetic')
    require(result['rstim_api'] == RSTIM_API and result['input_contract'] ==
        'identical native records_only circuit for every backend', 'compiled result contract')
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
    require(manifest['protocol']['batch_grid'] == BATCHES and
        manifest['protocol']['symft_candidates'] == batches('symft') and
        manifest['protocol']['symft_default'] == default_batch('symft'), 'frozen backend-specific tuning grids')
    for environment, distributions in [(result['packages'], ['symft','numpy']),
            (result['packages']['clifft_environment'], ['clifft','numpy'])]:
        for name in distributions:
            package = environment[name]
            require(package['version'] == manifest['baseline_versions'][name], 'package version')
            require(bool(package['files']) and all(isinstance(path,str) and Path(path).is_absolute() and
                digest(d) for path,d in package['files'].items()), 'package inventory locations/hashes')
    require(bool(result['symft_source_files']) and all(digest(d) for d in result['symft_source_files'].values()), 'SymFT sources')
    require(set(result['peer_loaded_files']) == {'clifft','symft'}, 'actual peer identity inventory')
    for name, files in result['peer_loaded_files'].items():
        bind_peer({'loaded_files': files, 'isolated': True}, name, result['packages'], result['peer_loaded_files'])
        require(all(digest(f['sha256']) and Path(f['path']).is_absolute() for f in files.values()), 'actual peer identity')
    summary=environment_summary(result['packages'], result['symft_source_files'],
        result['symft_source_revision'], result['peer_loaded_files'])
    require(set(result['environment_guard']) == {'before','after'} and
        result['environment_guard']['before'] == summary and result['environment_guard']['after'] == summary,
        'peer environment/source stability guard')
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
        adapted=native
        require(hashlib.sha256(native.encode()).hexdigest()==validation['native_sha256'] and
            hashlib.sha256(adapted.encode()).hexdigest()==validation['adapted_sha256'], 'native timed input identity')
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
                validation['native_sha256'],
                validation['parity_counts'][backend],selected, default_batch(backend),
                result['packages'], result['peer_loaded_files'])
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
            require([t['batch'] for t in choice['trials']] == batches(backend), 'tuning grid')
            valid = []
            for trial in choice['trials']:
                if 'error' in trial:
                    require(isinstance(trial['error'], str) and bool(trial['error']), 'invalid tuning failure')
                    continue
                timing(trial['raw'], backend, case['shots'], width, 3,
                    validation['native_sha256'], trial['batch'], True,
                    result['packages'], result['peer_loaded_files'])
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
                validation['native_sha256'],
                chosen_counts,validation['masks'], 'auto' if backend=='rstim' else tuning['backends'][backend]['selected_batch'],
                result['packages'], result['peer_loaded_files'])
        require(tuning['transcripts']['rstim']['peak_active_rank'] ==
            validation['transcripts']['rstim']['peak_active_rank'], 'selected compiled plan rank')
        require(len(case['runs']) == result['pairs'], 'paired-process cardinality')
        for pair, run in enumerate(case['runs']):
            offset = (index+pair)%4; order = BACKENDS[offset:]+BACKENDS[:offset]
            if pair%2: order = list(reversed(order))
            require(run['order'] == order and set(run) == set(BACKENDS+['order']), 'process rotation')
            for backend in BACKENDS:
                timing(run[backend], backend, case['shots'], width, result['repetitions'],
                    validation['native_sha256'],
                    None if backend=='rstim' else tuning['backends'][backend]['selected_batch'],
                    packages=result['packages'], identities=result['peer_loaded_files'])
            require(run['rstim']['peak_active_rank'] == validation['transcripts']['rstim']['peak_active_rank'],
                'timed compiled plan rank')
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
