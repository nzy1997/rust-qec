"""Independent repetition-graph objectives, including a strict conditioning witness."""
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


def circuit_for(wires):
    lines=[f'QUBIT_COORDS({q},0) {q}' for q in range(wires)]
    targets=' '.join(map(str,range(wires)))
    lines += [f'R {targets}',f'X_ERROR(0.1) {targets}',f'LOSS(0.2) {targets}',f'ML {targets}']
    for q in range(wires-1):
        lines.append(f'DETECTOR({q},0,0) rec[{2*q+1-2*wires}] rec[{2*q+3-2*wires}]')
    lines.append(f'OBSERVABLE_INCLUDE(0) rec[{1-2*wires}]')
    return '\n'.join(lines)+'\n'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def bundle(path, wires):
    path.mkdir()
    bits=2*wires; count=1<<bits; stride=(bits+7)//8
    text=circuit_for(wires)
    payload=b''.join(row.to_bytes(stride,'little') for row in range(count))
    circuit_sha, shots_sha=sha(text.encode()),sha(payload)
    identity=f'format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\ncircuit_sha256={circuit_sha}\nshots={count}\nrow_bits={bits}\nshots_b8_sha256={shots_sha}\n'
    manifest={'format':'rstim_decoder_dataset','schema_version':1,'dataset_id':sha(identity.encode()),
              'mode':'measurements_blinded','shots':count,
              'row':{'kind':'measurements','bits':bits,'encoding':'b8','bit_order':'lsb_first','bytes_per_shot':stride},
              'circuit':{'file':'circuit.stim','sha256':circuit_sha,'measurements':bits,'detectors':wires-1,'observables':1,'sweep_bits':0},
              'shots_file':{'file':'shots.b8','sha256':shots_sha,'bits':bits,'bytes_per_shot':stride}}
    (path/'circuit.stim').write_text(text); (path/'shots.b8').write_bytes(payload)
    save(path/'manifest.json',manifest)


def oracle(raw, conditioned=True, wires=3):
    flags=[(raw>>(2*q))&1 for q in range(wires)]
    values=[1 if flags[q] else (raw>>(2*q+1))&1 for q in range(wires)]
    syndrome=[a^b for a,b in zip(values,values[1:])]
    weights=[.5 if flag and conditioned else 1. for flag in flags]
    candidates=[]
    for correction in itertools.product([0,1],repeat=wires):
        if [a^b for a,b in zip(correction,correction[1:])]==syndrome:
            candidates.append((sum(w*x for w,x in zip(weights,correction)),correction[0]))
    minimum=min(cost for cost,_ in candidates)
    return {logical for cost,logical in candidates if abs(cost-minimum)<1e-9},syndrome,flags


def rejected_rows(predictions, allowed):
    """The same acceptance rule is used for healthy and deliberately broken outputs."""
    if len(predictions)!=len(allowed): raise ValueError('Incomplete oracle predictions')
    return [i for i,(p,a) in enumerate(zip(predictions,allowed)) if p not in a]


