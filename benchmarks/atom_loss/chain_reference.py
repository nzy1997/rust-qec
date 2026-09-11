"""Independent Stim fault propagation and exact finite-state decoder objectives.

Stim constructs independent noise distributions and loss candidate sets. Native
compiler outputs must pass those checks before their representation-dependent
MLE fault-configuration objective is evaluated by exact dynamic programming.
"""
import argparse
import copy
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import numpy as np
import stim
from . import reference
from .run import ROOT, save, digest

FIXTURE = ROOT/'benchmarks/atom_loss/fixtures/midswap_d3_r2.stim'


def normalized(text):
    circuit=stim.Circuit(); probes=[]; onsets=defaultdict(list); bases=defaultdict(list)
    for op in reference.parse(text):
        name=op.name
        if name=='LOSS':
            for q in map(int,op.targets): onsets[q].append(len(circuit))
        elif name=='CX':
            targets=list(map(int,op.targets)); changed=targets[1::2]
            # The declared envelope model resolves CX into H-CZ-H, including
            # both target-basis boundaries as possible Pauli randomizations.
            circuit.append('H',changed)
            for q in changed: bases[q].append(len(circuit))
            circuit.append('CZ',targets)
            circuit.append('H',changed)
            for q in changed: bases[q].append(len(circuit))
        elif name in ['ML','MRL']:
            for q in map(int,op.targets):
                flag=circuit.num_measurements
                circuit.append('MPAD',[0])
                probes.append({'qubit':q,'flag':flag,'onsets':onsets[q].copy(),
                               'bases':bases[q].copy(),'readout':len(circuit)})
                circuit.append('MR' if name=='MRL' else 'M',[q])
                if name=='MRL': onsets[q].clear();bases[q].clear()
        else:
            args='('+','.join(map(str,op.args))+')' if op.args else ''
            circuit += stim.Circuit(name+args+' '+' '.join(op.targets))
            if name=='R':
                for q in map(int,op.targets): onsets[q].clear();bases[q].clear()
            elif name=='H':
                for q in map(int,op.targets): bases[q].append(len(circuit))
    return circuit,probes


def mask_of(targets, detectors):
    mask=0
    for t in targets:
        if t.is_relative_detector_id(): mask ^= 1<<t.val
        elif t.is_logical_observable_id(): mask ^= 1<<(detectors+t.val)
    return mask


def independent_model(text):
    circuit,probes=normalized(text)
    count=circuit.num_detectors
    assert count==16 and circuit.num_observables==1
    coords=circuit.get_detector_coordinates()
    dem=circuit.detector_error_model(decompose_errors=True).flattened()
    effects=[];edges=[]
    for instruction in dem:
        if instruction.type!='error': continue
        probability=instruction.args_copy()[0]
        weight=math.log((1-probability)/probability)
        targets=instruction.targets_copy()
        effects.append((mask_of(targets,count),weight))
        components=[[]]
        for t in targets:
            if t.is_separator(): components.append([])
            else: components[-1].append(t)
        for component in components:
            ds=sorted(t.val for t in component if t.is_relative_detector_id())
            if not ds: continue
            assert len(ds)<=2
            factor=.25 if len(ds)==2 and coords[ds[0]][:2]==coords[ds[1]][:2] else .5
            edges.append({'mask':mask_of(component,count),'ds':tuple(ds),'weight':weight,'factor':factor})
    # Determine disconnected detector sectors without relying on Rust labels.
    sector=list(range(count))
    def find(q):
        while sector[q]!=q: q=sector[q]
        return q
    for e in edges:
        if len(e['ds'])==2: sector[find(e['ds'][0])]=find(e['ds'][1])
    memo={}
    def effect(site,q,pauli):
        key=(site,q,pauli)
        if key not in memo:
            injected=circuit[:site].without_noise()
            injected.append(pauli+'_ERROR',[q],.125)
            injected += circuit[site:].without_noise()
            terms=[i for i in injected.detector_error_model().flattened() if i.type=='error']
            assert len(terms)<=1
            memo[key]=mask_of(terms[0].targets_copy(),count) if terms else 0
        return memo[key]
    candidates=[];loss_edges=[]
    for p in probes:
        union=set(); primitive=set()
        for onset in p['onsets']:
            sites=sorted({onset,p['readout'],*(s for s in p['bases'] if onset<=s<=p['readout'])})
            states={0}
            for site in sites:
                choices={0,*(effect(site,p['qubit'],pauli) for pauli in 'XYZ')}
                primitive.update(choices)
                states={a^b for a in states for b in choices}
            union.update(states)
        assert union
        candidates.append(sorted(union))
        mapped=set()
        for fault in primitive:
            sectors=defaultdict(list)
            for d in range(count):
                if fault>>d&1: sectors[find(d)].append(d)
            selected=[]; recovered=0
            for ds in sectors.values():
                matching=[i for i,e in enumerate(edges) if e['ds']==tuple(ds)]
                assert matching, f'Unrepresented independent primitive {fault}'
                assert len({edges[i]['mask'] for i in matching})==1
                recovered ^= edges[matching[0]]['mask']
                selected += matching
            assert recovered==fault, f'Primitive logical label mismatch: {fault}'
            mapped.update(selected)
        loss_edges.append(mapped)
    return circuit,probes,effects,edges,candidates,loss_edges,len(memo)


