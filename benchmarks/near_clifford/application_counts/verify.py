"""Replay source-bound capabilities, count semantics, tuning and timing coverage."""
import argparse
import importlib.util
import sys
import json
from pathlib import Path
import statistics
import subprocess
from common import HERE,ROOT,NAMES,POLICIES,SHOTS,digest,require,annotations,raw_counts,counts,compare,check_result
spec=importlib.util.spec_from_file_location('application_driver',HERE/'run.py')
driver=importlib.util.module_from_spec(spec);spec.loader.exec_module(driver)
from evidence import expand
from projection import records_only
inc=driver.inc
sys.path.insert(0,str(HERE.parent))
from evidence_io import read_event_bytes


def validate(out,git_sources=False,allow_smoke=False):
    header=json.loads((out/'header.json').read_text());closure=json.loads((out/'closure.json').read_text())
    data=read_event_bytes(out)
    events=[json.loads(line) for line in data.splitlines()]
    require(header['schema'] in ['rstim.postselected-counts.v1','rstim.postselected-counts.v2','rstim.postselected-counts.v3'],'wrong schema')
    modern=header['schema']!='rstim.postselected-counts.v1'
    scalar_rejection=header['schema']=='rstim.postselected-counts.v3'
    route=header.get('rust_route') if modern else header.get('rust_route','structured')
    require(route in ['structured','native'] if modern else route=='structured','Rust route differs')
    native_rust=route=='native'
    require(closure['events']==len(events) and closure['events_sha256']==digest(data),'event closure mismatch')
    require([e['index'] for e in events]==list(range(len(events))),'event index mismatch')
    require(closure['sources_after']==header['sources'] and closure['binary_sha256']==header['binary_sha256'],
            'source or binary closure differs')
    require(closure['environment_after']==header['environment_before']==inc.environment_summary(header['packages'],
        header['symft_sources'],header['symft_source_revision'],header['peer_loaded_files']),'environment summary differs')
    require(header['symft_source_revision']=='c89b98514a919240b8afa53a271e08d926d3c987' and
        header['packages']['symft']['version']=='0.1.1' and header['packages']['clifft_environment']['clifft']['version']=='0.11.0',
        'peer versions differ')
    require(allow_smoke or header['pairs']>=5 and header['repetitions']>=7,'formal minimum not met')
    require(header['selected_names'] and set(header['selected_names'])<=set(NAMES) and
            header['selected_shots'] and set(header['selected_shots'])<=set(SHOTS),'unknown selection')
    require(len(set(header['selected_names']))==len(header['selected_names']) and
            len(set(header['selected_shots']))==len(header['selected_shots']),'duplicate selection')
    cases=[dict(id=f'{name}/{shots}/{policy}',name=name,shots=shots,policy=policy)
           for name in header['selected_names'] for shots in header['selected_shots'] for policy in POLICIES]
    require(header['cases']==cases,'incomplete deterministic case matrix')
    manifest=json.loads((HERE/'manifest.json').read_text());require(manifest==header['manifest'],'original corpus changed')
    texts={name:(ROOT/value['path']).read_text() for name,value in manifest['inputs'].items()}
    for name,value in manifest['inputs'].items(): require(digest(texts[name].encode())==value['sha256'],'input changed')
    for path,value in manifest['licenses'].items(): require(digest((ROOT/path).read_bytes())==value['sha256'],'license changed')
    required={'benchmarks/near_clifford/application_counts/'+name for name in
              ['.gitignore','Cargo.toml','Cargo.lock','main.rs','worker.py','common.py','run.py','verify.py','test_contract.py','README.md','manifest.json']}
    require(required<=set(header['sources']),'missing application harness sources')
    if modern:
        native_producer_path='benchmarks/near_clifford/application_counts/run.py'
        producer_bytes=subprocess.check_output(['git','show',header['source_revision']+':'+native_producer_path],cwd=ROOT)
        require(digest(producer_bytes)==header['sources'][native_producer_path] and header['schema'].encode() in producer_bytes and b'--rust-route' in producer_bytes, 'native schema requires source-bound native producer')
    producer_path='benchmarks/near_clifford/diagnostics/run.py'
    helper_path='benchmarks/near_clifford/evidence_io.py'
    require(producer_path in header['sources'],'missing diagnostic producer source')
    if header['sources'][producer_path]!='cf4ca29cf945c5d8cf5997ba3c7dcf8b0f269497e8cc21801c4eaf177e01f16d':
        require(helper_path in header['sources'],'nonlegacy producer omits event helper')
    if git_sources:
        names=subprocess.check_output(['git','ls-tree','-r','--name-only',header['source_revision']],cwd=ROOT,text=True).splitlines()
        expected={name for name in names if name.startswith('rstim/src/') and name.endswith('.rs') or
                  name in ['Cargo.toml','Cargo.lock','rstim/Cargo.toml','benchmarks/near_clifford/evidence_io.py'] or
                  name.startswith('benchmarks/near_clifford/diagnostics/') or
                  name.startswith('benchmarks/near_clifford/application_counts/') or
                  name.startswith('benchmarks/near_clifford/compiled_sota/') and
                  (name.endswith('.py') or name.endswith('.stim') or name.endswith('/manifest.json'))}
        # Exact historical producer implementation predates helper inventory.
        # Derive this exemption from immutable Git bytes, never a receipt flag.
        producer=subprocess.check_output(['git','show',header['source_revision']+
            ':benchmarks/near_clifford/diagnostics/run.py'],cwd=ROOT)
        if digest(producer)=='cf4ca29cf945c5d8cf5997ba3c7dcf8b0f269497e8cc21801c4eaf177e01f16d':
            expected.discard('benchmarks/near_clifford/evidence_io.py')
        else:
            require(helper_path in expected and helper_path in header['sources'],
                    'nonlegacy source revision must contain and inventory event helper')
        require(set(header['sources'])==expected and required<=expected,'incomplete source revision/inventory')
        for name,sha in header['sources'].items():
            source=subprocess.check_output(['git','show',header['source_revision']+':'+name],cwd=ROOT)
            require(digest(source)==sha,'source digest differs: '+name)
            if not modern and (name.startswith('rstim/src/') or name in ['Cargo.toml','Cargo.lock','rstim/Cargo.toml']):
                baseline=subprocess.check_output(['git','show','3ef5030db205b3e9b2126e31b2602f760d4665cc:'+name],cwd=ROOT)
                require(source==baseline,'production changed')
    cases_by_id={case['id']:case for case in cases};tuning={};native={};raw={};rust={};checked={};timed={};capabilities={};failures=[];rejected={}
    widths={name:annotations(texts[name])['width'] for name in NAMES}
    def bind(result,backend,name,projected=False):
        require(result['backend']==backend,'executor label differs')
        require(result['input_sha256']==digest((records_only(texts[name]) if projected else texts[name]).encode()),'consumed input differs')
        if backend!='rstim': inc.bind_peer(result,backend,header['packages'],header['peer_loaded_files'])
        else:
            expected_execution=('native raw postselected counts; scalar early rejection' if scalar_rejection else 'native raw postselected counts; no early rejection') if native_rust else 'full structured records then filter; no early rejection'
            require(result['execution']==expected_execution,'Rust execution route differs')
            if native_rust and 'measurements' in result:
                require(result.get('exact_native_counts_rng') is True,'missing native exact counts/RNG witness')
        if not projected and backend=='symft' and result.get('sampler_info') is not None:
            info=result['sampler_info']
            require(info['detector_postselection'] is True and info['reference_normalized'] is False,
                    'native sampler output contract differs')
            require(type(info['active_components']) is bool and type(info['threads']) is int and info['threads']==1,
                    'native sampler execution features differ')
            require(info['num_measurements']==widths[name] and
                    info['max_active_qubits']==result['peak_active_width'],'native sampler dimensions differ')
    for event in events:
        kind=event['kind'];result=event.get('result',{})
        if kind=='capability':
            key=(event['name'],event['policy']);require(key not in capabilities,'duplicate capability')
            require(key[0] in manifest['inputs'] and key[1] in POLICIES,'unknown capability input/policy')
            capabilities[key]=result
            if result.get('status')=='ok':
                require(result['input_sha256']==manifest['inputs'][key[0]]['sha256'] and result['arithmetic']==key[1],
                        'capability input/policy differs')
            else: failures.append(event)
            continue
        case=cases_by_id[event['id']];name,shots,policy=case['name'],case['shots'],case['policy'];backend=event.get('backend')
        key=(name,shots,backend)
        if kind=='tuning':
            require(key not in tuning,'duplicate tuning')
            require([v['batch'] for v in event['trials']]==inc.batches(backend),'missing tuning candidates')
            valid=[]
            for trial in event['trials']:
                value=trial['result']
                if value.get('status')!='ok': failures.append(trial);continue
                bind(value,backend,name);check_result(value,backend,shots,header['repetitions'],batch=trial['batch'])
                valid.append(trial)
            winner=min(valid,key=lambda v:statistics.median(o['ns_per_call'] for o in v['result']['observations']))['batch'] if valid else None
            require(event['selected']==winner,'tuning winner differs');tuning[key]=winner
        elif kind in ['counts-validation','raw-validation']:
            target=rust if backend=='rstim' else native if kind=='counts-validation' else raw
            target_key=event['id'] if backend=='rstim' else key
            require(target_key not in target,'duplicate validation')
            target[target_key]=result
            if result.get('status')!='ok' and 'measurements' not in result: failures.append(event);continue
            bind(result,backend,name,kind=='raw-validation')
            if kind=='counts-validation': check_result(result,backend,shots,1,True,policy,tuning.get(key))
            else:
                require(result['call_shots']==shots and result['shots']>=8192 and str(result['batch'])==str(tuning[key]),'raw witness configuration differs')
        elif kind=='validation-check':
            require(event['id'] not in checked,'duplicate validation check')
            masks=annotations(texts[name]);reference=raw_counts(expand(rust[event['id']]),masks)
            exact=reference==counts(rust[event['id']]);checks=[]
            for peer in inc.BACKENDS[1:]:
                k=(name,shots,peer);value=native.get(k,{});witness=raw.get(k,{})
                if value.get('status')!='ok' or 'measurements' not in witness:
                    checks.append(dict(backend=peer,passed=False,reason='missing witness'));continue
                alpha=0.001/max(1,len(cases)*3*4)
                checks.append(dict(backend=peer,against_rust=compare(counts(value),reference,alpha),
                    against_own_records=compare(counts(value),raw_counts(expand(witness),masks),alpha)))
            passed=exact and all(v.get('against_rust',{}).get('passed') and v.get('against_own_records',{}).get('passed') for v in checks)
            require(event['checks']==checks and event['exact_rust_counts']==exact and event['passed']==passed,'counts replay differs')
            checked[event['id']]=passed
        elif kind=='timing':
            key=(event['id'],event['pair'],backend)
            require(checked.get(event['id']) is True and key not in timed,'unvalidated or duplicate timing')
            require(type(event['pair']) is int and 0<=event['pair']<header['pairs'],'pair out of range')
            timed[key]=result
            if result.get('status')!='ok': failures.append(event);continue
            bind(result,backend,name);check_result(result,backend,shots,header['repetitions'],policy=policy,batch=tuning.get((name,shots,backend)))
        elif kind=='rejected':
            require(event['id'] not in rejected,'duplicate rejected cell');rejected[event['id']]=event['reason'];failures.append(event)
        else: raise ValueError('unknown event kind')
    require(set(capabilities)=={(n,p) for n in manifest['inputs'] for p in POLICIES},'missing capability observations')
    for case in cases:
        require(case['id'] in rust,'missing Rust counts witness')
        require(all((case['name'],case['shots'],backend) in tuning for backend in inc.BACKENDS[1:]),'missing peer tuning')
        for backend in inc.BACKENDS[1:]:
            key=(case['name'],case['shots'],backend)
            if tuning[key] is not None:
                require(key in native and key in raw,'selected peer validation event missing')
        require(checked.get(case['id']) is True or case['id'] in rejected,'cell disappeared without retained rejection')
        if case['id'] in rejected:
            executors_incomplete=(rust[case['id']].get('status')!='ok' or
                any(tuning[(case['name'],case['shots'],backend)] is None for backend in inc.BACKENDS[1:]))
            reason='counts executor unsupported or peers incomplete' if executors_incomplete else 'counts finite witness disagreement'
            require(rejected[case['id']]==reason,'rejection reason differs from retained executor evidence')
            require(executors_incomplete or checked.get(case['id']) is False,
                    'rejection lacks failed finite validation evidence')
        if checked.get(case['id']):
            require(case['id'] not in rejected,'validated cell also rejected')
            for pair in range(header['pairs']):
                order=inc.BACKENDS[pair%4:]+inc.BACKENDS[:pair%4]
                if pair%2: order=order[::-1]
                observed=[e['backend'] for e in events if e['kind']=='timing' and e['id']==case['id'] and e['pair']==pair]
                require(observed==order,'timing coverage/order differs')
    return dict(events=len(events),valid_cells=sum(checked.values()),selected_cells=len(cases),retained_failure_events=len(failures))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('out',type=Path)
    p.add_argument('--git-sources',action='store_true');p.add_argument('--allow-smoke',action='store_true')
    args=p.parse_args();print(json.dumps(validate(args.out,args.git_sources,args.allow_smoke),sort_keys=True))