def check_case(binary, exporter, work, wires):
    bundle(work/'public',wires)
    subprocess.run([exporter,work/'public',work/'graph.json'],check=True,capture_output=True)
    graph=json.loads((work/'graph.json').read_text())
    expected={(0,None,(0,)),(wires-2,None,())}
    expected.update((q,q+1,()) for q in range(wires-2))
    actual={(e['u'],e['v'],tuple(e['observables'])) for e in graph['edges']}
    assert actual==expected and len(graph['edges'])==wires, 'Hand-derived graph mismatch'
    assert all(abs(e['weight']-math.log(9))<1e-9 and e['loss_factor']==.5 for e in graph['edges'])
    assert len(graph['loss_edges'])==wires and all(len(es)==1 for es in graph['loss_edges'])
    subprocess.run([binary,'decode','--decoder','envelope-matching','--dataset',work/'public',
                    '--out',work/'predictions.b8','--stats-out',work/'stats.json'],check=True,capture_output=True)
    native=list((work/'predictions.b8').read_bytes()); python=[]; broken=[]; allowed=[]; canonical=[]
    for raw in range(1<<(2*wires)):
        acceptable,syndrome,flags=oracle(raw,wires=wires)
        allowed.append(acceptable)
        assert graph['syndromes'][raw]==syndrome
        mapped={i for loss in graph['losses'][raw] for i in graph['loss_edges'][loss]}
        expected_edges={i for i,e in enumerate(graph['edges']) if
                        (e['v'] is None and e['u']==0 and flags[0]) or
                        (e['v'] is not None and flags[e['u']+1]) or
                        (e['v'] is None and e['u']==wires-2 and flags[-1])}
        assert mapped==expected_edges
        values=np.array(syndrome,dtype=np.uint8)
        python.append(int(build_matching(graph,graph['losses'][raw]).decode(values)[0]))
        # Execute a real defective decoder, not merely compare two oracle sets.
        broken.append(int(build_matching(graph,[]).decode(values)[0]))
        canonical.append(raw | sum(1<<(2*q+1) for q in range(wires) if flags[q]))
    failures={'native':rejected_rows(native,allowed),'pymatching':rejected_rows(python,allowed)}
    mutated=native.copy();mutated[0]^=1
    flipped=rejected_rows(mutated,allowed)
    ignored=rejected_rows(broken,allowed)
    placeholder_ok=all(native[i]==native[c] and python[i]==python[c] for i,c in enumerate(canonical))
    witness=21 if wires==5 else None  # Lost first three wires; raw values all zero.
    if witness is not None:
        assert oracle(witness,False,wires)[0]=={0} and oracle(witness,True,wires)[0]=={1}
        assert witness in ignored, 'Ignore-conditioning decoder escaped strict witness'
    passed=not any(failures.values()) and placeholder_ok and 0 in flipped and (wires!=5 or bool(ignored))
    return {'wires':wires,'rows_checked':len(allowed),'status':'PASS' if passed else 'FAIL',
            'hand_derived_graph_pass':True,'loss_mapping_pass':True,'placeholder_invariance_pass':placeholder_ok,
            'rejected_rows':failures,'ignored_conditioning_rejected_rows':ignored,
            'flipped_prediction_rejected_rows':flipped,
            'strict_witness':None if witness is None else {'packed_row':witness,'fixed_optimum':[0],
                 'conditioned_optimum':[1],'fixed_decoder_prediction':broken[witness],
                 'native_prediction':native[witness],'pymatching_prediction':python[witness],
                 'fixed_costs':[3,2],'conditioned_costs':[1.5,2]}}


def run(binary, exporter):
    cases=[]
    with tempfile.TemporaryDirectory(prefix='loss-decoder-reference-') as tmp:
        for wires in [3,5]:
            work=Path(tmp)/str(wires);work.mkdir()
            cases.append(check_case(binary,exporter,work,wires))
    return {'status':'PASS' if all(c['status']=='PASS' for c in cases) else 'FAIL',
            'method':'hand-derived three/five-wire graphs; exhaustive correction enumeration; identical acceptance for healthy and mutated decoder outputs',
            'scope':'matching objective and public-row/loss transformation; not an independent general envelope compiler or physical Bayes-optimal decoder',
            'cases':cases}


if __name__=='__main__':
    p=argparse.ArgumentParser()
    p.add_argument('--binary',type=Path,default=Path('target/release/rustqec'))
    p.add_argument('--exporter',type=Path,default=Path('target/release/examples/export_matching_benchmark'))
    p.add_argument('--out',type=Path,default=Path('site/static/data/atom-loss/decoder-correctness.json'))
    a=p.parse_args();result=run(a.binary.resolve(),a.exporter.resolve());save(a.out,result)
    print(result['status']);raise SystemExit(result['status']!='PASS')
