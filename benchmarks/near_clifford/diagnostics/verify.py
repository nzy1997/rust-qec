"""Replay diagnostic coverage, timing arithmetic and finite parity witnesses."""
import argparse
import hashlib
import gzip
import itertools
import json
import math
import re
from pathlib import Path
import subprocess
import sys
import tempfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
sys.path.insert(0,str(HERE.parent/'compiled_sota'))
from evidence import expand,parity_counts
from projection import records_only
import run as incumbent
from corpus import build,BASELINE


def require(condition,message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def peer_warm(result,repetitions):
    values,totals,calls=result['warm_ns'],result['warm_totals_ns'],result['warm_calls']
    require(len(values)==len(totals)==len(calls)==repetitions,'peer warm array length mismatch')
    for value,total,count in zip(values,totals,calls):
        require(type(count) is int and count>0 and type(total) is int and total>=50_000_000,
                'invalid peer timing interval')
        require(math.isfinite(value) and value>0 and value==total/count,'invalid peer timing arithmetic')


def selected_trial(event,header):
    backend=event['backend']
    batches=incumbent.batches(backend)
    require([trial['batch'] for trial in event['trials']]==batches,'missing tuning candidates')
    valid=[]
    manifest=header['manifest']
    if event['kind']=='lifetime-tuning':
        fixture,shots='msc5',event['selection_shots']
    else:
        cell=next(c for c in manifest['cells'] if c['id']==event['id'])
        fixture,shots=cell['fixture'],cell['shots']
    for trial in event['trials']:
        result=trial['result']
        if 'warm_ns' in result:
            require(result.get('backend')==backend,'tuning executor differs from event backend')
            incumbent.bind_peer(result,backend,header['packages'],header['peer_loaded_files'])
            require(result['input_sha256']==manifest['fixtures'][fixture]['native_sha256']
                    and result['shots']==shots and result['threads']==1,'tuning input/shot/thread mismatch')
            require(result['batch']==trial['batch'],'tuning batch metadata mismatch')
            peer_warm(result,header['manifest']['repetitions'])
            valid.append(trial)
    require(valid,'no valid tuning candidates')
    import statistics
    chosen=min(valid,key=lambda trial:statistics.median(trial['result']['warm_ns']))['batch']
    require(event['selected']==chosen,'frozen tuning winner differs from trials')
    return chosen


def validate(out,git_sources=False,allow_smoke=False):
    header=json.loads((out/'header.json').read_text())
    closure=json.loads((out/'closure.json').read_text())
    events_bytes=(out/'events.jsonl').read_bytes() if (out/'events.jsonl').exists() else gzip.decompress((out/'events.jsonl.gz').read_bytes())
    events=[json.loads(line) for line in events_bytes.splitlines()]
    require(closure['events']==len(events),'event count mismatch')
    require(closure['events_sha256']==digest(events_bytes),'event digest mismatch')
    require([e['index'] for e in events]==list(range(len(events))),'event indices incomplete')
    require(closure['sources_after']==header['sources'],'source closure mismatch')
    require(closure['binary_sha256']==header['binary_sha256'],'binary closure mismatch')
    require(closure['environment_after']==header['environment_before'],'peer environment closure mismatch')
    require(bool(header['sources']) and bool(header['symft_sources']),'missing source inventories')
    mandatory={'benchmarks/near_clifford/diagnostics/'+name for name in
               ['.gitignore','Cargo.toml','Cargo.lock','main.rs','corpus.py','run.py',
                'peer_lifetime.py','verify.py','test_contract.py','README.md']}
    require(mandatory <= set(header['sources']),'mandatory diagnostic harness sources missing')
    require(incumbent.environment_summary(header['packages'],header['symft_sources'],
            header['symft_source_revision'],header['peer_loaded_files'])==header['environment_before'],
            'peer environment receipt mismatch')
    require(header['symft_source_revision']=='c89b98514a919240b8afa53a271e08d926d3c987',
            'wrong pinned SymFT source')
    require(header['packages']['symft']['version']=='0.1.1' and
            header['packages']['clifft_environment']['clifft']['version']=='0.11.0','wrong peer versions')
    manifest=header['manifest']
    require(manifest['schema']=='rstim.near-clifford-diagnostics.v1','wrong schema')
    require(manifest['baseline']==BASELINE,'wrong frozen production baseline')
    with tempfile.TemporaryDirectory() as temporary:
        expected=build(Path(temporary))
    require(manifest['cells']==expected['cells'] and manifest['histories']==expected['histories'],
            'diagnostic matrix differs from deterministic corpus')
    selected=[c['id'] for c in manifest['cells'] if set(c['groups']) & set(manifest['selected_groups'])
              and (not manifest.get('only') or c['fixture'] in manifest['only'])]
    require(selected==manifest['selected_cells'],'selected matrix incomplete')
    if not allow_smoke:
        require(manifest['pairs']>=5 and manifest['repetitions']>=7,'insufficient formal repetitions')
    full_texts={}
    for name,fixture in manifest['fixtures'].items():
        full=(out/'circuits'/f'{name}.stim').read_bytes()
        native=(out/'circuits'/f'{name}.records.stim').read_bytes()
        require(digest(full)==fixture['sha256'] and digest(native)==fixture['native_sha256'],'fixture hash mismatch')
        require(fixture['sha256']==expected['fixtures'][name]['sha256'],'fixture differs from deterministic corpus')
        require(records_only(full.decode()).encode()==native,'native projection mismatch')
        full_texts[name]=full.decode()
    if git_sources:
        tracked=subprocess.check_output(['git','ls-tree','-r','--name-only',header['source_revision']],cwd=ROOT,text=True).splitlines()
        prefix='benchmarks/near_clifford/'
        expected_paths={p for p in tracked if p.startswith('rstim/src/') and p.endswith('.rs')
            or p in ['Cargo.toml','Cargo.lock','rstim/Cargo.toml']
            or p.startswith(prefix+'diagnostics/') and not any(part in ['target','__pycache__'] for part in p.split('/'))
            or p.startswith(prefix+'compiled_sota/') and
               (p.endswith('.py') and p.count('/')==3 or p==prefix+'compiled_sota/manifest.json'
                or p.startswith(prefix+'compiled_sota/fixtures/') and p.endswith('.stim'))}
        require(set(header['sources'])==expected_paths,'source inventory incomplete or extra paths')
        require(mandatory <= expected_paths,'source revision predates diagnostic harness')
        for path,sha in header['sources'].items():
            data=subprocess.check_output(['git','show',header['source_revision']+':'+path],cwd=ROOT)
            require(digest(data)==sha,'source revision mismatch: '+path)
            if path.startswith('rstim/src/') or path.startswith(prefix+'compiled_sota/fixtures/') or path==prefix+'compiled_sota/manifest.json':
                baseline=subprocess.check_output(['git','show',manifest['baseline']+':'+path],cwd=ROOT)
                require(data==baseline,'production differs from frozen baseline: '+path)
    raw_peers={}
    tuning={}
    lifetime_tuning={}
    lifetime_peer_validation={}
    lifetime_rust_validation={}
    inspections={}
    validations={}
    timings={}
    lifetime_outputs={}
    lifetime_cells=set()
    peer_lifetime_cells=set()
    failures=[]
    cells={c['id']:c for c in manifest['cells']}
    identities=header['peer_loaded_files']
    for event in events:
        kind=event['kind']
        result=event.get('result',{})
        if result.get('status')=='ok' or 'loaded_files' in result or 'warm_ns' in result:
            require('input_sha256' in result,'successful payload missing consumed-input identity')
        peer_event=(kind in ['peer-validation','lifetime-peer-validation','peer-lifetime'] or
                    kind=='timing' and event.get('backend')!='rstim')
        if peer_event and ('measurements' in result or 'warm_ns' in result or result.get('status')=='ok'):
            require('loaded_files' in result and result.get('isolated') is True,
                    'successful peer payload missing import identity/isolation')
            require(result.get('backend')==event['backend'],'peer executor differs from event backend')
        if 'input_sha256' in result:
            name=event['id'].split('/')[0]
            if kind.startswith('lifetime') or kind=='peer-lifetime': name='msc5'
            require(result['input_sha256']==manifest['fixtures'][name]['native_sha256'],'input identity mismatch')
        if 'loaded_files' in result:
            backend=event['backend']
            name='clifft' if backend.startswith('clifft') else 'symft'
            require(result['loaded_files']==identities[name] and result['isolated'] is True,'peer identity mismatch')
            incumbent.bind_peer(result,backend,header['packages'],identities)
        if kind=='peer-tuning':
            cell=cells[event['id']]
            key=(cell['fixture'],cell['shots'],event['backend'])
            require(key not in tuning,'duplicate tuning selection')
            tuning[key]=selected_trial(event,header)
        elif kind=='lifetime-tuning':
            name=event['id']
            require(name in manifest['histories'],'unknown lifetime tuning history')
            require((name,event['backend']) not in lifetime_tuning,'duplicate lifecycle tuning')
            require(event['selection_shots']==max(v['shots'] for v in manifest['histories'][name]),
                    'wrong lifetime tuning shot size')
            lifetime_tuning[(name,event['backend'])]=selected_trial(event,header)
        if kind=='inspect':
            require(event['id'] in cells,'unknown inspected cell')
            inspections[event['id']]=result
            if result['status']!='ok': failures.append(event)
        elif kind=='peer-validation':
            raw=expand(result)
            key=(event['id'].split('/')[0],raw['call_shots'],event['backend'])
            require(key in tuning and raw['batch']==tuning[key],'validation batch differs from tuning')
            raw_peers[key]=raw
        elif kind=='validation':
            require(event['id'] in cells,'unknown validation cell')
            cell=cells[event['id']]
            raw=expand(result)
            require(raw['call_shots']==cell['shots'] and raw['shots']>=8192,'wrong validation shot count')
            require(raw['arithmetic']==cell['arithmetic'] and raw['cache_bytes']==cell['cache_bytes'],'validation configuration mismatch')
            masks=event['masks']
            require(masks==[list(mask) for mask in incumbent.masks(full_texts[cell['fixture']],raw['width'])],
                    'validation masks differ from full input')
            witnesses={'rstim':raw}
            for backend in ['clifft','clifft-scheduled','symft']:
                key=(cell['fixture'],cell['shots'],backend)
                if key in raw_peers: witnesses[backend]=raw_peers[key]
            counts={b:parity_counts(v,masks) for b,v in witnesses.items()}
            require(all(v['width']==raw['width'] for v in witnesses.values()),'backend record width mismatch')
            recomputed=[]
            order=[b for b in ['clifft','clifft-scheduled','symft','rstim'] if b in counts]
            for left,right in itertools.combinations(order,2):
                n,m=witnesses[left]['shots'],witnesses[right]['shots']
                alpha=0.0005/max(1,len(manifest['selected_cells'])*6*len(masks))
                bound=math.sqrt(math.log(2/alpha)/(2*n))+math.sqrt(math.log(2/alpha)/(2*m))
                delta=max(abs(a/n-b/m) for a,b in zip(counts[left],counts[right]))
                recomputed.append(dict(left=left,right=right,bound=bound,max_delta=delta,passed=delta<=bound))
            require(recomputed==event['comparisons'],'finite witness comparison mismatch')
            validations[event['id']]=len(witnesses)==4 and all(c['passed'] for c in recomputed)
        elif kind=='timing':
            cell=cells[event['id']]
            key=(event['id'],event['pair'],event['backend'])
            require(key not in timings,'duplicate timing')
            timings[key]=result
            if 'warm_ns' in result:
                require(result['shots']==cell['shots'],'peer timing shots mismatch')
                peer_warm(result,manifest['repetitions'])
                selected=tuning[(cell['fixture'],cell['shots'],event['backend'])]
                require(result['batch']==selected,'timing batch differs from validated frozen selection')
                require(result['threads']==1 and result['width']==raw_peers[(cell['fixture'],cell['shots'],event['backend'])]['width'],
                        'timed peer thread count or record width mismatch')
            elif result.get('status')=='ok':
                require(result['arithmetic']==cell['arithmetic'] and result['cache_bytes']==cell['cache_bytes'],'timing policy/budget mismatch')
                require(result['shots']==cell['shots'],'timing shots mismatch')
                require(len(result['warm'])==manifest['repetitions'],'missing probe observations')
                for obs in result['warm']:
                    require(obs['elapsed_ns']>=50_000_000 and obs['calls']>0
                            and obs['ns_per_call']==obs['elapsed_ns']/obs['calls'],'invalid probe timing arithmetic')
            else: failures.append(event)
            if result.get('status')=='ok':
                require(result['peak_rss_bytes']>0,'missing measured RSS')
        elif kind in ('lifetime-peer-validation','lifetime-validation'):
            name=event['id']
            requested=(event['kind_requested'],event['call_shots'])
            require(requested in {(v['kind'],v['shots']) for v in manifest['histories'][name]},
                    'unknown lifecycle validation call')
            if kind=='lifetime-peer-validation':
                key=(name,*requested,event['backend'])
                require(key not in lifetime_peer_validation,'duplicate lifecycle peer witness')
                require(result.get('batch')==lifetime_tuning[(name,event['backend'])],
                        'lifetime witness batch differs from frozen tuning')
                lifetime_peer_validation[key]=expand(result)
            else:
                policy,budget=event['arithmetic'],event['cache_bytes']
                require(result['arithmetic']==policy and result['cache_bytes']==budget and
                        result['call_kind']==requested[0] and result['call_shots']==requested[1],
                        'lifetime witness configuration mismatch')
                raw=expand(result)
                require(raw['shots']>=8192,'insufficient lifecycle validation samples')
                require((name,*requested,policy,budget) not in lifetime_rust_validation,
                        'duplicate lifecycle Rust witness')
                lifetime_rust_validation[(name,*requested,policy,budget)]=raw
        elif kind in ('lifetime','peer-lifetime'):
            require(type(event['pair']) is int and 0<=event['pair']<manifest['pairs'],'lifecycle pair out of range')
            key=((event['id'],event['pair']) if kind=='lifetime' else
                 (event['id'],event['pair'],event['backend'],event['arithmetic_context']))
            seen=lifetime_cells if kind=='lifetime' else peer_lifetime_cells
            require(key not in seen,'duplicate lifecycle observation')
            seen.add(key)
            if result.get('status')!='ok':
                failures.append(event)
                continue
            require(len(result['histories'])==manifest['repetitions'],'missing lifetime repetitions')
            if kind=='lifetime':
                parts=event['id'].split('/')
                require(len(parts)==5 and parts[:2]==['msc5','lifetime'],'invalid lifecycle event ID')
                name,budget,policy=parts[2],int(parts[3][1:]),parts[4]
                require(name in manifest['histories'] and budget in [0,1<<20,16<<20,64<<20]
                        and policy in ['strict','fused'],'unknown lifecycle cell')
                require(result['arithmetic']==policy and result['cache_bytes']==budget and
                        result['config']['history']==manifest['histories'][name] and
                        result['config']['repetitions']==manifest['repetitions'] and
                        result['config']['arithmetic']==policy and result['config']['cache_bytes']==budget and
                        result['input_sha256']==manifest['fixtures']['msc5']['native_sha256'],
                        'lifetime payload differs from event configuration')
            else:
                name=event['id']
                require(name in manifest['histories'] and
                        result['config']['history']==manifest['histories'][name] and
                        result['config']['backend']==event['backend'] and
                        event['arithmetic_context'] in ['strict','fused'] and
                        result['config']['arithmetic_context']==event['arithmetic_context'] and
                        result['config']['repetitions']==manifest['repetitions'] and
                        result['config']['batch']==lifetime_tuning[(name,event['backend'])],
                        'peer lifetime configuration mismatch')
            units=[['rstim',budget] for budget in [0,1<<20,16<<20,64<<20]]+[
                   [backend,lifetime_tuning[(name,backend)]] for backend in ['clifft','clifft-scheduled','symft']]
            offset=event['pair']%len(units)
            units=units[offset:]+units[:offset]
            if event['pair']%2: units=units[::-1]
            require(event['order']==units,'lifecycle process order differs from rotated/reversed protocol')
            for index,row in enumerate(result['histories']):
                require([c['request'] for c in row['calls']]==manifest['histories'][name],
                        'measured lifecycle calls differ from configured history')
                require(row['sampling_ns']==sum(c['ns'] for c in row['calls']),'lifetime API sum mismatch')
                require(row['phase_sum_ns']==row['compile_ns']+row['prepare_ns']+row['sampling_ns'],'lifetime phase sum mismatch')
                require(all(c['ns']>0 for c in row['calls']),'nonpositive lifetime call')
                if kind=='lifetime':
                    require(isinstance(row['outputs_sha256'],str) and
                            re.fullmatch('[0-9a-f]{64}',row['outputs_sha256']) is not None,
                            'invalid lifecycle record digest')
                    require(len(row['continuation'])==16 and
                            all(type(v) is int and 0<=v<2**64 for v in row['continuation']),
                            'invalid lifecycle RNG continuation')
                    history=result['config']['history']
                    require([c['request'] for c in row['calls']]==history,'lifetime history mismatch')
                    policy=result['arithmetic']
                    name=event['id'].split('/')[2]
                    key=(policy,name,event['pair'],index)
                    output=(row['outputs_sha256'],row['continuation'])
                    if key in lifetime_outputs:
                        require(lifetime_outputs[key]==output,'cache budgets changed same-plan records/RNG')
                    lifetime_outputs[key]=output
        elif kind.endswith('error') or kind=='rejected': failures.append(event)
    for id in manifest['selected_cells']:
        require(id in inspections,'missing inspection: '+id)
        if inspections[id]['status']=='ok':
            require(id in validations or any(e['id']==id for e in failures),'missing validation: '+id)
            if validations.get(id):
                for pair in range(manifest['pairs']):
                    for backend in ['rstim','clifft','clifft-scheduled','symft']:
                        require((id,pair,backend) in timings,'missing timing: '+id)
    if 'lifetime' in manifest['selected_groups']:
        actual={(e['id'],e['pair']) for e in events if e['kind']=='lifetime'}
        for policy in ['strict','fused']:
            for name in manifest['histories']:
                for budget in [0,1<<20,16<<20,64<<20]:
                    for pair in range(manifest['pairs']):
                        require((f'msc5/lifetime/{name}/c{budget}/{policy}',pair) in actual,
                                'missing lifetime cell')
        peer_events=peer_lifetime_cells
        for name,history in manifest['histories'].items():
            for backend in ['clifft','clifft-scheduled','symft']:
                require((name,backend) in lifetime_tuning,'missing lifetime tuning')
                for policy in ['strict','fused']:
                    for pair in range(manifest['pairs']):
                        require((name,pair,backend,policy) in peer_events,'missing peer lifetime timing')
            for kind,shots in sorted({(v['kind'],v['shots']) for v in history}):
                peers={b:lifetime_peer_validation[(name,kind,shots,b)]
                       for b in ['clifft','clifft-scheduled','symft']}
                masks=incumbent.masks(full_texts['msc5'],next(iter(peers.values()))['width'])
                peer_counts={b:parity_counts(raw,masks) for b,raw in peers.items()}
                for policy in ['strict','fused']:
                    for budget in [0,1<<20,16<<20,64<<20]:
                        raw=lifetime_rust_validation[(name,kind,shots,policy,budget)]
                        counts=parity_counts(raw,masks)
                        for backend,other in peers.items():
                            require(other['call_shots']==shots and other['shots']>=8192 and other['width']==raw['width'],
                                    'lifecycle witness shape mismatch')
                            alpha=0.0005/(len(manifest['histories'])*8*8*6*len(masks))
                            n,m=raw['shots'],other['shots']
                            bound=math.sqrt(math.log(2/alpha)/(2*n))+math.sqrt(math.log(2/alpha)/(2*m))
                            require(max(abs(a/n-b/m) for a,b in zip(counts,peer_counts[backend]))<=bound,
                                    'lifecycle finite distribution witness failed')
    return dict(events=len(events),selected_cells=len(manifest['selected_cells']),
                valid_cells=sum(validations.values()),retained_failure_events=len(failures),
                lifetime_comparison_keys=len(lifetime_outputs))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('out',type=Path)
    p.add_argument('--git-sources',action='store_true')
    p.add_argument('--allow-smoke',action='store_true')
    args=p.parse_args()
    print(json.dumps(validate(args.out,args.git_sources,args.allow_smoke),sort_keys=True))


if __name__=='__main__': main()
