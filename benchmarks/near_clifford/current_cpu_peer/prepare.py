"""Prepare exact-source native CPU peers and source-bound Rust probes."""
import argparse, hashlib, importlib.util, io, json, os, platform, re, shutil, signal, subprocess, sys, tarfile, time, zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
from wheel_bindings import bind_wheel
from peer_evidence import seal_preparation
from commands import THREADS, PEER_URL, package_script, require_peer_main
PEER='3f718e9e0c58b277a8fb506b4170863db5c3dbe6'
URL=PEER_URL
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def require(value,message):
    if not value:raise ValueError(message)
class Cancelled(RuntimeError):pass
def cancel(signum,frame):raise Cancelled('signal '+str(signum))
def invoke(out,label,command,cwd,env,*,timeout=600):
    started=time.time();child=None;error=None
    with (out/(label+'.stdout')).open('xb') as a,(out/(label+'.stderr')).open('xb') as b:
        try:
            previous=signal.pthread_sigmask(signal.SIG_BLOCK,{signal.SIGINT,signal.SIGTERM})
            try:child=subprocess.Popen(command,cwd=cwd,env=env,stdout=a,stderr=b,start_new_session=True,preexec_fn=lambda:signal.pthread_sigmask(signal.SIG_SETMASK,previous))
            finally:signal.pthread_sigmask(signal.SIG_SETMASK,previous)
            child.wait(timeout=timeout)
        except BaseException as exc:
            error=exc
            if child is not None:
                try:os.killpg(child.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                child.wait(timeout=10)
    receipt=dict(command=command,cwd=str(cwd),controller_pid=os.getpid(),child_pid=child.pid if child else None,child_waited=child is not None and child.returncode is not None,exit_code=child.returncode if child else None,started=started,closed=time.time(),cancellation=type(error).__name__ if error else None,environment={key:env.get(key) for key in THREADS+['CC','CXX','CFLAGS','CXXFLAGS','SYMFT_PY_NATIVE','SYMFT_PY_ENABLE_CUDA']},stdout_sha256=sha(out/(label+'.stdout')),stderr_sha256=sha(out/(label+'.stderr')))
    (out/(label+'.receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    if error:raise error
    require(child.returncode==0,'preparation failed; actual receipt retained: '+label)
    return (out/(label+'.stdout')).read_bytes()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--out',type=Path,required=True);parser.add_argument('--rust-ref',required=True);args=parser.parse_args()
    require(platform.system()=='Linux' and platform.machine()=='x86_64' and sys.version_info[:2]==(3,12),'Python3.12/Linux x86_64 required')
    require(re.fullmatch('[0-9a-f]{40}',args.rust_ref) is not None,'exact40 lowercase Rust ref required')
    out=args.out.resolve();out.mkdir(parents=True,exist_ok=False)
    signal.signal(signal.SIGTERM,cancel);signal.signal(signal.SIGINT,cancel)
    env=os.environ.copy();env.update({key:'1' for key in THREADS});env.update(CC='gcc',CXX='g++',CFLAGS='-O3',CXXFLAGS='-O3',SYMFT_PY_NATIVE='1',SYMFT_PY_ENABLE_CUDA='0')
    require(not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT),'protocol must be committed/clean')
    protocol=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    for compiler in ['gcc','g++']:invoke(out,compiler+'-version',[compiler,'--version'],ROOT,env)
    remote=invoke(out,'peer-official-refs',['git','ls-remote',URL,'refs/heads/main','refs/heads/symft-26-10-08'],ROOT,env).decode()
    require_peer_main(remote,PEER)
    source=out/'source';invoke(out,'peer-clone',['git','clone','--no-checkout',URL,str(source)],ROOT,env);invoke(out,'peer-checkout',['git','checkout','--detach',PEER],source,env)
    archive=invoke(out,'peer-git-archive',['git','archive',PEER],source,env)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        inventory={}
        for member in tar.getmembers():
            if not member.isfile():continue
            require(not Path(member.name).is_absolute() and '..' not in Path(member.name).parts,'unsafe archive member')
            data=tar.extractfile(member).read();require((source/member.name).read_bytes()==data,'peer Git/disk mismatch '+member.name)
            path=out/'peer-sources'/member.name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data);inventory[member.name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    (out/'checkout-receipt.json').write_text(json.dumps(dict(head=PEER,source_inventory=inventory),indent=2)+'\n')
    wheels=out/'wheels';wheels.mkdir()
    invoke(out,'clifft-venv',[sys.executable,'-m','venv',str(out/'clifft')],ROOT,env);invoke(out,'symft-venv',[sys.executable,'-m','venv',str(out/'symft')],ROOT,env)
    clifft=out/'clifft/bin/python';symft=out/'symft/bin/python'
    invoke(out,'pinned-wheel-download',[str(symft),'-I','-m','pip','download','--no-cache-dir','--only-binary=:all:','--no-deps','--dest',str(wheels),'numpy==2.4.6','clifft==0.11.0','setuptools==84.0.0','wheel==0.48.0'],ROOT,env)
    for backend,python,packages in [('clifft',clifft,['numpy==2.4.6','clifft==0.11.0']),('symft',symft,['numpy==2.4.6','setuptools==84.0.0','wheel==0.48.0'])]:
        invoke(out,backend+'-pinned-install',[str(python),'-I','-m','pip','install','--no-index','--no-deps','--find-links',str(wheels),*packages],ROOT,env)
    invoke(out,'native-symft-wheel',[str(symft),'-I','-m','pip','wheel','--verbose','--no-cache-dir','--no-build-isolation','--no-deps','--wheel-dir',str(wheels),str(source/'python')],ROOT,env)
    log=(out/'native-symft-wheel.stdout').read_bytes()+(out/'native-symft-wheel.stderr').read_bytes()
    # pip verbose is needed for a verifiable actual compiler command; the preparation
    # refuses a build without evidence of its native compiler flags.
    require(b'-march=native' in log and b'SYMFT_CPP_NATIVE_BUILD=1' in log,'actual compiler native flags missing; build with verbose logs')
    native=list(wheels.glob('symft-*.whl'));require(len(native)==1,'ambiguous native SymFT wheel')
    invoke(out,'native-symft-install',[str(symft),'-I','-m','pip','install','--no-index','--no-deps',str(native[0])],ROOT,env)
    for name,entry in inventory.items():require(sha(source/name)==entry['sha256'],'peer changed during native build '+name)
    packages={};identities={}
    for backend,python in [('clifft',clifft),('symft',symft)]:
        packages[backend]=json.loads(invoke(out,backend+'-package-inspection',[str(python),'-I','-c',package_script([backend,'numpy'])],ROOT,env))
        identities[backend]=json.loads(invoke(out,backend+'-import-inspection',[str(python),'-I',str(ROOT/'benchmarks/near_clifford/compiled_sota/worker.py'),backend,str(ROOT/'benchmarks/near_clifford/compiled_sota/manifest.json'),'1','--mode','identity'],ROOT,env))['loaded_files']
        for dist,meta in packages[backend].items():
            for filename,digest in meta['files'].items():
                actual=Path(filename);require(sha(actual)==digest,'installed code changed');relative=filename.split('/site-packages/',1)[1];retained=out/'installed-source'/backend/dist/relative;retained.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(actual,retained)
    wheel_bindings={}
    for backend in ['clifft','symft']:
        environment=packages[backend]
        for dist in [backend,'numpy']:
            meta=environment[dist]
            expected_versions={'symft':'2026.10.8','clifft':'0.11.0','numpy':'2.4.6'}
            require(meta['version']==expected_versions[dist],'installed version mismatch '+dist)
            matches=list(wheels.glob(dist+'-*.whl'));require(len(matches)==1,'ambiguous/missing wheel '+dist)
            wheel=matches[0]
            filename=next(iter(meta['files']))
            require('/site-packages/' in filename,'installed root absent')
            site_root=Path(filename.split('/site-packages/',1)[0]+'/site-packages')
            require(site_root.is_relative_to(out/backend),'installed package outside owned environment')
            wheel_bindings[backend+'/'+dist]=bind_wheel(wheel,meta,original_site_packages=str(site_root),installed_root=out/'installed-source'/backend/dist)
        modules={backend,backend+('._native' if backend=='symft' else '._clifft_core')}
        require(set(identities[backend])==modules,'unexpected actual import set')
        for name,actual in identities[backend].items():
            require(environment[backend]['files'].get(actual['path'])==actual['sha256'],'actual imported code outside pinned wheel distribution')
    (out/'wheel-installed-bindings.json').write_text(json.dumps(wheel_bindings,indent=2)+'\n')
    expected=dict(packages=dict(symft=packages['symft']['symft'],numpy=packages['symft']['numpy'],clifft_environment=packages['clifft']),identities=identities)
    (out/'expected-environment.json').write_text(json.dumps(expected,indent=2)+'\n')
    # Call reviewed Rust preparation in this controller, so its native child
    # cleanup remains under the same signal owner rather than orphaning sessions.
    spec=importlib.util.spec_from_file_location('cdf_prepare',ROOT/'benchmarks/near_clifford/cdf_source_pair/prepare.py');cdf=importlib.util.module_from_spec(spec);spec.loader.exec_module(cdf)
    previous=sys.argv;previous_signals={s:signal.getsignal(s) for s in [signal.SIGTERM,signal.SIGINT]}
    try:sys.argv=[str(ROOT/'benchmarks/near_clifford/cdf_source_pair/prepare.py'),'--out',str(out/'rust'),'--baseline-ref',args.rust_ref,'--candidate-ref',args.rust_ref];cdf.main()
    finally:
        sys.argv=previous
        for sig,handler in previous_signals.items():signal.signal(sig,handler)
    manifest=json.loads((HERE/'manifest.json').read_text())
    manifest.update(schema='rstim.current-cpu-peer.v1',protocol_revision=protocol,rust_source_head=args.rust_ref,symft_revision=PEER)
    manifest.pop('current_merged_head',None);manifest.pop('source_relation',None)
    for name,digest in manifest['inputs'].items():require(sha(ROOT/'benchmarks/near_clifford/application_counts/fixtures'/(name+'.stim'))==digest,'manifest fixture mutation')
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    (out/'preparation.json').write_text(json.dumps(dict(protocol_revision=protocol,protocol_directory=str(HERE),repository_directory=str(ROOT),rust_source_head=args.rust_ref,symft_revision=PEER,compiler_native_flags=['-march=native','SYMFT_CPP_NATIVE_BUILD=1'],preparation_directory=str(out),controller_pid=os.getpid(),finished=time.time(),scope='Pinned source/native compiler/wheel/import preparation; exact counts work; no performance admission'),indent=2)+'\n')
    protocol_paths=[*sorted(HERE.glob('*.py')),HERE/'manifest.json']
    protocol_paths += [ROOT/'benchmarks/near_clifford/application_counts/fixtures'/(name+'.stim') for name in manifest['names']]
    protocol_paths += [ROOT/'benchmarks/near_clifford'/name for name in ['application_counts/common.py','diagnostics/source_contract.py','compiled_sota/run.py','compiled_sota/worker.py','compiled_sota/projection.py','compiled_sota/evidence.py','compiled_sota/manifest.json']]
    for path in protocol_paths:
        retained=out/'protocol'/path.relative_to(ROOT);retained.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,retained)
    require(subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==protocol and not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT),'protocol changed during preparation')
    seal_preparation(out,protocol)
    print('source/native/wheel/import preparation closed and sealed; no performance admission',flush=True)
if __name__=='__main__':main()
