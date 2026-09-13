"""Shared publication contract and authoritative native timing boundary."""
import math
import json

REQUIRED_FILES = frozenset([
    'chain-correctness.json', 'midswap_d3_r2.stim', 'correctness.json',
    'decoder-correctness.json', 'sampling.json', 'decoding.json', 'tradeoff.json',
    'provenance-all.json', 'methodology.md', 'summary.csv', 'source-snapshot.json',
    'shot-data-v1.zip',
] + [f'{name}.{ext}' for name in [
    'sampling-throughput', 'logical-error-rate', 'logical-error-rate-full',
    'accuracy-time', 'adapter-stages'] for ext in ['svg', 'png']])
TIMING_FILES = frozenset(['provenance-timing.json', 'source-snapshot-timing.json'])


def required_files(root):
    """Either member on disk makes both retiming provenance files mandatory."""
    retimed = any((root/name).exists() for name in TIMING_FILES)
    for name in ['decoding.json', 'tradeoff.json']:
        if (root/name).exists():
            data = json.loads((root/name).read_text())
            cases = data if isinstance(data, list) else [data]
            retimed |= any(case.get('baseline_predictions_unchanged', False) for case in cases)
    return REQUIRED_FILES | (TIMING_FILES if retimed else set())


def native_total(run):
    if run.get('status') != 'ok' or run.get('exit_code') != 0 or 'stats' not in run:
        raise ValueError('Missing successful native timing stats')
    phases = [run['stats'][key] for key in ['compile_seconds', 'decode_seconds']]
    if any(not math.isfinite(value) or value < 0 for value in phases):
        raise ValueError('Invalid native phase time')
    return sum(phases)
