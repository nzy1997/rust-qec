"""Synthetic byte/receipt/guard negative checks, no native result."""
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import io, json, tarfile, tempfile
from common import executed, inventory, meta, require, seal, verify_seal, write
from prepare import unpack
from verify_native import receipt, closure

def rejects(function):
    try:function()
    except (ValueError,KeyError,TypeError):return
    raise ValueError("negative fixture accepted")

def main():
    with tempfile.TemporaryDirectory(prefix="synthetic-native-contract-") as temp:
        root=Path(temp)
        directory=root/"seal";directory.mkdir();(directory/"data").write_bytes(b"a");digest=seal(directory)
        require(verify_seal(directory,digest)==digest)
        (directory/"data").write_bytes(b"b");rejects(lambda:verify_seal(directory,digest));(directory/"data").write_bytes(b"a")
        (directory/"extra").write_bytes(b"a");rejects(lambda:verify_seal(directory,digest));(directory/"extra").unlink()
        (directory/"link").symlink_to(directory/"data");rejects(lambda:verify_seal(directory,digest));(directory/"link").unlink()
        guard="test exact_guard ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n"
        executed(guard,["exact_guard"])
        for bad in (guard.replace("exact_guard","wrong_guard"),guard.replace("... ok","... ignored"),guard.replace("1 passed","0 passed"),guard+guard):rejects(lambda:executed(bad,["exact_guard"]))
        commands=root/"commands";commands.mkdir();label="worker"
        (commands/(label+".stdout")).write_bytes(b"{}");(commands/(label+".stderr")).write_bytes(b"")
        start=dict(controller_pid=123,child_pid=124,argv=["synthetic"],cwd="/synthetic",started=1)
        item=dict(start,closed=2,exit_code=0,child_waited=True,cancellation=None,kill_error=None,stdout=meta(commands/(label+".stdout")),stderr=meta(commands/(label+".stderr")))
        write(commands/(label+".start.json"),start);write(commands/(label+".receipt.json"),item)
        closed=dict(controller_pid=123,children=[124],waited_children=[124],all_children_reaped=True,failure=None)
        write(commands/"controller-closed.json",closed);receipt(commands,label,["synthetic"]);closure(commands,[label])
        for key,value in (("exit_code",1),("child_waited",False),("cancellation","TimeoutExpired"),("kill_error","denied")):
            write(commands/(label+".receipt.json"),dict(item,**{key:value}));rejects(lambda:receipt(commands,label));write(commands/(label+".receipt.json"),item)
        (commands/(label+".stdout")).write_bytes(b"changed");rejects(lambda:receipt(commands,label));(commands/(label+".stdout")).write_bytes(b"{}")
        write(commands/"controller-closed.json",dict(closed,waited_children=[]));rejects(lambda:closure(commands,[label]));write(commands/"controller-closed.json",closed)
        write(commands/"extra.receipt.json",item);rejects(lambda:closure(commands,[label]));(commands/"extra.receipt.json").unlink()
        for key in ("controller_pid","child_pid"):
            for value in (-1,0,True,1.5):
                write(commands/(label+".start.json"),dict(start,**{key:value}))
                write(commands/(label+".receipt.json"),dict(item,**{key:value}))
                changed=dict(closed,controller_pid=value) if key=="controller_pid" else dict(closed,children=[value],waited_children=[value])
                write(commands/"controller-closed.json",changed)
                rejects(lambda:closure(commands,[label]))
        write(commands/(label+".start.json"),start);write(commands/(label+".receipt.json"),item);write(commands/"controller-closed.json",closed)
        for key in ("controller_pid","child_pid"):
            for value in (-1,0,True,float(start[key])):
                write(commands/(label+".start.json"),dict(start,**{key:value}))
                rejects(lambda:closure(commands,[label]))
        write(commands/(label+".start.json"),start)
        for key in ("children","waited_children"):
            for value in (-1,0,True,124.0):
                write(commands/"controller-closed.json",dict(closed,**{key:[value]}))
                rejects(lambda:closure(commands,[label]))
        write(commands/"controller-closed.json",closed)
        for index,name in enumerate(("../escape", "/absolute", "safe")):
            archive=root/f"bad-{index}.tar"
            with tarfile.open(archive,"w:") as stream:
                member=tarfile.TarInfo(name)
                if name=="safe":member.type=tarfile.SYMTYPE;member.linkname="../escape";stream.addfile(member)
                else:member.size=1;stream.addfile(member,io.BytesIO(b"x"))
            rejects(lambda:unpack(archive,root/f"extract-{index}"))
    print("PASS synthetic native primitives only: 3 sealed-byte, 4 named-guard, 31 receipt/closure including all PID field types, 3 unsafe-archive negative cases")
if __name__=="__main__":main()
