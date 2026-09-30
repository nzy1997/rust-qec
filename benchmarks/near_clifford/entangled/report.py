"""Render medians for the entangled campaign, keeping the scale timing contract."""
import statistics

def report(r):
    lines=['# Paired entangled near-Clifford campaign','',f"Host: {r['platform']}; {r['rustc']}; {r['created_utc']}",
        f"Baseline `{r['sources']['baseline']['revision']}`; candidate `{r['sources']['candidate']['revision']}`.",
        f"{r['pairs']} paired processes × {r['repetitions']} repetitions in each of six modes.",
        'Speedup is the ratio of the medians of process medians. RSS includes validation, samplers and outputs.',
        '', '| Fixture | Shots | Cold speedup | Warm flat speedup | Candidate warm flat ms | Candidate RSS MiB |',
        '| --- | ---: | ---: | ---: | ---: | ---: |']
    for c in r['cases']:
        def med(label,mode): return statistics.median(t[label]['measurements'][0][mode]['median_ns'] for t in c['runs'])
        def ratio(mode):
            b,a=med('baseline',mode),med('candidate',mode)
            return f'{b/a:.2f}×' if a and b else 'n/a'
        rss=statistics.median(t['candidate']['peak_rss_bytes'] for t in c['runs'])/1048576
        lines.append(f"| {c['fixture']} | {c['shots']} | {ratio('cold_prepared_structured')} | {ratio('warm_prepared_flat')} | {med('candidate','warm_prepared_flat')/1e6:.4f} | {rss:.1f} |")
    return '\n'.join(lines)+'\n'
