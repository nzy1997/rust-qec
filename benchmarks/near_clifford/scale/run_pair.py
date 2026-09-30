"""Run the retained scale matrix against selected clean source revisions."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import run as campaign

HERE=Path(__file__).resolve().parent

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
    sys.argv=[sys.argv[0],*remaining]
    campaign.main()

if __name__=='__main__':main()
