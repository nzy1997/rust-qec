"""Check the published evidence bundle; incomplete runs cannot become accuracy points."""
import argparse
import csv
import hashlib
import json
import math
import itertools
from pathlib import Path
from .artifacts import required_files, TIMING_FILES, CORRECTNESS_FILES, native_total, timing_rows


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
    listed = set(manifest['sha256'])
    required = required_files(root)
    if listed & TIMING_FILES:
        required = required | TIMING_FILES
    if listed & CORRECTNESS_FILES:
        required = required | CORRECTNESS_FILES
    missing = required - listed
    if missing:
        raise ValueError(f'Missing required artifact checksums: {sorted(missing)}')
    for name,expected in manifest['sha256'].items():
        path=root/name
        if path.parent != root or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
            raise ValueError(f'Artifact integrity mismatch: {name}')
    for provenance, snapshot in [('provenance-all.json', 'source-snapshot.json')] + (
            [('provenance-timing.json', 'source-snapshot-timing.json')] if listed & TIMING_FILES else []) + (
            [('provenance-correctness.json', 'source-snapshot-correctness.json')] if listed & CORRECTNESS_FILES else []):
        record = json.loads((root/provenance).read_text())
        source = json.loads((root/snapshot).read_text())
        if source['base_commit'] != record['source_commit']:
            raise ValueError('Source snapshot base differs from provenance')
        for name, expected in record['sources'].items():
            if name not in source['files'] or hashlib.sha256(source['files'][name].encode()).hexdigest() != expected:
                raise ValueError(f'Source snapshot mismatch: {name}')
    for name in ['correctness.json','decoder-correctness.json','chain-correctness.json']:
        data=json.loads((root/name).read_text())
        if data['status']!='PASS': raise ValueError(f'Correctness failed: {name}')
    sampler=json.loads((root/'correctness.json').read_text())
    analytic=sampler['analytic_noise_controls']
    assert analytic['status']=='PASS' and len(analytic['cases'])==16
    assert all(c['status']=='PASS' for c in analytic['cases'])
    assert set(analytic['channel_deletion_mutations'])=={'X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2'}
    assert all(c['rejected'] and c['failed_cases'] for c in analytic['channel_deletion_mutations'].values())
    distribution = analytic['distribution_probes']
    expected_names = {f'DEPOLARIZE{n}_bell_p{p}' for n in [1,2] for p in [0.,.17,.6,1.]}
    expected_names |= {'DEPOLARIZE2_product_' + ''.join(b) for b in itertools.product('XYZ', repeat=2)}
    expected_names |= {f'DEPOLARIZE2_{state}_{q}' for state in ['lost','restored','before_loss'] for q in [0,1]}
    expected_names |= {f'DEPOLARIZE1_product_{b}' for b in 'XYZ'}
    if (distribution['status'] != 'PASS' or len(distribution['cases']) != len(expected_names)
            or {c['case'] for c in distribution['cases']} != expected_names
            or any(c['status'] != 'PASS' for c in distribution['cases'])):
        raise ValueError('Incomplete or failed Pauli channel distribution probes')
    replacements = distribution['channel_replacement_mutations']
    if (set(replacements) != {'DEPOLARIZE2_ix_only','DEPOLARIZE2_xi_only','DEPOLARIZE2_independent_x',
                              'DEPOLARIZE1_x_only','DEPOLARIZE1_z_only'}
            or not all(m['rejected'] and m['failed_cases'] for m in replacements.values())):
        raise ValueError('Missing or escaped wrong-channel mutation')
    chain=json.loads((root/'chain-correctness.json').read_text())
    assert (chain['distance'],chain['rounds'],chain['detectors'])==(3,2,16)
    assert chain['physical_fault_traces']==5996 and chain['rows']>0 and chain['patterns']==4
    assert chain['independent_effects_candidates_and_m2d_pass']
    assert all(chain['compiler_output_mutations_rejected'].values())
    assert set(chain['backends'])=={'envelope-matching','envelope-mle'}
    for result in chain['backends'].values():
        assert not result['rejected_rows'] and result['unique_optimum_rows']>0
        assert all(result[k] for k in ['constant_zero_rejected','constant_one_rejected','flipped_prediction_rejected','placeholder_invariance'])
    assert hashlib.sha256((root/'midswap_d3_r2.stim').read_bytes()).hexdigest()==chain['fixture_sha256']
    oracle=json.loads((root/'decoder-correctness.json').read_text())
    assert [c['rows_checked'] for c in oracle['cases']]==[64,1024]
    for c in oracle['cases']:
        assert not any(c['rejected_rows'].values()) and c['placeholder_invariance_pass']
        assert 0 in c['flipped_prediction_rejected_rows']
    assert 21 in oracle['cases'][1]['ignored_conditioning_rejected_rows']
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
    assert set(tradeoff['decoders'])=={'envelope-matching','envelope-mle','pymatching-fixed','pymatching-envelope','pymatching-fixed-loop'}
    assert tradeoff['decoders']['pymatching-fixed']['prediction_sha256']==tradeoff['decoders']['pymatching-fixed-loop']['prediction_sha256']
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
                for rep,(run,total) in enumerate(zip(r['runs'],r['total_seconds'])):
                    if name.startswith('pymatching'):
                        assert run['export_repetition']==rep
                        assert total==run['compile_seconds']+run['transform_seconds']+run['decode_seconds']
                        if not name.endswith('-loop'):
                            assert run['batch_calls']>0
                            phases=[run[k] for k in ['topology_seconds','preprocess_seconds','graph_build_seconds','matching_seconds','output_seconds','adapter_overhead_seconds']]
                            assert min(phases)>=0 and math.isclose(sum(phases),run['decode_seconds'],rel_tol=1e-9)
                    else:
                        expected = native_total(run)
                        if not math.isfinite(total) or not math.isclose(total, expected, rel_tol=1e-12, abs_tol=0.):
                            raise ValueError(f'Native total differs from compile + decode: {name}')
                        assert run['stats']['attempted_shot_count']==r['shots']
                        assert run['stats']['timeout_count']==run['stats']['infeasible_shot_count']==0
                if 'paired_native_only_wrong' in r:
                    a,b=r['paired_native_only_wrong'],r['paired_python_only_wrong']
                    assert r['errors']-native['errors']==b-a
                    assert a+b==r['disagreements_with_native']
            else:
                assert 'logical_error_rate' not in r and 'errors' not in r and 'total_seconds' not in r
    with (root/'timing-sweep.csv').open() as stream:
        rows = list(csv.DictReader(stream))
    expected = [{key:str(value) for key,value in row.items()} for row in timing_rows(decoding)]
    if rows != expected or len(rows) != 135:
        raise ValueError('Timing sweep CSV differs from complete raw repetitions')
    from .shot_data import rescore, ARCHIVE
    rescore(root/ARCHIVE, root)
    return 'PASS'


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=Path,nargs='?',default=Path('site/static/data/atom-loss'))
    print(verify(p.parse_args().root))
