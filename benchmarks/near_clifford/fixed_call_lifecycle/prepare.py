"""Cloud-only native preparation of the immutable axis lifecycle pair."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))

import argparse, hashlib, json, os, platform, re, shutil, tarfile
from pathlib import Path, PurePosixPath
from common import Recorder, executed, meta, read, require, seal, write
from schedule import INPUTS
HERE=Path(__file__).resolve().parent
PACKAGE="benchmarks/near_clifford/fixed_call_lifecycle"
WORKFLOW=".github/workflows/near-clifford-x86-application-counts.yml"
PROTOCOL_FILES=("Cargo.lock","Cargo.toml","README.md","common.py","main.rs","prepare.py","probe_contract.py","run.py","schedule.py","test_contract.py","test_native_bundle.py","test_native_contract.py","verify.py","verify_native.py","workflow-overlay.yml")

def archive_files(archive):
    files={}
    with tarfile.open(archive,"r:") as stream:
        names=set()
        for member in stream:
            path=PurePosixPath(member.name)
            require(not path.is_absolute() and ".." not in path.parts and member.name not in names)
            require(member.isdir() or member.isfile());names.add(member.name)
            if member.isfile():
                with stream.extractfile(member) as source:data=source.read()
                files[member.name]={"bytes":len(data),"sha256":hashlib.sha256(data).hexdigest()}
    return files
BASELINE="6e079197ce9ba62079744417f3f701d4562fa149"
CANDIDATE="63a4fe7c30e2f894f2c15ec28da569d903638207"
COMPILER="1.93.1"
PUBLIC="wide_cached_amplitudes_preserve_noise_feedback_counts_records_and_rng"
BASELINE_PUBLIC="postselected_counts_preserve_noise_projection_feedback_and_rng_across_routes"
REVIEWED_PROBE={"main.rs":"63ed65b5a65016f19b40af0027acfe03f5d3a64bab8932dafb912d290067bbf1","Cargo.lock":"2cea45defc0dd77661d9053d75cb133fb30a4f9bb8abb27ebd27d47e4fc06325"}
AXIS=["near_clifford::compiled::cached_coefficients::tests::"+name for name in (
    "axis_storage_preserves_every_component_bit_and_tail",
    "mixed_components_and_small_vectors_keep_dense_storage",
    "original_default_cultivation_uses_axis_storage_with_exact_rows_and_rng")]
LAYOUT="near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage"

def unpack(archive,destination):
    destination.mkdir(parents=True,exist_ok=False)
    with tarfile.open(archive,"r:") as stream:
        members=stream.getmembers();names=set()
        for member in members:
            path=PurePosixPath(member.name)
            require(not path.is_absolute() and ".." not in path.parts and member.name not in names)
            require(member.isdir() or member.isfile());names.add(member.name)
        for member in members:
            path=destination/member.name
            if member.isdir():path.mkdir(parents=True,exist_ok=True)
            else:
                path.parent.mkdir(parents=True,exist_ok=True)
                with stream.extractfile(member) as source:path.write_bytes(source.read())
                path.chmod(member.mode & 0o777)

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--repo",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True);parser.add_argument("--scratch",type=Path,required=True)
    args=parser.parse_args();repo=args.repo.resolve();out=args.out.resolve();scratch=args.scratch.resolve()
    require(os.environ.get("GITHUB_ACTIONS")=="true" and platform.system()=="Linux" and platform.machine()=="x86_64")
    require(not out.exists() and not scratch.exists());out.mkdir(parents=True);scratch.mkdir(parents=True)
    recorder=Recorder(out/"commands");failure=None;context={}
    def command(label,argv,cwd=repo,env=None):return recorder.run(label,argv,cwd=cwd,env=env,timeout=600)
    try:
        command("protocol-head",["git","rev-parse","HEAD"]);revision=(out/"commands/protocol-head.stdout").read_text().strip();require(re.fullmatch("[0-9a-f]{40}",revision) is not None)
        command("protocol-status-before",["git","status","--porcelain"]);require((out/"commands/protocol-status-before.stdout").read_bytes()==b"")
        require(HERE==repo/PACKAGE and (repo/WORKFLOW).read_bytes()==(HERE/"workflow-overlay.yml").read_bytes())
        command("archive-protocol",["git","archive","--format=tar",revision,PACKAGE,WORKFLOW])
        protocol_archive=out/"commands/archive-protocol.stdout"
        protocol_source_files=archive_files(protocol_archive)
        expected_protocol={PACKAGE+"/"+name:meta(HERE/name) for name in PROTOCOL_FILES}
        expected_protocol.update({PACKAGE+"/fixtures/"+name+".stim":meta(HERE/"fixtures"/(name+".stim")) for name in INPUTS})
        expected_protocol[WORKFLOW]=meta(repo/WORKFLOW)
        require(protocol_source_files==expected_protocol)
        command("compiler",["rustup","run",COMPILER,"rustc","-Vv"]);compiler=(out/"commands/compiler.stdout").read_text();require(compiler.startswith("rustc "+COMPILER+" ") and "host: x86_64-unknown-linux-gnu" in compiler)
        require(all(meta(HERE/name)["sha256"]==digest for name,digest in REVIEWED_PROBE.items()))
        protocol=out/"protocol";protocol.mkdir()
        inputs={}
        for name in PROTOCOL_FILES:
            path=HERE/name;shutil.copyfile(path,protocol/name);inputs[name]=meta(path)
        shutil.copyfile(repo/WORKFLOW,out/"workflow.yml")
        fixtures=out/"fixtures";fixtures.mkdir()
        for name,expected in INPUTS.items():
            source=HERE/"fixtures"/(name+".stim")
            require(meta(source)["sha256"]==expected);shutil.copyfile(source,fixtures/source.name)
        env=os.environ.copy()
        removed=[key for key in env if key in ("RUSTC","RUSTDOC") or key.startswith(("RUSTFLAGS","RUSTC_","CARGO_PROFILE_","CARGO_ENCODED_RUSTFLAGS","CARGO_BUILD_","CARGO_TARGET_"))]
        for key in removed:env.pop(key)
        env["RUSTFLAGS"]="-C target-cpu=native"
        for key in ("OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS","RAYON_NUM_THREADS","VECLIB_MAXIMUM_THREADS"):env[key]="1"
        roles={}
        for role,head in (("baseline",BASELINE),("candidate",CANDIDATE)):
            command("archive-"+role,["git","archive","--format=tar",head])
            archive=out/"commands"/("archive-"+role+".stdout");source=scratch/role;unpack(archive,source)
            probe=out/role;probe.mkdir()
            for name in ("main.rs","Cargo.lock"):shutil.copyfile(HERE/name,probe/name)
            manifest=(HERE/"Cargo.toml").read_text();needle='path = "../../../rstim"';require(manifest.count(needle)==1)
            (probe/"Cargo.toml").write_text(manifest.replace(needle,"path = "+json.dumps(str(source/"rstim"))))
            source_files={str(path.relative_to(source)):meta(path) for path in source.rglob("*") if path.is_file()}
            buildenv=dict(env,CARGO_TARGET_DIR=str(scratch/(role+"-target")))
            command("build-"+role,["rustup","run",COMPILER,"cargo","build","--release","--locked","--manifest-path",str(probe/"Cargo.toml")],source,buildenv)
            binary=probe/"fixed-call.bin";shutil.copy2(Path(buildenv["CARGO_TARGET_DIR"])/"release/near-clifford-application-counts",binary)
            require(binary.read_bytes()[:5]==b"\x7fELF\x02" and int.from_bytes(binary.read_bytes()[18:20],"little")==62)
            selected_public=BASELINE_PUBLIC if role=="baseline" else PUBLIC
            selections=[("public",["--test","near_clifford_postselected_counts",selected_public,"--","--exact"],[selected_public])]
            if role=="candidate":
                guard=(source/"rstim/tests/near_clifford_postselected_counts.rs").read_text()
                require('assert!(errors > 0, "finite comparisons must witness logical errors")' in guard)
                selections += [("axis",["--lib","near_clifford::compiled::cached_coefficients::tests::"],AXIS),
                    ("layout",["--lib",LAYOUT,"--","--exact"],[LAYOUT])]
            guards=[]
            for label,selection,expected in selections:
                command("guard-"+role+"-"+label,["rustup","run",COMPILER,"cargo","test","--release","--locked","-p","rstim","--no-default-features",*selection],source,buildenv)
                executed((out/"commands"/("guard-"+role+"-"+label+".stdout")).read_text(),expected);guards.append({"label":label,"expected":expected})
            require(source_files=={str(path.relative_to(source)):meta(path) for path in source.rglob("*") if path.is_file()})
            roles[role]={"head":head,"archive":meta(archive),"source_files":source_files,"binary":meta(binary),
                "probe":{name:meta(probe/name) for name in ("main.rs","Cargo.toml","Cargo.lock")},"guards":guards,
                "build_environment":{key:buildenv[key] for key in ("RUSTFLAGS","CARGO_TARGET_DIR","OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS","RAYON_NUM_THREADS","VECLIB_MAXIMUM_THREADS")},"removed_environment_keys":removed}
        roles["control"]=roles["baseline"]
        command("protocol-status-after",["git","status","--porcelain"]);require((out/"commands/protocol-status-after.stdout").read_bytes()==b"")
        require(inputs=={name:meta(HERE/name) for name in inputs})
        context={"schema":"rstim.fixed-call-preparation.v1","protocol_revision":revision,"compiler":compiler,"roles":roles,"protocol_files":inputs,"compiler_version":COMPILER,"original_preparation_directory":str(out),"protocol_archive":meta(protocol_archive),"protocol_source_files":protocol_source_files}
        write(out/"preparation.json",context)
    except BaseException as exc:failure=repr(exc);write(out/"failure.json",{"error":failure});raise
    finally:
        recorder.close(failure,context);seal(out)
if __name__=="__main__":main()
