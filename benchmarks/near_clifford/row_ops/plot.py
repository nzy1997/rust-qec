"""Plot paired process ratios with complete whisker ranges and dynamic limits."""
import argparse
import json
from pathlib import Path
import statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from verify import verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = json.loads(args.results.read_text())
    verify(result)
    families = ['brick', 'parity', 'rounds'] if result['row_ops_suite'] == 'entangled' else ['random', 'rank', 'combo']
    fig, axes = plt.subplots(1, 3, figsize=(13, 4.8), layout='constrained')
    for ax, family in zip(axes, families):
        cases = [case for case in result['cases'] if case['fixture'].startswith(family + '_')]
        low, high, centers = [], [], []
        for case in cases:
            def ns(run, label):
                return run[label]['measurements'][0]['warm_prepared_flat']['median_ns']
            ratios = [ns(run, 'baseline') / ns(run, 'candidate') for run in case['runs']]
            center = statistics.median(ns(run, 'baseline') for run in case['runs']) / statistics.median(ns(run, 'candidate') for run in case['runs'])
            centers.append(center)
            low.append(center - min(ratios))
            high.append(max(ratios) - center)
        labels = [' / '.join(case['fixture'].split('_')[1:]) for case in cases]
        ax.errorbar(range(len(cases)), centers, yerr=[low, high], fmt='o', color='#0072B2', capsize=3)
        ax.axhline(1, color='#757575', linewidth=1)
        ax.set_xticks(range(len(cases)), labels, rotation=60, ha='right')
        ax.set_ylim(min(.9, min(c - l for c, l in zip(centers, low)) - .05), max(1.1, max(c + h for c, h in zip(centers, high)) * 1.05))
        ax.grid(axis='y', alpha=.2)
        ax.set_title(family)
        ax.set_xlabel('Fixture parameters in retained table')
        ax.spines[['top', 'right']].set_visible(False)
    axes[0].set_ylabel('Warm-flat speedup (baseline / candidate)')
    fig.suptitle(f"Near-Clifford row operations · {result['row_ops_suite']} · {result['sources']['baseline']['revision'][:8]} → {result['sources']['candidate']['revision'][:8]}\nApple M4 · synthetic workloads", fontsize=12)
    fig.supxlabel('Dots: ratio of process-median medians. Whiskers: range of three paired process ratios. >1 is faster.', fontsize=9)
    for suffix in ['.png', '.svg']:
        fig.savefig(args.output.with_suffix(suffix), dpi=170)
    svg = args.output.with_suffix('.svg')
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text().splitlines()) + '\n')


if __name__ == '__main__':
    main()
