"""Run source-bound diagnostic cells with frozen peer tuning and checkpoints."""
import argparse
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent / 'compiled_sota'))
import run as incumbent
from evidence import compact, parity_counts
from projection import records_only
from corpus import build, BUDGETS, POLICIES


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory():
    paths = list((ROOT/'rstim/src').rglob('*.rs'))
    paths += [ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'rstim/Cargo.toml']
    paths += [p for p in HERE.rglob('*') if p.is_file()
              and not any(part in ('target','__pycache__') for part in p.relative_to(HERE).parts)]
    paths += list((HERE.parent/'compiled_sota').glob('*.py'))
    paths += [HERE.parent/'compiled_sota/manifest.json']
    paths += list((HERE.parent/'compiled_sota/fixtures').glob('*.stim'))
    return {str(p.relative_to(ROOT)):sha(p) for p in sorted(paths)}


def invoke(command, timeout=180):
    started = time.monotonic()
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=timeout)
        if result.returncode:
            return dict(status='process-error', command=command, exit_code=result.returncode,
                        stderr=result.stderr[-4000:], stdout=result.stdout[-1000:])
        value = json.loads(result.stdout)
        value['launcher_elapsed_s'] = time.monotonic()-started
        return value
    except subprocess.TimeoutExpired:
        return dict(status='timeout', command=command, timeout_s=timeout)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--python', type=Path, required=True)
    p.add_argument('--symft-python', type=Path, required=True)
    p.add_argument('--symft-source', type=Path, required=True)
    p.add_argument('--groups', nargs='+', choices=['cache','boundary','structure','incumbent','lifetime'],
                   default=['cache','boundary','structure','incumbent','lifetime'])
    p.add_argument('--pairs', type=int, default=5)
    p.add_argument('--repetitions', type=int, default=7)
    p.add_argument('--only', nargs='+')
    args = p.parse_args()
    if not 1 <= args.pairs <= 20 or not 1 <= args.repetitions <= 64:
        p.error('pairs/repetitions out of bounds')
    out = args.out.resolve()
    if out.exists():
        p.error('out must be a new directory')
    out.mkdir(parents=True)
    manifest = build(out/'circuits')
    manifest['pairs'], manifest['repetitions'] = args.pairs, args.repetitions
    cells = [c for c in manifest['cells'] if set(c['groups']) & set(args.groups)
             and (not args.only or c['fixture'] in args.only)]
    manifest['selected_cells'] = [c['id'] for c in cells]
    manifest['selected_groups'] = args.groups
    manifest['only'] = args.only
    for fixture in manifest['fixtures'].values():
        full = Path(fixture['path'])
        native = full.with_suffix('.records.stim')
        native.write_text(records_only(full.read_text()))
        fixture.update(native_path=str(native), native_sha256=sha(native))
    files = inventory()
    source = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    source_clean = not subprocess.check_output(['git','diff','--name-only','HEAD','--','rstim/src'],cwd=ROOT,text=True)
    build_result = subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(HERE/'Cargo.toml')],cwd=ROOT)
    if build_result.returncode or inventory() != files or not source_clean:
        raise ValueError('build or source guard failed')
    binary = HERE/'target/release/near-clifford-diagnostics'
    packages = incumbent.capture_packages(args.python, args.symft_python)
    identities = {name:value['loaded_files'] for name,value in
                  incumbent.capture_identities(args.python,args.symft_python).items()}
    peer_revision, peer_sources = incumbent.capture_symft_source(args.symft_source)
    guard = incumbent.environment_summary(packages,peer_sources,peer_revision,identities)
    header = dict(schema=manifest['schema'], source_revision=source, sources=files,
                  manifest=manifest, packages=packages, peer_loaded_files=identities,
                  symft_source_revision=peer_revision, symft_sources=peer_sources,
                  environment_before=guard, binary_sha256=sha(binary),
                  host=dict(platform=platform.platform(),machine=platform.machine(),cpus=os.cpu_count(),
                            affinity=sorted(os.sched_getaffinity(0)) if hasattr(os,'sched_getaffinity') else None,
                            load=os.getloadavg(),python=sys.version),
                  compiler=subprocess.check_output(['rustc','-vV'],text=True),
                  compiler_environment={key:os.environ.get(key) for key in
                    ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CC','CXX','CFLAGS','CXXFLAGS']},
                  started_utc=datetime.now(timezone.utc).isoformat(),
                  timing_contract='public API only; lifecycle phase sum excludes diagnostic conversion/drop; no postselection')
    (out/'header.json').write_text(json.dumps(header,indent=2)+'\n')
    index = 0
    def record(kind, **payload):
        nonlocal index
        event = dict(index=index,kind=kind,**payload)
        index += 1
        with (out/'events.jsonl').open('a') as f:
            f.write(json.dumps(event,separators=(',',':'))+'\n')
            f.flush()
            os.fsync(f.fileno())
        print(f"{event['index']} {kind} {payload.get('id','')}",flush=True)
    def rust(cell, action, **extras):
        config = dict(cell, action=action, repetitions=args.repetitions,
                      circuit=manifest['fixtures'][cell['fixture']]['native_path'],**extras)
        path = out/'invocation.json'
        path.write_text(json.dumps(config))
        result = invoke([str(binary),str(path)])
        if result.get('status') == 'ok':
            if result['input_sha256'] != manifest['fixtures'][cell['fixture']]['native_sha256']:
                raise ValueError('probe input binding failed')
        return result
    def peer(backend, fixture, shots, batch, mode, **extras):
        python = args.symft_python if backend == 'symft' else args.python
        command = [str(python),'-I',str(HERE.parent/'compiled_sota/worker.py'),backend,
                   fixture['native_path'],str(shots),'--batch',str(batch),'--mode',mode,
                   '--repetitions',str(args.repetitions)]
        if 'total' in extras:
            command += ['--dump-total',str(extras['total'])]
        result = invoke(command)
        if 'loaded_files' in result:
            if result['input_sha256'] != fixture['native_sha256']:
                raise ValueError('peer input binding failed')
            incumbent.bind_peer(result,backend,packages if backend=='symft' else packages,identities)
        return result
    tuning = {}
    validation = {}
    rust_validation = {}
    # Conservative finite parity witnesses, not proof of arbitrary joint equality.
    # Threshold allocates alpha across every selected cell and backend-pair mask.
    for ordinal, cell in enumerate(cells):
        fixture = manifest['fixtures'][cell['fixture']]
        inspect = rust(cell,'inspect')
        record('inspect',id=cell['id'],result=inspect)
        if inspect.get('status') != 'ok':
            continue
        total = math.ceil(8192/cell['shots'])*cell['shots']
        raw = rust(cell,'dump',total=total)
        if raw.get('status') != 'ok':
            record('validation-error',id=cell['id'],result=raw)
            continue
        masks = incumbent.masks(Path(fixture['path']).read_text(),raw['width'])
        transcript_key = (cell['fixture'],cell['shots'])
        if transcript_key not in tuning:
            tuning[transcript_key] = {}
            validation[transcript_key] = {}
            for backend in incumbent.BACKENDS[1:]:
                trials = [dict(batch=batch,result=peer(backend,fixture,cell['shots'],batch,'tune'))
                          for batch in incumbent.batches(backend)]
                valid = [t for t in trials if 'warm_ns' in t['result']]
                if not valid:
                    record('tuning-error',id=cell['id'],backend=backend,trials=trials)
                    continue
                chosen = min(valid,key=lambda t:statistics.median(t['result']['warm_ns']))['batch']
                tuning[transcript_key][backend] = chosen
                witness = peer(backend,fixture,cell['shots'],chosen,'dump',total=total)
                record('peer-tuning',id=cell['id'],backend=backend,trials=trials,selected=chosen)
                if 'measurements' in witness:
                    validation[transcript_key][backend] = witness
                    record('peer-validation',id=cell['id'],backend=backend,result=compact(witness))
                else:
                    record('peer-validation-error',id=cell['id'],backend=backend,result=witness)
        witnesses = dict(validation[transcript_key],rstim=raw)
        counts = {backend:parity_counts(value,masks) for backend,value in witnesses.items()}
        comparisons = []
        for left,right in itertools.combinations(counts,2):
            alpha = 0.0005/max(1,len(cells)*6*len(masks))
            n,m = witnesses[left]['shots'],witnesses[right]['shots']
            bound = math.sqrt(math.log(2/alpha)/(2*n))+math.sqrt(math.log(2/alpha)/(2*m))
            delta = max(abs(a/n-b/m) for a,b in zip(counts[left],counts[right]))
            comparisons.append(dict(left=left,right=right,bound=bound,max_delta=delta,passed=delta<=bound))
        record('validation',id=cell['id'],result=compact(raw),masks=masks,comparisons=comparisons)
        if cell['fixture']=='msc5':
            rust_validation[(cell['fixture'],cell['shots'],cell['cache_bytes'],cell['arithmetic'],'flat')]=raw
        if len(witnesses) != 4 or not all(c['passed'] for c in comparisons):
            record('rejected',id=cell['id'],reason='incomplete or failed finite distribution witnesses')
            continue
        backends = incumbent.BACKENDS
        for pair in range(args.pairs):
            order = backends[pair%4:]+backends[:pair%4]
            if pair%2: order=order[::-1]
            for backend in order:
                value = rust(cell,'bench') if backend=='rstim' else peer(
                    backend,fixture,cell['shots'],tuning[transcript_key][backend],'bench')
                record('timing',id=cell['id'],pair=pair,order=order,backend=backend,result=value)
    if 'lifetime' in args.groups:
        reference = {}
        for policy in POLICIES:
            for name,history in manifest['histories'].items():
                fixture=manifest['fixtures']['msc5']
                representative=max(request['shots'] for request in history)
                chosen={}
                if policy==POLICIES[0]:
                    for backend in incumbent.BACKENDS[1:]:
                        trials=[dict(batch=batch,result=peer(backend,fixture,representative,batch,'tune'))
                                for batch in incumbent.batches(backend)]
                        valid=[t for t in trials if 'warm_ns' in t['result']]
                        if valid:
                            chosen[backend]=min(valid,key=lambda t:statistics.median(t['result']['warm_ns']))['batch']
                        record('lifetime-tuning',id=name,backend=backend,trials=trials,
                               selected=chosen.get(backend),selection_shots=representative)
                    for request in sorted({ (v['kind'],v['shots']) for v in history }):
                        kind,count=request
                        total=math.ceil(8192/count)*count
                        for backend,batch in chosen.items():
                            witness=peer(backend,fixture,count,batch,'dump',total=total)
                            record('lifetime-peer-validation',id=name,backend=backend,kind_requested=kind,
                                   call_shots=count,selected=batch,
                                   result=compact(witness) if 'measurements' in witness else witness)
                else:
                    chosen={}
                for kind,count in sorted({(v['kind'],v['shots']) for v in history}):
                    total=math.ceil(8192/count)*count
                    for budget in BUDGETS:
                        key=('msc5',count,budget,policy,kind)
                        if key not in rust_validation:
                            cell=dict(fixture='msc5',shots=count,cache_bytes=budget,arithmetic=policy)
                            rust_validation[key]=rust(cell,'dump',total=total,call_kind=kind)
                        witness=rust_validation[key]
                        record('lifetime-validation',id=name,call_shots=count,kind_requested=kind,
                               arithmetic=policy,cache_bytes=budget,
                               result=compact(witness) if 'measurements' in witness else witness)
                for pair in range(args.pairs):
                    budgets=BUDGETS[pair%4:]+BUDGETS[:pair%4]
                    if pair%2: budgets=budgets[::-1]
                    for budget in budgets:
                        cell = dict(id=f'msc5/lifetime/{name}/c{budget}/{policy}',fixture='msc5',
                                    shots=1,cache_bytes=budget,arithmetic=policy)
                        value=rust(cell,'lifetime',history=history)
                        if value.get('status')=='ok':
                            outputs=[(r['outputs_sha256'],r['continuation']) for r in value['histories']]
                            key=(policy,name,pair)
                            if key in reference and outputs != reference[key]:
                                raise ValueError('cache budget changed same-plan lifetime raw records/RNG')
                            reference[key]=outputs
                        record('lifetime',id=cell['id'],pair=pair,result=value)
                    for backend,batch in chosen.items():
                        config=dict(backend=backend,batch=batch,history=history,
                                    repetitions=args.repetitions,circuit=fixture['native_path'])
                        path=out/'invocation.json'
                        path.write_text(json.dumps(config))
                        python=args.symft_python if backend=='symft' else args.python
                        value=invoke([str(python),'-I',str(HERE/'peer_lifetime.py'),str(path)])
                        if value.get('status')=='ok':
                            incumbent.bind_peer(value,backend,packages,identities)
                            if value['input_sha256']!=fixture['native_sha256']:
                                raise ValueError('lifetime peer input changed')
                        record('peer-lifetime',id=name,pair=pair,backend=backend,result=value)
    packages_after=incumbent.capture_packages(args.python,args.symft_python)
    identities_after={name:value['loaded_files'] for name,value in
                      incumbent.capture_identities(args.python,args.symft_python).items()}
    revision_after,sources_after=incumbent.capture_symft_source(args.symft_source)
    after=incumbent.environment_summary(packages_after,sources_after,revision_after,identities_after)
    if after!=guard or inventory()!=files or sha(binary)!=header['binary_sha256']:
        raise ValueError('end source/environment/binary guard failed')
    (out/'closure.json').write_text(json.dumps(dict(completed_utc=datetime.now(timezone.utc).isoformat(),
        events=index,events_sha256=sha(out/'events.jsonl'),environment_after=after,
        sources_after=files,binary_sha256=sha(binary)),indent=2)+'\n')


if __name__=='__main__':
    main()
