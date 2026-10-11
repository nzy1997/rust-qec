"""Fixed full matrix native producer; no adaptive timing or hidden warm-up."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))

import argparse, os, platform
from pathlib import Path
from common import Recorder, meta, read, require, seal, verify_seal, write
from probe_contract import check
from schedule import INPUTS, schedule
HERE=Path(__file__).resolve().parent

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--preparation",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True);args=parser.parse_args()
    require(os.environ.get("GITHUB_ACTIONS")=="true" and platform.system()=="Linux" and platform.machine()=="x86_64")
    prep=args.preparation.resolve();out=args.out.resolve();prepseal=verify_seal(prep);info=read(prep/"preparation.json")
    require(not out.exists());out.mkdir(parents=True);recorder=Recorder(out/"streams");failure=None;context={}
    try:
        require(all(meta(HERE/name)==item for name,item in info["protocol_files"].items()))
        cpu=min(os.sched_getaffinity(0));os.sched_setaffinity(0,{cpu})
        host={"platform":platform.platform(),"selected_cpu":cpu,"actual_affinity":sorted(os.sched_getaffinity(0)),
            "cpuinfo":Path("/proc/cpuinfo").read_text(),"meminfo":Path("/proc/meminfo").read_text(),"virtual_machine":True}
        write(out/"host.json",host)
        events=schedule("validate")+schedule("bench");write(out/"streams/streams.json",{"schema":"rstim.fixed-call-streams.draft.v1","events":events})
        for event in events:
            role=event["role"];native_role="baseline" if role=="control" else role
            binary=prep/native_role/"fixed-call.bin";require(meta(binary)==info["roles"][role]["binary"])
            name,shots,policy,factor=event["cell"];fixture=prep/"fixtures"/(name+".stim");require(meta(fixture)["sha256"]==INPUTS[name])
            argv=[str(binary),str(fixture),str(shots),policy,factor,event["action"]]
            recorder.run(event["id"],argv,cwd=HERE,timeout=300)
            check(read(out/"streams"/(event["id"]+".stdout")),shots,policy,factor,event["action"],INPUTS[name])
        require(verify_seal(prep)==prepseal)
        context={"schema":"rstim.fixed-call-run.v1","preparation_seal":prepseal,"protocol_revision":info["protocol_revision"],"events":len(events),"cpu":cpu}
        write(out/"run.json",context)
    except BaseException as exc:failure=repr(exc);write(out/"failure.json",{"error":failure});raise
    finally:recorder.close(failure,context);seal(out)
if __name__=="__main__":main()
