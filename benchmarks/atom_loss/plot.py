"""Render three publication figures directly from recorded runs, never fitted data."""
import argparse
import csv
import json
from pathlib import Path
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from .verify import require_complete_sweep

STYLE = {
    'envelope-matching': ('RustQEC envelope', '#b95428','o','-'),
    'pymatching-envelope': ('PyMatching + envelope (batch)', '#386b80','s','--'),
    'pymatching-fixed': ('PyMatching, fixed (batch)', '#797471','^',':'),
    'pymatching-fixed-loop': ('PyMatching, fixed (loop)', '#a99374','x',':'),
    'envelope-mle': ('RustQEC envelope MLE', '#754c91','D','-'),
}
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':11,'axes.titlesize':13,
                     'axes.labelsize':11,'axes.spines.top':False,'axes.spines.right':False,
                     'axes.edgecolor':'#a5a19c','axes.labelcolor':'#292522','text.color':'#292522',
                     'xtick.color':'#605b56','ytick.color':'#605b56','grid.color':'#e5e0da',
                     'svg.fonttype':'path','savefig.facecolor':'white'})


def emit(fig, out, name):
    for ext in ['svg','png']:
        fig.savefig(out/f'{name}.{ext}',dpi=180,bbox_inches='tight',metadata={'Creator':'RustQEC atom-loss benchmark'} if ext=='svg' else {})
    svg=out/f'{name}.svg'
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text().splitlines())+'\n')
    plt.close(fig)


