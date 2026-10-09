"""Isolated postselected counts adapter, including SymFT compiled CPU sampling.

Derived from the reviewed October MSC experiment. Declined compiled requests never
fall through to timings from the legacy executor. No cross-backend RNG identity
is promised: compiled SymFT uses its own CPU RNG policy.
"""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import sys
import time

ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'benchmarks/near_clifford/compiled_sota'))
from worker import loaded_files


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('backend',choices=['clifft','clifft-scheduled','symft'])
    p.add_argument('circuit',type=Path)
    p.add_argument('shots',type=int)
    p.add_argument('--batch',default='auto')
    p.add_argument('--repetitions',type=int,default=7)
    p.add_argument('--validate',action='store_true')
    p.add_argument('--cpu-backend', choices=['legacy','compiled'], default='legacy')
    args=p.parse_args()
    if not sys.flags.isolated or not 1<=args.shots<=131072 or not 1<=args.repetitions<=64:
        p.error('Python -I and bounded positive shots/repetitions required')
    data=args.circuit.read_bytes()
    text=data.decode()
    batch=args.batch if args.batch in ['auto','scalar'] else int(args.batch)
    identity=loaded_files(args.backend)
    sampler_info=None
    start=time.perf_counter_ns()
    if args.backend.startswith('clifft'):
        import clifft
        manager=clifft.default_hir_pass_manager()
        if args.backend=='clifft-scheduled': manager.add(clifft.ActiveWidthSchedulePass())
        # Detector count discovery is part of this adapter's compile cost.
        initial=clifft.compile(text,hir_passes=manager)
        program=clifft.compile(text,postselection_mask=[1]*initial.num_detectors,
                              normalize_syndromes=False,hir_passes=manager)
        rank=int(program.peak_active_width)
        def sample(seed):
            r=clifft.sample_survivors(program,shots=args.shots,seed=seed,
                keep_records=False,threads=1,batch_size=batch)
            if not len(r.observable_ones): raise ValueError('observable 0 required')
            return dict(attempted=int(r.total_shots),accepted=int(r.passed_shots),
                        discarded=int(r.discards),logical_errors=int(r.observable_ones[0]))
        execution='native postselected counts; early rejection enabled'
    else:
        import symft
        circuit=symft.Circuit(text)
        sampler=circuit.compile_counts_sampler(cpu_backend=args.cpu_backend, batch=batch!='scalar',observable=0,
            postselect_detectors=True,reference_sample=False,
            batch_size=0 if batch in ['scalar','auto'] else batch,threads=1)
        sampler_info=sampler.info
        if args.cpu_backend=='compiled' and not sampler_info['cpu_compiled']:
            print(json.dumps(dict(backend=args.backend,status='declined',
                input_sha256=hashlib.sha256(data).hexdigest(),isolated=True,
                loaded_files=identity,batch=batch,shots=args.shots,
                requested_cpu_backend=args.cpu_backend,sampler_info=sampler_info)))
            return
        rank=int(sampler_info['max_active_qubits'])
        def sample(seed):
            r=sampler.sample(shots=args.shots,stream_id=seed)
            if r['active_threads']!=1: raise ValueError('peer used more than one thread')
            return dict(attempted=int(r['shots']),accepted=int(r['accepted']),
                        discarded=int(r['discarded']),logical_errors=int(r['logical_errors']))
        execution='native counts; actual SymFT CPU '+args.cpu_backend
    compile_ns=time.perf_counter_ns()-start
    start=time.perf_counter_ns()
    sample(739)
    first_ns=time.perf_counter_ns()-start
    observations=[]
    for rep in range(args.repetitions):
        elapsed=calls=0
        counts=dict(attempted=0,accepted=0,discarded=0,logical_errors=0)
        while calls==0 or (args.validate and counts['attempted']<8192) or (not args.validate and elapsed<50_000_000):
            start=time.perf_counter_ns()
            r=sample(1739+rep*1_000_000+calls)
            elapsed+=time.perf_counter_ns()-start
            if r['attempted']!=args.shots or r['accepted']+r['discarded']!=args.shots or not 0<=r['logical_errors']<=r['accepted']:
                raise ValueError('invalid counts semantics')
            for key in counts: counts[key]+=r[key]
            calls+=1
            if calls>=1_000_000: raise ValueError('timing call bound exceeded')
        observations.append(dict(elapsed_ns=elapsed,calls=calls,ns_per_call=elapsed/calls,**counts))
    if identity!=loaded_files(args.backend): raise ValueError('peer imports changed')
    rss=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss*(1024 if sys.platform.startswith('linux') else 1)
    print(json.dumps(dict(backend=args.backend,status='ok',input_sha256=hashlib.sha256(data).hexdigest(),
        loaded_files=identity,isolated=True,batch=batch,shots=args.shots,
        compile_ns=compile_ns,prepare_ns=0,first_ns=first_ns,observations=observations,
        peak_active_width=rank,peak_rss_bytes=rss,execution=execution,sampler_info=sampler_info,
        cpu_backend=args.cpu_backend if args.backend=='symft' else None,
        output_contract='all-zero raw detector postselection; raw observable 0 counts; no reference normalization')))


if __name__=='__main__': main()
