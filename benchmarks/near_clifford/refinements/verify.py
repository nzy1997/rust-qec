"""Check complete paired evidence, source identity, semantics and cache ledgers."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics
import subprocess

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('refinement_entry', HERE / 'run.py')
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)
MODES = {'unprepared', 'cold_prepared_structured', 'first_prepared_structured',
         'first_prepared_flat', 'warm_prepared_structured', 'warm_prepared_flat'}

def require(condition, message):
    if not condition: raise ValueError(message)

def verify(result, scratch=None, git_sources=False):
    require(result.get('schema') == 'near-clifford.refinements.v1', 'wrong schema')
    require(result.get('completed_utc') and not result.get('quick'), 'incomplete/smoke run')
    require(result['pairs'] == result['repetitions'] == 3, 'wrong repeat counts')
    require(result['entry_sha256'] == run.sha(HERE / 'run.py'), 'runner changed')
    require(result['harness_sha256'] == hashlib.sha256(run.driver().encode()).hexdigest(), 'driver changed')
    require(result['runner_sha256'] == run.sha(run.BASE / 'scale/run.py'), 'timing boundaries changed')
    old = run.load(run.BASE / 'scale/run.py', 'refinement_verify_counter_inputs')
    require(result['counter_names'] == old.COUNTERS and result['snapshot_names'] == old.SNAPSHOT, 'counter definitions changed')
    expected_inputs = {str(p.relative_to(run.ROOT)): run.sha(p) for p in [
        run.BASE / 'scale/run.py', run.BASE / 'scale/main.rs', run.BASE / 'scale/Cargo.unified.lock',
        run.BASE / 'entangled/run.py', run.BASE / 'entangled/fixtures.rs']}
    require(result['input_hashes'] == expected_inputs, 'input identity changed')
    require(result['fixtures_sha256'] == {p.name: run.sha(p) for p in (run.BASE / 'scale/fixtures').iterdir()}, 'fixtures changed')
    matrix = [tuple(row) for row in result['selected_matrix']]
    require(matrix and len(matrix) == len(set(matrix)), 'empty/duplicate matrix')
    require(all(row in run.matrix() for row in matrix), 'unknown matrix case')
    require(result['matrix'] == result['selected_matrix'], 'matrix binding changed')
    require([(c['fixture'], c['shots']) for c in result['cases']] == matrix, 'missing/reordered cases')
    names = {name for name, _ in matrix}
    payloads = result['verification_results']
    require(set(payloads) == {'baseline', 'candidate', 'baseline-diagnostic', 'candidate-diagnostic'}, 'missing semantic labels')
    require(set(result['sources']) == set(result['diagnostics']) == {'baseline', 'candidate'}, 'wrong revision labels')
    require(result['sources']['baseline']['revision'] != result['sources']['candidate']['revision'], 'same revisions')
    require(set(result['verification']) == names, 'missing verification')
    for label, rows in payloads.items():
        require(set(rows) == names, 'missing semantic cases')
        for name, payload in rows.items():
            require(payload == payloads['baseline'][name], 'output/RNG/physics mismatch')
            require(payload['fixture'] == name and len(payload['shots']) == 256, 'wrong semantic fixture/count')
            require(type(payload['continuation']) is int and 0 <= payload['continuation'] < 2**64, 'invalid RNG continuation')
            for shot in payload['shots']:
                require(set(shot) == {'m', 'd', 'o'}, 'wrong semantic fields')
                require(all(type(v) is bool for key in ['m', 'd'] for v in shot[key]), 'invalid measurement/detector values')
                require(all(len(v) == 2 and type(v[0]) is int and v[0] >= 0 and type(v[1]) is bool for v in shot['o']), 'invalid observables')
            check = result['verification'][name]
            require(check['status'] == 'pass' and check['output_sha256'] == hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest(), 'semantic hash mismatch')
    ent = run.load(run.BASE / 'entangled/run.py', 'refinement_verify_oracle')
    for label, source in result['sources'].items():
        require(set(result['diagnostics'][label]) == names, 'missing diagnostics')
        for suffix, metadata in [('', source), ('-diagnostic', source['diagnostic'])]:
            require(metadata['revision'] == source['revision'], 'revision mismatch')
            require(type(metadata['has_cache_ledger']) is bool and metadata['has_cache_ledger'] == source['has_cache_ledger'], 'ledger availability mismatch')
            require(metadata['lock_sha256'] == run.sha(run.BASE / 'scale/Cargo.unified.lock'), 'lock mismatch')
            if git_sources:
                def blob(path):
                    return subprocess.check_output(['git', 'show', source['revision'] + ':' + path], cwd=run.ROOT)
                near = blob('rstim/src/near_clifford.rs')
                require(metadata['has_cache_ledger'] == ('reserved_cache_bytes: usize' in near.decode()), 'ledger source mismatch')
                if suffix: near = run.instrument(near.decode().strip() + '\n').encode()
                require(hashlib.sha256(near).hexdigest() == metadata['near_clifford_source_sha256'], 'near-Clifford source mismatch')
                require(hashlib.sha256(blob('rstim/src/sim/tableau.rs')).hexdigest() == metadata['tableau_source_sha256'], 'tableau mismatch')
                oracle = blob('rstim/tests/support/near_clifford_oracle.rs')
                require(hashlib.sha256(oracle).hexdigest() == metadata['oracle_source_sha256'], 'oracle source mismatch')
                require(hashlib.sha256(ent.adapt_oracle(oracle)).hexdigest() == metadata['oracle_sha256'], 'oracle overlay mismatch')
            if scratch:
                folder = scratch / (label + suffix)
                require(run.sha(folder / 'near-clifford-scale') == metadata['binary_sha256'], 'binary mismatch')
                require(run.sha(folder / 'source/rstim/src/near_clifford.rs') == metadata['near_clifford_source_sha256'], 'built source mismatch')
                require(run.sha(folder / 'source/rstim/src/sim/tableau.rs') == metadata['tableau_source_sha256'], 'built tableau mismatch')
                require(run.sha(folder / 'harness/main.rs') == result['harness_sha256'], 'built driver mismatch')
                require(run.sha(folder / 'harness/oracle.rs') == metadata['oracle_sha256'], 'built oracle mismatch')
        for name, diag in result['diagnostics'][label].items():
            require(diag['fixture'] == name and diag['semantic_verification'] == 'pass', 'diagnostic semantics failed')
            require(len(diag['counters']) == len(diag['warmup_counters']) == 8, 'counter shape mismatch')
            require(all(type(n) is int and n >= 0 for key in ['counters', 'warmup_counters'] for n in diag[key]), 'invalid counters')
            for key in ['initial', 'after_warmup', 'after_probe']:
                value = diag[key]
                require(len(value) == 5 and all(type(n) is int and n >= 0 for n in value), 'invalid snapshot')
                require(value[1] == 1 << value[0] and value[2] <= value[3], 'rank/node snapshot mismatch')
            ledger = diag['cache_reservation']
            require((ledger is not None) == (source['has_cache_ledger'] and bool(diag['after_probe'][4])), 'missing/unexpected cache ledger')
            if ledger is not None:
                require(len(ledger) == 4 and all(type(n) is int and n >= 0 for n in ledger), 'invalid ledger')
                require(ledger[1] == 64 * 1024 * 1024, 'changed byte limit')
                require(ledger[2] <= 1 and ledger[3] <= 2, 'invalid root metadata')
                if ledger[0] > ledger[1]:
                    require(diag['after_probe'][2] == 1 and ledger[2:] == [0, 0], 'over-budget optional caching')
    for index, case in enumerate(result['cases']):
        require(len(case['runs']) == 3, 'missing process pairs')
        for pair, values in enumerate(case['runs']):
            require(values['order'] == (['baseline', 'candidate'] if (index + pair) % 2 == 0 else ['candidate', 'baseline']), 'wrong paired order')
            require(values['baseline']['circuit'] == values['candidate']['circuit'], 'different timed circuits')
            for label in ['baseline', 'candidate']:
                require(values[label]['fixture'] == case['fixture'] and len(values[label]['measurements']) == 1, 'wrong timed fixture')
                measured = values[label]['measurements'][0]
                require(measured['shots'] == case['shots'] and set(measured) - {'shots'} == MODES, 'missing timing modes')
                for mode in MODES:
                    data = measured[mode]
                    require(len(data['raw_ns']) == 3 and all(type(n) is int and n >= 0 for n in data['raw_ns']), 'invalid raw timing')
                    require(data['median_ns'] == statistics.median(data['raw_ns']), 'invalid median')
    return f'PASS: {len(matrix)} configurations; pristine/diagnostic source, binary, 256-shot semantics and cache-ledger contracts'

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('results', type=Path)
    p.add_argument('--binaries', type=Path)
    p.add_argument('--git-sources', action='store_true')
    a = p.parse_args()
    print(verify(json.loads(a.results.read_text()), a.binaries, a.git_sources))
