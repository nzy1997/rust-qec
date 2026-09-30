"""Run the retained scale matrix against selected clean source revisions."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import statistics
import run as campaign

HERE=Path(__file__).resolve().parent

def report(result):
    lines=['# Paired near-Clifford scale campaign','',
           f"Host: {result['platform']}; {result['rustc']}; {result['created_utc']}",
           f"Pristine baseline `{result['sources']['baseline']['revision']}` versus candidate `{result['sources']['candidate']['revision']}`.",
           f"{result['pairs']} paired process invocations × {result['repetitions']} repetitions per mode.",
           'Speedup = baseline / candidate; n/a means a duration was below clock resolution.',
           'RSS includes validation, multiple live samplers and outputs; it is not single-cache memory.',
           '', '| Fixture | Shots | Cold speedup | Warm flat speedup | Candidate warm flat ms | Candidate RSS MiB |',
           '| --- | ---: | ---: | ---: | ---: | ---: |']
    for case in result['cases']:
        def med(label,mode):
            return statistics.median([run[label]['measurements'][0][mode]['median_ns'] for run in case['runs']])
        def ratio(mode):
            baseline,candidate=med('baseline',mode),med('candidate',mode)
            return f'{baseline/candidate:.2f}×' if baseline>0 and candidate>0 else 'n/a'
        rss=statistics.median([run['candidate']['peak_rss_bytes'] for run in case['runs']])/1048576
        lines.append(f"| {case['fixture']} | {case['shots']} | {ratio('cold_prepared_structured')} | {ratio('warm_prepared_flat')} | {med('candidate','warm_prepared_flat')/1e6:.4f} | {rss:.1f} |")
    return '\n'.join(lines)+'\n'


def main():
    parser=argparse.ArgumentParser(add_help=False)
    parser.add_argument('--baseline',default='f4b5966129aee3d617181414f805eb1250e1cc5d')
    parser.add_argument('--candidate',default='HEAD')
    options,remaining=parser.parse_known_args()
    campaign.REVISIONS={label:subprocess.check_output(['git','rev-parse',revision+'^{commit}'],cwd=campaign.ROOT,text=True).strip()
                        for label,revision in [('baseline',options.baseline),('candidate',options.candidate)]}
    if campaign.REVISIONS['baseline']==campaign.REVISIONS['candidate']:parser.error('revisions must differ')
    if '--scratch' not in remaining:remaining+=['--scratch',str(campaign.ROOT/'drafts/near-clifford-pauli')]
    save=campaign.atomic_save
    def save_with_entry(path,result):
        result['campaign_entry_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        result['campaign_entry']='run_pair.py'
        save(path,result)
    campaign.atomic_save=save_with_entry
    campaign.report=report
    sys.argv=[sys.argv[0],*remaining]
    campaign.main()

if __name__=='__main__':main()
