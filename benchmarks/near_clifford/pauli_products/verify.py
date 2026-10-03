"""Fail-closed support-control validation, independent GHZ phase and source bindings."""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import statistics
spec = importlib.util.spec_from_file_location('pauli_controls_entry', Path(__file__).with_name('run.py'))
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)
bound_source_bytes = run.load(run.BASE/'row_ops/verify.py','pauli_source_bindings').source_bytes


def require(condition, message):
    if not condition: raise ValueError(message)


def finite(value):
    return type(value) in [int,float] and math.isfinite(value)


def probabilities(record, expected):
    require(set(record) >= {'zero','one'}, 'missing probability fields')
    require(all(finite(record[key]) for key in ['zero','one']), 'invalid probability values')
    require(abs(record['zero']-expected)<1e-10 and abs(record['one']-(1-expected))<1e-10, 'conditional Born probability differs')


def circuit(topology, width):
    return 'H 0\nT 0\n'+''.join(f'CX {q-1 if topology=="chain" else 0} {q}\n' for q in range(1,width))


def physics(payload, topology, width):
    require(set(payload)=={'topology','width','circuit','probability','physics'}, 'incomplete semantic payload')
    require(payload['topology']==topology and payload['width']==width and payload['circuit']==circuit(topology,width), 'wrong semantic fixture')
    require(isinstance(payload['probability'],list) and len(payload['probability'])==2 and
            all(finite(v) and abs(v-.5)<1e-10 for v in payload['probability']), 'wrong initial probability')
    require([w['seed'] for w in payload['physics']]==[739,20261005], 'missing physics seeds')
    for witness in payload['physics']:
        require(set(witness)=={'seed','steps','final','continuation'}, 'incomplete physics witness')
        require(type(witness['continuation']) is int and 0<=witness['continuation']<2**64, 'invalid RNG continuation')
        require(len(witness['steps'])==width-1, 'missing full-width steps')
        phase=math.pi/4
        for q,step in zip(range(width-1,0,-1),witness['steps']):
            basis='X' if (q+witness['seed'])%2==0 else 'Y'
            require(set(step)=={'q','basis','zero','one','outcome'} and step['q']==q and step['basis']==basis, 'wrong physics measurement')
            require(type(step['outcome']) is bool, 'invalid physics outcome')
            probabilities(step,.5)
            if basis=='Y': phase-=math.pi/2
            if step['outcome']: phase+=math.pi
            phase %= 2*math.pi
        require([p['basis'] for p in witness['final']]==['X','Y','Z'], 'missing final bases')
        for record,expected in zip(witness['final'],[(1+math.cos(phase))/2,(1+math.sin(phase))/2,.5]):
            require(set(record)=={'basis','zero','one'}, 'invalid final probability fields')
            probabilities(record,expected)


def verify(result, binaries=None, git_sources=False):
    require(result.get('schema')=='near-clifford.pauli-products.v1' and result.get('completed_utc'), 'incomplete/unknown support campaign')
    require(result['matrix']==[list(row) for row in run.MATRIX] and result['pairs']==result['repetitions']==3 and result['calls']==run.CALLS, 'wrong support matrix or repetitions')
    require(result['inputs']=={name:run.sha(run.HERE/name) for name in ['main.rs','run.py']}, 'support entry/driver differs')
    require(result['build_runner_sha256']==run.sha(run.BASE/'scale/run.py'), 'build runner differs')
    labels={'baseline','candidate'}
    require(set(result['sources'])==set(result['diagnostics'])==labels, 'wrong source labels')
    require(set(result['verification'])==labels|{'baseline-diagnostic','candidate-diagnostic'}, 'missing semantic labels')
    require(result['sources']['baseline']['revision']!=result['sources']['candidate']['revision'], 'identical revisions')
    names={f'{t}_{w}' for t,w in run.MATRIX}
    for label in labels:
        source=result['sources'][label]
        diagnostic=source['diagnostic']
        require(source['revision']==diagnostic['revision'], 'wrong diagnostic revision')
        require(source['tableau_source_sha256']==diagnostic['tableau_source_sha256'], 'diagnostic tableau differs')
        for metadata in [source,diagnostic]:
            require(metadata['lock_sha256']==run.sha(run.BASE/'scale/Cargo.unified.lock'), 'support lock differs')
            for key in ['binary_sha256','near_clifford_source_sha256','tableau_source_sha256']:
                digest=metadata[key]
                require(len(digest)==64 and all(c in '0123456789abcdef' for c in digest), 'invalid source digest')
        if git_sources:
            for path,key in [('rstim/src/sim/tableau.rs','tableau_source_sha256'),('rstim/src/near_clifford.rs','near_clifford_source_sha256')]:
                raw=bound_source_bytes(source['revision'],path,source[key])
                require(hashlib.sha256(raw).hexdigest()==source[key], 'support git source differs')
                if key=='near_clifford_source_sha256':
                    overlay=run.instrument(raw.decode().strip()+'\n')
                    require(hashlib.sha256(overlay.encode()).hexdigest()==diagnostic[key], 'support overlay differs')
        if binaries:
            for suffix,metadata in [('',source),('-diagnostic',diagnostic)]:
                require(run.sha(binaries/(label+suffix)/'near-clifford-scale')==metadata['binary_sha256'], 'support binary differs')
        require(set(result['diagnostics'][label])==names, 'missing work counters')
        for topology,width in run.MATRIX:
            record=result['diagnostics'][label][f'{topology}_{width}']
            expected_rows=width+1 if topology=='star' else 3
            require(record['topology']==topology and record['width']==width and type(record['rank']) is int and record['rank']==1, 'wrong diagnostic fixture/rank')
            require(record['row_work']==[expected_rows,expected_rows*width] and all(type(v) is int for v in record['row_work']), 'wrong selected row/entry work')
    for label,payloads in result['verification'].items():
        require(set(payloads)==names, 'missing semantic fixtures')
        require(payloads==result['verification']['baseline'], 'support output/RNG differs')
        for topology,width in run.MATRIX: physics(payloads[f'{topology}_{width}'],topology,width)
    require([(c['topology'],c['width']) for c in result['cases']]==run.MATRIX, 'missing timing fixtures')
    for index,case in enumerate(result['cases']):
        require(len(case['runs'])==3, 'missing timing pairs')
        for pair,record in enumerate(case['runs']):
            require(set(record)==labels|{'order'}, 'wrong timed labels')
            require(record['order']==(['baseline','candidate'] if (index+pair)%2==0 else ['candidate','baseline']), 'wrong paired order')
            for label in labels:
                value=record[label]
                require(value['topology']==case['topology'] and value['width']==case['width'] and value['circuit']==circuit(case['topology'],case['width']), 'wrong timed circuit')
                require(value['calls']==run.CALLS and value['warmup_calls']==32, 'wrong timed boundary')
                require(len(value['raw_ns'])==3 and all(type(v) is int and v>0 for v in value['raw_ns']), 'wrong raw support times')
                require(type(value['median_ns']) is int and value['median_ns']==statistics.median(value['raw_ns']), 'wrong support median')
    return 'PASS: 14 full-width sparse/dense controls, all four semantic payloads and work/source bindings'


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('result',type=Path)
    parser.add_argument('--git-sources',action='store_true')
    parser.add_argument('--binaries',type=Path)
    args=parser.parse_args()
    print(verify(json.loads(args.result.read_text()),args.binaries,args.git_sources))
