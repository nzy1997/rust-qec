"""Build both exact source refs before the source-only CDF campaign."""
import argparse
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
spec=importlib.util.spec_from_file_location('cdf_evidence', HERE/'evidence.py')
evidence=importlib.util.module_from_spec(spec);spec.loader.exec_module(evidence)

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def git(root,*args):return subprocess.check_output(['git',*args],cwd=root,text=True,timeout=30).strip()
def sources(root,head):
    if git(root,'rev-parse','HEAD')!=head or git(root,'status','--porcelain'):
        raise ValueError('source checkout changed or dirty')
    paths=[name for name in git(root,'ls-files').splitlines() if name.endswith('.rs') or Path(name).name in ['Cargo.toml','Cargo.lock']]
    result={name:sha(root/name) for name in paths}
    for name,digest in result.items():
        data=subprocess.check_output(['git','show',head+':'+name],cwd=root,timeout=30)
        if hashlib.sha256(data).hexdigest()!=digest:raise ValueError('Git/disk source mismatch '+name)
    return result

def probe_manifest(original, library):
    needle = 'path = "../../../rstim"'
    if original.count(needle) != 1:
        raise ValueError('probe library path missing or ambiguous')
    return original.replace(needle, 'path = ' + json.dumps(str(library)))

def require_candidate_tests(log, *, layout=False):
    expected = {'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage'} if layout else {
        'near_clifford::compiled::coherent_packet::diagonal_projection_offset_tests::large_diagonal_projection_preserves_frozen_plane_bits_for_masks_and_pivots',
        'near_clifford::compiled::coherent_packet::diagonal_projection_offset_tests::diagonal_projection_preserves_first_error_and_partial_scratch_bits'
    }
    count = len(expected)
    records, summaries = [], []
    for line in log.splitlines():
        if not line.startswith('test '):
            continue
        if line.startswith('test result:'):
            summaries.append(line)
        else:
            record = re.fullmatch(r'test (\S+) \.\.\. (.+)', line)
            if record is None:
                raise ValueError('malformed diagonal-projection candidate test result record')
            records.append(record.groups())
    valid_summary = len(summaries) == 1 and re.fullmatch(
        rf'test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?',
        summaries[0],
    )
    if (len(records) != count or {name for name, _ in records} != expected
            or any(status != 'ok' for _, status in records) or not valid_summary):
        raise ValueError('every named diagonal-projection candidate test must execute successfully')

