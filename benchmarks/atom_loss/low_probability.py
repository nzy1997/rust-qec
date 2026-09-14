"""Finite-power checks at the benchmark's actual primitive noise probabilities.

Unit probes use exact binomial acceptance, not an absolute-error tolerance.
The real-circuit check exercises the blinded dataset exporter; its independent
reference sees the private input mask only to prepare the same logical input.
No decoder, envelope compiler, or private answer is used here.
"""
from .shot_data import require
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile
import numpy as np
from scipy.stats import binom, fisher_exact
from . import reference, channel_probes
from .shot_data import validate_dataset

ALPHA = 1e-7  # Each of the analytic and real-circuit families.
from .probe_specs import LOSS_RATES, low_specs as specifications

MUTATIONS = ('pauli', 'loss', 'both')
FIXTURE = Path(__file__).parent/'fixtures/midswap_d3_r2.stim'


def remove_low_noise(text, kind):
    """Actual input mutation; preserve all p=0 and p>=.01 instructions."""
    lines, removed = [], 0
    for line in text.splitlines():
        match = re.match(r'^\s*(X_ERROR|Y_ERROR|Z_ERROR|DEPOLARIZE1|DEPOLARIZE2|LOSS)\(([^)]+)\)', line)
        affected = (match and 0 < float(match[2]) < .01 and
                    (kind == 'both' or (match[1] == 'LOSS') == (kind == 'loss')))
        if affected:
            removed += 1
        else:
            lines.append(line)
    return '\n'.join(lines), removed




def event_counts(rows, columns):
    selected = rows[:,columns].astype(np.uint64)
    ids = (selected * (1 << np.arange(len(columns),dtype=np.uint64))).sum(axis=1).astype(int)
    # Full joint distribution, individual bits, and total nonidentity probability.
    return np.concatenate([np.bincount(ids,minlength=1<<len(columns)), selected.sum(axis=0),
                           [np.count_nonzero(ids)]]).astype(int)


def analytic(binary, sampler, shots):
    # At least 26 expected LOSS events; deleting the channel cannot sit in the
    # accepted zero-count tail. No amplification changes the primitive channel.
    shots = max(shots, 262144)
    probes = specifications()
    events = sum(len(expected)+len(columns)+1 for _,_,columns,expected in probes)
    tail = ALPHA/(4*events)  # two tails, two healthy implementations
    records, mutations = [], {k:[] for k in MUTATIONS}
    with tempfile.TemporaryDirectory(prefix='low-noise-') as tmp:
        for name,text,columns,expected in probes:
            count = max(shots, math.ceil(26/float(name[5:]))) if name.startswith('LOSS') else shots
            probabilities = expected + [sum(p for i,p in enumerate(expected) if i & (1<<bit))
                                          for bit in range(len(columns))] + [1-expected[0]]
            intervals = [[int(binom.ppf(tail,count,p)),int(binom.ppf(1-tail,count,p))] for p in probabilities]
            bounds = np.array(intervals)
            observations = {}
            for backend,rows in [('rust',sampler(binary,text,count,991,Path(tmp))),
                                 ('reference',reference.sample(text,count,773))]:
                counts = event_counts(rows,columns)
                observations[backend] = counts.tolist()
            passed = all(np.all((bounds[:,0] <= counts) & (counts <= bounds[:,1])) for counts in observations.values())
            records.append(dict(case=name,primitive_probability=.001 if not name.startswith('LOSS') else float(name[5:]),
                                shots_per_sampler=count,expected_probabilities=probabilities,
                                accepted_counts=intervals,counts=observations,status='PASS' if passed else 'FAIL'))
            for kind in MUTATIONS:
                changed,removed = remove_low_noise(text,kind)
                if not removed:
                    continue
                counts = event_counts(sampler(binary,changed,count,991,Path(tmp)),columns)
                if np.any((counts < bounds[:,0]) | (counts > bounds[:,1])):
                    mutations[kind].append(name)
    expected_failures = {'pauli':5,'loss':9,'both':14}
    controls = {k:dict(rejected=len(v)==expected_failures[k],failed_cases=v) for k,v in mutations.items()}
    return dict(status='PASS' if all(r['status']=='PASS' for r in records) and all(m['rejected'] for m in controls.values()) else 'FAIL',
                familywise_alpha_bound=ALPHA,method='Exact binomial equal-tail count intervals; joint bins, marginals, nonidentity',
                cases=records,low_probability_deletion_mutations=controls)


def export_rows(binary,text,shots,seed,work):
    circuit = work/'input.stim'
    circuit.write_text(text+'\n')
    public,private = work/'public',work/'private'
    subprocess.run([str(binary),'dataset','export','--circuit',str(circuit),'--shots',str(shots),
                    '--seed',str(seed),'--mode','measurements_blinded','--logical-x-qubits','1,8,15',
                    '--public-out',str(public),'--private-out',str(private)],check=True,capture_output=True,timeout=120)
    manifest = json.loads((public/'manifest.json').read_text())
    packed = np.frombuffer((public/'shots.b8').read_bytes(),dtype=np.uint8).reshape(shots,-1)
    rows = np.unpackbits(packed,axis=1,bitorder='little')[:,:manifest['row']['bits']].astype(bool)
    masks = np.frombuffer((private/'masks.b8').read_bytes(),dtype=np.uint8).astype(bool)
    return rows,masks


