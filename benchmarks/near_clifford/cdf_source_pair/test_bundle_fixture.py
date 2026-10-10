"""Explicit synthetic 720-event fixture for offline verifier regressions.

No production program is built or measured. Toy source/binary bytes and fabricated
process receipts exercise artifact consistency, never performance admission.
"""
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import statistics
import sys
import time

HERE=Path(__file__).resolve().parent

def build_bundle(root):
    spec=importlib.util.spec_from_file_location('fixture_verifier',HERE/'verify.py')
    v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    e=v.evidence
    def write(path,value):
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text(json.dumps(value,separators=(',',':'))+'\n')
    prep=root/'preparation';output=root/'output';control=root/'control'
    for path in [prep,output,control]:path.mkdir(parents=True)
    protocol=prep/'protocol/benchmarks/near_clifford/cdf_source_pair';shutil.copytree(HERE,protocol,ignore=shutil.ignore_patterns('__pycache__'))
    heads=dict(baseline='a'*40,candidate='b'*40,control='a'*40)
    write(prep/'preparation.json',dict(heads=heads,protocol_revision='c'*40,compiler='rustc 1.93.1 (synthetic fixture)',preparation_directory=str(prep),protocol_directory=str(HERE),roots={role:str(prep/'source'/('baseline' if role=='control' else role)) for role in heads}))
    identities={};receipt_base=dict(child_waited=True,exit_code=0,timed_out=False,cancellation=None,controller_pid=32169,child_pid=32179)
    for role in ['baseline','candidate']:
        src=prep/'production-sources'/role/'toy.rs';src.parent.mkdir(parents=True);src.write_text('// synthetic source '+role+'\n');source={'toy.rs':e.sha(src)};probes={}
        for kind,binary in [('structural','near-clifford-diagnostics'),('counts','near-clifford-application-counts')]:
            probe=prep/role/'native-probes'/kind;probe.mkdir(parents=True)
            package='diagnostics' if kind=='structural' else 'application_counts'
            canonical=prep/'protocol/benchmarks/near_clifford'/package
            canonical.mkdir(parents=True,exist_ok=True)
            for name in ['main.rs','Cargo.toml','Cargo.lock']:
                shutil.copyfile(HERE.parent/package/name,canonical/name)
                if name=='Cargo.toml':(probe/name).write_text(v.prepare_driver.probe_manifest((canonical/name).read_text(),prep/'source'/role/'rstim'))
                else:shutil.copyfile(canonical/name,probe/name)
            hashes={name:e.sha(probe/name) for name in ['main.rs','Cargo.toml','Cargo.lock']}
            retained=prep/role/(binary+'.bin');retained.write_bytes(b'fixture, not executable')
            log=prep/role/('native-build-'+kind+'.log');log.write_text('synthetic build log\n')
            write(log.with_suffix('.receipt.json'),dict(receipt_base,head=heads[role],sources=source,sources_after=source,probe=hashes,binary=dict(path=str(retained),bytes=retained.stat().st_size,sha256=e.sha(retained),mode='0o755'),environment=dict(RUSTFLAGS='-C target-cpu=native',CARGO_TARGET_DIR=str(prep/role/'native-target'),CARGO_ENCODED_RUSTFLAGS=None,CARGO_PROFILE_RELEASE_OPT_LEVEL=None),log_sha256=e.sha(log),command=['rustup','run','1.93.1','cargo','build','--release','--locked','--manifest-path',str(probe/'Cargo.toml')]))
            probes[kind]=dict(hashes,binary=e.sha(retained))
        identities[role]=dict(head=heads[role],sources=source,probes=probes)
    identities['control']=copy.deepcopy(identities['baseline'])
    for index in [0,1,2,3]:
        log=prep/('native-check-'+str(index)+'.log');log.write_text('test result: ok. 1 passed; 0 failed\n')
        selection=['--lib','phase_specialized_cdf_tests'] if index==0 else ['--test','near_clifford_compiled','compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails','--','--exact'] if index==1 else ['--lib','zero_noise_summary'] if index==2 else ['--lib','near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage','--','--exact']
        if index >= 2:
            names = ['near_clifford::compiled::random_event_runs::random_event_runs_tests::zero_noise_summary_keeps_every_event_and_frozen_rng_continuation', 'near_clifford::compiled::noise_schedule::tests::zero_noise_summary_skips_sign_refs_but_unknown_rows_still_scan', 'near_clifford::compiled::noise_schedule::tests::zero_noise_summary_preserves_scheduled_records_counts_and_carry'] if index == 2 else ['near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage']
            log.write_text(''.join('test '+name+' ... ok\n' for name in names)+f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n')
        write(log.with_suffix('.receipt.json'),dict(receipt_base,head=heads['candidate'],log_sha256=e.sha(log),command=['rustup','run','1.93.1','cargo','test','--release','--locked','-p','rstim','--no-default-features',*selection],environment={'RUSTFLAGS':'-C target-cpu=native'}))
    manifest=v.read(HERE/'manifest.json');cases,schedule=v.schedule(manifest);events=[];values={(k,c):{role:[] for role in identities} for k,c in cases}
    for index,(route,kind,case,action,pair,call) in enumerate(schedule):
        name,shots,policy=case;digest=manifest['inputs'][name+'.stim'];basic=dict(status='ok',input_sha256=digest,arithmetic=policy)
        if kind=='structural':
            if action=='dump':data=dict(basic,call_kind=call,call_shots=1 if call=='structured' else 129,shots=129,width=3,measurements=[0]*387,continuation=list(range(16)))
            else:
                cfg=dict(action='bench',arithmetic=policy,cache_bytes=64<<20,call_kind='flat',circuit=str(HERE/'fixtures'/(name+'.stim')),repetitions=7,shots=shots,total=shots)
                data=dict(basic,shots=shots,config=cfg,warm=[dict(elapsed_ns=50_000_000,calls=2,ns_per_call=25_000_000)]*7)
                write(output/(f'{index:05d}.config.json'),cfg)
        else:
            finite=action=='validate';calls=8192//shots if finite else 2;attempted=calls*shots
            obs=dict(elapsed_ns=50_000_000,calls=calls,ns_per_call=50_000_000/calls,attempted=attempted,accepted=attempted,discarded=0,logical_errors=0)
            data=dict(basic,backend='rstim',shots=shots,observations=[obs]*(1 if finite else 7),output_contract=v.driver.semantics.CONTRACT,compile_ns=1,prepare_ns=0,first_ns=0 if finite else 1,peak_active_rank=1,peak_rss_bytes=1,cache_reserved_bytes=0)
            if finite:
                width=v.driver.semantics.annotations((protocol/'fixtures'/(name+'.stim')).read_text())['width']
                data.update(shots=8192,call_shots=shots,width=width,measurements=[0]*(8192*width),exact_native_counts_rng=True)
        if kind=='structural':
            cfg=dict(circuit=str(HERE/'fixtures'/(name+'.stim')),arithmetic=policy,cache_bytes=64<<20,
                     shots=(1 if call=='structured' else 129) if action=='dump' else shots,repetitions=7,
                     action=action,call_kind=call,total=129 if action=='dump' else shots)
            write(output/f'{index:05d}.config.json',cfg)
            command=[str(prep/('candidate' if route=='candidate' else 'baseline')/'near-clifford-diagnostics.bin'),str(output/f'{index:05d}.config.json')]
        else:
            command=[str(prep/('candidate' if route=='candidate' else 'baseline')/'near-clifford-application-counts.bin'),str(HERE/'fixtures'/(name+'.stim')),str(shots),'1' if finite else '7',policy,action,'native']
        raw=output/f'{index:05d}.stdout';write(raw,data);stderr=output/f'{index:05d}.stderr';stderr.write_bytes(b'')
        event=dict(receipt_base,index=index,kind=kind,route=route,case=list(case),action=action,pair=pair,call_kind=call,result=data,stdout_sha256=e.sha(raw),stderr_sha256=e.sha(stderr),command=command,controller_pid=32179)
        events.append(event)
        if action=='bench':values[kind,case][route].append(statistics.median(o['ns_per_call'] for o in data['warm' if kind=='structural' else 'observations']))
    def ledger():
        with (output/'events.jsonl').open('w') as f:
            for event in events:f.write(json.dumps(event,separators=(',',':'))+'\n')
    ledger()
    summary=[]
    for kind,case in cases:
        data=values[kind,case];b,c,a=[statistics.median(data[role]) for role in ['baseline','candidate','control']]
        summary.append(dict(kind=kind,case=list(case),complete=True,process_medians_ns=data,baseline_ns=b,candidate_ns=c,median_ratio=b/c,paired_ratios=[x/y for x,y in zip(data['baseline'],data['candidate'])],null_ratio=b/a,null_paired_ratios=[x/y for x,y in zip(data['baseline'],data['control'])]))
    write(output/'summary.json',summary);(control/'producer.log').write_text('synthetic output\n')
    def reseal():
        (prep/'seal.json').unlink(missing_ok=True);e.seal_preparation(prep,'c'*40);pd=e.sha(prep/'seal.json')
        write(output/'header.json',dict(manifest=dict(manifest,source_pair=heads),inputs=manifest['inputs'],protocol_revision='c'*40,driver_sha256=e.sha(protocol/'run.py'),cases_sha256=e.sha(protocol/'manifest.json'),preparation_seal_sha256=pd,identities=identities,rustc='rustc 1.93.1 (synthetic fixture)'))
        write(output/'closure.json',dict(identities_after=identities,inputs_after=manifest['inputs'],preparation_seal_after=pd,driver_sha256=e.sha(protocol/'run.py'),events_sha256=e.sha(output/'events.jsonl'),events=720))
        write(control/'preparation-process-absence.json',dict(exit_code=1,stdout='',stderr='',pids=[32169,32179],command=['ps','-p','32169,32179','-o','pid=,comm=']))
        write(control/'closure.json',dict(receipt_base,command=[sys.executable,'-I',str(HERE/'run.py'),str(prep),str(output)],preparation_seal_before=pd,preparation_seal_after=pd,post_run_seal_error=None,producer_log_sha256=e.sha(control/'producer.log')))
        write(root/'process-absence.json',dict(child_waited=True,exit_code=1,stdout='',stderr='',pids=[32169,32179],command=['ps','-p','32169,32179','-o','pid=,comm=']))
        write(root/'original-seal.json',dict(created=time.time(),scope='SYNTHETIC REVIEW FIXTURE, NO PRODUCTION EVIDENCE',files=v.files(root)))
    reseal()
    return v