def costs(terms, bits):
    # Exhaust all binary fault choices by min-plus dynamic programming, keeping
    # the cheapest history for every detector+logical parity state (not MWPM/ILP).
    indices=np.arange(1<<bits)
    values=np.full(1<<bits,np.inf);values[0]=0.
    for mask,weight in terms:
        values=np.minimum(values,values[indices^mask]+weight)
    return values


def allowed(values,syndrome,detectors):
    pair=np.array([values[syndrome],values[syndrome|(1<<detectors)]])
    best=pair.min()
    if not np.isfinite(best): raise ValueError('Unreachable oracle syndrome')
    return {int(i) for i,v in enumerate(pair) if abs(v-best)<1e-7}


def write_bundle(path,text,rows,circuit):
    path.mkdir()
    payload=np.packbits(rows,axis=1,bitorder='little').tobytes()
    sha=lambda data: hashlib.sha256(data).hexdigest()
    csha,rsha=sha(text.encode()),sha(payload)
    bits=rows.shape[1];shots=len(rows);stride=(bits+7)//8
    identity=f'format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\ncircuit_sha256={csha}\nshots={shots}\nrow_bits={bits}\nshots_b8_sha256={rsha}\n'
    (path/'circuit.stim').write_text(text);(path/'shots.b8').write_bytes(payload)
    save(path/'manifest.json',{'format':'rstim_decoder_dataset','schema_version':1,'dataset_id':sha(identity.encode()),
         'mode':'measurements_blinded','shots':shots,'row':{'kind':'measurements','bits':bits,'encoding':'b8','bit_order':'lsb_first','bytes_per_shot':stride},
         'circuit':{'file':'circuit.stim','sha256':csha,'measurements':bits,'detectors':circuit.num_detectors,'observables':1,'sweep_bits':0},
         'shots_file':{'file':'shots.b8','sha256':rsha,'bits':bits,'bytes_per_shot':stride}})


def native_mask(e, count):
    return sum(1<<d for d in e['detectors']) | sum(1<<(count+o) for o in e['observables'])


def coalesce(terms):
    products=defaultdict(lambda:1.)
    for mask,weight in terms: products[mask] *= 1-2/(1+math.exp(weight))
    return {mask:(1-value)/2 for mask,value in products.items() if mask}


def validate_model(model, effects, candidates, count):
    native=coalesce([(native_mask(e,count),e['weight']) for e in model['independent_effects']])
    expected=coalesce(effects)
    if native.keys()!=expected.keys() or any(abs(native[k]-expected[k])>1e-9 for k in expected):
        raise ValueError('Independent Pauli distribution mismatch')
    if any(e['weight']!=0 for es in model['loss_candidates'] for e in es):
        raise ValueError('Independent loss candidate weight mismatch')
    if [{native_mask(e,count) for e in es} for es in model['loss_candidates']]!=list(map(set,candidates)):
        raise ValueError('Independent loss candidate mismatch')


