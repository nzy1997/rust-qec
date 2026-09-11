"""Render and seal a completed evidence bundle for the static documentation site."""
import argparse
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import shutil
from .plot import render
from .run import ROOT, save


def publish(out):
    render(out)
    shutil.copyfile(ROOT/'benchmarks/atom_loss/README.md',out/'methodology.md')
    shutil.copyfile(ROOT/'benchmarks/atom_loss/fixtures/midswap_d3_r2.stim',out/'midswap_d3_r2.stim')
    files=['chain-correctness.json','midswap_d3_r2.stim','correctness.json','decoder-correctness.json','sampling.json','decoding.json','tradeoff.json',
           'provenance-all.json','methodology.md','summary.csv','source-snapshot.json']
    files += [name for name in ['provenance-timing.json','source-snapshot-timing.json'] if (out/name).exists()]
    files += [f'{name}.{ext}' for name in ['sampling-throughput','logical-error-rate','logical-error-rate-full','accuracy-time','adapter-stages'] for ext in ['svg','png']]
    save(out/'bundle.json',{'completed_utc':datetime.now(timezone.utc).isoformat(),
                           'sha256':{name:hashlib.sha256((out/name).read_bytes()).hexdigest() for name in files}})


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss'))
    publish(p.parse_args().out)
