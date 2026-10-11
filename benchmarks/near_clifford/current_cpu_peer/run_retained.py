"""Outer controller: preserve campaign stdout and actual producer closure."""
import hashlib, json, os, signal, subprocess, sys, time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[2]
PREPARATION=None;CONTROL=None;ACTIVE=None;OUT=None
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def verify():
    import importlib.util
    spec = importlib.util.spec_from_file_location('peer_evidence', HERE/'peer_evidence.py')
    evidence = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(evidence)
    return evidence.verify_preparation(PREPARATION, live=True)

def stop_producer(child, active_path, grace=15):
    cleanup=[];active=None
    if active_path.exists():
        active=json.loads(active_path.read_text())
        if active['controller_pid']!=child.pid:raise ValueError('active worker belongs to another producer')
    child.send_signal(signal.SIGTERM)
    try:child.wait(timeout=grace)
    except subprocess.TimeoutExpired:
        if active_path.exists():active=json.loads(active_path.read_text())
        if active:
            if active['controller_pid']!=child.pid:raise ValueError('active worker belongs to another producer')
            worker=active['child_pid']
            try:
                if os.getpgid(worker)!=worker:raise ValueError('worker process group changed')
                os.killpg(worker,signal.SIGKILL)
            except ProcessLookupError:pass
        os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=10)
        cleanup.append(dict(action='forced-producer-group-kill',producer_pid=child.pid))
    if active:
        worker=active['child_pid']
        # Absence is an observation; only the producer can reap this worker.
        for _ in range(40):
            status=subprocess.run(['ps','-p',str(worker),'-o','pid=,command='],capture_output=True,text=True)
            if status.returncode==1 and not status.stdout:break
            time.sleep(0.05)
        cleanup.append(dict(worker_pid=worker,command=status.args,exit_code=status.returncode,stdout=status.stdout,stderr=status.stderr,worker_waited_by_outer=False))
        if status.returncode!=1 or status.stdout:raise ValueError('worker remains after producer cancellation')
    return cleanup

class CampaignCancelled(RuntimeError):pass
def cancel(signum, frame):raise CampaignCancelled('outer controller cancelled by signal '+str(signum))
def main():
    import argparse
    global PREPARATION,CONTROL,ACTIVE,OUT
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--control',type=Path,required=True)
    args=parser.parse_args();PREPARATION=args.preparation.resolve();OUT=args.out.resolve();CONTROL=args.control.resolve();ACTIVE=OUT/'active-child.json'
    if OUT.exists():raise ValueError('benchmark output must be fresh')
    pids=[]
    for name in __import__('json').loads((PREPARATION/'seal.json').read_text())['files']:
        if not name.endswith('.receipt.json'):continue
        path=PREPARATION/name
        if {'source','native-target','test-target'}&set(path.relative_to(PREPARATION).parts):continue
        receipt=json.loads(path.read_text())
        if not receipt['child_waited'] or receipt['exit_code']!=0:raise ValueError('preparation child did not close successfully')
        pids.extend([receipt['controller_pid'],receipt['child_pid']])
    if not pids:raise ValueError('missing actual preparation process receipts')
    result=subprocess.run(['ps','-p',','.join(map(str,sorted(set(pids)))),'-o','pid=,comm='],capture_output=True,text=True)
    if result.returncode!=1 or result.stdout:raise ValueError('preparation build/test process still active')
    seal=verify();CONTROL.mkdir(exist_ok=False)
    (CONTROL/'preparation-process-absence.json').write_text(json.dumps(dict(pids=sorted(set(pids)),command=result.args,exit_code=result.returncode,stdout=result.stdout,stderr=result.stderr),indent=2)+'\n')
    signal.signal(signal.SIGTERM,cancel);signal.signal(signal.SIGINT,cancel)
    env=os.environ.copy();env.update(OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',RAYON_NUM_THREADS='1',VECLIB_MAXIMUM_THREADS='1',NUMEXPR_NUM_THREADS='1')
    command=[sys.executable,'-I',str(HERE/'run.py'),'--preparation',str(PREPARATION),'--out',str(OUT)];started=time.time();timeout=False;cancellation=None;cleanup=[];child=None
    with (CONTROL/'producer.log').open('wb') as file:
        try:
            previous=signal.pthread_sigmask(signal.SIG_BLOCK,{signal.SIGTERM,signal.SIGINT})
            try:child=subprocess.Popen(command,cwd=ROOT,env=env,stdout=file,stderr=subprocess.STDOUT,start_new_session=True,preexec_fn=lambda:signal.pthread_sigmask(signal.SIG_SETMASK,previous))
            finally:signal.pthread_sigmask(signal.SIG_SETMASK,previous)
            child.wait(timeout=6000)
        except subprocess.TimeoutExpired:
            timeout=True;cleanup=stop_producer(child,ACTIVE)
        except BaseException as error:
            cancellation=error
            if child is not None and child.poll() is None:cleanup=stop_producer(child,ACTIVE)
    after=None;seal_error=None
    try:after=verify()
    except BaseException as error:seal_error=repr(error)
    receipt=dict(controller_pid=os.getpid(),child_pid=child.pid if child else None,child_waited=child is not None and child.returncode is not None,exit_code=child.returncode if child else None,timed_out=timeout,cancellation=type(cancellation).__name__ if cancellation else None,cleanup=cleanup,command=command,started=started,finished=time.time(),producer_log_sha256=sha(CONTROL/'producer.log'),preparation_seal_before=seal,preparation_seal_after=after,post_run_seal_error=seal_error,thread_environment={key:env[key] for key in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS','VECLIB_MAXIMUM_THREADS','NUMEXPR_NUM_THREADS']})
    (CONTROL/'closure.json').write_text(json.dumps(receipt,indent=2)+'\n')
    if seal_error is not None:raise ValueError('post-run seal verification failed: '+seal_error)
    if cancellation is not None:raise cancellation
    print('producer actual closed exit '+str(child.returncode),flush=True)
    raise SystemExit(child.returncode if not timeout else 124)
if __name__=='__main__':main()
