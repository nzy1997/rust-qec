"""One isolated competitor process. Imports, tuning and destruction are untimed."""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import sys
import time


def loaded_files(backend):
    """Small actual import identity, rather than distribution version alone."""
    name = 'clifft' if backend.startswith('clifft') else 'symft'
    modules = [name, name + ('._clifft_core' if name == 'clifft' else '._native')]
    result = {}
    for module_name in modules:
        module = importlib.import_module(module_name)
        path = Path(module.__file__).resolve(strict=True)
        result[module_name] = {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    return result


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
    sampler = circuit.compile_sampler(batch=batch_size != 'scalar',
        batch_size=0 if batch_size in ('scalar', 'auto') else batch_size)
    return (lambda shots, seed: sampler.sample(shots=shots, seed=seed)), int(sampler.max_active_qubits), int(sampler.num_measurements)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('backend', choices=['clifft', 'clifft-scheduled', 'symft'])
    p.add_argument('circuit', type=Path)
    p.add_argument('shots', type=int)
    p.add_argument('--batch', help='SymFT default is scalar; Clifft default is auto')
    p.add_argument('--repetitions', type=int, default=7)
    p.add_argument('--warmups', type=int, default=3)
    p.add_argument('--mode', choices=['bench', 'dump', 'tune', 'inspect', 'identity'], default='bench')
    p.add_argument('--dump-total', type=int)
    args = p.parse_args()
    if not sys.flags.isolated:
        p.error('worker requires Python -I isolation')
    if args.shots < 1 or args.repetitions < 1 or args.warmups < 0:
        p.error('positive shots/repetitions and nonnegative warmups required')
    requested = args.batch or ('scalar' if args.backend == 'symft' else 'auto')
    batch = requested if requested in ('auto', 'scalar') else int(requested)
    if batch == 'scalar' and args.backend != 'symft':
        p.error('scalar is a SymFT-only candidate')
    if batch not in ('auto', 'scalar') and batch < 1:
        p.error('batch must be scalar/auto or positive')
    identity = loaded_files(args.backend)
    if args.mode == 'identity':
        print(json.dumps({'loaded_files': identity, 'isolated': True})); return
    # Hash the same bytes decoded for every prepare call. read_text would apply
    # newline conversion, weakening the exact consumed-input contract.
    input_bytes = args.circuit.read_bytes()
    text = input_bytes.decode('utf-8')
    input_sha256 = hashlib.sha256(input_bytes).hexdigest()
    # Load extension modules before measuring compilation.
    importlib.import_module('clifft' if args.backend.startswith('clifft') else 'symft')
    start = time.perf_counter_ns()
    sample, rank, width = prepare(args.backend, text, batch)
    compile_ns = time.perf_counter_ns()-start
    distribution = 'clifft' if args.backend.startswith('clifft') else 'symft'
    metadata = {'backend': args.backend, 'version': importlib.metadata.version(distribution),
        'input_sha256': input_sha256,
        'loaded_files': identity, 'isolated': True,
        'compile_ns': compile_ns, 'peak_active_width': rank, 'width': width, 'shots': args.shots,
        'batch': batch, 'threads': 1}
    if distribution == 'symft':
        import symft
        metadata['simd_backend'] = symft.simd_backend()
        metadata['batch_enabled'] = batch != 'scalar'
    def emit():
        if loaded_files(args.backend) != identity:
            raise ValueError('actual imported peer files changed during worker execution')
        print(json.dumps(metadata))
    if args.mode == 'inspect':
        if distribution != 'clifft':
            p.error('inspect requires Clifft')
        import clifft
        metadata['physical_width'] = int(clifft.compile(text).num_qubits)
        emit(); return
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
        emit(); return
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
    emit()


if __name__ == '__main__':
    main()