class Cancelled(RuntimeError):pass
def cancel(signum,frame):raise Cancelled('prepare cancelled by signal '+str(signum))
def invoke(command,cwd,env,path,context):
    start=time.time();child=None;cancellation=None;timed_out=False
    with path.open('xb') as log:
        try:
            previous=signal.pthread_sigmask(signal.SIG_BLOCK,{signal.SIGTERM,signal.SIGINT})
            try:child=subprocess.Popen(command,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True,preexec_fn=lambda:signal.pthread_sigmask(signal.SIG_SETMASK,previous))
            finally:signal.pthread_sigmask(signal.SIG_SETMASK,previous)
            child.wait(timeout=600)
        except BaseException as error:
            cancellation=error;timed_out=isinstance(error,subprocess.TimeoutExpired)
            if child is not None:
                try:os.killpg(child.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                child.wait(timeout=10)
    receipt=dict(command=command,controller_pid=os.getpid(),child_pid=child.pid if child else None,child_waited=child is not None and child.returncode is not None,exit_code=child.returncode if child else None,started=start,finished=time.time(),timed_out=timed_out,cancellation=type(cancellation).__name__ if cancellation else None,log_sha256=sha(path),**context)
    path.with_suffix('.receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    if cancellation is not None:raise cancellation
    if child.returncode:raise ValueError('build/check failed; receipt retained')
    return receipt

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--baseline-ref',required=True)
    p.add_argument('--candidate-ref',required=True)
    args=p.parse_args()
    for ref in [args.baseline_ref,args.candidate_ref]:
        if not re.fullmatch('[0-9a-f]{40}',ref):p.error('exact40character lowercase source SHAs required')
    if git(ROOT,'status','--porcelain'):raise ValueError('protocol checkout must be clean')
    protocol=git(ROOT,'rev-parse','HEAD');out=args.out.resolve();out.mkdir(parents=True,exist_ok=False)
    signal.signal(signal.SIGTERM,cancel);signal.signal(signal.SIGINT,cancel)
    roots={};heads=dict(baseline=args.baseline_ref,candidate=args.candidate_ref,control=args.baseline_ref)
    env=os.environ.copy();env.pop('CARGO_ENCODED_RUSTFLAGS',None);env.pop('CARGO_PROFILE_RELEASE_OPT_LEVEL',None);env['RUSTFLAGS']='-C target-cpu=native'
    for key in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS']:env[key]='1'
    for role in ['baseline','candidate']:
        checkout=out/'source'/role
        checkout.parent.mkdir(exist_ok=True)
        subprocess.run(['git','worktree','add','--detach',str(checkout),heads[role]],cwd=ROOT,check=True,timeout=120)
        roots[role]=str(checkout);before=sources(checkout,heads[role]);directory=out/role;directory.mkdir()
        for name in before:
            snapshot=out/'production-sources'/role/name
            snapshot.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(checkout/name,snapshot)
            if sha(snapshot)!=before[name]:raise ValueError('source changed during snapshot')
        for kind,binary,package in [('structural','near-clifford-diagnostics','diagnostics'),('counts','near-clifford-application-counts','application_counts')]:
            probe=directory/'native-probes'/kind;probe.mkdir(parents=True)
            public=ROOT/'benchmarks/near_clifford'/package
            for name in ['main.rs','Cargo.lock']:shutil.copyfile(public/name,probe/name)
            original=(public/'Cargo.toml').read_text()
            altered=probe_manifest(original, checkout/'rstim')
            (probe/'Cargo.toml').write_text(altered)
            inputs={name:sha(probe/name) for name in ['main.rs','Cargo.toml','Cargo.lock']}
            build_env=dict(env,CARGO_TARGET_DIR=str(directory/'native-target'))
            command=['rustup','run','1.93.1','cargo','build','--release','--locked','--manifest-path',str(probe/'Cargo.toml')]
            context=dict(head=heads[role],sources=before,probe=inputs,environment={key:build_env.get(key) for key in ['RUSTFLAGS','CARGO_TARGET_DIR','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_OPT_LEVEL']})
            receipt=invoke(command,checkout,build_env,directory/('native-build-'+kind+'.log'),context)
            retained=directory/(binary+'.bin');shutil.copy2(directory/'native-target/release'/binary,retained)
            receipt.update(sources_after=sources(checkout,heads[role]),binary=dict(path=str(retained),bytes=retained.stat().st_size,sha256=sha(retained),mode=oct(retained.stat().st_mode&0o777)))
            if before!=receipt['sources_after'] or inputs!={name:sha(probe/name) for name in inputs}:raise ValueError('source/probe changed during build')
            (directory/('native-build-'+kind+'.receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    roots['control']=roots['baseline']
    # Exercise the candidate's native arithmetic/public RNG before performance collection.
    candidate=Path(roots['candidate']);test_env=dict(env,CARGO_TARGET_DIR=str(out/'candidate/test-target'))
    tests=[['--lib','phase_specialized_cdf_tests'],['--test','near_clifford_compiled','compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails','--','--exact'],['--lib','diagonal_projection_offset_tests'],['--lib','near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage','--','--exact']]
    for index,selection in enumerate(tests):
        command=['rustup','run','1.93.1','cargo','test','--release','--locked','-p','rstim','--no-default-features',*selection]
        invoke(command,candidate,test_env,out/f'native-check-{index}.log',dict(head=heads['candidate'],environment={'RUSTFLAGS':env['RUSTFLAGS']}))
        log=(out/f'native-check-{index}.log').read_text()
        evidence.require_executed_tests(log)
        if index >= 2:require_candidate_tests(log, layout=index == 3)
    metadata=dict(protocol_revision=protocol,protocol_directory=str(HERE),preparation_directory=str(out),roots=roots,heads=heads,created=time.time(),compiler=subprocess.check_output(['rustup','run','1.93.1','rustc','-Vv'],text=True))
    (out/'preparation.json').write_text(json.dumps(metadata,indent=2)+'\n')
    protocol_paths = [path for path in HERE.rglob('*') if path.is_file() and '__pycache__' not in path.parts]
    protocol_paths += [ROOT/'benchmarks/near_clifford'/name for name in ['application_counts/common.py','application_counts/manifest.json','diagnostics/corpus.py']]
    protocol_paths += [ROOT/'benchmarks/near_clifford'/package/name for package in ['application_counts','diagnostics'] for name in ['main.rs','Cargo.toml','Cargo.lock']]
    for path in protocol_paths:
        snapshot=out/'protocol'/path.relative_to(ROOT)
        snapshot.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(path,snapshot)
    evidence.seal_preparation(out, protocol)
    if git(ROOT,'rev-parse','HEAD')!=protocol or git(ROOT,'status','--porcelain'):raise ValueError('protocol changed during preparation')
    print('bothsource native builds and arithmetic/RNG checks closed; preparation sealed',flush=True)
if __name__=='__main__':main()
