"""Replay all frozen Rust-only zero-noise-span scouts; never collect timings."""
from pathlib import Path
import sys
from verify_coefficient_intern_scouts import ROOT, load, require, verify_campaign

DEFAULT=ROOT/'benchmarks/near_clifford/results/apple-m4-zero-noise-spans-rust-ablation-2026-10-08'
CAMPAIGNS=['first-warm','confirmation-warm','first-cold']

def verify_archive(archive):
    bindings=load(archive/'bindings.json')['campaigns']
    require(set(bindings)==set(CAMPAIGNS),'all warm, confirmation and cold campaigns required')
    for label in CAMPAIGNS:
        verify_campaign(archive,label,bindings[label],schema_prefix='zero-noise-spans')
        print('PASS',label,'source/probe/driver/closure/paired schedule/semantics/statistics',flush=True)

if __name__=='__main__':
    require(len(sys.argv)<=2,'usage: verify_zero_noise_spans_scouts.py [ARCHIVE]')
    verify_archive(Path(sys.argv[1]) if len(sys.argv)==2 else DEFAULT)
