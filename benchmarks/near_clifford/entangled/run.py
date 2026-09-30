"""Paired entangled-workload campaign; adapts the immutable scale timing driver."""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
SCALE = HERE.parent / 'scale'
ROOT = HERE.parents[2]
MATRIX = []
for family in ['brick', 'parity']:
    for rank in [8, 11, 12, 14, 16]:
        MATRIX.append((f'{family}_{rank}_{rank + (family == "parity")}_3', 64 if rank <= 11 else 16 if rank <= 14 else 8))
    for rank, width in [(8,65),(8,129),(12,65),(12,129),(12,193)]:
        MATRIX.append((f'{family}_{rank}_{width}_3',64 if rank == 8 else 16))
MATRIX += [(f'rounds_{rank}_{rank+1}_{rounds}',64) for rank,rounds in [(4,2),(4,8),(8,2),(8,8),(12,2)]]
MATRIX += [(f'rounds_8_{width}_4',64) for width in [65,129,193]]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path, name):
    spec = importlib.util.spec_from_file_location(name,path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def adapt_oracle(raw):
    """Raise only the small-test allocation guard in this standalone harness."""
    old=b'assert!(num_qubits <= 10, "dense oracle is limited to 10 qubits");'
    new=b'assert!(num_qubits <= 16, "benchmark dense oracle is limited to 16 qubits");'
    if raw.count(old)!=1:
        raise ValueError('nonunique dense oracle allocation guard')
    return raw.replace(old,new,1)


def driver():
    original = (SCALE/'main.rs').read_text()
    start = original.index('fn circuit(')
    end = original.index('\nfn median(',start)
    result = original[:start] + '''mod fixtures;
mod oracle;
fn circuit(name: &str) -> Result<String, String> { fixtures::circuit(name) }
''' + original[end:]
    anchor = 'if args.get(1).map(String::as_str) == Some("verify") {'
    if result.count(anchor) != 1:
        raise ValueError('nonunique verification anchor')
    result = result.replace(anchor, anchor+'\n        let physics = fixtures::validate(name)?;',1)
    anchor = '"continuation":rng.next_u64()'
    if result.count(anchor) != 1:
        raise ValueError('nonunique output anchor')
    return result.replace(anchor,anchor+',"physics":physics,"circuit":text',1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline',required=True)
    parser.add_argument('--candidate',required=True)
    parser.add_argument('--scratch',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--quick',action='store_true')
    parser.add_argument('--only',nargs='+')
    args = parser.parse_args()
    args.scratch = args.scratch.resolve()
    if args.scratch.exists():
        parser.error('scratch must be a fresh nonexistent directory')
    if args.only and set(args.only) - {n for n,_ in MATRIX}:
        parser.error('unknown --only fixture')
    revisions = {label:subprocess.check_output(['git','rev-parse',ref+'^{commit}'],cwd=ROOT,text=True).strip()
                 for label,ref in [('baseline',args.baseline),('candidate',args.candidate)]}
    if len(set(revisions.values())) != 2:
        parser.error('source revisions must differ')
    args.scratch.mkdir(parents=True)
    generated = args.scratch/'driver.rs'
    generated.write_text(driver())
    campaign = load(SCALE/'run.py','entangled_scale_campaign')
    # Reuse all timing boundaries, process isolation, parity of paired order,
    # diagnostic overlay and output/RNG checks without changing historic files.
    class Inputs:
        def __truediv__(self,name):
            return generated if name=='main.rs' else SCALE/('Cargo.unified.lock' if name=='Cargo.lock' else name)
    campaign.HARNESS = Inputs()
    campaign.REVISIONS = revisions
    campaign.MATRIX = MATRIX
    report = load(HERE/'report.py','entangled_report').report
    campaign.report = report
    original_build = campaign.build
    def build(scratch,label,revision,diagnostic=False):
        directory=scratch/(label+('-diagnostic' if diagnostic else ''))/'harness'
        directory.mkdir(parents=True)
        (directory/'fixtures.rs').write_bytes((HERE/'fixtures.rs').read_bytes())
        oracle = subprocess.check_output(['git','show',revision+':rstim/tests/support/near_clifford_oracle.rs'],cwd=ROOT)
        (directory/'oracle.rs').write_bytes(adapt_oracle(oracle))
        binary,metadata=original_build(scratch,label,revision,diagnostic)
        metadata['oracle_source_sha256']=hashlib.sha256(oracle).hexdigest()
        metadata['oracle_sha256']=sha(directory/'oracle.rs')
        if not diagnostic:
            subprocess.run(['cargo','test','--release','--locked'],cwd=directory,
                env={**{k:v for k,v in os.environ.items() if k != 'CARGO_ENCODED_RUSTFLAGS'},'CARGO_TARGET_DIR':str(scratch/'target'),'RUSTFLAGS':''},check=True)
        return binary,metadata
    campaign.build = build
    save = campaign.atomic_save
    def save_with_inputs(path,result):
        result['schema']='near-clifford.entangled.v1'
        result['entangled_inputs']={name:sha(HERE/name) for name in ['run.py','report.py','fixtures.rs']}
        result['scale_inputs']={name:sha(SCALE/name) for name in ['run.py','main.rs','Cargo.unified.lock']}
        result['verification_results'] = verification_results
        save(path,result)
    # Preserve the actual verification payload, including the independent physics
    # witness; the inherited runner's hash alone would hide validation coverage.
    verification_results={}
    original_output=campaign.output
    def output(cmd,**kwargs):
        value=original_output(cmd,**kwargs)
        if len(cmd)==3 and cmd[-1]=='verify':
            label=Path(cmd[0]).parent.name
            verification_results.setdefault(label,{})[cmd[1]]=json.loads(value)
        return value
    campaign.output=output
    campaign.atomic_save=save_with_inputs
    names=args.only or [n for n,_ in MATRIX]
    sys.argv=[sys.argv[0],'--scratch',str(args.scratch),'--output',str(args.output),'--only',*names]
    if args.quick: sys.argv+=['--quick','--pairs','1','--repetitions','1']
    campaign.main()


if __name__=='__main__':
    main()
