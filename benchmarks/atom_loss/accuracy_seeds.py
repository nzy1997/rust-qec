"""Prospectively fixed three-seed accuracy experiment; no timing claims."""
import argparse
import json
from pathlib import Path
import zipfile
from .run import ROOT, generate, checked, logical_x, digest, save, score, native_decode, export_graph, python_decode
from .shot_data import SEEDS, validate_dataset, seed_summaries, rescore_seeds
import numpy as np


def pack(work,out):
    report=json.loads((out/'accuracy-seeds.json').read_text())
    payload={'accuracy-seeds.json':(out/'accuracy-seeds.json').read_bytes(),
             'rescore.py':(ROOT/'benchmarks/atom_loss/shot_data.py').read_bytes()}
    for case in report['cases']:
        prefix=f"{case['setting']}-s{case['seed']}"
        for name in ['public/circuit.stim','public/manifest.json','public/shots.b8','private/manifest.json','private/answers.b8','private/masks.b8']+[name+'.b8' for name in case['decoders']]:
            payload[prefix+'/'+name]=(work/prefix/name).read_bytes()
    import hashlib
    payload['index.json']=json.dumps({'sha256':{n:hashlib.sha256(v).hexdigest() for n,v in payload.items()}},indent=2).encode()
    with zipfile.ZipFile(out/'accuracy-seeds.zip','w',compression=zipfile.ZIP_DEFLATED) as z:
        for name,value in sorted(payload.items()):
            info=zipfile.ZipInfo(name,(2026,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,value)
    print(rescore_seeds(out/'accuracy-seeds.zip',out))


def capture_sources(out):
    from .source_contract import capture
    capture(out, 'seeds', {'seeds': SEEDS, 'shots_per_seed': 5000,
                           'timing': 'Accuracy only; no timing inference.'})


def run(work,out):
    capture_sources(out)
    binary=ROOT/'target/release/rustqec';exporter=ROOT/'target/release/examples/export_matching_benchmark'
    report={'seeds':SEEDS,'shots_per_seed':5000,'cases':[]}
    settings=[(f'd{d}-p{p}',d,d,p) for d in [3,5,7] for p in [.0001,.0003,.001,.003,.01]]+[('tradeoff',3,2,.003)]
    for label,d,rounds,p in settings:
        for seed in SEEDS:
            path=work/f'{label}-s{seed}';path.mkdir(parents=True,exist_ok=False)
            circuit=path/'circuit.stim';generate(binary,circuit,d,rounds,p)
            checked([binary,'dataset','export','--circuit',circuit,'--shots',5000,'--seed',seed,
                     '--mode','measurements_blinded','--logical-x-qubits',logical_x(circuit.read_text(),d),
                     '--public-out',path/'public','--private-out',path/'private'])
            answers=np.frombuffer(validate_dataset(lambda n:(path/n).read_bytes()),dtype=np.uint8)
            public=json.loads((path/'public/manifest.json').read_text())
            case={'setting':label,'distance':d,'rounds':rounds,'loss_probability':p,'pauli_probability':.001,'shots':5000,'seed':seed,
                  'dataset_id':public['dataset_id'],'circuit_sha256':digest(path/'public/circuit.stim'),
                  'public_rows_sha256':digest(path/'public/shots.b8'),'answers_sha256':digest(path/'private/answers.b8'),
                  'decoders':{},'paired':{}}
            native,record=native_decode(binary,path,'envelope-matching',0)
            if record['status']!='ok':raise ValueError(record)
            predictions={'envelope-matching':native}
            graph=export_graph(exporter,path,'accuracy')
            for name in ['pymatching-envelope','pymatching-fixed']:
                predictions[name]=python_decode(graph,name=='pymatching-envelope')[0]
            if label=='tradeoff':
                mle,record=native_decode(binary,path,'envelope-mle',0)
                if record['status']!='ok':raise ValueError(record)
                predictions['envelope-mle']=mle
            for name,pred in predictions.items():
                (path/(name+'.b8')).write_bytes(pred.tobytes());case['decoders'][name]=score(pred,answers)
                if name!='envelope-matching':
                    case['paired'][name]={'native_only_wrong':int(np.count_nonzero((native!=answers)&(pred==answers))),
                                          'other_only_wrong':int(np.count_nonzero((native==answers)&(pred!=answers)))}
            report['cases'].append(case);save(out/'accuracy-seeds.json',report)
            print(label,seed,{n:r['errors'] for n,r in case['decoders'].items()},flush=True)
    report['pooled']=seed_summaries(report['cases']);save(out/'accuracy-seeds.json',report);pack(work,out)


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--work',type=Path,required=True);p.add_argument('--out',type=Path,required=True)
    a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);run(a.work,a.out)
