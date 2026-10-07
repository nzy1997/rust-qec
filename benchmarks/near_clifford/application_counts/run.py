"""Source-bound original-circuit capability and postselected-counts campaign."""
import argparse
from datetime import datetime,timezone
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
from common import HERE,ROOT,NAMES,POLICIES,SHOTS,digest,require,annotations,raw_counts,counts,compare

sys.path.insert(0,str(HERE.parent/'diagnostics'))
spec=importlib.util.spec_from_file_location('diagnostic_driver',HERE.parent/'diagnostics/run.py')
diag=importlib.util.module_from_spec(spec);spec.loader.exec_module(diag)
inc=diag.incumbent
from evidence import compact
from projection import records_only


def inventory():
    files=diag.inventory()
    files.update({str(p.relative_to(ROOT)):digest(p.read_bytes()) for p in HERE.rglob('*')
                  if p.is_file() and not {'target','__pycache__'}&set(p.relative_to(HERE).parts)})
    return files


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--python',type=Path,required=True)
    p.add_argument('--symft-python',type=Path,required=True)
    p.add_argument('--symft-source',type=Path,required=True)
    p.add_argument('--pairs',type=int,default=5)
    p.add_argument('--repetitions',type=int,default=7)
    p.add_argument('--only',nargs='+',choices=NAMES,default=NAMES)
    p.add_argument('--shots',nargs='+',type=int,choices=SHOTS,default=SHOTS)
    p.add_argument('--rust-route',choices=['structured','native'],default='structured')
    args=p.parse_args()
    require(1<=args.pairs<=20 and 1<=args.repetitions<=64,'bounded pairs/repetitions required')
    out=args.out.resolve();require(not out.exists(),'out must be fresh');out.mkdir(parents=True)
    manifest=json.loads((HERE/'manifest.json').read_text())
    for value in manifest['inputs'].values():
        original=ROOT/value['path'];require(digest(original.read_bytes())==value['sha256'],'original input changed')
    for path,value in manifest['licenses'].items():
        require(digest((ROOT/path).read_bytes())==value['sha256'],'upstream license changed')
    sources=inventory()
    revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    for package in [HERE,HERE.parent/'diagnostics']:
        subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(package/'Cargo.toml')],check=True,cwd=ROOT)
    binaries={name:str(path/'target/release'/name) for name,path in
              [('near-clifford-application-counts',HERE),('near-clifford-diagnostics',HERE.parent/'diagnostics')]}
    binary_hashes={name:digest(Path(path).read_bytes()) for name,path in binaries.items()}
    packages=inc.capture_packages(args.python,args.symft_python)
    identities={name:v['loaded_files'] for name,v in inc.capture_identities(args.python,args.symft_python).items()}
    peer_revision,peer_sources=inc.capture_symft_source(args.symft_source)
    env=inc.environment_summary(packages,peer_sources,peer_revision,identities)
    cases=[dict(id=f'{name}/{shots}/{policy}',name=name,shots=shots,policy=policy)
           for name in args.only for shots in args.shots for policy in POLICIES]
    header=dict(schema='rstim.postselected-counts.v2' if args.rust_route=='native' else 'rstim.postselected-counts.v1',manifest=manifest,cases=cases,
        selected_names=args.only,selected_shots=args.shots,pairs=args.pairs,repetitions=args.repetitions,
        source_revision=revision,sources=sources,binary_sha256=binary_hashes,packages=packages,
        peer_loaded_files=identities,symft_source_revision=peer_revision,symft_sources=peer_sources,
        environment_before=env,raw_reference_timeout_s=600,
        host=dict(platform=platform.platform(),machine=platform.machine(),cpus=os.cpu_count(),
            affinity=sorted(os.sched_getaffinity(0)) if hasattr(os,'sched_getaffinity') else None,load=os.getloadavg()),
        compiler=subprocess.check_output(['rustc','-vV'],text=True),
        compiler_environment={key:os.environ.get(key) for key in
            ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CC','CXX','CFLAGS','CXXFLAGS']},
        started_utc=datetime.now(timezone.utc).isoformat())
    if args.rust_route=='native': header['rust_route']='native'
    (out/'header.json').write_text(json.dumps(header,indent=2)+'\n')
    index=0
    def record(kind,**payload):
        nonlocal index
        event=dict(index=index,kind=kind,**payload);index+=1
        with (out/'events.jsonl').open('a') as f:
            f.write(json.dumps(event,separators=(',',':'))+'\n');f.flush();os.fsync(f.fileno())
        print(event['index'],kind,payload.get('id',payload.get('name','')),flush=True)
    def peer(backend,name,shots,batch,validate=False):
        python=args.symft_python if backend=='symft' else args.python
        result=diag.invoke([str(python),'-I',str(HERE/'worker.py'),backend,
            str(ROOT/manifest['inputs'][name]['path']),str(shots),'--batch',str(batch),
            '--repetitions',str(1 if validate else args.repetitions)]+(['--validate'] if validate else []))
        if result.get('status')=='ok': inc.bind_peer(result,backend,packages,identities)
        return result
    def rust(case,validate=False):
        return diag.invoke([binaries['near-clifford-application-counts'],str(ROOT/manifest['inputs'][case['name']]['path']),
            str(case['shots']),str(1 if validate else args.repetitions),case['policy'],'validate' if validate else 'bench',args.rust_route])
    # No gate lowering: inspect all eleven complete original inputs independently.
    for name,value in manifest['inputs'].items():
        for policy in POLICIES:
            invocation=out/'invocation.json'
            invocation.write_text(json.dumps(dict(action='inspect',circuit=str(ROOT/value['path']),shots=1,
                repetitions=1,arithmetic=policy,cache_bytes=67108864)))
            record('capability',name=name,policy=policy,result=diag.invoke([binaries['near-clifford-diagnostics'],str(invocation)]))
    selected={};count_witnesses={};raw_witnesses={}
    for case in cases:
        name,shots=case['name'],case['shots'];key=(name,shots)
        if key not in selected:
            selected[key]={}
            for backend in inc.BACKENDS[1:]:
                trials=[dict(batch=batch,result=peer(backend,name,shots,batch)) for batch in inc.batches(backend)]
                valid=[trial for trial in trials if trial['result'].get('status')=='ok']
                batch=min(valid,key=lambda t:statistics.median(o['ns_per_call'] for o in t['result']['observations']))['batch'] if valid else None
                record('tuning',id=case['id'],backend=backend,trials=trials,selected=batch)
                if batch is None: continue
                selected[key][backend]=batch
                count_witnesses[(name,shots,backend)]=peer(backend,name,shots,batch,True)
                record('counts-validation',id=case['id'],backend=backend,result=count_witnesses[(name,shots,backend)])
                original=ROOT/manifest['inputs'][name]['path'];native=out/(name+'.records.stim')
                native.write_text(records_only(original.read_text()))
                python=args.symft_python if backend=='symft' else args.python
                raw=diag.invoke([str(python),'-I',str(HERE.parent/'compiled_sota/worker.py'),backend,str(native),str(shots),
                    '--batch',str(batch),'--mode','dump','--dump-total',str(math.ceil(8192/shots)*shots)],timeout=600)
                raw_witnesses[(name,shots,backend)]=raw
                record('raw-validation',id=case['id'],backend=backend,result=compact(raw) if 'measurements' in raw else raw)
        raw=rust(case,True)
        record('counts-validation',id=case['id'],backend='rstim',result=compact(raw) if 'measurements' in raw else raw)
        if raw.get('status')!='ok' or len(selected[key])!=3:
            record('rejected',id=case['id'],reason='counts executor unsupported or peers incomplete');continue
        masks=annotations((ROOT/manifest['inputs'][name]['path']).read_text())
        reference=raw_counts(raw,masks);checks=[]
        exact=reference==counts(raw)
        for backend in inc.BACKENDS[1:]:
            native=count_witnesses[(name,shots,backend)];witness=raw_witnesses[(name,shots,backend)]
            if native.get('status')!='ok' or 'measurements' not in witness:
                checks.append(dict(backend=backend,passed=False,reason='missing witness'));continue
            alpha=0.001/max(1,len(cases)*3*4)
            checks.append(dict(backend=backend,against_rust=compare(counts(native),reference,alpha),
                against_own_records=compare(counts(native),raw_counts(witness,masks),alpha)))
        passed=exact and all(c.get('against_rust',{}).get('passed') and
                            c.get('against_own_records',{}).get('passed') for c in checks)
        record('validation-check',id=case['id'],exact_rust_counts=exact,checks=checks,passed=passed)
        if not passed:
            record('rejected',id=case['id'],reason='counts finite witness disagreement');continue
        for pair in range(args.pairs):
            order=inc.BACKENDS[pair%4:]+inc.BACKENDS[:pair%4]
            if pair%2: order=order[::-1]
            for backend in order:
                result=rust(case) if backend=='rstim' else peer(backend,name,shots,selected[key][backend])
                record('timing',id=case['id'],pair=pair,backend=backend,result=result)
    after=inventory();require(after==sources,'source changed during campaign')
    packages_after=inc.capture_packages(args.python,args.symft_python)
    identities_after={name:v['loaded_files'] for name,v in inc.capture_identities(args.python,args.symft_python).items()}
    rev_after,peer_after=inc.capture_symft_source(args.symft_source)
    require(inc.environment_summary(packages_after,peer_after,rev_after,identities_after)==env,'peer environment changed')
    hashes_after={name:digest(Path(path).read_bytes()) for name,path in binaries.items()}
    require(hashes_after==binary_hashes,'binary changed')
    closure=dict(events=index,events_sha256=digest((out/'events.jsonl').read_bytes()),sources_after=after,
        binary_sha256=hashes_after,environment_after=env,finished_utc=datetime.now(timezone.utc).isoformat())
    (out/'closure.json').write_text(json.dumps(closure,indent=2)+'\n')


if __name__=='__main__': main()
