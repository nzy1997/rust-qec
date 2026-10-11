"""Replay complete native lifecycle bytes, receipt closure, guards and streams."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))

import argparse, hashlib, json, tarfile
from pathlib import Path
from common import Recorder, executed, meta, read, require, verify_seal, write
from prepare import BASELINE, CANDIDATE, COMPILER, PUBLIC, BASELINE_PUBLIC, REVIEWED_PROBE, AXIS, LAYOUT, PACKAGE, WORKFLOW, PROTOCOL_FILES, archive_files
from schedule import INPUTS, schedule
from verify import verify

def receipt(directory,label,argv=None):
    item=read(directory/(label+".receipt.json"))
    require(all(type(item[key]) is int and item[key]>0 for key in ("controller_pid","child_pid")))
    require(item["exit_code"]==0 and item["child_waited"] and item["cancellation"] is None and item["kill_error"] is None)
    require(item["stdout"]==meta(directory/(label+".stdout")) and item["stderr"]==meta(directory/(label+".stderr")))
    require(item["started"]<=item["closed"])
    start=read(directory/(label+".start.json"))
    require(all(type(start[key]) is int and start[key]>0 for key in ("controller_pid","child_pid")))
    for key in ("argv","cwd","controller_pid","child_pid","started"):require(item[key]==start[key])
    if argv is not None:require(item["argv"]==argv)
    return item

def closure(directory,labels):
    item=read(directory/"controller-closed.json")
    require(type(item["controller_pid"]) is int and item["controller_pid"]>0)
    require(all(type(item[key]) is list and all(type(pid) is int and pid>0 for pid in item[key]) for key in ("children","waited_children")))
    require(item["failure"] is None and item["all_children_reaped"])
    children=[receipt(directory,label)["child_pid"] for label in labels]
    require(item["children"]==children and item["waited_children"]==children and len(set(children))==len(children))
    require(all(receipt(directory,label)["controller_pid"]==item["controller_pid"] for label in labels))
    require({path.name for path in directory.glob("*.receipt.json")}=={label+".receipt.json" for label in labels})
    return item

def verify_native(preparation,output,expected_protocol):
    prep=Path(preparation);out=Path(output)
    prepseal=verify_seal(prep);verify_seal(prep/"commands");verify_seal(out);verify_seal(out/"streams")
    info=read(prep/"preparation.json");run=read(out/"run.json")
    require(info["schema"]=="rstim.fixed-call-preparation.v1" and run["schema"]=="rstim.fixed-call-run.v1")
    require(len(expected_protocol)==40 and all(c in "0123456789abcdef" for c in expected_protocol) and info["protocol_revision"]==expected_protocol)
    require(run["preparation_seal"]==prepseal and run["protocol_revision"]==info["protocol_revision"])
    require(info["compiler_version"]==COMPILER and info["roles"]["control"]==info["roles"]["baseline"])
    require(set(info["roles"])=={"baseline","candidate","control"})
    require({path.name for path in (prep/"protocol").iterdir()}==set(info["protocol_files"])==set(PROTOCOL_FILES))
    require(all(meta(prep/"protocol"/name)==item for name,item in info["protocol_files"].items()))
    require(all(meta(Path(__file__).resolve().parent/name)==item for name,item in info["protocol_files"].items()))
    require(meta(prep/"workflow.yml")==meta(prep/"protocol/workflow-overlay.yml"))
    require(info["compiler"].startswith("rustc "+COMPILER+" ") and "host: x86_64-unknown-linux-gnu" in info["compiler"])
    require(all(meta(prep/"protocol"/name)["sha256"]==digest for name,digest in REVIEWED_PROBE.items()))
    original_prep=Path(info["original_preparation_directory"]);require(original_prep.is_absolute())
    command_dir=prep/"commands"
    receipt(command_dir,"compiler",["rustup","run",COMPILER,"rustc","-Vv"])
    require((command_dir/"compiler.stdout").read_text()==info["compiler"])
    require((command_dir/"protocol-head.stdout").read_text().strip()==info["protocol_revision"])
    for label in ("protocol-status-before","protocol-status-after"):
        receipt(command_dir,label,["git","status","--porcelain"]);require((command_dir/(label+".stdout")).read_bytes()==b"")
    receipt(command_dir,"archive-protocol",["git","archive","--format=tar",expected_protocol,PACKAGE,WORKFLOW])
    protocol_archive=command_dir/"archive-protocol.stdout"
    expected_files={PACKAGE+"/"+name:meta(prep/"protocol"/name) for name in PROTOCOL_FILES}
    expected_files.update({PACKAGE+"/fixtures/"+name+".stim":meta(prep/"fixtures"/(name+".stim")) for name in INPUTS})
    expected_files[WORKFLOW]=meta(prep/"workflow.yml")
    require(meta(protocol_archive)==info["protocol_archive"] and archive_files(protocol_archive)==info["protocol_source_files"]==expected_files)
    labels=["protocol-head","protocol-status-before","archive-protocol","compiler"]
    for role,head in (("baseline",BASELINE),("candidate",CANDIDATE)):
        binding=info["roles"][role];require(binding["head"]==head)
        receipt(command_dir,"archive-"+role,["git","archive","--format=tar",head]);labels.append("archive-"+role)
        archive=command_dir/("archive-"+role+".stdout");require(meta(archive)==binding["archive"])
        files={}
        with tarfile.open(archive,"r:") as stream:
            for member in stream:
                require(member.isdir() or member.isfile())
                if member.isfile():
                    require(member.name not in files and not Path(member.name).is_absolute() and ".." not in Path(member.name).parts)
                    with stream.extractfile(member) as source:data=source.read()
                    files[member.name]={"bytes":len(data),"sha256":hashlib.sha256(data).hexdigest()}
        require(files==binding["source_files"])
        build=receipt(command_dir,"build-"+role);labels.append("build-"+role)
        require(build["argv"][:7]==["rustup","run",COMPILER,"cargo","build","--release","--locked"] and len(build["argv"])==9 and build["argv"][7]=="--manifest-path")
        require(build["argv"][8]==str(original_prep/role/"Cargo.toml"))
        env=build["environment"]
        require(env["RUSTFLAGS"]=="-C target-cpu=native")
        require(all(env[key] is None for key in ("RUSTC","RUSTC_WRAPPER","RUSTC_WORKSPACE_WRAPPER","CARGO_ENCODED_RUSTFLAGS")))
        require(all(env[key]=="1" for key in ("OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS","RAYON_NUM_THREADS","VECLIB_MAXIMUM_THREADS")))
        require(all(env[key]==value for key,value in binding["build_environment"].items()))
        for name in ("main.rs","Cargo.lock"):
            require(meta(prep/role/name)==meta(prep/"protocol"/name))
        template=(prep/"protocol/Cargo.toml").read_text();require(template.count('path = "../../../rstim"')==1)
        expected=template.replace('path = "../../../rstim"',"path = "+json.dumps(str(Path(build["cwd"])/"rstim")))
        require((prep/role/"Cargo.toml").read_text()==expected)
        require(binding["probe"]=={name:meta(prep/role/name) for name in ("main.rs","Cargo.toml","Cargo.lock")})
        binary=prep/role/"fixed-call.bin";require(meta(binary)==binding["binary"])
        data=binary.read_bytes();require(data[:6]==b"\x7fELF\x02\x01" and int.from_bytes(data[18:20],"little")==62)
        selected_public=BASELINE_PUBLIC if role=="baseline" else PUBLIC
        guards=[("public",[selected_public])]
        if role=="candidate":
            with tarfile.open(archive,"r:") as stream:
                text=stream.extractfile("rstim/tests/near_clifford_postselected_counts.rs").read().decode()
                require('assert!(errors > 0, "finite comparisons must witness logical errors")' in text)
            guards += [("axis",AXIS),("layout",[LAYOUT])]
        require(binding["guards"]==[{"label":label,"expected":expected} for label,expected in guards])
        for label,expected in guards:
            name="guard-"+role+"-"+label;test=receipt(command_dir,name);labels.append(name)
            require(test["argv"][:9]==["rustup","run",COMPILER,"cargo","test","--release","--locked","-p","rstim"])
            if label=="public": selection=["--test","near_clifford_postselected_counts",selected_public,"--","--exact"]
            elif label=="axis": selection=["--lib","near_clifford::compiled::cached_coefficients::tests::"]
            else: selection=["--lib",LAYOUT,"--","--exact"]
            require(test["argv"]==["rustup","run",COMPILER,"cargo","test","--release","--locked","-p","rstim","--no-default-features",*selection])
            require(test["cwd"]==build["cwd"] and test["environment"]==build["environment"])
            executed((command_dir/(name+".stdout")).read_text(),expected)
    labels.append("protocol-status-after");preparation_closure=closure(command_dir,labels)
    for name,expected in INPUTS.items():require(meta(prep/"fixtures"/(name+".stim"))["sha256"]==expected)
    events=schedule("validate")+schedule("bench");require(run["events"]==len(events))
    worker_labels=[]
    for event in events:
        native_role="baseline" if event["role"]=="control" else event["role"]
        item=receipt(out/"streams",event["id"]);worker_labels.append(event["id"])
        name,shots,policy,factor=event["cell"]
        # Exact original paths are retained in argv; they may relocate after download.
        require(item["argv"]==[str(original_prep/native_role/"fixed-call.bin"),str(original_prep/"fixtures"/(name+".stim")),str(shots),policy,factor,event["action"]])
    run_closure=closure(out/"streams",worker_labels)
    host=read(out/"host.json");require(host["actual_affinity"]==[host["selected_cpu"]] and host["selected_cpu"]==run["cpu"])
    result=verify(out/"streams")
    result.update(preparation_seal=prepseal,protocol_revision=info["protocol_revision"],
        candidate=CANDIDATE,baseline=BASELINE,native_guard_tests=6,
        native_pid_union=sorted({preparation_closure["controller_pid"],*preparation_closure["children"],run_closure["controller_pid"],*run_closure["children"]}),
        qualification="Integrity replay only; complete independent review/replication and full production CI remain separate. Positive native joint guard is not a rare-logical peer confidence statement.")
    return result

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("preparation",type=Path);parser.add_argument("output",type=Path)
    parser.add_argument("--git-repo",type=Path,required=True);parser.add_argument("--audit-out",type=Path,required=True)
    parser.add_argument("--expected-protocol",required=True);args=parser.parse_args()
    require((args.git_repo is None)==(args.audit_out is None))
    result=verify_native(args.preparation,args.output,args.expected_protocol)
    result["git_source_replay"]=False
    if args.git_repo is not None:
        recorder=Recorder(args.audit_out.resolve());failure=None
        try:
            recorder.run("archive-protocol",["git","archive","--format=tar",args.expected_protocol,PACKAGE,WORKFLOW],cwd=args.git_repo.resolve())
            require(meta(args.audit_out/"archive-protocol.stdout")==meta(args.preparation/"commands/archive-protocol.stdout"))
            for role,head in (("baseline",BASELINE),("candidate",CANDIDATE)):
                recorder.run("archive-"+role,["git","archive","--format=tar",head],cwd=args.git_repo.resolve())
                require(meta(args.audit_out/("archive-"+role+".stdout"))==meta(args.preparation/"commands"/("archive-"+role+".stdout")))
            result["git_source_replay"]=True
        except BaseException as exc:failure=repr(exc);raise
        finally:recorder.close(failure,{"scope":"Independent exact Git archive replay; no timed calls"})
    print(json.dumps(result,indent=2))