def indicators(text, rows):
    cursor,flags,detectors,observable = 0,[],[],[]
    for op in reference.parse(text):
        if op.name in {'M','MZ','MR','MRZ'}:
            cursor += len(op.targets)
        elif op.name in {'ML','MZL','MRL','MRZL'}:
            flags.extend(range(cursor,cursor+2*len(op.targets),2))
            cursor += 2*len(op.targets)
        elif op.name in {'DETECTOR','OBSERVABLE_INCLUDE'}:
            indices = [cursor+int(re.fullmatch(r'rec\[(-\d+)\]',t)[1]) for t in op.targets]
            parity = np.logical_xor.reduce(rows[:,indices],axis=1)
            (detectors if op.name=='DETECTOR' else observable).append(parity)
    require((cursor == rows.shape[1] == 50 and len(detectors)==16 and len(observable)==1), 'low_probability: cursor == rows.shape[1] == 50 and len(detectors)==16 and len(observable)==1')
    columns = list(rows.T) + detectors + observable
    names = [f'measurement_{i}' for i in range(rows.shape[1])] + [f'detector_{i}' for i in range(len(detectors))] + ['observable']
    columns += [a & b for a,b in zip(detectors,detectors[1:])]
    names += [f'adjacent_detector_joint_{i}' for i in range(len(detectors)-1)]
    no_loss = ~rows[:,flags].any(axis=1)
    return names,np.stack(columns,axis=1),no_loss,np.stack(detectors,axis=1)


def compare_real(text, observed, independent, masks):
    names,a,clean_a,da = indicators(text,observed)
    _,b,clean_b,db = indicators(text,independent)
    # Two logical input strata. No-visible-loss detector statistics additionally
    # isolate the Pauli channel from the much larger loss-induced syndrome rate.
    records=[]
    for mask in [0,1]:
        selected = masks == mask
        for label,x,y in [('all',a[selected],b[selected]),
                          ('no_visible_loss',da[selected & clean_a],db[selected & clean_b])]:
            labels = names if label=='all' else [f'detector_{i}' for i in range(16)]
            for name,kx,ky in zip(labels,x.sum(axis=0),y.sum(axis=0)):
                pvalue = float(fisher_exact([[int(kx),len(x)-int(kx)],[int(ky),len(y)-int(ky)]]).pvalue)
                records.append(dict(event=f'input_{mask}/{label}/{name}',rust_events=int(kx),reference_events=int(ky),
                                    rust_shots=len(x),reference_shots=len(y),pvalue=pvalue))
    threshold=ALPHA/len(records)
    failed=[r['event'] for r in records if r['pvalue'] < threshold]
    return dict(status='FAIL' if failed else 'PASS',threshold=threshold,failed_events=failed,events=records)


def real_circuit(binary, shots, exporter=export_rows):
    text = FIXTURE.read_text()
    shots=max(shots,65536)
    with tempfile.TemporaryDirectory(prefix='low-noise-export-') as tmp:
        work=Path(tmp)
        (work/'healthy').mkdir()
        observed,masks=exporter(binary,text,shots,179,work/'healthy')
        checked_answers=validate_dataset(lambda name:(work/'healthy'/name).read_bytes())
        independent=np.empty_like(observed)
        marker='TICK[rstim:logical_flip_point]'
        require((text.count(marker)==1), 'low_probability: text.count(marker)==1')
        for mask in [0,1]:
            selected=masks==mask
            prepared=text.replace(marker,marker+'\nX 1 8 15') if mask else text
            independent[selected]=reference.sample(prepared,int(selected.sum()),827+mask)
        healthy=compare_real(text,observed,independent,masks)
        controls={}
        for kind in MUTATIONS:
            changed,removed=remove_low_noise(text,kind)
            (work/kind).mkdir()
            bad,bad_masks=exporter(binary,changed,shots,179,work/kind)
            validate_dataset(lambda name:(work/kind/name).read_bytes())
            # Mask RNG stream is independent of physical sampling.
            if not np.array_equal(masks,bad_masks):
                raise ValueError('Input mask stream changed across physical-noise mutation')
            result=compare_real(text,bad,independent,masks)
            controls[kind]=dict(rejected=removed>0 and result['status']=='FAIL',removed_instructions=removed,
                                failed_events=result['failed_events'])
    return dict(status='PASS' if healthy['status']=='PASS' and all(c['rejected'] for c in controls.values()) else 'FAIL',
                fixture_sha256=hashlib.sha256(text.encode()).hexdigest(),shots_per_sampler=shots,
                scoring_key_check={'status':'PASS','checked_shots':len(checked_answers)},
                familywise_alpha_bound=ALPHA,mode='measurements_blinded',pauli_probability=.001,loss_probability=.003,
                method='Independent Stim lowering, logical-input strata, exact Fisher tests with Bonferroni correction',
                comparison=healthy,low_probability_deletion_mutations=controls)


def run(binary,sampler,shots):
    unit=analytic(binary,sampler,shots)
    actual=real_circuit(binary,shots)
    return dict(status='PASS' if unit['status']==actual['status']=='PASS' else 'FAIL',
                analytic=unit,real_circuit=actual,familywise_alpha_bound=2*ALPHA)