def run(binary,exporter):
    text=FIXTURE.read_text()
    circuit,probes,effects,edges,candidates,loss_edges,probe_count=independent_model(text)
    count=circuit.num_detectors; indices=np.arange(1<<(count+1))
    # External physical samples: fixed private onset histories at early/middle/late
    # opportunities. Enumerate every single Pauli fault on the lowered circuit.
    ops=reference.parse(text)
    nloss=sum(len(op.targets) for op in ops if op.name=='LOSS')
    rows=[]
    for seed,site in enumerate([None,0,nloss//2,nloss-1]):
        history=np.zeros(nloss,dtype=bool)
        if site is not None: history[site]=1
        lowered=reference.lower(ops,history)
        rows.extend(lowered.without_noise().compile_sampler(seed=701+seed).sample(2))
        for position,instruction in enumerate(lowered):
            if instruction.name not in ['X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2']: continue
            targets=[t.value for t in instruction.targets_copy()]
            if instruction.name=='DEPOLARIZE2':
                faults=[[(a,q),(b,r)] for q,r in zip(targets[::2],targets[1::2]) for a in 'IXYZ' for b in 'IXYZ' if a+b!='II']
            else:
                paulis='XYZ' if instruction.name=='DEPOLARIZE1' else instruction.name[0]
                faults=[[(pauli,q)] for q in targets for pauli in paulis]
            for fault in faults:
                injected=lowered[:position].without_noise()
                for pauli,q in fault:
                    if pauli!='I': injected.append(pauli,[q])
                injected += lowered[position+1:].without_noise()
                rows.extend(injected.compile_sampler(seed=701+seed).sample(2))
    fault_traces=len(rows)
    rows=np.unique(np.array(rows,dtype=np.uint8),axis=0)
    flags=[p['flag'] for p in probes]
    # Include alternate placeholders, which must not affect decoder outputs.
    alternate=rows.copy()
    for flag in flags: alternate[alternate[:,flag]==1,flag+1]^=1
    rows=np.concatenate([rows,alternate])
    canonical=rows.copy()
    for flag in flags: canonical[canonical[:,flag]==1,flag+1]=1
    detections=circuit.compile_m2d_converter().convert(measurements=canonical.astype(bool),separate_observables=True)[0]
    patterns=[tuple(np.flatnonzero(row[flags])) for row in rows]
    syndromes=[sum(int(v)<<i for i,v in enumerate(row)) for row in detections]
    mean=np.mean([e['weight'] for e in edges])
    expected={'envelope-matching':[]}; cache={}
    for pattern,syndrome in zip(patterns,syndromes):
        if pattern not in cache:
            active=set().union(*(loss_edges[i] for i in pattern))
            matching=costs([(e['mask'],e['factor']*mean if i in active else e['weight']) for i,e in enumerate(edges)],count+1)
            cache[pattern]=matching
        expected['envelope-matching'].append(allowed(cache[pattern],syndrome,count))
    with tempfile.TemporaryDirectory(prefix='midswap-oracle-') as tmp:
        work=Path(tmp);write_bundle(work/'public',text,rows,circuit)
        subprocess.run([exporter,work/'public',work/'graph.json'],check=True,capture_output=True)
        graph=json.loads((work/'graph.json').read_text())
        assert graph['syndromes']==detections.astype(int).tolist(), 'Independent m2d mismatch'
        assert graph['losses']==[list(p) for p in patterns], 'Independent visible-loss mapping mismatch'
        # DEM decomposition is not unique; compare full correlated Pauli
        # effects after coalescing equal parity masks, not a raw edge count.
        oracle_export=exporter.parent/'export_decoder_oracle'
        subprocess.run([oracle_export,work/'public',work/'models.json'],check=True,capture_output=True)
        model=json.loads((work/'models.json').read_text())
        validate_model(model,effects,candidates,count)
        mutations={}
        for name in ['pauli_weight','loss_candidate']:
            defective=copy.deepcopy(model)
            if name=='pauli_weight': defective['independent_effects'][0]['weight']+=.25
            else:
                envelope=next(es for es in defective['loss_candidates'] if len(es)>1)
                envelope.pop()
            try:
                validate_model(defective,effects,candidates,count)
                mutations[name]=False
            except ValueError:
                mutations[name]=True
        # Most-likely FAULT CONFIGURATION depends on how equal physical effects
        # are split into Bernoulli variables. Stim and Rust use different valid
        # decompositions. First prove distribution equivalence above; only then
        # exhaust the validated native representation with a non-ILP algorithm.
        native_base=costs([(native_mask(e,count),e['weight']) for e in model['independent_effects']],count+1)
        native_costs={}
        expected['envelope-mle']=[]
        for pattern,syndrome in zip(patterns,syndromes):
            if pattern not in native_costs:
                values=native_base.copy()
                for i in pattern: values=np.minimum.reduce([values[indices^candidate] for candidate in candidates[i]])
                native_costs[pattern]=values
            expected['envelope-mle'].append(allowed(native_costs[pattern],syndrome,count))

        results={}
        for backend in expected:
            out=work/(backend+'.b8')
            subprocess.run([binary,'decode','--decoder',backend,'--dataset',work/'public','--out',out,
                            '--stats-out',work/(backend+'.json')],check=True,capture_output=True,timeout=120)
            predictions=list(out.read_bytes())
            rejected=[i for i,(value,choices) in enumerate(zip(predictions,expected[backend],strict=True)) if value not in choices]
            singleton=[i for i,a in enumerate(expected[backend]) if len(a)==1]
            mutated=predictions.copy();mutated[singleton[0]]^=1
            mutation=[i for i,(value,choices) in enumerate(zip(mutated,expected[backend])) if value not in choices]
            results[backend]={'rejected_rows':rejected,'unique_optimum_rows':len(singleton),'predicted_ones':sum(predictions),
                              'constant_zero_rejected':any(0 not in a for a in expected[backend]),
                              'constant_one_rejected':any(1 not in a for a in expected[backend]),
                              'flipped_prediction_rejected':singleton[0] in mutation,
                              'placeholder_invariance':predictions[:len(rows)//2]==predictions[len(rows)//2:],
                              'prediction_sha256':hashlib.sha256(out.read_bytes()).hexdigest()}
    passed=all(mutations.values()) and all(not r['rejected_rows'] and r['flipped_prediction_rejected'] and r['placeholder_invariance'] and r['constant_zero_rejected'] and r['constant_one_rejected'] for r in results.values())
    return {'status':'PASS' if passed else 'FAIL','fixture_sha256':digest(FIXTURE),'distance':3,'rounds':2,
            'detectors':count,'rows':len(rows),'physical_fault_traces':fault_traces,'patterns':len(cache),'stim_pauli_probes':probe_count,
            'independent_effects':len(effects),'independent_graph_edges':len(edges),
            'independent_effects_candidates_and_m2d_pass':True,'compiler_output_mutations_rejected':mutations,'native_graph_edges':len(graph['edges']),'backends':results,
            'method':'Stim-derived DEM and Pauli fault propagation; exact min-plus enumeration of detector/logical states; external persistent-loss measurement records',
            'mle_objective':'minimum-weight fault configuration in an independently distribution-validated native DEM representation; zero-cost loss-envelope choices',
            'scope':'finite Mid-SWAP fixture and declared envelope model, including MLE; not proof of arbitrary compiler inputs or physical logical-class Bayes optimality'}


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,default=Path('drafts/midswap-oracle.json'))
    a=p.parse_args();r=run(ROOT/'target/release/rustqec',ROOT/'target/release/examples/export_matching_benchmark');save(a.out,r);print(r['status'])
    raise SystemExit(r['status']!='PASS')
