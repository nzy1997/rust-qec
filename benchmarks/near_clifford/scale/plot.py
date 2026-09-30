"""Plot normalized warm-flat timings from a completed paired campaign."""
import argparse
import json
from pathlib import Path
import statistics
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results',type=Path)
    parser.add_argument('--title',help='override the figure title for selected revisions')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();r=json.loads(args.results.read_text())
    if 'completed_utc' not in r: parser.error('campaign incomplete')
    groups=[[c for c in r['cases'] if c['fixture'].startswith(prefix)] for prefix in ['random_','rank_','combo_']]
    fig,axes=plt.subplots(1,3,figsize=(13,3.9),layout='constrained')
    for ax,cases,title in zip(axes,groups,['Independent random measurements','Independent T axes','Random prefix before active measurements']):
        positions=range(len(cases))
        for label,color,marker in [('baseline','#757575','s'),('candidate','#0072B2','o')]:
            medians=[];lows=[];highs=[]
            for c in cases:
                samples=[run[label]['measurements'][0]['warm_prepared_flat']['median_ns']/c['shots']/1000 for run in c['runs']]
                medians.append(statistics.median(samples));lows.append(min(samples));highs.append(max(samples))
            ax.plot(positions,medians,color=color,marker=marker,label=label,linewidth=1.7,markersize=4)
            ax.fill_between(positions,lows,highs,color=color,alpha=.15,linewidth=0)
        labels=[c['fixture'].removeprefix('random_').removeprefix('rank_').removeprefix('combo_').replace('_',' / ') for c in cases]
        ax.set_xticks(list(positions),labels,rotation=45 if title.startswith('Random prefix') else 0)
        ax.set_yscale('log');ax.grid(axis='y',alpha=.2);ax.set_title(title,fontsize=10)
        ax.set_xlabel('rank / random width' if title.startswith('Random prefix') else 'width' if title.startswith('Independent random') else 'prepared rank')
        ax.spines[['top','right']].set_visible(False)
    axes[0].set_ylabel('Warm flat µs / shot (log scale)');axes[0].legend(frameon=False,fontsize=9)
    fig.suptitle(args.title or '#764 → #766 · Apple M4 · paired process medians',fontsize=12)
    fig.supxlabel('Shading: range of three process medians. Rank ≥12 uses 8–16 shots; combinations use 16–64.',fontsize=9)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    fig.savefig(args.output.with_suffix('.svg'));fig.savefig(args.output.with_suffix('.png'),dpi=170)

if __name__=='__main__':main()
