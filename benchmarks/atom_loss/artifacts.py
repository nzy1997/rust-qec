"""Shared publication contract and authoritative native timing boundary."""
import math
import json
import statistics

REQUIRED_FILES = frozenset([
    'chain-correctness.json', 'midswap_d3_r2.stim', 'correctness.json',
    'decoder-correctness.json', 'sampling.json', 'decoding.json', 'tradeoff.json',
    'provenance-all.json', 'methodology.md', 'summary.csv', 'source-snapshot.json',
    'shot-data-v1.zip', 'timing-sweep.csv',
] + [f'{name}.{ext}' for name in [
    'sampling-throughput', 'logical-error-rate', 'logical-error-rate-full',
    'accuracy-time', 'adapter-stages', 'sampling-reference-cost', 'timing-sweep'] for ext in ['svg', 'png']])
TIMING_FILES = frozenset(['provenance-timing.json', 'source-snapshot-timing.json'])


CORRECTNESS_FILES = frozenset(['provenance-correctness.json', 'source-snapshot-correctness.json'])


def required_files(root):
    """Either member on disk makes both retiming provenance files mandatory."""
    retimed = any((root/name).exists() for name in TIMING_FILES)
    for name in ['decoding.json', 'tradeoff.json']:
        if (root/name).exists():
            data = json.loads((root/name).read_text())
            cases = data if isinstance(data, list) else [data]
            retimed |= any(case.get('baseline_predictions_unchanged', False) for case in cases)
    correctness = any((root/name).exists() for name in CORRECTNESS_FILES)
    if (root/'correctness.json').exists():
        correctness |= json.loads((root/'correctness.json').read_text()).get('provenance_file') == 'provenance-correctness.json'
    return REQUIRED_FILES | (TIMING_FILES if retimed else set()) | (CORRECTNESS_FILES if correctness else set())


def native_total(run):
    if run.get('status') != 'ok' or run.get('exit_code') != 0 or 'stats' not in run:
        raise ValueError('Missing successful native timing stats')
    phases = [run['stats'][key] for key in ['compile_seconds', 'decode_seconds']]
    if any(not math.isfinite(value) or value < 0 for value in phases):
        raise ValueError('Invalid native phase time')
    return sum(phases)


def timing_rows(decoding):
    """All measured sweep repetitions, with explicitly different cache policies."""
    for case in decoding:
        for name,result in case['decoders'].items():
            for rep,(record,total) in enumerate(zip(result['runs'],result['total_seconds'])):
                native=name=='envelope-matching'
                stats=record['stats'] if native else record
                yield {**{k:case[k] for k in ['distance','rounds','loss_probability']},
                    'decoder':name,'repetition':rep,'microseconds_per_shot':total/case['shots']*1e6,
                    'input_loss_patterns':case['graph']['loss_patterns'],
                    'graph_builds':stats['matching_graph_builds'] if native else stats['graph_builds'],
                    'cache_hits':stats['cache_hits'] if native else '',
                    'policy':'FIFO/work-budget streaming' if native else 'offline batch groups'}


SUMMARY_FIELDS = ['experiment','distance','rounds','loss_probability','decoder','status',
                  'shots','errors','logical_error_rate','ci95_low','ci95_high',
                  'median_microseconds_per_shot']


def wilson(errors, shots):
    z = 1.959963984540054
    p = errors / shots
    center = (p + z*z/(2*shots))/(1+z*z/shots)
    delta = z*math.sqrt(p*(1-p)/shots+z*z/(4*shots*shots))/(1+z*z/shots)
    return [max(0., center-delta), min(1., center+delta)]


def summary_rows(decoding, tradeoff):
    """Derive downloads from counts and raw phase timings, not cached summaries."""
    for experiment, cases in [('loss_sweep', decoding), ('accuracy_time', [tradeoff])]:
        for case in cases:
            for name, result in case['decoders'].items():
                row = dict.fromkeys(SUMMARY_FIELDS, '')
                row.update({k:case[k] for k in ['distance','rounds','loss_probability','shots']})
                row.update(experiment=experiment, decoder=name, status=result['status'])
                if result['status'] == 'ok':
                    shots, errors = result['shots'], result['errors']
                    if shots != case['shots'] or not 0 <= errors <= shots:
                        raise ValueError('Invalid summary counts')
                    lo, hi = wilson(errors, shots)
                    times = [sum(run[k] for k in ['compile_seconds','transform_seconds','decode_seconds'])
                             if name.startswith('pymatching') else native_total(run) for run in result['runs']]
                    row.update(errors=errors, logical_error_rate=errors/shots, ci95_low=lo, ci95_high=hi,
                               median_microseconds_per_shot=statistics.median(times)/shots*1e6)
                yield row
