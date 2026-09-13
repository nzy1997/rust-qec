"""Analytic distribution probes for the specified stochastic Pauli channels.

Bell readout maps I,Z,X,Y on each data wire to 00,10,01,11 measurement
bits (little endian). It resolves all Pauli components, unlike one parity.
This finite statistical check is not tomography of arbitrary quantum channels.
"""
import itertools
import math
from pathlib import Path
import re
import tempfile
import numpy as np
from . import reference

ALPHA = 1e-7


from .probe_specs import bell_text, basis_ops, distribution_specs as specifications


def observed_probabilities(rows, columns):
    selected = rows[:, columns].astype(np.uint64)
    ids = (selected * (1 << np.arange(len(columns), dtype=np.uint64))).sum(axis=1).astype(int)
    joint = np.bincount(ids, minlength=1 << len(columns)) / len(rows)
    return joint, selected.mean(axis=0)


def evaluate(binary, sampler, shots=32768):
    probes = specifications()
    events = sum(len(p['expected']) + len(p['columns']) for p in probes)
    tolerance = math.sqrt(math.log(4*events/ALPHA)/(2*shots))
    records = []
    with tempfile.TemporaryDirectory(prefix='pauli-channel-') as tmp:
        for probe in probes:
            expected = np.array(probe['expected'])
            expected_marginal = np.array([sum(prob for i,prob in enumerate(expected) if i & (1 << bit))
                                          for bit in range(len(probe['columns']))])
            observations = {}
            passed = True
            for name, rows in [('rust', sampler(binary, probe['text'], shots, 991, Path(tmp))),
                               ('reference', reference.sample(probe['text'], shots, 773))]:
                joint, marginal = observed_probabilities(rows, probe['columns'])
                for actual, target in [(joint,expected),(marginal,expected_marginal)]:
                    passed &= bool(np.all(np.abs(actual-target) <= tolerance))
                    passed &= bool(np.all(actual[target == 0] == 0) and np.all(actual[target == 1] == 1))
                observations[name] = {'joint':joint.tolist(), 'marginals':marginal.tolist()}
            records.append({'case':probe['name'], 'channel':probe['channel'],
                            'expected_joint':expected.tolist(), 'expected_marginals':expected_marginal.tolist(),
                            **observations, 'shots_per_sampler':shots, 'tolerance':tolerance,
                            'status':'PASS' if passed else 'FAIL'})
    return records


MUTATIONS = ('DEPOLARIZE2_ix_only', 'DEPOLARIZE2_xi_only',
             'DEPOLARIZE2_independent_x', 'DEPOLARIZE1_x_only', 'DEPOLARIZE1_z_only')


def replace_channel(text, mutation):
    """Actual input mutation, preserving forced-loss skipping in these probes."""
    if mutation not in MUTATIONS:
        raise ValueError(f'Unknown channel mutation: {mutation}')
    lost, output = set(), []
    channel = mutation.split('_', 1)[0]
    for line in text.splitlines():
        loss = re.fullmatch(r'LOSS\(1\) (.*)', line)
        if loss: lost.update(map(int, loss[1].split()))
        if line.startswith('R '): lost.difference_update(map(int, line[2:].split()))
        noise = re.fullmatch(channel + r'\(([^)]+)\) (.*)', line)
        if not noise:
            output.append(line)
            continue
        p = float(noise[1]); targets = list(map(int, noise[2].split()))
        if channel == 'DEPOLARIZE2':
            for first, second in zip(targets[::2], targets[1::2]):
                if first in lost or second in lost: continue
                chosen = [second] if mutation.endswith('ix_only') else (
                    [first] if mutation.endswith('xi_only') else [first, second])
                output.extend(f'X_ERROR({8*p/15}) {q}' for q in chosen)
        else:
            axis = 'Z' if mutation.endswith('z_only') else 'X'
            output.extend(f'{axis}_ERROR({2*p/3}) {q}' for q in targets if q not in lost)
    return '\n'.join(output)


def run(binary, sampler, shots=32768):
    records = evaluate(binary, sampler, shots)
    mutations = {}
    for name in MUTATIONS:
        def defective(binary, text, shots, seed, work):
            return sampler(binary, replace_channel(text, name), shots, seed, work)
        failed = [r['case'] for r in evaluate(binary, defective, shots) if r['status'] == 'FAIL']
        mutations[name] = {'rejected':bool(failed), 'failed_cases':failed}
    return {'status':'PASS' if all(r['status']=='PASS' for r in records) and all(m['rejected'] for m in mutations.values()) else 'FAIL',
            'method':'Bell Pauli-component readout; X/Y/Z product-basis joints and marginals; both loss directions',
            'familywise_alpha_bound':ALPHA, 'cases':records, 'channel_replacement_mutations':mutations}
