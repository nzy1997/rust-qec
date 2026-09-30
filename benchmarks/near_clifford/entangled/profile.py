"""Profile selected exact entangled circuits after verified timing completes."""
import argparse
import json
from pathlib import Path
import sys
from run import HERE, SCALE, MATRIX, load, sha
from verify import verify


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('results',type=Path)
    p.add_argument('--scratch',type=Path,required=True)
    p.add_argument('--fixtures',nargs='+',required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    if set(a.fixtures)-{n for n,_ in MATRIX}:p.error('unknown profile fixture')
    verify(json.loads(a.results.read_text()),git_sources=True)
    # Reuse the historical profiling implementation with the unified dependency
    # lock. Its separate debug/frame-pointer build remains outside campaign timing.
    module=load(SCALE/'profile.py','entangled_scale_profile')
    class Inputs:
        def __truediv__(self,n): return SCALE/('Cargo.unified.lock' if n=='Cargo.lock' else n)
    module.HERE=Inputs()
    sys.argv=[sys.argv[0],str(a.results),'--scratch',str(a.scratch),'--fixtures',*a.fixtures,'--output',str(a.output)]
    module.main()
    out=a.output/'metadata.json'
    r=json.loads(out.read_text());r['entry_sha256']=sha(HERE/'profile.py');out.write_text(json.dumps(r,indent=2)+'\n')

if __name__=='__main__': main()
