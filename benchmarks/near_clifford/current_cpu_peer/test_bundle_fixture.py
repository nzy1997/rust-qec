"""Fabricated complete native-peer bundle for offline verifier regressions.

Toy ELF prefixes, wheels and process receipts are synthetic test inputs. This
module never builds a program, samples a production circuit, or provides timing
or source-admission evidence.
"""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import sys
import tarfile
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0,str(HERE))
from commands import HOST_SCRIPT, THREADS, package_script, setup_commands
from peer_evidence import inventory, preparation_pids, read, seal_preparation, sha
from wheel_bindings import bind_wheel
from validation import compare, counts, masks
from verify import schedule


def write(path, value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,separators=(',',':'))+'\n')


def elf():
    data = bytearray(128)
    data[:5] = b'\x7fELF\x02'
    data[18:20] = (62).to_bytes(2,'little')
    return bytes(data)


def build_preparation(prep):
    prep.mkdir(parents=True)
    # Reuse the existing fake Rust preparation, discarding its unrelated events.
    template = prep.parent/'rust-template'
    spec = importlib.util.spec_from_file_location('cdf_fixture',HERE.parent/'cdf_source_pair/test_bundle_fixture.py')
    fixture = importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
    fixture.build_bundle(template)
    shutil.move(template/'preparation',prep/'rust')
    original_rust = str(template/'preparation')
    rust = prep/'rust'
    for path in rust.rglob('*'):
        if path.is_file() and path.suffix in {'.json','.toml'}:
            path.write_text(path.read_text().replace(original_rust,str(rust)))
    shutil.rmtree(template)
    meta = read(rust/'preparation.json')
    meta['heads'] = {role:'a'*40 for role in ['baseline','candidate','control']}
    write(rust/'preparation.json',meta)
    for role in ['baseline','candidate']:
        (rust/'production-sources'/role/'toy.rs').write_text('// synthetic Rust source\n')
        sources = {'toy.rs':sha(rust/'production-sources'/role/'toy.rs')}
        for kind,binary in [('counts','near-clifford-application-counts'),('structural','near-clifford-diagnostics')]:
            retained = rust/role/(binary+'.bin');retained.write_bytes(elf())
            path = rust/role/('native-build-'+kind+'.receipt.json');receipt=read(path)
            receipt.update(head='a'*40,sources=sources,sources_after=sources,
                           probe={n:sha(rust/role/'native-probes'/kind/n) for n in ['Cargo.toml','Cargo.lock','main.rs']})
            receipt['binary'].update(bytes=retained.stat().st_size,sha256=sha(retained))
            write(path,receipt)
    (rust/'seal.json').unlink();seal_preparation(rust,'c'*40)
    for path in [*HERE.glob('*.py'),HERE/'manifest.json',*[ROOT/'benchmarks/near_clifford'/n for n in ['application_counts/common.py','diagnostics/source_contract.py','compiled_sota/run.py','compiled_sota/worker.py','compiled_sota/projection.py','compiled_sota/evidence.py','compiled_sota/manifest.json']]]:
        target=prep/'protocol'/path.relative_to(ROOT);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,target)
    manifest=read(HERE/'manifest.json')
    manifest.update(protocol_revision='c'*40,rust_source_head='a'*40)
    for name in manifest['names']:
        target=prep/'protocol/benchmarks/near_clifford/application_counts/fixtures'/(name+'.stim')
        target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(HERE.parent/'application_counts/fixtures'/(name+'.stim'),target)
    write(prep/'manifest.json',manifest)
    write(prep/'preparation.json',dict(protocol_revision='c'*40,rust_source_head='a'*40,symft_revision=manifest['symft_revision'],preparation_directory=str(prep),repository_directory=str(ROOT),protocol_directory=str(HERE)))
    peer_data=b'# synthetic peer source\n'
    (prep/'peer-sources').mkdir();(prep/'peer-sources/toy.py').write_bytes(peer_data)
    buffer=io.BytesIO()
    with tarfile.open(fileobj=buffer,mode='w') as tar:
        member=tarfile.TarInfo('toy.py');member.size=len(peer_data);tar.addfile(member,io.BytesIO(peer_data))
    write(prep/'checkout-receipt.json',dict(head=manifest['symft_revision'],source_inventory=inventory(prep/'peer-sources')))
    packages, identities, bindings = {},{},{}
    wheels=prep/'wheels';wheels.mkdir()
    for dist in ['numpy','clifft','symft']:
        contents={dist+'/__init__.py':b'# synthetic wrapper\n'}
        if dist != 'numpy':contents[dist+('/_native.so' if dist=='symft' else '/_clifft_core.so')]=elf()
        wheel=wheels/(dist+'-synthetic.whl')
        with zipfile.ZipFile(wheel,'w') as archive:
            for name,data in contents.items():archive.writestr(name,data)
    for backend in ['clifft','symft']:
        packages[backend]={}
        for dist in [backend,'numpy']:
            files={};site=str(prep/backend/'lib/python3.12/site-packages')
            with zipfile.ZipFile(wheels/(dist+'-synthetic.whl')) as archive:
                for name in archive.namelist():
                    data=archive.read(name);target=prep/'installed-source'/backend/dist/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
                    files[site+'/'+name]=hashlib.sha256(data).hexdigest()
            packages[backend][dist]=dict(version=manifest['versions'][dist],files=files)
            bindings[backend+'/'+dist]=bind_wheel(wheels/(dist+'-synthetic.whl'),packages[backend][dist],original_site_packages=site,installed_root=prep/'installed-source'/backend/dist)
        identities[backend]={}
        for module,suffix in [(backend,'/__init__.py'),(backend+('._native' if backend=='symft' else '._clifft_core'),'/_native.so' if backend=='symft' else '/_clifft_core.so')]:
            path=str(prep/backend/'lib/python3.12/site-packages')+'/'+backend+suffix
            identities[backend][module]=dict(path=path,sha256=packages[backend][backend]['files'][path])
    expected=dict(packages=dict(symft=packages['symft']['symft'],numpy=packages['symft']['numpy'],clifft_environment=packages['clifft']),identities=identities)
    write(prep/'expected-environment.json',expected);write(prep/'wheel-installed-bindings.json',bindings)
    commands=setup_commands(prep,ROOT,sys.executable,manifest['symft_revision'],'symft-synthetic.whl')
    for index,(label,command) in enumerate(commands.items()):
        stdout=prep/(label+'.stdout');stderr=prep/(label+'.stderr');stderr.write_bytes(b'')
        if label=='peer-git-archive':stdout.write_bytes(buffer.getvalue())
        elif label=='peer-official-refs':stdout.write_text(manifest['symft_revision']+'\trefs/heads/main\n')
        elif label=='native-symft-wheel':stdout.write_text('synthetic -march=native SYMFT_CPP_NATIVE_BUILD=1\n')
        elif label.endswith('-package-inspection'):write(stdout,packages[label.split('-')[0]])
        elif label.endswith('-import-inspection'):write(stdout,dict(isolated=True,loaded_files=identities[label.split('-')[0]]))
        else:stdout.write_text('synthetic setup\n')
        write(prep/(label+'.receipt.json'),dict(command=command,cwd=str(prep/'source' if label in {'peer-checkout','peer-git-archive'} else ROOT),environment=dict({k:'1' for k in THREADS},SYMFT_PY_NATIVE='1',SYMFT_PY_ENABLE_CUDA='0',CC='gcc',CXX='g++'),controller_pid=32169,child_pid=33000+index,child_waited=True,exit_code=0,cancellation=None,started=0,closed=1,stdout_sha256=sha(stdout),stderr_sha256=sha(stderr)))
    seal_preparation(prep,'c'*40)
    return manifest,expected


