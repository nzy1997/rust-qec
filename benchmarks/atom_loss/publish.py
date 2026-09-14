"""Render and seal a completed evidence bundle for the static documentation site."""
import argparse
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import shutil
from .plot import render
from .artifacts import required_files
from .run import ROOT, save


def publish(out):
    from .source_contract import verify_bundle_source
    verify_bundle_source(out)
    render(out)
    shutil.copyfile(ROOT/'benchmarks/atom_loss/README.md',out/'methodology.md')
    shutil.copyfile(ROOT/'benchmarks/atom_loss/fixtures/midswap_d3_r2.stim',out/'midswap_d3_r2.stim')
    files = sorted(required_files(out))
    save(out/'bundle.json',{'completed_utc':datetime.now(timezone.utc).isoformat(),
                           'sha256':{name:hashlib.sha256((out/name).read_bytes()).hexdigest() for name in files}})


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss'))
    publish(p.parse_args().out)
