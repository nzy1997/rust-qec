"""Derive full retained timing tables and de-duplicated inclusive profile counts."""
import argparse
import json
import importlib.util
from pathlib import Path
import re
import statistics

spec=importlib.util.spec_from_file_location('pauli_report_entry',Path(__file__).with_name('run.py'))
run=importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)


def table(result):
    support=result.get('schema')=='near-clifford.pauli-products.v1'
    lines=['# Pauli reconstruction '+('support controls' if support else result['row_ops_suite']+' campaign'),'',
           f"Baseline `{result['sources']['baseline']['revision']}`; candidate `{result['sources']['candidate']['revision']}`.",
           'Apple M4; three alternating process pairs, three repetitions. Paired ranges are not confidence intervals.','']
    if support:
        lines += ['1024 direct Y-probability queries per repetition after 32 warmup queries; preparation/physics/counters untimed.','',
                  '| Frame | Width | Selected rows | Entries | Baseline ms | Candidate ms | Speedup | Paired range |',
                  '| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |']
    else:
        lines += ['All six timing modes are retained in JSON; this table shows warmed flat results. RSS is whole-process high-water memory.','',
                  '| Fixture | Shots | Baseline ms | Candidate ms | Speedup | Paired range | Candidate RSS MiB |',
                  '| --- | ---: | ---: | ---: | ---: | --- | ---: |']
    for c in result['cases']:
        def ns(pair,label):
            return pair[label]['median_ns'] if support else pair[label]['measurements'][0]['warm_prepared_flat']['median_ns']
        a=statistics.median(ns(p,'baseline') for p in c['runs'])/1e6
        b=statistics.median(ns(p,'candidate') for p in c['runs'])/1e6
        ratios=[ns(p,'baseline')/ns(p,'candidate') for p in c['runs'] if ns(p,'baseline')>0 and ns(p,'candidate')>0]
        speedup=f'{a/b:.3f}×' if a>0 and b>0 else 'n/a (zero duration)'
        spread=f'{min(ratios):.3f}–{max(ratios):.3f}×' if len(ratios)==3 else 'n/a (zero duration)'
        common=f'{a:.4f} | {b:.4f} | {speedup} | {spread}'
        if support:
            work=result['diagnostics']['candidate'][f"{c['topology']}_{c['width']}"]['row_work']
            lines.append(f"| {c['topology']} | {c['width']} | {work[0]} | {work[1]} | {common} |")
        else:
            rss=statistics.median(p['candidate']['peak_rss_bytes'] for p in c['runs'])/1048576
            lines.append(f"| {c['fixture']} | {c['shots']} | {common} | {rss:.1f} |")
    return '\n'.join(lines)+'\n'


def screen(result):
    hits=[];zero=[]
    for c in result['cases']:
        for mode in ['unprepared','cold_prepared_structured','first_prepared_structured','first_prepared_flat','warm_prepared_structured','warm_prepared_flat']:
            if any(p['baseline']['measurements'][0][mode]['median_ns']==0 for p in c['runs']):
                zero.append({'fixture':c['fixture'],'shots':c['shots'],'mode':mode})
                continue
            ratios=[p['candidate']['measurements'][0][mode]['median_ns']/p['baseline']['measurements'][0][mode]['median_ns'] for p in c['runs']]
            if statistics.median(ratios)>=1.15 and min(ratios)>=1.10:
                hits.append({'fixture':c['fixture'],'shots':c['shots'],'mode':mode,'paired_slowdowns':ratios})
    return {'hits':hits,'zero_duration_exclusions':zero}


def profile_counts(path):
    tokens=['project_active_measurement','pauli_probability_zero','single_qubit_pauli','reconstruct_physical_pauli','Pauli::multiply_tableau_row',
            'absorb_independent_measurement','row_mult_near_clifford','apply_clifford','canonicalize_active_axes','rebase_origin_into_frame','canonical_snapshot']
    counts=dict.fromkeys(tokens,0);stack=[];active=False;total=None;graph=False
    for line in path.read_text().splitlines():
        if line.strip()=='Call graph:':
            graph=True
            continue
        if graph and line.startswith(('Total number in stack','Sort by top of stack','Binary images')):
            break
        if not graph: continue
        match=re.match(r'^([ +!:|]*)(\d+) (.*)$',line)
        if not match: continue
        prefix,count,name=match.groups();count=int(count)
        if name.startswith('Thread_'):
            if active: break
            active='com.apple.main-thread' in name
            if active: total=count
            continue
        if not active: continue
        depth=len(prefix)
        while stack and stack[-1][0]>=depth: stack.pop()
        for token in tokens:
            if token+'::' in name and not any(token+'::' in parent for _,parent in stack): counts[token]+=count
        stack.append((depth,name))
    if not total: raise ValueError('missing main-thread call graph')
    return {'main_samples':total,'inclusive_counts':counts}


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results',type=Path,nargs='?')
    parser.add_argument('--profiles',type=Path)
    args=parser.parse_args()
    if args.profiles:
        print(json.dumps({p.stem.removesuffix('.sample'):profile_counts(p) for p in args.profiles.glob('*.sample.txt')},indent=2))
    else:
        result=json.loads(args.results.read_text())
        verifier=run.load(run.HERE/'verify.py' if result.get('schema')=='near-clifford.pauli-products.v1' else run.BASE/'row_ops/verify.py','pauli_report_verify')
        verifier.verify(result)
        print(table(result),end='')
