"""Matched direct-query sparse/dense support controls; separate diagnostic builds."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import subprocess

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
ROOT = BASE.parents[1]
MATRIX = [(topology, width) for topology in ['star', 'chain'] for width in [63, 64, 65, 127, 128, 129, 193]]
CALLS = 1024


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def instrument(source):
    scale = load(BASE/'scale/run.py', 'pauli_instrument')
    source = scale.instrument(source)
    source = scale.replace_once(source, 'fn multiply_tableau_row(&mut self, frame: &StabilizerState, row: usize) {',
        'fn multiply_tableau_row(&mut self, frame: &StabilizerState, row: usize) {\n        PAULI_WORK.with(|c| { let mut c=c.borrow_mut(); c[0]+=1; c[1]+=frame.num_qubits(); });')
    return source + '''
thread_local! { static PAULI_WORK: std::cell::RefCell<[usize; 2]> = const { std::cell::RefCell::new([0; 2]) }; }
pub fn benchmark_pauli_work() -> [usize; 2] { PAULI_WORK.with(|c| *c.borrow()) }
pub fn benchmark_pauli_reset() { PAULI_WORK.with(|c| *c.borrow_mut()=[0; 2]); }
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.scratch = args.scratch.resolve()
    if args.scratch.exists(): parser.error('scratch must be fresh')
    args.output = args.output.resolve()
    if not args.output.is_relative_to(args.scratch): parser.error('output must be inside fresh scratch')
    revisions = {label: subprocess.check_output(['git','rev-parse',ref+'^{commit}'],cwd=ROOT,text=True).strip()
                 for label,ref in [('baseline',args.baseline),('candidate',args.candidate)]}
    if len(set(revisions.values())) != 2: parser.error('source revisions must differ')
    campaign = load(BASE/'scale/run.py','pauli_build')
    class Inputs:
        def __truediv__(self,name):
            return HERE/'main.rs' if name=='main.rs' else BASE/'scale'/('Cargo.unified.lock' if name=='Cargo.lock' else name)
    campaign.HARNESS = Inputs()
    campaign.instrument = instrument
    args.scratch.mkdir(parents=True)
    result = {'schema':'near-clifford.pauli-products.v1','created_utc':datetime.now(timezone.utc).isoformat(),
              'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','-Vv'],text=True),
              'inputs':{name:sha(HERE/name) for name in ['main.rs','run.py']},
              'build_runner_sha256':sha(BASE/'scale/run.py'),'matrix':MATRIX,'pairs':3,'repetitions':3,'calls':CALLS,
              'sources':{},'verification':{},'cases':[],'diagnostics':{}}
    def save():
        args.output.parent.mkdir(parents=True,exist_ok=True)
        campaign.atomic_save(args.output,result)
    def build(label, diagnostic=False):
        binary, metadata = campaign.build(args.scratch,label,revisions[label],diagnostic)
        folder = args.scratch/(label+('-diagnostic' if diagnostic else ''))/'source'
        metadata['tableau_source_sha256']=sha(folder/'rstim/src/sim/tableau.rs')
        return binary,metadata
    def invoke(binary,topology,width,mode):
        command=[str(binary),topology,str(width),mode]
        if mode=='time': command += [str(CALLS),'3']
        return json.loads(campaign.output(command,timeout=180))
    binaries={}
    for label in revisions:
        binaries[label],result['sources'][label]=build(label)
        result['verification'][label]={}
    for topology,width in MATRIX:
        key=f'{topology}_{width}'
        values={label:invoke(binary,topology,width,'verify') for label,binary in binaries.items()}
        if values['baseline'] != values['candidate']: raise ValueError('physics/output/RNG mismatch: '+key)
        for label,value in values.items(): result['verification'][label][key]=value
    for index,(topology,width) in enumerate(MATRIX):
        case={'topology':topology,'width':width,'runs':[]}
        for pair in range(3):
            order=list(binaries) if (index+pair)%2==0 else list(reversed(binaries))
            run={'order':order}
            for label in order:
                print('Measure',topology,width,'pair',pair+1,label,flush=True)
                run[label]=invoke(binaries[label],topology,width,'time')
            case['runs'].append(run)
        result['cases'].append(case); save()
    for label in revisions:
        binary,metadata=build(label,True)
        result['sources'][label]['diagnostic']=metadata
        result['verification'][label+'-diagnostic']={}
        result['diagnostics'][label]={}
        for topology,width in MATRIX:
            key=f'{topology}_{width}'
            value=invoke(binary,topology,width,'verify')
            if value != result['verification'][label][key]: raise ValueError('diagnostics changed semantics: '+key)
            result['verification'][label+'-diagnostic'][key]=value
            result['diagnostics'][label][key]=invoke(binary,topology,width,'diagnose')
    result['completed_utc']=datetime.now(timezone.utc).isoformat(); save()
    print(args.output,flush=True)


if __name__=='__main__': main()
