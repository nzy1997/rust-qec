"""Check the published evidence bundle; incomplete runs cannot become accuracy points."""
from .shot_data import require
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
from .artifacts import require_complete_sweep, required_files, TIMING_FILES, CORRECTNESS_FILES, native_total, timing_rows, summary_rows, SUMMARY_FIELDS, wilson


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
    for provenance, snapshot in [('provenance-all.json', 'source-snapshot.json'),('provenance-seeds.json','source-snapshot-seeds.json')] + (
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
    from .report_contract import verify_sampler
    verify_sampler(sampler)
    if sampler['low_probability_controls']['real_circuit']['fixture_sha256']!=hashlib.sha256((root/'midswap_d3_r2.stim').read_bytes()).hexdigest():
        raise ValueError('Real-circuit sampling fixture mismatch')
    chain=json.loads((root/'chain-correctness.json').read_text())
    from .chain_contract import verify_chain
    verify_chain(chain)
    require((hashlib.sha256((root/'midswap_d3_r2.stim').read_bytes()).hexdigest()==chain['fixture_sha256']), "verify: hashlib.sha256((root/'midswap_d3_r2.stim').read_bytes()).hexdigest()==chain['fixture_sha256']")
    oracle=json.loads((root/'decoder-correctness.json').read_text())
    require(([c['rows_checked'] for c in oracle['cases']]==[64,1024]), "verify: [c['rows_checked'] for c in oracle['cases']]==[64,1024]")
    for c in oracle['cases']:
        require((not any(c['rejected_rows'].values()) and c['placeholder_invariance_pass']), "verify: not any(c['rejected_rows'].values()) and c['placeholder_invariance_pass']")
        require((0 in c['flipped_prediction_rejected_rows']), "verify: 0 in c['flipped_prediction_rejected_rows']")
    require((21 in oracle['cases'][1]['ignored_conditioning_rejected_rows']), "verify: 21 in oracle['cases'][1]['ignored_conditioning_rejected_rows']")
    def duration(value, *, positive=False):
        if type(value) not in [int,float] or not math.isfinite(value) or (value<=0 if positive else value<0):
            raise ValueError('Invalid finite phase duration')
    sampling=json.loads((root/'sampling.json').read_text())
    require(([c['distance'] for c in sampling]==[3,5,7]), "verify: [c['distance'] for c in sampling]==[3,5,7]")
    for c in sampling:
        if (c['shots']!=256 or c['rounds']!=c['distance'] or c['pauli_probability']!=.001 or c['loss_probability']!=.003
                or c['rust']['shots']!=c['shots'] or c['rust']['warmups']!=2 or c['reference']['warmups']!=1):
            raise ValueError('Sampling workload metadata mismatch')
        duration(c['rust']['parse_seconds'])
        for backend in ['rust','reference']:
            require((len(c[backend]['records'])==3), "verify: len(c[backend]['records'])==3")
            for r in c[backend]['records']:
                duration(r['sample_seconds'],positive=True);duration(r['packing_seconds'])
                require((type(r['bytes']) is int and r['bytes']>0), "verify: type(r['bytes']) is int and r['bytes']>0")
        require((c['rust']['records'][0]['bytes']==c['reference']['records'][0]['bytes']), "verify: c['rust']['records'][0]['bytes']==c['reference']['records'][0]['bytes']")
    decoding=json.loads((root/'decoding.json').read_text())
    require_complete_sweep(decoding)
    import zipfile
    from .shot_data import circuit_layout, ARCHIVE
    with zipfile.ZipFile(root/ARCHIVE) as archive:
        for case in sampling:
            d=case['distance'];circuit=archive.read(f'd{d}-p0.003/public/circuit.stim')
            if hashlib.sha256(circuit).hexdigest()!=case['circuit_sha256']:
                raise ValueError('Sampling circuit differs from corresponding archived sweep circuit')
            expected_bytes=((circuit_layout(circuit.decode())[0]+7)//8)*case['shots']
            if any(r['bytes']!=expected_bytes for backend in ['rust','reference'] for r in case[backend]['records']):
                raise ValueError('Sampling byte count differs from circuit layout and shots')
    tradeoff=json.loads((root/'tradeoff.json').read_text())
    require((set(tradeoff['decoders'])=={'envelope-matching','envelope-mle','pymatching-fixed','pymatching-envelope','pymatching-fixed-loop','envelope-matching-offline'}), "verify: set(tradeoff['decoders'])=={'envelope-matching','envelope-mle','pymatching-fixed','pymatching-envelope','pymatching-fixed-loop','envelope-matching-offline'}")
    require((tradeoff['decoders']['pymatching-fixed']['prediction_sha256']==tradeoff['decoders']['pymatching-fixed-loop']['prediction_sha256']), "verify: tradeoff['decoders']['pymatching-fixed']['prediction_sha256']==tradeoff['decoders']['pymatching-fixed-loop']['prediction_sha256']")
    for c in decoding+[tradeoff]:
        require(c.get('pauli_probability') == .001, 'Declared Pauli probability differs from fixed workload')
        require((c['shots']==5000 and c['decoders'] and 'export_failure' not in c), "verify: c['shots']==5000 and c['decoders'] and 'export_failure' not in c")
        require(c['seed']==(20260912 if c is tradeoff else 20260911), 'Predeclared original seed')
        native=c['decoders']['envelope-matching']
        require(c['decoders']['envelope-matching-offline']['prediction_sha256']==native['prediction_sha256'],'Offline predictions')
        for name,r in c['decoders'].items():
            if r['status']=='ok':
                require((type(r['shots']) is int and type(r['errors']) is int and r['shots']==c['shots'] and 0<=r['errors']<=r['shots']), "verify: type(r['shots']) is int and type(r['errors']) is int and r['shots']==c['shots'] and 0<=r['errors']<=r['shots']")
                require((r['logical_error_rate']==r['errors']/r['shots']), "verify: r['logical_error_rate']==r['errors']/r['shots']")
                require((r['wilson_95']==wilson(r['errors'],r['shots'])), "verify: r['wilson_95']==wilson(r['errors'],r['shots'])")
                require((r['wilson_95'][1]>0 and len(r['runs'])==len(r['total_seconds'])==3), "verify: r['wilson_95'][1]>0 and len(r['runs'])==len(r['total_seconds'])==3")
                for value in r['total_seconds']:duration(value,positive=True)
                for rep,(run,total) in enumerate(zip(r['runs'],r['total_seconds'])):
                    if name.startswith('pymatching'):
                        for key in ['compile_seconds','transform_seconds','decode_seconds']:duration(run[key])
                        duration(run['write_seconds'])
                        require((run['export_repetition']==rep), "verify: run['export_repetition']==rep")
                        require((total==run['compile_seconds']+run['transform_seconds']+run['decode_seconds']), "verify: total==run['compile_seconds']+run['transform_seconds']+run['decode_seconds']")
                        if not name.endswith('-loop'):
                            require((run['batch_calls']>0), "verify: run['batch_calls']>0")
                            phases=[run[k] for k in ['topology_seconds','preprocess_seconds','graph_build_seconds','matching_seconds','output_seconds','adapter_overhead_seconds']]
                            require((min(phases)>=0 and math.isclose(sum(phases),run['decode_seconds'],rel_tol=1e-9)), "verify: min(phases)>=0 and math.isclose(sum(phases),run['decode_seconds'],rel_tol=1e-9)")
                    else:
                        if name == 'envelope-matching-offline':
                            batch=run['batch']
                            for key in ['graph_build_seconds','matching_seconds','write_seconds','batch_overhead_seconds']:
                                duration(batch[key])
                            require(math.isclose(sum(batch[k] for k in ['graph_build_seconds','matching_seconds','write_seconds','batch_overhead_seconds']),batch['decode_seconds'],rel_tol=1e-9),'Offline phases')
                            require(run['stats']['decode_seconds']==run['transform_seconds']+batch['decode_seconds'],'Offline total')
                            require(batch['graph_builds']==c['graph']['loss_patterns'],'Offline graph count')
                        expected = native_total(run)
                        if not math.isfinite(total) or not math.isclose(total, expected, rel_tol=1e-12, abs_tol=0.):
                            raise ValueError(f'Native total differs from compile + decode: {name}')
                        require((run['stats']['attempted_shot_count']==r['shots']), "verify: run['stats']['attempted_shot_count']==r['shots']")
                        require((run['stats']['timeout_count']==run['stats']['infeasible_shot_count']==0), "verify: run['stats']['timeout_count']==run['stats']['infeasible_shot_count']==0")
                if 'paired_native_only_wrong' in r:
                    a,b=r['paired_native_only_wrong'],r['paired_python_only_wrong']
                    require((r['errors']-native['errors']==b-a), "verify: r['errors']-native['errors']==b-a")
                    require((a+b==r['disagreements_with_native']), "verify: a+b==r['disagreements_with_native']")
            else:
                require(('logical_error_rate' not in r and 'errors' not in r and 'total_seconds' not in r), "verify: 'logical_error_rate' not in r and 'errors' not in r and 'total_seconds' not in r")
    with (root/'timing-sweep.csv').open() as stream:
        rows = list(csv.DictReader(stream))
    expected = [{key:str(value) for key,value in row.items()} for row in timing_rows(decoding)]
    if rows != expected or len(rows) != 180:
        raise ValueError('Timing sweep CSV differs from complete raw repetitions')
    with (root/'summary.csv').open() as stream:
        reader = csv.DictReader(stream)
        rows = list(reader)
        fields = reader.fieldnames
    expected = [{key:str(value) for key,value in row.items()} for row in summary_rows(decoding,tradeoff)]
    if fields != SUMMARY_FIELDS or rows != expected or len(rows) != 66:
        raise ValueError('Summary CSV differs from complete raw counts and phase timings')
    from .shot_data import rescore, ARCHIVE
    rescore(root/ARCHIVE, root)
    from .shot_data import rescore_seeds
    rescore_seeds(root/'accuracy-seeds.zip',root)
    from .source_contract import verify_bundle_source
    verify_bundle_source(root)
    from .figure_contract import verify_presentation
    verify_presentation(root)
    return 'PASS'


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=Path,nargs='?',default=Path('site/static/data/atom-loss'))
    print(verify(p.parse_args().root))
