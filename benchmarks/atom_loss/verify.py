"""Check the published evidence bundle; incomplete runs cannot become accuracy points."""
import argparse
import hashlib
import json
from pathlib import Path


def require_complete_sweep(cases):
    expected={(d,p) for d in [3,5,7] for p in [.0001,.0003,.001,.003,.01]}
    if len(cases)!=15 or {(c['distance'],c['loss_probability']) for c in cases}!=expected:
        raise ValueError('Loss sweep is incomplete; keep raw failures and do not publish a partial curve')
    for case in cases:
        decoders=case.get('decoders',{})
        if set(decoders)!={'envelope-matching','pymatching-fixed','pymatching-envelope'}:
            raise ValueError('Missing loss-sweep comparator')
        if any(r.get('status')!='ok' for r in decoders.values()):
            raise ValueError('Loss sweep includes failed runs; retain raw records without publishing a partial curve')


def verify(root):
    manifest=json.loads((root/'bundle.json').read_text())
    for name,expected in manifest['sha256'].items():
        path=root/name
        if path.parent != root or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
            raise ValueError(f'Artifact integrity mismatch: {name}')
    for name in ['correctness.json','decoder-correctness.json']:
        data=json.loads((root/name).read_text())
        if data['status']!='PASS': raise ValueError(f'Correctness failed: {name}')
    sampling=json.loads((root/'sampling.json').read_text())
    assert [c['distance'] for c in sampling]==[3,5,7]
    for c in sampling:
        for backend in ['rust','reference']:
            assert len(c[backend]['records'])==3
            for r in c[backend]['records']:
                assert r['sample_seconds']>0 and r['packing_seconds']>=0 and r['bytes']>0
        assert c['rust']['records'][0]['bytes']==c['reference']['records'][0]['bytes']
    decoding=json.loads((root/'decoding.json').read_text())
    require_complete_sweep(decoding)
    tradeoff=json.loads((root/'tradeoff.json').read_text())
    assert set(tradeoff['decoders'])=={'envelope-matching','envelope-mle','pymatching-fixed','pymatching-envelope'}
    for c in decoding+[tradeoff]:
        assert c['shots']==5000 and c['decoders'] and 'export_failure' not in c
        native=c['decoders']['envelope-matching']
        for name,r in c['decoders'].items():
            if r['status']=='ok':
                assert r['shots']==c['shots'] and 0<=r['errors']<=r['shots']
                assert r['logical_error_rate']==r['errors']/r['shots']
                assert r['wilson_95'][0]<=r['logical_error_rate']<=r['wilson_95'][1]
                assert r['wilson_95'][1]>0 and len(r['runs'])==len(r['total_seconds'])==3
                assert min(r['total_seconds'])>0
                for run in r['runs']:
                    if 'stats' in run:
                        assert run['stats']['attempted_shot_count']==r['shots']
                        assert run['stats']['timeout_count']==run['stats']['infeasible_shot_count']==0
                if 'paired_native_only_wrong' in r:
                    a,b=r['paired_native_only_wrong'],r['paired_python_only_wrong']
                    assert r['errors']-native['errors']==b-a
                    assert a+b==r['disagreements_with_native']
            else:
                assert 'logical_error_rate' not in r and 'errors' not in r and 'total_seconds' not in r
    return 'PASS'


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=Path,nargs='?',default=Path('site/static/data/atom-loss'))
    print(verify(p.parse_args().root))
