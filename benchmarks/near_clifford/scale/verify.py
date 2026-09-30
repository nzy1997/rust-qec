"""Check campaign completeness, retained-source hashes, and diagnostic consistency."""
import argparse
import hashlib
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('results',type=Path)
    p.add_argument('--binaries',type=Path,help='optional campaign scratch directory')
    a=p.parse_args();r=json.loads(a.results.read_text())
    assert r.get('completed_utc') and not r['quick'], 'campaign incomplete or smoke only'
    assert len(r['cases'])==len(r['matrix'])==43, 'missing configurations'
    assert len(r['verification'])==35, 'missing circuits'
    assert [(c['fixture'],c['shots']) for c in r['cases']]==[tuple(c) for c in r['matrix']]
    assert sha(HERE/'main.rs')==r['harness_sha256'], 'harness hash differs'
    assert sha(HERE/'run.py')==r['runner_sha256'], 'runner hash differs'
    assert {p.name:sha(p) for p in (HERE/'fixtures').iterdir()}==r['fixtures_sha256']
    for label in ['baseline','candidate']:
        assert len(r['diagnostics'][label])==35
        assert r['sources'][label]['lock_sha256']==sha(HERE/'Cargo.lock')
        assert all(x['semantic_verification']=='pass' for x in r['diagnostics'][label].values())
        for row in r['diagnostics'][label].values():
            assert len(row['counters'])==len(r['counter_names'])
            assert len(row['initial'])==len(r['snapshot_names'])
            assert row['after_probe'][2]<=row['after_probe'][3]
        if a.binaries:
            assert sha(a.binaries/label/'near-clifford-scale')==r['sources'][label]['binary_sha256']
    for case in r['cases']:
        assert len(case['runs'])==r['pairs']==3
        for run in case['runs']:
            assert sorted(run['order'])==['baseline','candidate']
            assert run['candidate']['circuit']==run['baseline']['circuit']
            for label in ['baseline','candidate']:
                m=run[label]['measurements'][0];assert m['shots']==case['shots']
                for key,value in m.items():
                    if key!='shots':assert len(value['raw_ns'])==r['repetitions']==3
    print('PASS: 43 configurations, 35 cross-revision circuit checks, 70 diagnostic checks; hashes, repetitions and node budgets consistent')

if __name__=='__main__':main()
