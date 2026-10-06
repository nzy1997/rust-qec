"""One isolated competitor process. Imports, tuning and destruction are untimed."""
import argparse
import importlib.metadata
import json
from pathlib import Path
import time


def prepare(backend, text, batch_size):
    if backend.startswith('clifft'):
        import clifft
        if backend == 'clifft-scheduled':
            manager = clifft.default_hir_pass_manager()
            manager.add(clifft.ActiveWidthSchedulePass())
            program = clifft.compile(text, hir_passes=manager)
        else:
            program = clifft.compile(text)
        return (lambda shots, seed: clifft.sample(program, shots=shots, seed=seed,
            threads=1, batch_size=batch_size).measurements), int(program.peak_active_width), int(program.num_measurements)
    import symft
    circuit = symft.Circuit(text)
    sampler = circuit.compile_sampler(batch=True, batch_size=0 if batch_size == 'auto' else batch_size)
    return (lambda shots, seed: sampler.sample(shots=shots, seed=seed)), int(sampler.max_active_qubits), int(sampler.num_measurements)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('backend', choices=['clifft', 'clifft-scheduled', 'symft'])
    p.add_argument('circuit', type=Path)
    p.add_argument('shots', type=int)
    p.add_argument('--batch', default='auto')
    p.add_argument('--repetitions', type=int, default=7)
    p.add_argument('--warmups', type=int, default=3)
    p.add_argument('--mode', choices=['bench', 'dump', 'tune', 'inspect'], default='bench')
    p.add_argument('--dump-total', type=int)
    args = p.parse_args()
    if args.shots < 1 or args.repetitions < 1 or args.warmups < 0:
        p.error('positive shots/repetitions and nonnegative warmups required')
    batch = args.batch if args.batch == 'auto' else int(args.batch)
    if batch != 'auto' and batch < 1:
        p.error('batch must be auto or positive')
    text = args.circuit.read_text()
    # Load extension modules before measuring compilation.
    importlib.import_module('clifft' if args.backend.startswith('clifft') else 'symft')
    start = time.perf_counter_ns()
    sample, rank, width = prepare(args.backend, text, batch)
    compile_ns = time.perf_counter_ns()-start
    distribution = 'clifft' if args.backend.startswith('clifft') else 'symft'
    metadata = {'backend': args.backend, 'version': importlib.metadata.version(distribution),
        'compile_ns': compile_ns, 'peak_active_width': rank, 'width': width, 'shots': args.shots,
        'batch': batch, 'threads': 1}
    if distribution == 'symft':
        import symft
        metadata['simd_backend'] = symft.simd_backend()
    if args.mode == 'inspect':
        if distribution != 'clifft':
            p.error('inspect requires Clifft')
        import clifft
        metadata['physical_width'] = int(clifft.compile(text).num_qubits)
        print(json.dumps(metadata)); return
    if args.mode == 'dump':
        total = args.dump_total or args.shots
        if total < args.shots or total % args.shots:
            p.error('dump total must be a positive multiple of call shots')
        records = []
        for call in range(total//args.shots):
            output = sample(args.shots, 739+call)
            if output.shape != (args.shots, width):
                raise ValueError('unexpected measurement output shape')
            records.extend(output.astype('uint8').reshape(-1).tolist())
        metadata.update(measurements=records, shots=total, call_shots=args.shots)
        print(json.dumps(metadata)); return
    first_ns = []
    if args.mode == 'bench':
        for rep in range(args.repetitions):
            first, _, _ = prepare(args.backend, text, batch)
            start = time.perf_counter_ns()
            output = first(args.shots, 739+rep)
            first_ns.append(time.perf_counter_ns()-start)
            if output.shape != (args.shots, width):
                raise ValueError('unexpected measurement output shape')
            del output, first
    for rep in range(args.warmups):
        output = sample(args.shots, 1739+rep)
        del output
    warm_ns, totals, calls = [], [], []
    for rep in range(args.repetitions):
        elapsed = count = 0
        while elapsed < 50_000_000:
            seed = 2739 + rep*1_000_000 + count
            start = time.perf_counter_ns()
            output = sample(args.shots, seed)
            elapsed += time.perf_counter_ns()-start
            if output.shape != (args.shots, width):
                raise ValueError('unexpected measurement output shape')
            del output
            count += 1
            if count >= 1_000_000:
                raise ValueError('timing interval not reached in one million calls')
        warm_ns.append(elapsed/count); totals.append(elapsed); calls.append(count)
    metadata.update(first_ns=first_ns, warm_ns=warm_ns, warm_totals_ns=totals, warm_calls=calls)
    print(json.dumps(metadata))


if __name__ == '__main__':
    main()
