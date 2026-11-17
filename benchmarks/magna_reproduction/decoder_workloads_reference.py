#!/usr/bin/env python3
"""Verify the pinned code-switching curves and decoder workload timing trials."""

import argparse
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
from statistics import median
import zipfile

import reference
from remaining_contracts import require


CODE_SWITCHING = {
    'dk': (184, 2026101100, (1 << 18,) * 3),
    'wg-S': (730, 2026101200, (1 << 22, 1 << 21, 1 << 22)),
    'wg-T': (730, 2026101300, (1 << 22, 1 << 21, 1 << 22)),
}
PHYSICAL_RATES = (Fraction(1, 1000), Fraction(3, 1000), Fraction(1, 100))
PROPOSAL_RATES = (PHYSICAL_RATES[1], PHYSICAL_RATES[1], PHYSICAL_RATES[2])
TIMING = {
    'dk': ('raw/overview/dk', 1024, 1),
    'wg-T': ('raw/overview/wg', 1024, 0),
    'tqd3': ('raw/tqd/timing/tqd3', 256, 19),
    'tqd4': ('raw/tqd/timing/tqd4', 256, 22),
    'tqd5': ('raw/tqd/timing/tqd5', 256, 28),
}
BORN_SEED_OFFSET = 100_000
TIMING_SEED = 2026100800


def number(value, label):
    require(type(value) in (int, float) and math.isfinite(value), 'invalid ' + label)
    return value


def matches(actual, expected, label):
    require(math.isclose(number(actual, label), expected, rel_tol=2e-12, abs_tol=1e-25),
            'deposited ' + label + ' differs')


def precision(read, family, design):
    sites, seed, shot_counts = design
    source = read(f'raw/code-switching/precision-{family}/summary.json')
    require(source['status'] == 'complete' and source['case'] == family and len(source['rows']) == 3,
            'deposited code-switching precision case differs: ' + family)
    points = []
    for index, (row, p, q, shots) in enumerate(
            zip(source['rows'], PHYSICAL_RATES, PROPOSAL_RATES, shot_counts, strict=True)):
        fields = (Fraction(str(row['p'])), Fraction(str(row['proposal'])),
                  row['shots'], row['seed'], row['born_seed'], row['noise_categories'])
        require(fields == (p, q, shots, seed + index, seed + BORN_SEED_OFFSET + index, sites),
                'deposited code-switching draw design differs: ' + family)
        a, e = [number(value, 'contribution sum') / shots for value in row['sums']]
        (aa, ae), (ea, ee) = row['cross_products']
        require(0 < a <= 1 and 0 <= e <= a and ae == ea
                and type(row['positive']) is int and 0 <= row['positive'] <= shots,
                'invalid code-switching contributions')
        variance_a, covariance, variance_e = (
            (number(total, 'contribution cross product') - shots * left * right) / (shots * (shots - 1))
            for total, left, right in ((aa, a, a), (ae, a, e), (ee, e, e))
        )
        error = e / a
        variance_error = (variance_e + error ** 2 * variance_a - 2 * error * covariance) / a ** 2
        require(variance_a >= 0 and variance_e >= 0 and variance_error >= 0,
                'invalid deposited code-switching variance')
        for field, expected in (('acceptance', a), ('joint_error', e), ('conditional_error', error),
                                ('acceptance_se', math.sqrt(variance_a)),
                                ('conditional_error_se', math.sqrt(variance_error))):
            matches(row[field], expected, family + ' ' + field)
        seconds = number(row['seconds'], 'code-switching total seconds')
        require(seconds > 0, 'nonpositive code-switching total seconds')
        points.append(dict(p=str(p), q=str(q), draws=shots, noise_seed=seed + index,
                           born_seed=seed + BORN_SEED_OFFSET + index, positive=row['positive'],
                           acceptance=a, accepted_error=e, conditional_error=error,
                           variance_acceptance=variance_a, variance_accepted_error=variance_e,
                           covariance=covariance, variance_conditional_error=variance_error,
                           seconds=seconds))
    return dict(noise_categories=sites, points=points)


def timing(read, family, design):
    folder, shots, calls_per_draw = design
    source = read(f'{folder}/result.json')
    require(source['status'] == 'complete' and len(source['trials']) == 3,
            'deposited decoder timing trial count differs: ' + family)
    config = source['config']
    if family.startswith('tqd'):
        require(config['task'] == 'branch-0.task.json' and config['shots'] == shots
                and config['warmup'] == 1 and config['repeats'] == 3 and config['seed'] == TIMING_SEED,
                'deposited TQD timing configuration differs: ' + family)
    else:
        path = 'dk-recovered' if family == 'dk' else family
        require(config == {'input': 'examples/paper-numerics/inputs/' + path, 'dk': family == 'dk'},
                'deposited code-switching timing input differs: ' + family)
    trials = []
    for index, trial in enumerate(source['trials']):
        raw = read(f'{folder}/draw-batch-{index}.json')
        require(raw['status'] == 'complete' and raw['requested'] == raw['completed'] == trial['shots'] == shots
                and raw['failed'] == raw['not_attempted'] == 0 and not raw['failures']
                and raw['seed'] == TIMING_SEED + index
                and raw['timings'] == trial['timings'] and raw['estimates'] == trial['estimates']
                and raw['standard_errors'] == trial['standard_errors']
                and trial['decoder_calls'] == shots * calls_per_draw,
                'deposited decoder timing batch differs: ' + family)
        seconds = number(trial['seconds'], 'outer wall seconds')
        decoder = number(trial['decoder_seconds'], 'subtracted decoder seconds')
        require(0 <= decoder < seconds and ((decoder > 0) == (calls_per_draw > 0)),
                'invalid deposited outer timing or decoder duration: ' + family)
        # The archive reports both an internal timing breakdown and the measured
        # callback portion of the outer trial. Subtract the latter in each trial.
        trials.append(dict(seed=raw['seed'], attempts=shots, decoder_calls=trial['decoder_calls'],
                           wall_seconds=seconds, decoder_seconds=decoder, internal_timings=raw['timings'],
                           seconds_per_attempt=seconds / shots,
                           decoder_subtracted_seconds_per_attempt=(seconds - decoder) / shots))
    return dict(input_hashes=source['input_hashes'], config=config, trials=trials,
                median_seconds_per_attempt=median(trial['seconds_per_attempt'] for trial in trials),
                median_decoder_subtracted_seconds_per_attempt=median(
                    trial['decoder_subtracted_seconds_per_attempt'] for trial in trials))


def inspect(archive_path, pinned=None):
    """Return historical statistics from the pinned archive, without running Rust draws."""
    pinned = reference.load_manifest() if pinned is None else pinned
    paper = pinned['paper']
    require(reference.digest(archive_path) == paper['archive_sha256'], 'decoder workload archive changed')
    used = {}
    with zipfile.ZipFile(archive_path) as archive:
        def read(name):
            data = archive.read('magna-data-deposit/' + name)
            digest = hashlib.sha256(data).hexdigest()
            require(digest == paper['files'].get(name), 'deposited decoder workload member changed: ' + name)
            used[name] = digest
            return json.loads(data)

        curves = {family: precision(read, family, design) for family, design in CODE_SWITCHING.items()}
        timings = {family: timing(read, family, design) for family, design in TIMING.items()}
    return dict(schema='rmagna.paper-decoder-workloads-historical.v1',
                archive_sha256=paper['archive_sha256'], paper=paper['arxiv'],
                scope='historical_archive_only', raw_sha256=used, curves=curves, timings=timings)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect(args.archive), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
