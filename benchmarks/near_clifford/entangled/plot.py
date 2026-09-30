"""Plot paired warm-flat speedups without joining unrelated widths/ranks."""
import argparse
import json
from pathlib import Path
import statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from verify import verify


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('results',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    r=json.loads(a.results.read_text());verify(r)
    fig,axes=plt.subplots(1,3,figsize=(13,4.6),layout='constrained')
    for ax,family in zip(axes,['brick','parity','rounds']):
        cases=[c for c in r['cases'] if c['fixture'].startswith(family+'_')]
        centers=[];low=[];high=[]
        for c in cases:
            def ns(t,label):return t[label]['measurements'][0]['warm_prepared_flat']['median_ns']
            center=statistics.median(ns(t,'baseline') for t in c['runs'])/statistics.median(ns(t,'candidate') for t in c['runs'])
            ratios=[ns(t,'baseline')/ns(t,'candidate') for t in c['runs']]
            centers.append(center);low.append(center-min(ratios));high.append(max(ratios)-center)
        labels=[' / '.join(c['fixture'].split('_')[1:]) for c in cases]
        ax.errorbar(range(len(cases)),centers,yerr=[low,high],fmt='o',color='#0072B2',capsize=3)
        ax.axhline(1,color='#757575',linewidth=1);ax.set_xticks(range(len(cases)),labels,rotation=60,ha='right')
        ax.set_ylim(.85,1.2);ax.grid(axis='y',alpha=.2);ax.set_title(family);ax.set_xlabel('Initial T/T† injections / width / depth or rounds')
        ax.spines[['top','right']].set_visible(False)
    axes[0].set_ylabel('Warm-flat speedup (baseline / candidate)')
    fig.suptitle(f"Entangled workloads · {r['sources']['baseline']['revision'][:8]} → {r['sources']['candidate']['revision'][:8]} · {len(r['cases'])} configurations\n{r['platform']}",fontsize=12)
    fig.supxlabel('Dots: ratio of process-median medians. Whiskers: range of three paired process ratios. >1 is faster.',fontsize=9)
    a.output.parent.mkdir(parents=True,exist_ok=True)
    for suffix in ['.png','.svg']:fig.savefig(a.output.with_suffix(suffix),dpi=170)
    svg=a.output.with_suffix('.svg');svg.write_text('\n'.join(s.rstrip() for s in svg.read_text().splitlines())+'\n')

if __name__=='__main__':main()
