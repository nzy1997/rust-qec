"""Verify row-operation evidence, including pristine/diagnostic tableau bindings."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
ROOT = BASE.parents[1]
MODES = {'unprepared', 'cold_prepared_structured', 'first_prepared_structured',
         'first_prepared_flat', 'warm_prepared_structured', 'warm_prepared_flat'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_bytes(revision, path, expected):
    # Squash merging can remove a measured commit. Verify selected input
    # identity against exact bytes in HEAD ancestry, not other branches or
    # working-tree files. This does not prove the lost original commit/tree.
    result = subprocess.run(['git', 'show', revision + ':' + path], cwd=ROOT, capture_output=True)
    if result.returncode == 0:
        require(hashlib.sha256(result.stdout).hexdigest() == expected, 'git source differs: ' + path)
        return result.stdout
    present = subprocess.run(['git', 'cat-file', '-e', revision + '^{commit}'], cwd=ROOT, capture_output=True)
    require(present.returncode != 0, 'available source is missing selected input: ' + path)
    current = subprocess.run(['git', 'show', 'HEAD:' + path], cwd=ROOT, capture_output=True)
    if current.returncode == 0 and hashlib.sha256(current.stdout).hexdigest() == expected:
        return current.stdout
    history = subprocess.check_output(['git', 'log', '--full-history', '--format=%H', 'HEAD', '--', path], cwd=ROOT, text=True)
    for ancestor in history.splitlines():
        raw = subprocess.run(['git', 'show', ancestor + ':' + path], cwd=ROOT, capture_output=True)
        if raw.returncode == 0 and hashlib.sha256(raw.stdout).hexdigest() == expected:
            return raw.stdout
    raise ValueError('unavailable source has no exact input in HEAD ancestry: ' + path)


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verify(result, binaries=None, git_sources=False):
    require(result.get('row_ops_schema') == 'near-clifford.row-ops.v1', 'unknown row-ops schema')
    require(result.get('row_ops_suite') in ['scale', 'entangled'], 'unknown suite')
    require(result.get('row_ops_entry_sha256') == sha(HERE / 'run.py'), 'row-ops entry differs')
    require(result.get('completed_utc') and not result.get('quick', True), 'incomplete/smoke campaign')
    require(result['pairs'] == result['repetitions'] == 3, 'incorrect repetition counts')
    suite = result['row_ops_suite']
    if suite == 'entangled':
        sys.path.insert(0, str(BASE / 'entangled'))
        inherited = load(BASE / 'entangled/verify.py', 'row_ops_entangled_verify')
        inherited.verify(result, binaries, False)
    else:
        inherited = load(BASE / 'scale/run.py', 'row_ops_scale_inputs')
        require(result['schema'] == 2, 'wrong scale schema')
        require(result['matrix'] == [list(row) for row in inherited.MATRIX], 'wrong scale matrix')
        require(result['harness_sha256'] == sha(BASE / 'scale/main.rs'), 'scale driver differs')
        require(result['runner_sha256'] == sha(BASE / 'scale/run.py'), 'scale runner differs')
        require(result['campaign_entry'] == 'run_unified.py' and
                result['campaign_entry_sha256'] == sha(BASE / 'scale/run_unified.py'), 'unified entry differs')
        require(result['fixtures_sha256'] == {p.name: sha(p) for p in (BASE / 'scale/fixtures').iterdir()}, 'fixtures differ')
        require(result['counter_names'] == inherited.COUNTERS and result['snapshot_names'] == inherited.SNAPSHOT, 'counter definitions differ')
        require(result.get('row_ops_scale_entry_sha256') == sha(HERE / 'scale.py'), 'scale semantic entry differs')
        payloads = result.get('row_ops_verification_results', {})
        require(set(payloads) == {'baseline', 'candidate', 'baseline-diagnostic', 'candidate-diagnostic'}, 'missing semantic payload labels')
        names = {name for name, _ in result['matrix']}
        for label in payloads:
            require(set(payloads[label]) == names, 'missing semantic payloads')
            for name in names:
                payload = payloads[label][name]
                require(payload == payloads['baseline'][name], 'output/RNG payloads differ')
                require(payload['fixture'] == name and len(payload['shots']) == 16, 'wrong semantic fixture/shot count')
                for shot in payload['shots']:
                    require(set(shot) == {'m', 'd', 'o'}, 'invalid shot fields')
                    require(all(isinstance(shot[key], list) and all(type(bit) is bool for bit in shot[key]) for key in ['m', 'd']), 'invalid measurement/detector bits')
                    require(isinstance(shot['o'], list) and all(isinstance(event, list) and len(event) == 2 and type(event[0]) is int and event[0] >= 0 and type(event[1]) is bool for event in shot['o']), 'invalid observable events')
                require(type(payload['continuation']) is int and 0 <= payload['continuation'] < 2**64, 'invalid RNG continuation')
                require(result['verification'][name]['output_sha256'] == hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest(), 'semantic hash differs')
        historical = json.loads((BASE / 'results/apple-m4-probability-2026-09-30.json').read_text())
        circuits = {case['fixture']: case['runs'][0]['candidate']['circuit'] for case in historical['cases']}
        for case in result['cases']:
            for pair in case['runs']:
                require(pair['baseline']['circuit'] == circuits[case['fixture']], 'historical scale circuit differs')
    names = {name for name, _ in result['matrix']}
    require(set(result['sources']) == set(result['diagnostics']) == {'baseline', 'candidate'}, 'wrong source labels')
    require(set(result['verification']) == names, 'missing circuit checks')
    require([(c['fixture'], c['shots']) for c in result['cases']] == [tuple(row) for row in result['matrix']], 'missing configurations')
    require(result['sources']['baseline']['revision'] != result['sources']['candidate']['revision'], 'identical revisions')
    for label in ['baseline', 'candidate']:
        source = result['sources'][label]
        diagnostic = source['diagnostic']
        require(diagnostic['revision'] == source['revision'], 'diagnostic revision differs')
        require(source['lock_sha256'] == diagnostic['lock_sha256'] == sha(BASE / 'scale/Cargo.unified.lock'), 'lock differs')
        require(source['tableau_source_sha256'] == diagnostic['tableau_source_sha256'], 'diagnostic tableau differs')
        for metadata in [source, diagnostic]:
            for key in ['binary_sha256', 'tableau_source_sha256', 'near_clifford_source_sha256']:
                digest = metadata.get(key, '')
                require(len(digest) == 64 and all(c in '0123456789abcdef' for c in digest), 'invalid source digest')
        if git_sources:
            for path, key in [('rstim/src/sim/tableau.rs', 'tableau_source_sha256'),
                              ('rstim/src/near_clifford.rs', 'near_clifford_source_sha256')]:
                raw = source_bytes(source['revision'], path, source[key])
                require(hashlib.sha256(raw).hexdigest() == source[key], 'git source differs: ' + path)
                if key == 'near_clifford_source_sha256':
                    overlay = load(BASE / 'scale/run.py', 'row_ops_overlay').instrument(raw.decode().strip() + '\n')
                    require(hashlib.sha256(overlay.encode()).hexdigest() == diagnostic[key], 'diagnostic overlay differs')
            if suite == 'entangled':
                raw = source_bytes(source['revision'], 'rstim/tests/support/near_clifford_oracle.rs', source['oracle_source_sha256'])
                require(hashlib.sha256(raw).hexdigest() == source['oracle_source_sha256'], 'oracle source differs')
                require(hashlib.sha256(inherited.adapt_oracle(raw)).hexdigest() == source['oracle_sha256'], 'adapted oracle differs')
        if binaries:
            for suffix, metadata in [('', source), ('-diagnostic', diagnostic)]:
                require(sha(binaries / (label + suffix) / 'near-clifford-scale') == metadata['binary_sha256'], 'binary differs')
                require(sha(binaries / (label + suffix) / 'source/rstim/src/sim/tableau.rs') == metadata['tableau_source_sha256'], 'built tableau differs')
        require(set(result['diagnostics'][label]) == names, 'missing diagnostics')
        for name in names:
            require(result['verification'][name]['status'] == 'pass', 'semantic check failed')
            row = result['diagnostics'][label][name]
            require(row['fixture'] == name and row['semantic_verification'] == 'pass', 'diagnostic semantics failed')
            for key in ['counters', 'warmup_counters']:
                require(len(row[key]) == 8 and all(type(n) is int and n >= 0 for n in row[key]), 'invalid counters')
            for key in ['initial', 'after_warmup', 'after_probe']:
                v = row[key]
                require(len(v) == 5 and all(type(n) is int and n >= 0 for n in v), 'invalid snapshot')
                require(v[0] <= 16 and v[1] == 1 << v[0] and v[2] <= v[3] and v[4] in [0, 1], 'invalid cache/rank snapshot')
    for index, case in enumerate(result['cases']):
        require(len(case['runs']) == 3, 'missing pairs')
        for pair, run in enumerate(case['runs']):
            expected = ['baseline', 'candidate'] if (index + pair) % 2 == 0 else ['candidate', 'baseline']
            require(run['order'] == expected, 'wrong alternating order')
            require(run['baseline']['circuit'] == run['candidate']['circuit'], 'timed circuits differ')
            for label in ['baseline', 'candidate']:
                require(run[label]['fixture'] == case['fixture'] and len(run[label]['measurements']) == 1, 'wrong timed fixture')
                measurement = run[label]['measurements'][0]
                require(measurement['shots'] == case['shots'] and set(measurement) - {'shots'} == MODES, 'wrong timing modes')
                for mode in MODES:
                    value = measurement[mode]
                    raw = value['raw_ns']
                    require(len(raw) == 3 and all(type(n) is int and n >= 0 for n in raw), 'invalid timings')
                    require(value['median_ns'] == sorted(raw)[1], 'wrong timing median')
    return f'PASS: {suite}, {len(result["cases"])} configurations; tableau/source/binary bindings and diagnostic contracts'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    parser.add_argument('--binaries', type=Path)
    parser.add_argument('--git-sources', action='store_true')
    args = parser.parse_args()
    print(verify(json.loads(args.results.read_text()), args.binaries, args.git_sources))


if __name__ == '__main__':
    main()
