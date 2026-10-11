"""Native receipts and closed byte inventories; no assertion-only gates."""
import hashlib, json, os, re, signal, stat, subprocess, time
from pathlib import Path
from probe_contract import read, require

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def meta(path):return {"bytes":Path(path).stat().st_size,"sha256":sha(path)}
def write(path, value):Path(path).write_text(json.dumps(value,indent=2)+"\n")
def inventory(root):
    result={}
    for path in sorted(Path(root).rglob("*")):
        mode=path.lstat().st_mode
        require(stat.S_ISREG(mode) or stat.S_ISDIR(mode))
        if stat.S_ISREG(mode) and path != Path(root)/"seal.json":result[str(path.relative_to(root))]=meta(path)
    return result

def seal(root):
    require(not (Path(root)/"seal.json").exists())
    write(Path(root)/"seal.json",{"files":inventory(root)})
    return sha(Path(root)/"seal.json")

def verify_seal(root, expected=None):
    root=Path(root)
    if expected is not None:require(sha(root/"seal.json")==expected)
    document=read(root/"seal.json");require(set(document)=={"files"})
    require(document["files"]==inventory(root))
    return sha(root/"seal.json")

def executed(log, expected):
    records=[];summaries=[]
    for line in log.splitlines():
        if line.startswith("test result:"):summaries.append(line)
        elif line.startswith("test "):
            match=re.fullmatch(r"test (\S+) \.\.\. (.+)",line)
            require(match is not None);records.append(match.groups())
    require(len(records)==len(expected) and {name for name,status in records}==set(expected))
    require(all(status=="ok" for name,status in records))
    require(len(summaries)==1 and re.fullmatch(rf"test result: ok\. {len(expected)} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?",summaries[0]) is not None)

class Cancelled(RuntimeError):pass
def cancel(signum, frame):raise Cancelled("signal "+str(signum))
class Recorder:
    def __init__(self,directory):
        self.directory=Path(directory);self.directory.mkdir(parents=True,exist_ok=False)
        self.children=[];self.waited=[]
        signal.signal(signal.SIGTERM,cancel);signal.signal(signal.SIGINT,cancel)
    def run(self,label,argv,*,cwd,env=None,timeout=600):
        require(re.fullmatch(r"[a-zA-Z0-9_-]+",label) is not None)
        require(all(type(x) is str for x in argv))
        start=time.time();child=None;error=None;kill_error=None
        with (self.directory/(label+".stdout")).open("xb") as output, (self.directory/(label+".stderr")).open("xb") as errors:
            try:
                previous=signal.pthread_sigmask(signal.SIG_BLOCK,{signal.SIGTERM,signal.SIGINT})
                try:
                    child=subprocess.Popen(argv,cwd=cwd,env=env,stdout=output,stderr=errors,start_new_session=True,
                        preexec_fn=lambda:signal.pthread_sigmask(signal.SIG_SETMASK,previous))
                    self.children.append(child.pid)
                    write(self.directory/(label+".start.json"),{"controller_pid":os.getpid(),"child_pid":child.pid,"argv":argv,"cwd":str(cwd),"started":start})
                finally:signal.pthread_sigmask(signal.SIG_SETMASK,previous)
                child.wait(timeout=timeout)
            except BaseException as exc:
                error=exc
                if child is not None:
                    try:os.killpg(child.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                    except BaseException as kill_exc:kill_error=repr(kill_exc)
                    child.wait(timeout=20)
            if child is not None and child.returncode is not None:self.waited.append(child.pid)
        receipt={"controller_pid":os.getpid(),"child_pid":child.pid if child else None,"child_waited":child is not None and child.returncode is not None,
            "argv":argv,"cwd":str(cwd),"started":start,"closed":time.time(),"exit_code":child.returncode if child else None,
            "cancellation":type(error).__name__ if error else None,"kill_error":kill_error,
            "stdout":meta(self.directory/(label+".stdout")),"stderr":meta(self.directory/(label+".stderr"))}
        effective_env = os.environ if env is None else env
        receipt["environment"] = {key:effective_env.get(key) for key in (
            "RUSTFLAGS","CARGO_TARGET_DIR","RUSTC","RUSTC_WRAPPER","RUSTC_WORKSPACE_WRAPPER","CARGO_ENCODED_RUSTFLAGS",
            "OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS","RAYON_NUM_THREADS","VECLIB_MAXIMUM_THREADS")}
        write(self.directory/(label+".receipt.json"),receipt)
        if error is not None:raise error
        require(receipt["exit_code"]==0 and receipt["child_waited"])
        return receipt
    def close(self,failure,context):
        write(self.directory/"controller-closed.json",{"controller_pid":os.getpid(),"children":self.children,"waited_children":self.waited,
            "all_children_reaped":self.children==self.waited,"failure":failure,"context":context,"closed":time.time()})
        return seal(self.directory)
