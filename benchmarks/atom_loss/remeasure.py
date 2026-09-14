"""Remeasure decoding on retained, hash-checked corpora without changing samples."""
from .shot_data import require
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
    from .source_contract import capture
    capture(args.out, 'timing', {'baseline_sha256': {name: digest(args.baseline/name)
            for name in ['decoding.json', 'tradeoff.json']},
            'timing': 'Serial three-repetition decoding remeasurement on retained corpora. Same boundaries as run.py.'})
    result = check_decoder(binary, exporter)
    require((result['status']=='PASS'), "remeasure: result['status']=='PASS'")
    save(args.out/'decoder-correctness.json', result)
    from . import correctness, chain_reference
    for name,check in [('correctness',lambda:correctness.run(binary)),('chain-correctness',lambda:chain_reference.run(binary,exporter))]:
        report=check();save(args.out/(name+'.json'),report);require((report['status']=='PASS'), "remeasure: report['status']=='PASS'")
    for filename in ['decoding.json','tradeoff.json']:
        old = json.loads((args.baseline/filename).read_text())
        cases = old if isinstance(old,list) else [old]
        results = []
        for case in cases:
            label = f"d{case['distance']}-p{case['loss_probability']}" if filename=='decoding.json' else 'tradeoff'
            source = args.corpora/label
            for path,key in [('circuit.stim','circuit_sha256'),('public/shots.b8','public_rows_sha256'),('private/answers.b8','answers_sha256')]:
                require((digest(source/path)==case[key]), f'Corpus mismatch: {label}/{path}')
            work = args.work/label
            work.mkdir()
            shutil.copyfile(source/'circuit.stim',work/'circuit.stim')
            for kind in ['public','private']:
                shutil.copytree(source/kind,work/kind)
            fresh = decoder_case(binary,exporter,work,case['distance'],case['rounds'],case['loss_probability'],
                                 case['shots'],case['seed'],3,include_mle=filename=='tradeoff.json',reuse=True)
            for key in ['dataset_id','circuit_sha256','public_rows_sha256','answers_sha256']:
                require((fresh[key]==case[key]), 'remeasure: fresh[key]==case[key]')
            for name, previous in case['decoders'].items():
                current = fresh['decoders'][name]
                require((current['status']=='ok'), current)
                require((current['prediction_sha256']==previous['prediction_sha256']), f'Prediction changed: {label}/{name}')
            fresh['baseline_predictions_unchanged']=True
            results.append(fresh)
            save(args.out/filename, results if isinstance(old,list) else fresh)
            print(f'{label}: all corpus and prediction hashes unchanged; fresh timings complete',flush=True)


if __name__=='__main__':
    main()
