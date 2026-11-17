#!/usr/bin/env python3
"""Verify the pinned code-switching curves, TQD terminal comparisons, and timing trials."""

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
TERMINAL_KERNEL = '47a2213f443ccee840442a88164697f6c5f31c40'
TQD_SCALES = ((3, 5), (4, 6), (5, 8))
TQD_RATES = ('.003', '.005', '.01', '.015', '.02')
TQD_DRAWS = (1024, 1024, 1024, 1536, 1536)
TQD_METRICS = {
    'matched_physical': 'terminal_closure',
    'matched_author': 'approximation',
    'full': 'full_terminal_closure',
    'paired_difference': 'after_minus_approximation',
}


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


def terminal_curves(read, paper_kernel):
    campaign = read('raw/tqd/terminal-summary.json')
    published = read('data/tqd-comparison.json')
    history = campaign['campaign']
    require(published['status'] == history['status'] == 'complete'
            and published['kernel_commit'] == history['kernel_commit'] == paper_kernel
            and history['checkout_commit'] == TERMINAL_KERNEL and history['physical_draws'] == 18432,
            'deposited TQD terminal campaign identity differs')
    grid = [(size, depth, rate, draws, 202610080000 + size * 100000 + index * 10000)
            for size, depth in TQD_SCALES
            for index, (rate, draws) in enumerate(zip(TQD_RATES, TQD_DRAWS, strict=True))]
    points = []
    for job, summary, figure, (size, depth, rate, draws, seed) in zip(
            history['jobs'], campaign['points'], published['points'], grid, strict=True):
        name = f'tqd-L{size}-p{rate}'
        require(job['name'] == summary['name'] == name and job['status'] == 'completed'
                and job['exit_code'] == 0 and job['size'] == figure['distance'] == size
                and job['p'] == rate and Fraction(str(figure['p'])) == Fraction(rate)
                and job['shots'] == figure['shots'] == draws and job['seed'] == figure['seed'] == seed,
                'deposited TQD terminal draw design differs: ' + name)
        folder = f'raw/tqd/terminal/{name}'
        manifest = read(f'{folder}/manifest.json')
        result = read(f'{folder}/result.json')
        require(manifest == summary['manifest'] and manifest['kernel_commit'] == TERMINAL_KERNEL
                and result['status'] == 'complete' and result['summary'] == summary['summary']
                and result['draws_sha256'] == summary['draws_sha256'] == figure['draws_sha256']
                and manifest['requested'] == draws and manifest['seed'] == seed
                and result['seconds'] == summary['sampling_seconds']
                and result['preparation_seconds'] == summary['preparation_seconds'],
                'deposited TQD terminal result linkage differs: ' + name)
        metadata = manifest['metadata']
        recovery = metadata['terminal_recovery']
        require(metadata['size'] == size and metadata['depth'] == depth
                and metadata['px'] == metadata['pz'] == rate and metadata['noise_window'] == 'author'
                and metadata['branch'] == '0' and metadata['branch_target_norm'] == '5/8'
                and metadata['charge_decoder'] == 'naive' and metadata['source_hashes'] == figure['input_hashes']
                and recovery['kind'] == 'noiseless-charge-round' and recovery['rounds'] == 1
                and recovery['charge_depth'] == depth + 1 and recovery['memory_depth'] == depth
                and set(metadata['author_source_hashes'])
                == {'geometry.py', 'lattice.py', 'decoder.py', 'twisted.py', 'runner.py'},
                'deposited TQD terminal task semantics differ: ' + name)
        measured = result['summary']
        require(measured['shots'] == draws and measured['flux_success'] == figure['flux_success']
                and 0 < measured['flux_success'] <= draws and figure['estimates'].keys() == TQD_METRICS.keys(),
                'deposited TQD terminal normalization differs: ' + name)
        for curve, source in TQD_METRICS.items():
            raw, plotted = measured[source], figure['estimates'][curve]
            matches(plotted['mean'], raw['mean'], name + ' ' + curve + ' mean')
            matches(plotted['se'], raw['se'], name + ' ' + curve + ' uncertainty')
            require(plotted['se'] >= 0, 'negative TQD uncertainty: ' + name)
            if curve in ('matched_physical', 'matched_author'):
                require(plotted['zero_observed'] is (raw['mean'] == 0), 'changed TQD zero flag: ' + name)
        matches(measured['after_minus_approximation']['mean'],
                measured['terminal_closure']['mean'] - measured['approximation']['mean'],
                name + ' paired terminal-minus-author mean')
        points.append(dict(distance=size, depth=depth, p=str(Fraction(rate)), draws=draws, seed=seed,
                           flux_success=measured['flux_success'], estimates=figure['estimates'],
                           reported_draws_sha256=result['draws_sha256'],
                           author_source_hashes=metadata['author_source_hashes'],
                           input_hashes=metadata['source_hashes'],
                           historical_native_sha256=manifest['native_sha256'],
                           branch_target_norm=metadata['branch_target_norm']))
    return dict(kernel=TERMINAL_KERNEL, paper_kernel=paper_kernel, physical_draws=sum(p['draws'] for p in points),
                scope='deposited_summaries_and_draw_references', points=points)


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
        terminal = terminal_curves(read, paper['kernel_commit'])
    return dict(schema='rmagna.paper-decoder-workloads-historical.v1',
                archive_sha256=paper['archive_sha256'], paper=paper['arxiv'],
                scope='historical_archive_only', raw_sha256=used, curves=curves, timings=timings,
                tqd_terminal=terminal)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect(args.archive), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
