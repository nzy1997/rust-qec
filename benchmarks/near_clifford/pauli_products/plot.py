"""Plot paired ranges for support controls and selected end-to-end families."""
import argparse
import json
import importlib.util
from pathlib import Path
import statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
spec=importlib.util.spec_from_file_location('pauli_plot_entry',Path(__file__).with_name('run.py'))
run=importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    result=json.loads(args.results.read_text())
    support=result.get('schema')=='near-clifford.pauli-products.v1'
    verifier=run.load(run.HERE/'verify.py' if support else run.BASE/'row_ops/verify.py','pauli_plot_verify')
    verifier.verify(result)
    families=['star','chain'] if support else (['brick','parity','rounds'] if result['row_ops_suite']=='entangled' else ['random','rank','combo'])
    fig,axes=plt.subplots(1,len(families),figsize=(4.4*len(families),4.8),layout='constrained')
    for ax,family in zip(axes,families):
        cases=[c for c in result['cases'] if (c['topology']==family if support else c['fixture'].startswith(family+'_'))]
        def time(pair,label):
            return pair[label]['median_ns'] if support else pair[label]['measurements'][0]['warm_prepared_flat']['median_ns']
        centers,low,high=[],[],[]
        for case in cases:
            pairs=case['runs']
            ratios=[time(p,'baseline')/time(p,'candidate') for p in pairs]
            center=statistics.median(time(p,'baseline') for p in pairs)/statistics.median(time(p,'candidate') for p in pairs)
            centers.append(center);low.append(max(0,center-min(ratios)));high.append(max(0,max(ratios)-center))
        labels=[str(c['width']) if support else ' / '.join(c['fixture'].split('_')[1:]) for c in cases]
        ax.errorbar(range(len(cases)),centers,yerr=[low,high],fmt='o',color='#0072B2',capsize=3)
        ax.axhline(1,color='#757575',linewidth=1)
        ax.set_xticks(range(len(cases)),labels,rotation=60,ha='right')
        ax.set_ylim(min(.9,min(c-l for c,l in zip(centers,low))-.05),max(1.1,max(c+h for c,h in zip(centers,high))*1.05))
        ax.grid(axis='y',alpha=.2);ax.set_title(family)
        ax.set_xlabel('Physical width' if support else 'Fixture parameters in retained table')
        ax.spines[['top','right']].set_visible(False)
    axes[0].set_ylabel('Query speedup (baseline / candidate)' if support else 'Warm-flat speedup (baseline / candidate)')
    scope='direct probability controls' if support else result['row_ops_suite']+' selected families'
    fig.suptitle(f"Near-Clifford Pauli reconstruction · {scope}\nApple M4 · {result['sources']['baseline']['revision'][:8]} → {result['sources']['candidate']['revision'][:8]}",fontsize=12)
    fig.supxlabel('Dots: ratio of process-median medians. Whiskers: range of three paired ratios. >1 is faster.',fontsize=9)
    for suffix in ['.png','.svg']: fig.savefig(args.output.with_suffix(suffix),dpi=170)
    svg=args.output.with_suffix('.svg')
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text().splitlines())+'\n')


if __name__=='__main__': main()
