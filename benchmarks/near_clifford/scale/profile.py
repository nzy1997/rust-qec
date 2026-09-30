"""Run macOS sample on a separate release build after campaign timing finishes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results',type=Path)
    parser.add_argument('--scratch',type=Path,default=ROOT/'drafts/near-clifford-scale')
    parser.add_argument('--fixtures',nargs='+',default=['combo_11_129','rank_12','qec_32'])
    parser.add_argument('--output',type=Path,default=ROOT/'drafts/near-clifford-scale/profiles')
    args=parser.parse_args()
    if platform.system()!='Darwin': parser.error('this driver requires macOS sample')
    result=json.loads(args.results.read_text())
    if 'completed_utc' not in result: parser.error('finish timing before profiling')
    source=args.scratch.resolve()/'candidate/source'
    if sha(source/'rstim/src/near_clifford.rs')!=result['sources']['candidate']['near_clifford_source_sha256']:
        parser.error('candidate source differs from campaign')
    crate=args.scratch.resolve()/'profile-harness';crate.mkdir(exist_ok=True)
    (crate/'main.rs').write_bytes((HERE/'profile.rs').read_bytes())
    (crate/'Cargo.lock').write_bytes((HERE/'Cargo.lock').read_bytes())
    (crate/'Cargo.toml').write_text('''[package]
name = "near-clifford-scale"
version = "0.1.0"
edition = "2024"
[workspace]
[[bin]]
name = "near-clifford-scale"
path = "main.rs"
[profile.release]
debug = 1
[dependencies]
rand = "=0.8.7"
serde_json = "1"
libc = "0.2"
'''+f'rstim = {{ path = "{source}/rstim" }}\n')
    env=os.environ.copy();env.pop('CARGO_ENCODED_RUSTFLAGS',None)
    env['CARGO_TARGET_DIR']=str(args.scratch.resolve()/'target-profile')
    env['RUSTFLAGS']='-Cforce-frame-pointers=yes'
    subprocess.run(['cargo','build','--release','--locked'],cwd=crate,env=env,check=True)
    binary=Path(env['CARGO_TARGET_DIR'])/'release/near-clifford-scale'
    args.output.mkdir(parents=True,exist_ok=True)
    metadata={'revision':result['sources']['candidate']['revision'],'binary_sha256':sha(binary),
              'driver_sha256':sha(HERE/'profile.rs'),'lock_sha256':sha(crate/'Cargo.lock'),
              'build':'cargo build --release --locked; release.debug=1; RUSTFLAGS=-Cforce-frame-pointers=yes',
              'sample_command':'sample PID 8 1 -file OUTPUT','cases':[]}
    for name in args.fixtures:
        case=next(c for c in result['cases'] if c['fixture']==name)
        circuit=args.output/(name+'.stim');circuit.write_text(case['runs'][0]['candidate']['circuit'])
        proc=subprocess.Popen([str(binary),str(circuit.resolve()),'12'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        time.sleep(1)
        profile=args.output/(name+'.sample.txt')
        subprocess.run(['/usr/bin/sample',str(proc.pid),'8','1','-file',str(profile.resolve())],check=True)
        stdout,stderr=proc.communicate(timeout=60)
        if proc.returncode: raise RuntimeError(stderr)
        metadata['cases'].append({'fixture':name,'circuit_sha256':sha(circuit),'profile_sha256':sha(profile),'output':stdout.strip()})
        (args.output/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
        print(name,stdout.strip(),flush=True)

if __name__=='__main__': main()
