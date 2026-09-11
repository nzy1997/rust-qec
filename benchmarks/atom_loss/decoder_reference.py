"""Exhaustive, hand-specified three-wire matching oracle (not a loss compiler)."""
import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from .run import build_matching, save

CIRCUIT = '''QUBIT_COORDS(0,0) 0
QUBIT_COORDS(1,0) 1
QUBIT_COORDS(2,0) 2
R 0 1 2
X_ERROR(0.1) 0 1 2
LOSS(0.2) 0 1 2
ML 0 1 2
DETECTOR(0,0,0) rec[-5] rec[-3]
DETECTOR(1,0,0) rec[-3] rec[-1]
OBSERVABLE_INCLUDE(0) rec[-5]
'''


def sha(data):
    return hashlib.sha256(data).hexdigest()


def bundle(path, text, payload):
    path.mkdir()
    circuit_sha, shots_sha = sha(text.encode()), sha(payload)
    identity = f'format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\ncircuit_sha256={circuit_sha}\nshots={len(payload)}\nrow_bits=6\nshots_b8_sha256={shots_sha}\n'
    manifest = {'format':'rstim_decoder_dataset', 'schema_version':1,'dataset_id':sha(identity.encode()),
                'mode':'measurements_blinded','shots':len(payload),
                'row':{'kind':'measurements','bits':6,'encoding':'b8','bit_order':'lsb_first','bytes_per_shot':1},
                'circuit':{'file':'circuit.stim','sha256':circuit_sha,'measurements':6,'detectors':2,'observables':1,'sweep_bits':0},
                'shots_file':{'file':'shots.b8','sha256':shots_sha,'bits':6,'bytes_per_shot':1}}
    (path/'circuit.stim').write_text(text)
    (path/'shots.b8').write_bytes(payload)
    save(path/'manifest.json',manifest)


def oracle(raw, conditioned=True):
    flags = [(raw >> (2*q)) & 1 for q in range(3)]
    values = [1 if flags[q] else (raw >> (2*q+1)) & 1 for q in range(3)]
    syndrome = [values[0]^values[1], values[1]^values[2]]
    # Equal base edge weights log(9). Each visible erasure halves its edge weight.
    weights = [0.5 if flag and conditioned else 1.0 for flag in flags]
    candidates = []
    for correction in itertools.product([0,1],repeat=3):
        if [correction[0]^correction[1],correction[1]^correction[2]] == syndrome:
            candidates.append((sum(w*x for w,x in zip(weights,correction)),correction[0]))
    minimum = min(cost for cost,_ in candidates)
    return {logical for cost,logical in candidates if abs(cost-minimum)<1e-9}, syndrome, flags


def run(binary, exporter):
    with tempfile.TemporaryDirectory(prefix='loss-decoder-reference-') as temp:
        work=Path(temp)
        bundle(work/'public',CIRCUIT,bytes(range(64)))
        subprocess.run([exporter,work/'public',work/'graph.json'],check=True,capture_output=True)
        graph=json.loads((work/'graph.json').read_text())
        # Hand-derived graph contract: two parity checks and one physical-X edge per wire.
        actual={(e['u'],e['v'],tuple(e['observables'])) for e in graph['edges']}
        expected={(0,None,(0,)),(0,1,()),(1,None,())}
        graph_ok = actual == expected and len(graph['edges']) == 3
        graph_ok &= all(abs(e['weight']-math.log(9))<1e-9 and e['loss_factor']==.5 for e in graph['edges'])
        loss_map_ok = len(graph['loss_edges'])==3 and all(len(es)==1 for es in graph['loss_edges'])
        if not graph_ok or not loss_map_ok:
            raise AssertionError(f'Hand-derived graph mismatch: {graph}')
        subprocess.run([binary,'decode','--decoder','envelope-matching','--dataset',work/'public',
                        '--out',work/'predictions.b8','--stats-out',work/'stats.json'],check=True,capture_output=True)
        predictions=(work/'predictions.b8').read_bytes()
        assert len(predictions)==64
        rows=[]
        mutation_rejected=False
        for raw in range(64):
            acceptable, syndrome, flags=oracle(raw)
            assert graph['syndromes'][raw]==syndrome
            mapped={i for loss in graph['losses'][raw] for i in graph['loss_edges'][loss]}
            expected_edges={i for i,e in enumerate(graph['edges']) if
                            ((e['v'] is None and e['u']==0 and flags[0]) or
                             (e['v']==1 and flags[1]) or (e['v'] is None and e['u']==1 and flags[2]))}
            assert mapped==expected_edges
            matching=build_matching(graph,graph['losses'][raw])
            py=int(matching.decode(np.array(syndrome,dtype=np.uint8))[0])
            status='PASS' if predictions[raw] in acceptable and py in acceptable else 'FAIL'
            rows.append({'packed_row':raw,'native':predictions[raw],'pymatching':py,'acceptable_minimum_weight_logicals':sorted(acceptable),'status':status})
            bad,_s,_f=oracle(raw,conditioned=False)
            mutation_rejected |= acceptable != bad
        assert mutation_rejected, 'Control must detect removing loss conditioning'
        return {'status':'PASS' if all(r['status']=='PASS' for r in rows) else 'FAIL',
                'method':'hand-derived repetition parity graph; enumerate all 8 correction masks for every one of 64 flag/value rows; allow exact ties',
                'scope':'matching graph objective and public row/loss transformation; not optimal physical logical-class likelihood and not an independent general envelope compiler',
                'hand_derived_graph_pass':bool(graph_ok),'loss_mapping_pass':bool(loss_map_ok),
                'loss_conditioning_changes_optimal_set':bool(mutation_rejected),
                'flipped_known_answer_rejected': (1 not in oracle(0)[0]), 'rows':rows}


if __name__=='__main__':
    p=argparse.ArgumentParser()
    p.add_argument('--binary',type=Path,default=Path('target/release/rustqec'))
    p.add_argument('--exporter',type=Path,default=Path('target/release/examples/export_matching_benchmark'))
    p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss/decoder-correctness.json'))
    a=p.parse_args()
    result=run(a.binary.resolve(),a.exporter.resolve())
    save(a.out,result)
    print(result['status'])
    raise SystemExit(result['status']!='PASS')
