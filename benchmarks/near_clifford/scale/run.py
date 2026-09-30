"""Paired scale campaign; pristine timing builds and separate diagnostic builds."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HARNESS = Path(__file__).resolve().parent
REVISIONS = {'baseline': '7cc2fa86525b75a9cef8858108be41336f3884ef',
             'candidate': 'f4b5966129aee3d617181414f805eb1250e1cc5d'}
# 23 scale + 12 mixed + 8 additional usage configurations.
MATRIX = [(f'random_{n}', 1000) for n in [32,64,65,80,128,129,193,256]]
MATRIX += [(f'rank_{n}', 1000 if n <= 11 else 16 if n <= 14 else 8) for n in [8,10,11,12,13,14,16]]
MATRIX += [(f'combo_{r}_{w}', 64 if r <= 11 else 16) for r in [10,11,12] for w in [64,129]]
MATRIX += [(f'records_{n}',1000) for n in [256,1024]]
MATRIX += [(f'qec_{n}',1000) for n in [1,3,8,32]]
MATRIX += [(f'interleaved_{n}',1000) for n in [2,8,32]]
MATRIX += [(f'feedback_{n}',1000) for n in [1,8,32]]
MATRIX += [('terminal_20q',1000), ('repeated_20q',1000)]
MATRIX += [('terminal_20q',n) for n in [0,1,2,16,63,64,100000,1000000]]
COUNTERS = ['peak_rank_after_instruction', 'symbolic_suffix_calls', 'measurement_cache_reuses',
            'measurement_cache_builds', 'coefficient_limit_fallbacks', 'depth_limit_fallbacks', 'node_limit_fallbacks', 'unplanned_shots']
SNAPSHOT = ['prepared_rank','prepared_coefficients','cached_nodes','max_cached_nodes','has_terminal_plan']


def output(cmd, **kwargs):
    return subprocess.check_output(cmd, text=True, **kwargs).strip()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'instrumentation anchor is not unique: {old[:70]}')
    return source.replace(old,new,1)


def instrument(source):
    """Untimed overlay only. Counter events have the same definition on both revisions."""
    source += '''
thread_local! { static BENCH_COUNTERS: std::cell::RefCell<[usize; 8]> = const { std::cell::RefCell::new([0; 8]) }; }
fn bench_event(index: usize) { BENCH_COUNTERS.with(|c| c.borrow_mut()[index] += 1); }
fn bench_rank(rank: usize) { BENCH_COUNTERS.with(|c| { let mut c = c.borrow_mut(); c[0] = c[0].max(rank); }); }
pub fn benchmark_counters() -> [usize; 8] { BENCH_COUNTERS.with(|c| *c.borrow()) }
pub fn benchmark_reset_counters() { BENCH_COUNTERS.with(|c| *c.borrow_mut() = [0; 8]); }
impl NearCliffordSampler<'_> {
    pub fn benchmark_snapshot(&self) -> [usize; 5] {
        [self.prepared.active_rank(), self.prepared.coefficients.len(),
         self.terminal.as_ref().map_or(0, |t| t.cached.cached_nodes),
         self.terminal.as_ref().map_or(0, |t| t.cached.max_cached_nodes), usize::from(self.terminal.is_some())]
    }
}
'''
    source = replace_once(source, 'plan.sample(&targets[depth..], &mut push, rng);', 'bench_event(1);\n                    plan.sample(&targets[depth..], &mut push, rng);')
    source = replace_once(source, 'if node.measurement.is_none() {', 'if node.measurement.is_some() { bench_event(2); } else { bench_event(3); }\n            if node.measurement.is_none() {')
    source = replace_once(source, 'push(outcome ^ inverted);\n                    depth += 1;', 'if state.coefficients.len() > BENCH_COEFFICIENT_LIMIT { bench_event(4); } else { bench_event(6); }\n                    push(outcome ^ inverted);\n                    depth += 1;')
    source = replace_once(source, 'let mut state = uncached_state.unwrap_or_else(|| node.state.clone());',
                          'if uncached_state.is_none() { if node.state.coefficients.len() > BENCH_COEFFICIENT_LIMIT { bench_event(4); }\n            else if depth >= self.cached_depth { bench_event(5); }\n            else { bench_event(6); } }\n            let mut state = uncached_state.unwrap_or_else(|| node.state.clone());')
    source = source.replace('BENCH_COEFFICIENT_LIMIT', 'MAX_CACHED_COEFFICIENTS' if 'const MAX_CACHED_COEFFICIENTS' in source else '1024')
    source = replace_once(source, 'let mut state = self.prepared.clone();', 'bench_event(7);\n        let mut state = self.prepared.clone();')
    source = source.replace('state.retire_fixed_axes();', 'bench_rank(state.active_rank()); state.retire_fixed_axes();')
    start=source.index('fn run_near_block('); end=source.index('\nfn near_record(',start)
    block=source[start:end]
    block=replace_once(block,'\n        }\n    }\n    Ok(())\n}', '\n        }\n        bench_rank(state.active_rank());\n    }\n    Ok(())\n}')
    source=source[:start]+block+source[end:]
    return source


def build(scratch, label, revision, diagnostic=False):
    suffix='-diagnostic' if diagnostic else ''
    directory=scratch/(label+suffix)
    directory.mkdir(parents=True,exist_ok=True)
    source=directory/'source'
    source.mkdir(exist_ok=True)
    # Restore tracked inputs on every build, including after an earlier diagnostic run.
    archive=subprocess.Popen(['git','archive',revision],cwd=ROOT,stdout=subprocess.PIPE)
    subprocess.run(['tar','-x','-C',str(source)],stdin=archive.stdout,check=True)
    archive.stdout.close()
    if archive.wait(): raise RuntimeError('git archive failed')
    original=source/'rstim/src/near_clifford.rs'
    if diagnostic:
        pristine=output(['git','show',f'{revision}:rstim/src/near_clifford.rs'],cwd=ROOT)+'\n'
        original.write_text(instrument(pristine))
    crate=directory/'harness'
    crate.mkdir(exist_ok=True)
    (crate/'main.rs').write_bytes((HARNESS/'main.rs').read_bytes())
    (crate/'fixtures').mkdir(exist_ok=True)
    for path in (HARNESS/'fixtures').iterdir(): (crate/'fixtures'/path.name).write_bytes(path.read_bytes())
    manifest='''[package]
name = "near-clifford-scale"
version = "0.1.0"
edition = "2024"
[workspace]
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(diagnostics)'] }
[[bin]]
name = "near-clifford-scale"
path = "main.rs"
[dependencies]
rand = "=0.8.7"
serde_json = "1"
libc = "0.2"
'''+f'rstim = {{ path = "{source}/rstim" }}\n'
    (crate/'Cargo.toml').write_text(manifest)
    (crate/'Cargo.lock').write_bytes((HARNESS/'Cargo.lock').read_bytes())
    env=os.environ.copy(); env.pop('RUSTFLAGS',None); env.pop('CARGO_ENCODED_RUSTFLAGS',None)
    env['CARGO_TARGET_DIR']=str(scratch/('target-diagnostic' if diagnostic else 'target'))
    env['RUSTFLAGS']='--cfg diagnostics' if diagnostic else ''
    subprocess.run(['cargo','build','--release','--locked'],cwd=crate,env=env,check=True)
    target=Path(env['CARGO_TARGET_DIR'])/'release/near-clifford-scale'
    binary=directory/'near-clifford-scale'; binary.write_bytes(target.read_bytes()); binary.chmod(0o755)
    return binary, {'revision':revision,'binary_sha256':sha(binary),'lock_sha256':sha(crate/'Cargo.lock'),
                    'near_clifford_source_sha256':sha(original)}


def atomic_save(path, result):
    temp=path.with_suffix('.tmp');temp.write_text(json.dumps(result,indent=2)+'\n');temp.replace(path)


def report(result):
    lines=['# Near-Clifford scale campaign','',f"Host: {result['platform']}; {result['rustc']}. {result['created_utc']}",
           f"Pristine #764 versus #766, {result['pairs']} paired process runs × {result['repetitions']} repetitions.",
           'Ratio = baseline / candidate; >1 means candidate faster. RSS is process peak, not cache allocation.',
           '', '| Fixture | Shots | Cold speedup | Warm flat speedup | Candidate warm flat ms | Candidate RSS MiB |',
           '| --- | ---: | ---: | ---: | ---: | ---: |']
    for case in result['cases']:
        runs=case['runs']
        def med(label,mode):
            return statistics.median([r[label]['measurements'][0][mode]['median_ns'] for r in runs])
        cold=med('baseline','cold_prepared_structured')/med('candidate','cold_prepared_structured')
        warm=med('baseline','warm_prepared_flat')/med('candidate','warm_prepared_flat')
        rss=statistics.median([r['candidate']['peak_rss_bytes'] for r in runs])/1048576
        lines.append(f"| {case['fixture']} | {case['shots']} | {cold:.2f}× | {warm:.2f}× | {med('candidate','warm_prepared_flat')/1e6:.4f} | {rss:.1f} |")
    return '\n'.join(lines)+'\n'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch',type=Path,default=ROOT/'drafts/near-clifford-scale')
    parser.add_argument('--output',type=Path)
    parser.add_argument('--pairs',type=int,default=3)
    parser.add_argument('--repetitions',type=int,default=3)
    parser.add_argument('--quick',action='store_true')
    parser.add_argument('--only',nargs='*')
    parser.add_argument('--skip-diagnostics',action='store_true')
    args=parser.parse_args();args.scratch=args.scratch.resolve()
    if args.pairs < 1 or args.repetitions < 1: parser.error('counts must be positive')
    result={'schema':2,'created_utc':datetime.now(timezone.utc).isoformat(),'platform':platform.platform(),
            'rustc':output(['rustc','--version']),'pairs':args.pairs,'repetitions':args.repetitions,
            'quick':args.quick,'matrix':MATRIX,'sources':{},'harness_sha256':sha(HARNESS/'main.rs'),
            'runner_sha256':sha(Path(__file__)), 'fixtures_sha256':{p.name:sha(p) for p in (HARNESS/'fixtures').iterdir()},'counter_names':COUNTERS,'snapshot_names':SNAPSHOT,
            'cases':[],'verification':{},'diagnostics':{}}
    args.scratch.mkdir(parents=True,exist_ok=True)
    destination=args.output or args.scratch/'results.json'; destination.parent.mkdir(parents=True,exist_ok=True)
    binaries={}
    for label,revision in REVISIONS.items():
        binaries[label],result['sources'][label]=build(args.scratch,label,revision)
    if len({v['lock_sha256'] for v in result['sources'].values()}) != 1: raise RuntimeError('dependency lock mismatch')
    names=list(dict.fromkeys(n for n,_ in MATRIX))
    if args.only: names=[n for n in names if n in args.only]
    if args.quick: names=['random_129','rank_12','combo_11_129','feedback_8','qec_8','terminal_20q'] if not args.only else names
    for name in names:
        print(f'Verify {name}',flush=True)
        values={label:json.loads(output([str(binary),name,'verify'],timeout=180)) for label,binary in binaries.items()}
        if values['baseline']!=values['candidate']: raise RuntimeError(f'cross-revision output/RNG mismatch: {name}')
        result['verification'][name]={'status':'pass','output_sha256':hashlib.sha256(json.dumps(values['candidate'],sort_keys=True).encode()).hexdigest()}
        atomic_save(destination,result)
    for index,(name,shots) in enumerate(MATRIX):
        if name not in names: continue
        if args.quick: shots=min(shots,16)
        case={'fixture':name,'shots':shots,'runs':[]}
        for pair in range(args.pairs):
            run={};order=list(binaries) if (index+pair)%2==0 else list(reversed(binaries))
            for label in order:
                print(f'Measure {index+1}/{len(MATRIX)} {name} shots={shots} pair={pair+1} {label}',flush=True)
                run[label]=json.loads(output([str(binaries[label]),name,str(shots),str(args.repetitions)],timeout=300))
            run['order']=order;case['runs'].append(run)
        result['cases'].append(case);atomic_save(destination,result)
        destination.with_suffix('.md').write_text(report(result))
    if not args.skip_diagnostics:
        for label,revision in REVISIONS.items():
            binary,metadata=build(args.scratch,label,revision,diagnostic=True)
            result['sources'][label]['diagnostic']=metadata;result['diagnostics'][label]={}
            for name in names:
                print(f'Diagnose {label} {name}',flush=True)
                verified=json.loads(output([str(binary),name,'verify'],timeout=180))
                verified_sha=hashlib.sha256(json.dumps(verified,sort_keys=True).encode()).hexdigest()
                if verified_sha != result['verification'][name]['output_sha256']: raise RuntimeError(f'diagnostic overlay changed semantics: {label} {name}')
                value=json.loads(output([str(binary),name,'diagnose'],timeout=300))
                value['semantic_verification']='pass'
                result['diagnostics'][label][name]=value;atomic_save(destination,result)
    result['completed_utc']=datetime.now(timezone.utc).isoformat();atomic_save(destination,result)
    print(destination,flush=True)

if __name__=='__main__': main()
