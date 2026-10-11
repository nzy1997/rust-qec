"""Complete synthetic native bundle replay; contains fake ELF/receipts, no execution evidence."""
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import hashlib, io, json, shutil, tarfile, tempfile
from common import meta, seal, write
from prepare import AXIS, BASELINE, CANDIDATE, COMPILER, LAYOUT, PUBLIC, BASELINE_PUBLIC, PROTOCOL_FILES, PACKAGE, WORKFLOW, archive_files
from schedule import INPUTS, schedule
from test_contract import payload
from test_native_contract import rejects
from verify_native import verify_native
HERE=Path(__file__).resolve().parent

def main():
    with tempfile.TemporaryDirectory(prefix="synthetic-complete-native-") as temporary:
        root=Path(temporary);prep=root/"preparation";out=root/"output";commands=prep/"commands";streams=out/"streams"
        commands.mkdir(parents=True);streams.mkdir(parents=True);protocol=prep/"protocol";protocol.mkdir();fixtures=prep/"fixtures";fixtures.mkdir()
        for name in PROTOCOL_FILES:shutil.copyfile(HERE/name,protocol/name)
        shutil.copyfile(HERE/"workflow-overlay.yml",prep/"workflow.yml")
        for name in INPUTS:shutil.copyfile(HERE/"fixtures"/(name+".stim"),fixtures/(name+".stim"))
        env=dict(RUSTFLAGS="-C target-cpu=native",CARGO_TARGET_DIR="/synthetic/target",RUSTC=None,RUSTC_WRAPPER=None,RUSTC_WORKSPACE_WRAPPER=None,CARGO_ENCODED_RUSTFLAGS=None,
            OMP_NUM_THREADS="1",OPENBLAS_NUM_THREADS="1",MKL_NUM_THREADS="1",RAYON_NUM_THREADS="1",VECLIB_MAXIMUM_THREADS="1")
        ids=[]
        def command(label,argv,stdout=b"",cwd="/synthetic/source"):
            index=len(ids);ids.append(1000+index)
            (commands/(label+".stdout")).write_bytes(stdout);(commands/(label+".stderr")).write_bytes(b"")
            start=dict(controller_pid=999,child_pid=ids[-1],argv=argv,cwd=cwd,started=index)
            write(commands/(label+".start.json"),start)
            write(commands/(label+".receipt.json"),dict(start,closed=index+0.5,exit_code=0,child_waited=True,cancellation=None,kill_error=None,stdout=meta(commands/(label+".stdout")),stderr=meta(commands/(label+".stderr")),environment=env))
        revision="a"*40;compiler="rustc "+COMPILER+" (synthetic)\nhost: x86_64-unknown-linux-gnu\n"
        command("protocol-head",["git","rev-parse","HEAD"],(revision+"\n").encode())
        command("protocol-status-before",["git","status","--porcelain"])
        buffer=io.BytesIO()
        with tarfile.open(fileobj=buffer,mode="w:") as output:
            paths={PACKAGE+"/"+name:protocol/name for name in PROTOCOL_FILES}
            paths.update({PACKAGE+"/fixtures/"+name+".stim":fixtures/(name+".stim") for name in INPUTS})
            paths[WORKFLOW]=prep/"workflow.yml"
            for name,path in paths.items():
                data=path.read_bytes();item=tarfile.TarInfo(name);item.size=len(data);output.addfile(item,io.BytesIO(data))
        command("archive-protocol",["git","archive","--format=tar",revision,PACKAGE,WORKFLOW],buffer.getvalue())
        command("compiler",["rustup","run",COMPILER,"rustc","-Vv"],compiler.encode())
        roles={}
        for role,head in (("baseline",BASELINE),("candidate",CANDIDATE)):
            directory=prep/role;directory.mkdir()
            archive=commands/("archive-"+role+".stdout")
            source=b'assert!(errors > 0, "finite comparisons must witness logical errors");'
            buffer=io.BytesIO()
            with tarfile.open(fileobj=buffer,mode="w:") as output:
                item=tarfile.TarInfo("rstim/tests/near_clifford_postselected_counts.rs");item.size=len(source);output.addfile(item,io.BytesIO(source))
            command("archive-"+role,["git","archive","--format=tar",head],buffer.getvalue())
            for name in ("main.rs","Cargo.lock"):shutil.copyfile(protocol/name,directory/name)
            text=(protocol/"Cargo.toml").read_text().replace('path = "../../../rstim"','path = "/synthetic/source/rstim"');(directory/"Cargo.toml").write_text(text)
            command("build-"+role,["rustup","run",COMPILER,"cargo","build","--release","--locked","--manifest-path",str(directory/"Cargo.toml")])
            elf=bytearray(64);elf[:6]=b"\x7fELF\x02\x01";elf[18:20]=(62).to_bytes(2,"little");(directory/"fixed-call.bin").write_bytes(elf)
            selected_public=BASELINE_PUBLIC if role=="baseline" else PUBLIC
            guards=[("public",[selected_public])]
            if role=="candidate":guards += [("axis",AXIS),("layout",[LAYOUT])]
            for label,expected in guards:
                log="".join("test "+name+" ... ok\n" for name in expected)+f"test result: ok. {len(expected)} passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.01s\n"
                if label=="public":selection=["--test","near_clifford_postselected_counts",selected_public,"--","--exact"]
                elif label=="axis":selection=["--lib","near_clifford::compiled::cached_coefficients::tests::"]
                else:selection=["--lib",LAYOUT,"--","--exact"]
                command("guard-"+role+"-"+label,["rustup","run",COMPILER,"cargo","test","--release","--locked","-p","rstim","--no-default-features",*selection],log.encode())
            roles[role]=dict(head=head,archive=meta(archive),source_files={"rstim/tests/near_clifford_postselected_counts.rs":{"bytes":len(source),"sha256":hashlib.sha256(source).hexdigest()}},binary=meta(directory/"fixed-call.bin"),probe={name:meta(directory/name) for name in ("main.rs","Cargo.toml","Cargo.lock")},guards=[dict(label=label,expected=expected) for label,expected in guards],build_environment=env)
        roles["control"]=roles["baseline"]
        command("protocol-status-after",["git","status","--porcelain"])
        write(commands/"controller-closed.json",dict(controller_pid=999,children=ids,waited_children=ids,all_children_reaped=True,failure=None));seal(commands)
        info=dict(schema="rstim.fixed-call-preparation.v1",protocol_revision=revision,compiler=compiler,compiler_version=COMPILER,roles=roles,protocol_files={name:meta(protocol/name) for name in PROTOCOL_FILES},original_preparation_directory=str(prep),protocol_archive=meta(commands/"archive-protocol.stdout"),protocol_source_files=archive_files(commands/"archive-protocol.stdout"))
        write(prep/"preparation.json",info);prepseal=seal(prep)
        events=schedule("validate")+schedule("bench");write(streams/"streams.json",dict(schema="rstim.fixed-call-streams.draft.v1",events=events));children=[]
        for index,event in enumerate(events):
            label=event["id"];name,shots,policy,factor=event["cell"];role="baseline" if event["role"]=="control" else event["role"]
            write(streams/(label+".stdout"),payload(event));(streams/(label+".stderr")).write_bytes(b"")
            start=dict(controller_pid=9999,child_pid=10000+index,argv=[str(prep/role/"fixed-call.bin"),str(fixtures/(name+".stim")),str(shots),policy,factor,event["action"]],cwd=str(HERE),started=index)
            children.append(start["child_pid"]);write(streams/(label+".start.json"),start)
            write(streams/(label+".receipt.json"),dict(start,closed=index+0.5,exit_code=0,child_waited=True,cancellation=None,kill_error=None,stdout=meta(streams/(label+".stdout")),stderr=meta(streams/(label+".stderr"))))
        write(streams/"controller-closed.json",dict(controller_pid=9999,children=children,waited_children=children,all_children_reaped=True,failure=None));seal(streams)
        write(out/"run.json",dict(schema="rstim.fixed-call-run.v1",preparation_seal=prepseal,protocol_revision=revision,events=len(events),cpu=0));write(out/"host.json",dict(actual_affinity=[0],selected_cpu=0));seal(out)
        result=verify_native(prep,out,revision)
        if len(result["comparisons"])!=216 or result["native_guard_tests"]!=6:raise ValueError("complete replay coverage")
        # Restore a valid complete bundle between coherent, resealed counterexamples.
        original={path.relative_to(root):path.read_bytes() for path in root.rglob("*") if path.is_file()}
        def restore():
            for rel,data in original.items():(root/rel).write_bytes(data)
        def reseal():
            for directory in (commands,streams,prep,out):
                (directory/"seal.json").unlink();seal(directory)
                if directory==prep:
                    run=json.loads((out/"run.json").read_text());run["preparation_seal"]=meta(prep/"seal.json")["sha256"];write(out/"run.json",run)
        def change_receipt(directory,label,key,value):
            for suffix in ("start","receipt"):
                path=directory/(label+"."+suffix+".json");item=json.loads(path.read_text());item[key]=value;write(path,item)
        def reject_change(mutator):
            restore();mutator();reseal();rejects(lambda:verify_native(prep,out,revision))
        def wrong_head():
            info=json.loads((prep/"preparation.json").read_text());info["roles"]["candidate"]["head"]="b"*40;write(prep/"preparation.json",info)
        reject_change(wrong_head)
        guard="guard-candidate-public"
        for key,value in (("RUSTC_WRAPPER","/wrong/wrapper"),("RUSTFLAGS","-C opt-level=0")):
            def wrong_env(key=key,value=value):
                path=commands/(guard+".receipt.json");item=json.loads(path.read_text());item["environment"][key]=value;write(path,item)
            reject_change(wrong_env)
        reject_change(lambda:change_receipt(commands,guard,"cwd","/different/source"))
        argv=json.loads((commands/(guard+".receipt.json")).read_text())["argv"]
        reject_change(lambda:change_receipt(commands,guard,"argv",argv[:-1]+["--ignored"]))
        label=events[0]["id"];worker=json.loads((streams/(label+".receipt.json")).read_text())["argv"]
        reject_change(lambda:change_receipt(streams,label,"argv",["/different-root/baseline/fixed-call.bin",*worker[1:]]))
        restore();rejects(lambda:verify_native(prep,out,"b"*40))
        def changed_probe():
            # Update all self-declared byte bindings and copied probe; immutable reviewed pin must reject.
            path=protocol/"main.rs";path.write_bytes(path.read_bytes()+b"\n// changed clock\n")
            info=json.loads((prep/"preparation.json").read_text());info["protocol_files"]["main.rs"]=meta(path)
            for role in ("baseline","candidate"):
                shutil.copyfile(path,prep/role/"main.rs");info["roles"][role]["probe"]["main.rs"]=meta(path)
            info["roles"]["control"]=info["roles"]["baseline"];write(prep/"preparation.json",info)
        reject_change(changed_probe)
        def missing_protocol():
            (protocol/"README.md").unlink();info=json.loads((prep/"preparation.json").read_text());del info["protocol_files"]["README.md"];write(prep/"preparation.json",info)
        reject_change(missing_protocol)
        restore()
        def wrong_workflow():
            (prep/"workflow.yml").write_bytes(b"unreviewed workflow")
        reject_change(wrong_workflow)
        restore();verify_native(prep,out,revision)
    print("PASS complete SYNTHETIC native bundle: 72 finite + 864 timing streams, all receipts/closures/sources/6 guard names/216 comparisons; 10 coherent resealed source/protocol/guard/path counterexamples rejected. Fake ELF/receipts are not native evidence.")
if __name__=="__main__":main()
