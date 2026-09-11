"""Remeasure decoding on retained, hash-checked corpora without changing samples."""
import argparse
from datetime import datetime, timezone
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
from .run import ROOT, cpu_model, decoder_case, digest, save
from .decoder_reference import run as check_decoder


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--corpora', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=False)
    args.out.mkdir(parents=True, exist_ok=True)
    binary = ROOT/'target/release/rustqec'
    exporter = ROOT/'target/release/examples/export_matching_benchmark'
    sources = {str(p.relative_to(ROOT)):p.read_text() for p in sorted((ROOT/'benchmarks/atom_loss').glob('*.py'))}
    base = subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    save(args.out/'source-snapshot-timing.json', {'base_commit':base,'files':sources})
    save(args.out/'provenance-timing.json', {
        'started_utc':datetime.now(timezone.utc).isoformat(),'command':sys.argv,'source_commit':base,
        'working_tree_dirty':True,'os':platform.platform(),'cpu':cpu_model(),'python':sys.version,
        'dependencies':{p:importlib.metadata.version(p) for p in ['stim','numpy','pymatching','matplotlib']},
        'binaries':{str(p.relative_to(ROOT)):digest(p) for p in [binary,exporter]},
        'sources':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'benchmarks/atom_loss').glob('*.py'))},
        'environment':{k:os.environ.get(k) for k in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','RAYON_NUM_THREADS']},
        'baseline_sha256':{name:digest(args.baseline/name) for name in ['decoding.json','tradeoff.json']},
        'timing':'Serial processes, three cold decoder runs, no CPU affinity. Each Python repetition freshly exports and times compilation and public-row transformation, then times array conversion, grouping, graph construction, decode_batch and output reordering. Native totals sum fresh compile and decode stats including buffered reads and packing/flush. Export transformation includes input reads; JSON transport/loading, process startup and scoring excluded. Fixed Python loop retained as API control; batch and loop predictions must match.',
        'sampling':'Unchanged; provenance-all.json and source-snapshot.json describe the original sampling run.'})
    result = check_decoder(binary, exporter)
    assert result['status']=='PASS'
    save(args.out/'decoder-correctness.json', result)
    for filename in ['decoding.json','tradeoff.json']:
        old = json.loads((args.baseline/filename).read_text())
        cases = old if isinstance(old,list) else [old]
        results = []
        for case in cases:
            label = f"d{case['distance']}-p{case['loss_probability']}" if filename=='decoding.json' else 'tradeoff'
            source = args.corpora/label
            for path,key in [('circuit.stim','circuit_sha256'),('public/shots.b8','public_rows_sha256'),('private/answers.b8','answers_sha256')]:
                assert digest(source/path)==case[key], f'Corpus mismatch: {label}/{path}'
            work = args.work/label
            work.mkdir()
            shutil.copyfile(source/'circuit.stim',work/'circuit.stim')
            for kind in ['public','private']:
                shutil.copytree(source/kind,work/kind)
            fresh = decoder_case(binary,exporter,work,case['distance'],case['rounds'],case['loss_probability'],
                                 case['shots'],case['seed'],3,include_mle=filename=='tradeoff.json',reuse=True)
            for key in ['dataset_id','circuit_sha256','public_rows_sha256','answers_sha256']:
                assert fresh[key]==case[key]
            for name, previous in case['decoders'].items():
                current = fresh['decoders'][name]
                assert current['status']=='ok', current
                assert current['prediction_sha256']==previous['prediction_sha256'], f'Prediction changed: {label}/{name}'
            fresh['baseline_predictions_unchanged']=True
            results.append(fresh)
            save(args.out/filename, results if isinstance(old,list) else fresh)
            print(f'{label}: all corpus and prediction hashes unchanged; fresh timings complete',flush=True)


if __name__=='__main__':
    main()
