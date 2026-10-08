"""Replay frozen Rust-only coefficient-intern scouts; never collect timings."""
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
DEFAULT = ROOT / 'benchmarks/near_clifford/results/apple-m4-coefficient-intern-rust-ablation-2026-10-08'
NAMES = ['msc_d3_inject_cultivate_p1e-3', 'msc_d5_inject_cultivate_p1e-3',
         'pure_surface_d7_r7_p1e-3', 'pure_surface_d9_r9_p1e-3']
CASES = [[name, shots, policy] for name in NAMES for shots in [1, 64, 1024]
         for policy in ['strict', 'fused']]
CAMPAIGNS = ['full-hash-warm', 'admission-warm', 'admission-confirmation',
             'admission-cold', 'sparse-warm', 'sparse-cold', 'deferred-warm',
             'deferred-cold', 'large-warm', 'large-confirmation', 'large-cold']


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_text())


def number(value, positive=False):
    return type(value) is int and value >= (1 if positive else 0)


def verify_campaign(archive, label, binding, *, schema_prefix="coefficient-intern"):
    require(schema_prefix in ("coefficient-intern", "zero-noise-spans", "strict-coefficient-pairs"), "supported scout family")
    path = archive / label
    header, closure, summary = [load(path / name) for name in
                                ['header.json', 'closure.json', 'summary.json']]
    data = (path / 'events.jsonl').read_bytes()
    require(len(data) < 10 * 1024 * 1024, 'unbounded scout event file')
    events = [json.loads(line) for line in data.splitlines()]
    cold = label.endswith('-cold')
    # The Strict warm collectors froze this historical spelling; retain their bytes.
    warm_schema = ('exploratory.zero-strict-pair-ablation.v1' if schema_prefix == 'strict-coefficient-pairs'
                   else 'exploratory.' + schema_prefix + '-ablation.v1')
    require(header['schema'] == ('exploratory.' + schema_prefix + '-cold.v1' if cold
                                else warm_schema), 'schema')
    require(header['cases'] == CASES and header['pairs'] == 5, 'case/round coverage')
    require(header['observations_per_process'] == (32 if cold else 7), 'observation coverage')
    require(header['identities'] == closure['identities_after'], 'identity closure')
    require(closure['events'] == len(events) == (240 if cold else 288), 'event coverage')
    require(closure['events_sha256'] == digest(data), 'event digest')
    for role in ['baseline', 'candidate']:
        identity, route = header['identities'][role], binding['routes'][role]
        require(identity['dirty'] == '' and identity['head'] == route['source_equivalent_commit'], 'clean producer')
        require(identity['driver_sha256'] == digest((path / 'original-driver.py').read_bytes()), 'driver identity')
        mapping = route['untracked_input_mapping']
        require(len(mapping) == 7 and set(mapping) == {p for p in identity['sources'] if p.startswith('drafts/')}, 'probe coverage')
        rust_paths = subprocess.check_output(['git', '-C', str(ROOT), 'ls-tree', '-r', '--name-only', identity['head'], 'rstim/src'], text=True).splitlines()
        expected_sources = {p for p in rust_paths if p.endswith('.rs')}
        expected_sources.update(['Cargo.toml', 'Cargo.lock', 'rstim/Cargo.toml'])
        expected_sources.update('benchmarks/near_clifford/application_counts/fixtures/' + name + '.stim' for name in NAMES)
        expected_sources.update(mapping)
        require(set(identity['sources']) == expected_sources, 'complete producer source inventory')
        require({Path(p).name for p in mapping} == {'main.rs', 'Cargo.toml', 'Cargo.lock'} | {name + '.masks.json' for name in NAMES}, 'probe names')
        for name, sha in identity['sources'].items():
            if name in mapping:
                contents = (archive / mapping[name]).read_bytes()
            else:
                contents = subprocess.check_output(['git', '-C', str(ROOT), 'show', identity['head'] + ':' + name])
            require(digest(contents) == sha, f'source identity: {label}/{role}/{name}')
    expected_order = []
    if not cold:
        expected_order.extend((case, role, 'validate') for case in CASES for role in ['baseline', 'candidate'])
    for round_index in range(5):
        rotated = CASES[round_index:] + CASES[:round_index]
        if round_index % 2:
            rotated.reverse()
        for index, case in enumerate(rotated):
            roles = ['candidate', 'baseline'] if (index + round_index) % 2 else ['baseline', 'candidate']
            expected_order.extend((case, role, 'cold' if cold else 'bench') for role in roles)
    for index, (event, expected) in enumerate(zip(events, expected_order)):
        require(event['index'] == index and (event['case'], event['route'], event['action']) == expected, 'rotated paired schedule')
        result = event['result']
        require(result['shots'] == event['case'][1] and result['policy'] == event['case'][2], 'executor case')
        if event['action'] == 'validate':
            require(result['status'] == 'ok' and result['seeds'] == 4 and result['exact_records_counts_rng'] is True
                    and result['native_counts_rng'] is True, 'independent raw/counts/RNG validation')
            continue
        require(result['route'] == 'native', 'native counts route')
        observations = result['observations']
        require(len(observations) == (32 if cold else 7), 'observation count')
        if cold:
            require([o['seed'] for o in observations] == list(range(739, 771)), 'fresh fixed seeds')
        for obs in observations:
            if cold:
                require(all(number(obs[f]) for f in ['compile_ns', 'prepare_ns', 'first_ns', 'attempted', 'accepted', 'logical_errors']), 'cold integer values')
                require(obs['attempted'] == event['case'][1]
                        and 0 <= obs['logical_errors'] <= obs['accepted'] <= obs['attempted'], 'cold counts')
                require(len(obs['continuation']) == 16 and all(number(v) and v < 2**64 for v in obs['continuation']), 'RNG continuation')
                require(number(obs['cache_reserved_bytes']) and obs['cache_reserved_bytes'] <= 64 * 1024 * 1024, 'cache cap')
            else:
                require(number(obs['elapsed_ns']) and obs['elapsed_ns'] >= 50_000_000
                        and number(obs['calls'], True)
                        and obs['ns_per_call'] == obs['elapsed_ns'] / obs['calls'], 'warm timing arithmetic')
    expected_summary = []
    for case in CASES:
        groups = {role: [e['result']['observations'] for e in events if e['case'] == case
                        and e['route'] == role and e['action'] != 'validate']
                  for role in ['baseline', 'candidate']}
        require(all(len(v) == 5 for v in groups.values()), 'five paired processes')
        if cold:
            for left, right in zip(groups['baseline'], groups['candidate']):
                for a, b in zip(left, right):
                    require(all(a[f] == b[f] for f in ['seed', 'attempted', 'accepted', 'logical_errors', 'continuation']), 'cross-source counts/RNG')
        phases = ['compile_ns', 'prepare_ns', 'first_ns', 'phase_sum_ns'] if cold else [None]
        for phase in phases:
            def value(obs):
                if phase == 'phase_sum_ns':
                    return obs['compile_ns'] + obs['prepare_ns'] + obs['first_ns']
                return obs[phase] if cold else obs['ns_per_call']
            samples = {role: [statistics.median(value(o) for o in group) for group in rows]
                       for role, rows in groups.items()}
            a, b = samples['baseline'], samples['candidate']
            require(all(v > 0 for v in a + b), 'positive process timings')
            ratios = [x / y for x, y in zip(a, b)]
            row = dict(case=case, baseline_ns=statistics.median(a), candidate_ns=statistics.median(b),
                       speedup=statistics.median(a) / statistics.median(b), paired_range=[min(ratios), max(ratios)])
            if cold:
                row['phase'] = phase
            expected_summary.append(row)
    require(summary == expected_summary, 'derived summary differs')


def main():
    require(len(sys.argv) <= 2, 'usage: verify_coefficient_intern_scouts.py [ARCHIVE]')
    archive = Path(sys.argv[1]) if len(sys.argv) == 2 else DEFAULT
    bindings = load(archive / 'bindings.json')['campaigns']
    require(set(bindings) == set(CAMPAIGNS), 'complete selected and rejected campaigns')
    for label in CAMPAIGNS:
        verify_campaign(archive, label, bindings[label])
        print('PASS', label, 'sources/closure/paired schedule/semantics/derived statistics', flush=True)


if __name__ == '__main__':
    main()
