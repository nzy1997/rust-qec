"""Deterministic, bounded diagnostic matrices; original corpus is immutable."""
from pathlib import Path
import hashlib
import json

HERE = Path(__file__).resolve().parent
INCUMBENT = HERE.parent / 'compiled_sota'
BASELINE = '3ef5030db205b3e9b2126e31b2602f760d4665cc'
BUDGETS = [0, 1 << 20, 16 << 20, 64 << 20]
POLICIES = ['strict', 'fused']
BOUNDARY_SHOTS = [1, 8, 32, 63, 64, 65, 256, 1024, 4096]


def synthetic(rank=8, qubits=32, depth=8, noise=0.001, reset_every=8):
    if not 1 <= rank <= 16 or qubits < rank or depth < 1:
        raise ValueError('invalid synthetic bounds')
    lines = ['R ' + ' '.join(map(str, range(qubits)))]
    # A random measurement precedes rotations, so coefficients cannot all be
    # hidden in deterministic-prefix compilation. All qubits participate.
    lines += ['H 0', 'M 0', 'R 0']
    for cycle in range(depth):
        lines += ['H ' + ' '.join(map(str, range(rank))),
                  'T ' + ' '.join(map(str, range(rank)))]
        lines += [f'CX {q} {rank + (q + cycle) % (qubits-rank)}'
                  for q in range(rank)] if qubits > rank else []
        lines += [f'DEPOLARIZE1({noise}) ' + ' '.join(map(str, range(qubits))),
                  'MPP ' + ' '.join(f'X{q}*Z{(q+1)%qubits}' for q in range(0,rank,2)),
                  'DETECTOR rec[-1]', 'CX rec[-1] 0']
        if (cycle + 1) % reset_every == 0:
            lines += ['MR ' + ' '.join(map(str, range(rank)))]
    lines += ['M ' + ' '.join(map(str, range(qubits))), 'OBSERVABLE_INCLUDE(0) rec[-1]']
    return '\n'.join(lines) + '\n'


def build(out):
    out.mkdir(parents=True, exist_ok=True)
    incumbent = json.loads((INCUMBENT / 'manifest.json').read_text())
    fixtures = {}
    for item in incumbent['cases']:
        data=(INCUMBENT/item['file']).read_bytes()
        if hashlib.sha256(data).hexdigest()!=item['sha256']:
            raise ValueError('incumbent fixture digest changed: '+item['id'])
        fixtures[item['id']] = {'text': (INCUMBENT / item['file']).read_text(),
                               'family': item['family'], 'source': item['source']}
    axes = {
        'rank': [4, 8, 12, 16], 'qubits': [16, 64, 129, 256],
        'depth': [1, 8, 32], 'noise': [0, 0.0001, 0.001, 0.01],
        'reset_every': [1, 4, 32],
    }
    for axis, values in axes.items():
        for value in values:
            name = f'{axis}-{value}'
            fixtures[name] = {'text': synthetic(**{axis: value}), 'family': 'structural',
                              'source': {'generator': 'corpus.synthetic', 'axis': axis, 'value': value}}
    for name, fixture in fixtures.items():
        path = out / (name + '.stim')
        path.write_text(fixture.pop('text'))
        fixture.update(path=str(path.resolve()), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    cells = {}
    def add(name, shots, budget, policy, group):
        key = f'{name}/s{shots}/c{budget}/{policy}'
        if key not in cells:
            cells[key] = dict(id=key, fixture=name, shots=shots, cache_bytes=budget,
                              arithmetic=policy, groups=[])
        cells[key]['groups'].append(group)
    for policy in POLICIES:
        for shots in [1, 64, 1024]:
            for budget in BUDGETS:
                add('msc5', shots, budget, policy, 'cache')
        for name in ['msc5', 'terminal', 'msc3', 'qec32']:
            for shots in BOUNDARY_SHOTS:
                add(name, shots, BUDGETS[-1], policy, 'boundary')
        for name in fixtures:
            if fixtures[name]['family'] == 'structural':
                for shots in [1, 64, 1024]:
                    add(name, shots, BUDGETS[-1], policy, 'structure')
        for name in ['brick16', 'parity129', 'rounds129']:
            for shots in [1, 64, 1024]:
                add(name, shots, BUDGETS[-1], policy, 'incumbent')
    histories = {
        'single': [{'kind': 'flat', 'shots': 1}],
        'batch-once': [{'kind': 'flat', 'shots': 1024}],
        'batch-repeat': [{'kind': 'flat', 'shots': 1024}] * 8,
        'small-repeat': [{'kind': 'flat', 'shots': 64}] * 32,
        'scalar-to-batch': [{'kind': 'structured', 'shots': 1}, {'kind': 'flat', 'shots': 1024}],
        'batch-to-scalar': [{'kind': 'flat', 'shots': 1024}, {'kind': 'structured', 'shots': 1}],
        'mixed': [{'kind': 'flat', 'shots': n} for n in [1,64,65,1024,1,64]],
        'structured-repeat': [{'kind': 'structured', 'shots': 64}] * 8,
    }
    return dict(schema='rstim.near-clifford-diagnostics.v1', baseline=BASELINE,
                fixtures=fixtures, cells=list(cells.values()), histories=histories,
                pairs=5, repetitions=7, validation_shots=8192,
                warm_minimum_ns=50_000_000, output_contract='full raw records, no postselection')
