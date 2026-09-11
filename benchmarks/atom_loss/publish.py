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
    files=['correctness.json','decoder-correctness.json','sampling.json','decoding.json','tradeoff.json',
           'provenance-all.json','methodology.md','summary.csv','source-snapshot.json']
    files += [f'{name}.{ext}' for name in ['sampling-throughput','logical-error-rate','accuracy-time'] for ext in ['svg','png']]
    save(out/'bundle.json',{'completed_utc':datetime.now(timezone.utc).isoformat(),
                           'sha256':{name:hashlib.sha256((out/name).read_bytes()).hexdigest() for name in files}})


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss'))
    publish(p.parse_args().out)
