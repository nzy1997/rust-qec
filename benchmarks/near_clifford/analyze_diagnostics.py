"""Derive tables from independently verified, source-bound benchmark events."""
import argparse
import csv
import gzip
import json
from pathlib import Path
import statistics
import subprocess
import sys

HERE=Path(__file__).resolve().parent
BACKENDS=['rstim','clifft','clifft-scheduled','symft']


def median(values): return statistics.median(values)


def table(path,rows):
    if not rows: return
    keys=list(rows[0])
    with path.open('w',newline='') as f:
        writer=csv.DictWriter(f,fieldnames=keys);writer.writeheader();writer.writerows(rows)


def summarize(out,kind):
    verification=json.loads(subprocess.check_output([sys.executable,str(HERE/kind/'verify.py'),str(out),'--git-sources'],text=True))
    header=json.loads((out/'header.json').read_text())
    data=(out/'events.jsonl').read_bytes() if (out/'events.jsonl').exists() else gzip.decompress((out/'events.jsonl.gz').read_bytes())
    events=[json.loads(line) for line in data.splitlines()]
    timing={}
    for event in events:
        if event['kind']=='timing': timing[(event['id'],event['pair'],event['backend'])]=event['result']
    cells=header['cases'] if kind=='application_counts' else [c for c in header['manifest']['cells'] if c['id'] in header['manifest']['selected_cells']]
    pairs=header['pairs'] if kind=='application_counts' else header['manifest']['pairs']
    rows=[];comparisons=[]
    for cell in cells:
        raw={backend:[timing.get((cell['id'],pair,backend),{}) for pair in range(pairs)] for backend in BACKENDS}
        def warm(result,backend):
            if kind=='application_counts': return median(o['ns_per_call'] for o in result['observations'])
            return median(o['ns_per_call'] for o in result['warm']) if backend=='rstim' else median(result['warm_ns'])
        if not all(all(r.get('status')=='ok' or 'warm_ns' in r for r in values) for values in raw.values()): continue
        process={backend:[warm(r,backend) for r in values] for backend,values in raw.items()}
        medians={backend:median(values) for backend,values in process.items()}
        winner=min(BACKENDS[1:],key=medians.get)
        ratios=[p/r for p,r in zip(process[winner],process['rstim'])]
        comparisons.append(dict(id=cell['id'],shots=cell['shots'],arithmetic=cell.get('policy',cell.get('arithmetic')),
            cache_bytes=cell.get('cache_bytes'),fastest_peer=winner,rstim_ns=medians['rstim'],peer_ns=medians[winner],
            speedup=medians[winner]/medians['rstim'],paired_min=min(ratios),paired_max=max(ratios)))
        for backend,results in raw.items():
            phases={};accepted_rate=None;acceptance=None
            if kind=='application_counts':
                phases={phase:median(r[phase] for r in results) for phase in ['compile_ns','prepare_ns','first_ns']}
                accepted_rate=median(median(o['accepted']*1e9/o['elapsed_ns'] for o in r['observations']) for r in results)
                acceptance=median(sum(o['accepted'] for o in r['observations'])/sum(o['attempted'] for o in r['observations']) for r in results)
            elif backend=='rstim':
                phases={phase:median(median(c[phase] for c in r['cold']) for r in results) for phase in ['compile_ns','prepare_ns','first_ns']}
            else:
                phases=dict(compile_ns=median(r['compile_ns'] for r in results),prepare_ns=0,
                            first_ns=median(median(r['first_ns']) for r in results))
            rows.append(dict(id=cell['id'],backend=backend,shots=cell['shots'],arithmetic=cell.get('policy',cell.get('arithmetic')),
                cache_budget_bytes=cell.get('cache_bytes'),warm_ns=medians[backend],process_min_ns=min(process[backend]),
                process_max_ns=max(process[backend]),attempted_shots_per_s=cell['shots']*1e9/medians[backend],
                accepted_shots_per_s=accepted_rate,acceptance_rate=acceptance,**phases,
                peak_active_rank=results[0].get('peak_active_rank',results[0].get('peak_active_width')),
                peak_rss_bytes=max(r['peak_rss_bytes'] for r in results) if all('peak_rss_bytes' in r for r in results) else None,
                cache_reserved_bytes=max(r['cache_reserved_bytes'] for r in results) if all('cache_reserved_bytes' in r for r in results) else None))
    histories={}
    for event in events:
        if event['kind'] not in ['lifetime','peer-lifetime'] or event['result'].get('status')!='ok': continue
        if event['kind']=='lifetime':
            _,_,name,budget,policy=event['id'].split('/');backend='rstim';budget=int(budget[1:])
        else: name=event['id'];budget=None;policy=event['arithmetic_context'];backend=event['backend']
        histories.setdefault((name,policy,budget,backend),[]).append(event['result'])
    lifecycle=[]
    for (name,policy,budget,backend),results in histories.items():
        phases={key:median(median(h[key] for h in result['histories']) for result in results)
                for key in ['compile_ns','prepare_ns','sampling_ns','phase_sum_ns']}
        total_shots=sum(request['shots'] for request in header['manifest']['histories'][name])
        lifecycle.append(dict(history=name,arithmetic=policy,cache_budget_bytes=budget,backend=backend,
            total_shots=total_shots,**phases,peak_rss_bytes=max(r['peak_rss_bytes'] for r in results),
            cache_reserved_bytes=max(r.get('cache_reserved_bytes',0) for r in results)))
    capabilities=[dict(input=e['name'],arithmetic=e['policy'],status=e['result'].get('status'),
                       error=e['result'].get('error',e['result'].get('stderr','')),
                       input_sha256=header['manifest']['inputs'][e['name']]['sha256'])
                  for e in events if e['kind']=='capability']
    summary=dict(source_revision=header['source_revision'],host=header['host'],verification=verification,
        statistical_unit='median of observations within each independent process; process medians summarized across rounds',
        warm_comparisons=comparisons,lifecycle=lifecycle,capabilities=capabilities)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    table(out/'warm.csv',rows);table(out/'comparisons.csv',comparisons);table(out/'lifecycle.csv',lifecycle);table(out/'capability.csv',capabilities)
    lines=['# Verified near-Clifford diagnostic measurements','',
        f"Measured source: `{header['source_revision']}`. Host: `{header['host']['platform']}`.",
        f"Verification: `{json.dumps(verification,sort_keys=True)}`.",
        'Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.',
        'Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.',
        'Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.',
        'OS RSS is whole-process high-water, including probe/interpreter allocations. Mac measurements have no pinned-core claim.',
        'Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.',
        'Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.',
        'See warm.csv for all backends, cold phases, actual rank, throughput and RSS; lifecycle.csv for every history/budget.', '',
        '| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |',
        '| --- | ---: | ---: | ---: | --- | --- |']
    for value in comparisons:
        lines.append(f"| {value['id']} | {value['rstim_ns']/1000:.3f} | {value['peer_ns']/1000:.3f} | {value['speedup']:.3f}× | {value['paired_min']:.3f}–{value['paired_max']:.3f} | {value['fastest_peer']} |")
    if kind=='application_counts':
        lines+=['','Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.',
            'Rust builds full structured records then filters/counts; peers use native counts/early rejection.',
            'This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.',
            'Original-circuit capability failures are retained in capability.csv and events, without gate lowering.']
    (out/'analysis.md').write_text('\n'.join(lines)+'\n')
    return verification


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('out',type=Path)
    p.add_argument('--kind',choices=['diagnostics','application_counts'],default='diagnostics')
    args=p.parse_args();print(json.dumps(summarize(args.out,args.kind),sort_keys=True))