def render(out):
    sampling=json.loads((out/'sampling.json').read_text())
    decoding=json.loads((out/'decoding.json').read_text())
    require_complete_sweep(decoding)
    tradeoff=json.loads((out/'tradeoff.json').read_text())
    fig,ax=plt.subplots(figsize=(8.4,4.4),layout='constrained')
    for key,label,color,marker in [('rust','RustQEC (loss-visible records)','#b95428','o'),
                                    ('reference','Stim reference + Python loss lowering','#386b80','s')]:
        x,y,lower,upper=[],[],[],[]
        for case in sampling:
            rates=np.array([case['shots']/(r['sample_seconds']+r['packing_seconds']) for r in case[key]['records']])
            median=np.median(rates)
            x.append(case['distance']);y.append(median);lower.append(median-rates.min());upper.append(rates.max()-median)
        ax.errorbar(x,y,yerr=[lower,upper],label=label,color=color,marker=marker,capsize=4,lw=1.8)
    ax.set(yscale='log',xlabel='Code distance d (rounds = d)',ylabel='Samples / second',xticks=[3,5,7],
           title='Loss-visible sampling and b8 packing')
    ax.grid(axis='y',which='major');ax.legend(loc='best',frameon=False,fontsize=10)
    fig.get_layout_engine().set(rect=(0,0.07,1,1))
    fig.text(.5,.015,f"{sampling[0]['shots']} shots / batch · pPauli = 0.001 · pLoss = 0.003 · median and full range of 3 runs",ha='center',fontsize=9,color='#605b56')
    emit(fig,out,'sampling-throughput')
    displayed_cases=[c for c in decoding if c['distance'] in (3,5)]
    fig,axes=plt.subplots(1,2,figsize=(11.2,4.7),sharey=True,layout='constrained')
    for ax,distance in zip(axes,[3,5]):
        cases=sorted([c for c in decoding if c['distance']==distance],key=lambda c:c['loss_probability'])
        for name in ['envelope-matching','pymatching-envelope','pymatching-fixed']:
            label,color,marker,line=STYLE[name]
            xs,ys,lower,upper=[],[],[],[]
            for case in cases:
                result=case['decoders'].get(name,{})
                if result.get('status')!='ok': continue
                xs.append(case['loss_probability'])
                if result['errors'] == 0:
                    # Omit zero-failure points without joining across their gaps.
                    ys.append(np.nan);lower.append(np.nan);upper.append(np.nan)
                else:
                    ys.append(result['logical_error_rate'])
                    lower.append(ys[-1]-result['wilson_95'][0]);upper.append(result['wilson_95'][1]-ys[-1])
            ax.errorbar(xs,ys,yerr=[lower,upper],label=label,color=color,marker=marker,ls=line,lw=1.5,markersize=4,capsize=2)
        ax.set(xscale='log',yscale='log',title=f'd = {distance}, rounds = {distance}',xlabel='Loss probability per opportunity')
        ax.grid(axis='y')
        visible_x=sorted({c['loss_probability'] for c in cases
                          if any(r['status']=='ok' and r['errors']>0 for r in c['decoders'].values())})
        tick_labels=[]
        for value in visible_x:
            coefficient,exponent=f'{value:.0e}'.split('e')
            prefix='' if coefficient=='1' else coefficient + r'\times '
            tick_labels.append('$' + prefix + '10^{' + str(int(exponent)) + '}$')
        ax.set_xticks(visible_x,labels=tick_labels)
        # Equal fractional margins in log space, using only displayed points.
        log_min,log_max=np.log10([visible_x[0],visible_x[-1]])
        padding=.05*(log_max-log_min)
        ax.set_xlim(10**(log_min-padding),10**(log_max+padding))
    axes[0].set_ylabel('Logical failure probability')
    ymax=max(r['wilson_95'][1] for c in displayed_cases for r in c['decoders'].values())*1.1
    ymin=min(r['wilson_95'][0] for c in displayed_cases for r in c['decoders'].values() if r['errors'] > 0)*.7
    axes[0].set_ylim(ymin,ymax)
    handles,labels=axes[0].get_legend_handles_labels()
    fig.legend(handles,labels,loc='upper center',bbox_to_anchor=(.5,1.0),ncol=3,frameon=False,fontsize=10)
    fig.get_layout_engine().set(rect=(0,.07,1,.80))
    fig.text(.5,.01,f"{decoding[0]['shots']:,} shared shots / point · pPauli = 0.001 · 95% Wilson intervals · no threshold fit\nDisplay: d = 3, 5 only; zero-failure points omitted. Full data retained in CSV.",ha='center',fontsize=9,color='#605b56')
    emit(fig,out,'logical-error-rate')
    # All distances and opportunities remain visible; zero observations are limits.
    fig,axes=plt.subplots(1,3,figsize=(12.4,4.8),sharey=True,layout='constrained')
    for ax,distance in zip(axes,[3,5,7]):
        cases=sorted([c for c in decoding if c['distance']==distance],key=lambda c:c['loss_probability'])
        for name in ['envelope-matching','pymatching-envelope','pymatching-fixed']:
            label,color,marker,line=STYLE[name]
            xs=[c['loss_probability'] for c in cases]
            rates=[c['decoders'][name]['logical_error_rate'] if c['decoders'][name]['errors'] else np.nan for c in cases]
            ax.plot(xs,rates,label=label,color=color,marker=marker,ls=line,markersize=4)
            for c in cases:
                r=c['decoders'][name]; x=c['loss_probability']
                if r['errors']:
                    p=r['logical_error_rate'];lo,hi=r['wilson_95']
                    ax.errorbar(x,p,yerr=[[p-lo],[hi-p]],color=color,capsize=2)
                else:
                    upper=-np.expm1(np.log(.05)/r['shots'])
                    ax.errorbar(x,upper,yerr=upper*.3,uplims=True,color=color,marker=marker,markersize=4)
        ax.set(xscale='log',yscale='log',title=f'd = {distance}, rounds = {distance}',
               xlabel='Loss probability per opportunity',xlim=(.00008,.0125))
        ax.set_xticks([.0001,.001,.01],labels=['$10^{-4}$','$10^{-3}$','$10^{-2}$'])
        ax.grid(axis='y')
    axes[0].set_ylabel('Logical failure probability / upper limit')
    handles,labels=axes[0].get_legend_handles_labels()
    fig.legend(handles,labels,loc='upper center',ncol=3,frameon=False,fontsize=9)
    fig.get_layout_engine().set(rect=(0,.10,1,.86))
    fig.text(.5,.015,'All 15 settings · 5,000 shared shots / point · nonzero: 95% Wilson intervals\nDown arrows: zero failures, one-sided exact 95% upper limit (not an estimated failure rate). Coincident limits overlap.',
             ha='center',fontsize=9,color='#605b56')
    emit(fig,out,'logical-error-rate-full')
    fig,ax=plt.subplots(figsize=(8.4,4.8),layout='constrained')
    failed=[]
    for i,(name,result) in enumerate(tradeoff['decoders'].items()):
        label,color,marker,_line=STYLE[name]
        if result['status']!='ok':
            failed.append(label+': incomplete (no accuracy point)');continue
        times=np.array(result['total_seconds'])/result['shots']*1e6
        median=np.median(times);p=result['logical_error_rate'];lo,hi=result['wilson_95']
        ax.errorbar(median,p,xerr=[[median-times.min()],[times.max()-median]],yerr=[[p-lo],[hi-p]],
                    color=color,marker=marker,markersize=8,capsize=4,ls='none',label=label)
        timing_label = f'{median:.2f} µs' if median < 10 else f'{median:,.0f} µs'
        ax.annotate(timing_label, (median,p), xytext=(-8,9) if name=='envelope-mle' else (8,3),
                    textcoords='offset points', ha='right' if name=='envelope-mle' else 'left',
                    fontsize=9, color=color)
    ax.set(xscale='log',xlabel='Amortized compile + decode time (µs / shot)',ylabel='Logical failure probability / experiment',
           title=f"Accuracy and time on the same {tradeoff['shots']:,} shots",ylim=(0,None))
    ax.grid(axis='y');ax.legend(loc='best',frameon=False,fontsize=10)
    fig.get_layout_engine().set(rect=(0,.12,1,1))
    fig.text(.5,.035,f"Mid-SWAP d = 3, rounds = 2 · pPauli = 0.001 · pLoss = 0.003\n95% Wilson intervals; timing median and range of 3 cold-cache runs",ha='center',fontsize=9,color='#605b56')
    if failed: fig.text(.5,.005,'; '.join(failed),ha='center',fontsize=8,color='#9c392a')
    emit(fig,out,'accuracy-time')
    with (out/'summary.csv').open('w') as f:
        writer=csv.DictWriter(f,lineterminator='\n',fieldnames=['experiment','distance','rounds','loss_probability','decoder','status','shots','errors','logical_error_rate','ci95_low','ci95_high','median_microseconds_per_shot'])
        writer.writeheader()
        for experiment,cases in [('loss_sweep',decoding),('accuracy_time',[tradeoff])]:
            for case in cases:
                for name,result in case['decoders'].items():
                    row={k:case[k] for k in ['distance','rounds','loss_probability','shots']}
                    row.update(experiment=experiment,decoder=name,status=result['status'])
                    if result['status']=='ok':
                        row.update(errors=result['errors'],logical_error_rate=result['logical_error_rate'],
                                   ci95_low=result['wilson_95'][0],ci95_high=result['wilson_95'][1],
                                   median_microseconds_per_shot=np.median(result['total_seconds'])/result['shots']*1e6)
                    writer.writerow(row)


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss'))
    render(p.parse_args().out)