def reseal(root):
    """Reseal mutations so tests exercise semantic checks, not hash mismatch alone."""
    prep=root/'preparation';(prep/'rust/seal.json').unlink();seal_preparation(prep/'rust','c'*40)
    (prep/'seal.json').unlink();seal_preparation(prep,'c'*40);digest=sha(prep/'seal.json')
    for file,key in [('output/header.json','preparation_seal_sha256'),('output/closure.json','preparation_seal_after')]:
        data=read(root/file);data[key]=digest
        if file.endswith('closure.json'):data['events_sha256']=sha(root/'output/events.jsonl')
        write(root/file,data)
    data=read(root/'control/closure.json');data.update(preparation_seal_before=digest,preparation_seal_after=digest);write(root/'control/closure.json',data)
    files=inventory(root,bundle=True);files.pop('original-seal.json',None)
    write(root/'original-seal.json',dict(scope='SYNTHETIC TEST ONLY; no production evidence',files=files))


def build_bundle(root):
    prep,output,control=[root/n for n in ['preparation','output','control']]
    manifest,expected=build_preparation(prep)
    output.mkdir();control.mkdir()
    selected={(n,s):{b:dict(batch='auto',**({'cpu_backend':'compiled'} if b=='symft' else {})) for b in ['clifft','clifft-scheduled','symft']} for n in manifest['names'] for s in manifest['shots']}
    write(output/'frozen-selections.json',[dict(name=n,shots=s,backends=v) for (n,s),v in selected.items()])
    parsed={}
    for n in manifest['names']:
        parsed[n],projection=masks((HERE.parent/'application_counts/fixtures'/(n+'.stim')).read_text());(output/(n+'.records.stim')).write_text(projection)
    source=read(prep/'rust/baseline/native-build-counts.receipt.json')
    harness=[*HERE.glob('*.py'),HERE/'manifest.json',*[ROOT/'benchmarks/near_clifford'/n for n in ['application_counts/common.py','diagnostics/source_contract.py','compiled_sota/run.py','compiled_sota/worker.py','compiled_sota/projection.py','compiled_sota/evidence.py']]]
    before=dict(protocol_revision='c'*40,rust_build_head='a'*40,rust_sources=source['sources'],peer_sources={n:v['sha256'] for n,v in inventory(prep/'peer-sources').items()},rust_binary_sha256=source['binary']['sha256'],inputs=manifest['inputs'],harness={str(p):sha(p) for p in harness}|{str(prep/n):sha(prep/n) for n in ['manifest.json','expected-environment.json']})
    host=dict(uname=['Linux','test','0','0','x86_64',''],python='3.12.0',affinity=[0],thread_environment={k:'1' for k in THREADS})
    write(output/'header.json',dict(schema=manifest['schema'],manifest=manifest,before=before,preparation_seal_sha256=sha(prep/'seal.json'),controller_pid=32179,packages=expected['packages'],identities=expected['identities'],host=host))
    _,schedule_events=schedule(manifest,selected);events=[];payloads={}
    for index,item in enumerate(schedule_events):
        kind,backend=item['kind'],item.get('backend')
        if kind in {'package-inspection','import-inspection'}:
            command=setup_commands(prep,ROOT,sys.executable,manifest['symft_revision'],'symft-synthetic.whl')[backend+'-'+kind]
            payload=read(prep/(backend+'-'+kind+'.stdout'))
        elif kind=='host-inspection':command=[sys.executable,'-I','-c',HOST_SCRIPT];payload=host
        else:
            name,shots=item['name'],item['shots'];finite=kind=='counts-validation';rust=backend in {'rstim','control'}
            circuit=str(HERE.parent/'application_counts/fixtures'/(name+'.stim'))
            if rust:command=[str(prep/'rust/baseline/near-clifford-application-counts.bin'),circuit,str(shots),'1' if finite else '7',item['policy'],'validate' if finite else 'bench','native']
            else:
                selection=item['selection'];interpreter=str(prep/('symft' if backend=='symft' else 'clifft')/'bin/python')
                if kind=='raw-validation':command=[interpreter,'-I',str(HERE.parent/'compiled_sota/worker.py'),backend,str(output/(name+'.records.stim')),str(shots),'--batch',str(selection['batch']),'--mode','dump','--dump-total','8192']
                else:command=[interpreter,'-I',str(HERE/'worker.py'),backend,circuit,str(shots),'--batch',str(selection['batch']),'--cpu-backend',selection.get('cpu_backend','legacy'),'--repetitions','1' if finite else '7']+(['--validate'] if finite else [])
            calls=8192//shots if finite else (4 if kind=='tuning' and item['selection']==selected[name,shots][backend] else 2);attempted=calls*shots
            observation=dict(calls=calls,elapsed_ns=50_000_000,ns_per_call=50_000_000/calls,attempted=attempted,accepted=attempted,discarded=0,logical_errors=0)
            payload=dict(status='ok',backend='rstim' if rust else backend,shots=shots,compile_ns=1,prepare_ns=0,first_ns=0 if finite and rust else 1,peak_rss_bytes=1,input_sha256=manifest['inputs'][name],output_contract='all-zero raw detector postselection; raw observable 0 counts; no reference normalization',observations=[observation]*(1 if finite else 7))
            if rust:payload.update(arithmetic=item['policy'],peak_active_rank=1,cache_reserved_bytes=0)
            else:
                module='symft' if backend=='symft' else 'clifft';payload.update(batch=selection['batch'],isolated=True,loaded_files=expected['identities'][module],peak_active_width=1)
                if module=='symft':payload.update(cpu_backend=selection['cpu_backend'],sampler_info=dict(cpu_compiled=selection['cpu_backend']=='compiled',threads=1,detector_postselection=True,reference_normalized=False))
            if finite and rust:payload.update(call_shots=shots,shots=8192,width=parsed[name]['width'],measurements=[0]*(8192*parsed[name]['width']),exact_native_counts_rng=True)
            if kind=='raw-validation':payload.update(shots=8192,width=parsed[name]['width'],measurements=[0]*(8192*parsed[name]['width']),input_sha256=sha(output/(name+'.records.stim')))
        write(output/f'{index:05d}.stdout',payload);(output/f'{index:05d}.stderr').write_bytes(b'')
        compact=payload
        if 'measurements' in payload:
            import base64,zlib
            compact=dict(payload,measurements=dict(codec='zlib-packbits-big-v1',data=base64.b64encode(zlib.compress(bytes((payload['shots']*payload['width']+7)//8))).decode()))
        events.append(dict(item,index=index,command=command,controller_pid=32179,child_pid=34000+index,child_waited=True,exit_code=0,timed_out=False,cancellation=None,result_compaction_error=None,start=index*2,end=index*2+1,process_status='closed',stdout_sha256=sha(output/f'{index:05d}.stdout'),stderr_sha256=sha(output/f'{index:05d}.stderr'),result=compact))
        payloads[index]=payload
    (output/'events.jsonl').write_text(''.join(json.dumps(e,separators=(',',':'))+'\n' for e in events))
    rows=[];alpha=.001/(12*3*12)
    for (name,shots),backends in selected.items():
        own=dict(attempted=8192,accepted=8192,discarded=0,logical_errors=0)
        for backend in backends:rows.append(dict(name=name,shots=shots,backend=backend,selection=backends[backend],counts=own,own_records=own,rust={p:own for p in manifest['policies']},alpha_per_population=alpha,checks=dict(own_records=compare(own,own,alpha),against_rust={p:compare(own,own,alpha) for p in manifest['policies']})))
    (output/'validation-checks.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in rows))
    write(output/'closure.json',dict(before=before,after=before,preparation_seal_after=sha(prep/'seal.json'),controller_pid=32179,all_children_waited=True,events=759,events_sha256=sha(output/'events.jsonl'),timing_children=600,packages_after=expected['packages'],identities_after=expected['identities']))
    (control/'producer.log').write_text('synthetic fixture\n')
    write(control/'closure.json',dict(command=[sys.executable,'-I',str(HERE/'run.py'),'--preparation',str(prep),'--out',str(output)],controller_pid=32169,child_pid=32179,child_waited=True,exit_code=0,timed_out=False,cancellation=None,post_run_seal_error=None,preparation_seal_before=sha(prep/'seal.json'),preparation_seal_after=sha(prep/'seal.json'),producer_log_sha256=sha(control/'producer.log'),thread_environment={k:'1' for k in THREADS}))
    pids=sorted(set(preparation_pids(prep)));write(control/'preparation-process-absence.json',dict(pids=pids,command=['ps','-p',','.join(map(str,pids)),'-o','pid=,comm='],exit_code=1,stdout='',stderr=''))
    pids=sorted(set(pids+[32169,32179]+[e['child_pid'] for e in events]));write(root/'process-absence.json',dict(pids=pids,command=['ps','-p',','.join(map(str,pids)),'-o','pid=,comm='],exit_code=1,stdout='',stderr=''))
    write(root/'original-seal.json',dict(scope='SYNTHETIC TEST ONLY; no production evidence',files=inventory(root,bundle=True)))
