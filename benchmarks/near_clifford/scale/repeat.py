"""Confirm one suspect configuration with the campaign's exact timing binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from datetime import datetime,timezone

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('campaign',type=Path);p.add_argument('fixture');p.add_argument('shots',type=int)
    p.add_argument('--scratch',type=Path,default=ROOT/'drafts/near-clifford-scale')
    p.add_argument('--pairs',type=int,default=6);p.add_argument('--repetitions',type=int,default=3)
    p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    if a.pairs<1 or a.repetitions<1 or a.shots<0:p.error('invalid count')
    r=json.loads(a.campaign.read_text())
    if 'completed_utc' not in r:p.error('campaign incomplete')
    binaries={l:a.scratch/l/'near-clifford-scale' for l in ['baseline','candidate']}
    for l,b in binaries.items():
        if sha(b)!=r['sources'][l]['binary_sha256']:p.error('binary mismatch')
    result={'campaign_sha256':sha(a.campaign),'fixture':a.fixture,'shots':a.shots,'sources':r['sources'],
            'created_utc':datetime.now(timezone.utc).isoformat(),'runs':[]}
    a.output.parent.mkdir(parents=True,exist_ok=True)
    for pair in range(a.pairs):
        order=list(binaries) if pair%2==0 else list(reversed(binaries));run={'order':order}
        for label in order:
            print(pair+1,label,flush=True)
            run[label]=json.loads(subprocess.check_output([str(binaries[label]),a.fixture,str(a.shots),str(a.repetitions)],text=True,timeout=300))
        result['runs'].append(run);a.output.write_text(json.dumps(result,indent=2)+'\n')
    result['completed_utc']=datetime.now(timezone.utc).isoformat();a.output.write_text(json.dumps(result,indent=2)+'\n')

if __name__=='__main__':main()
