"""Reproducible, bounded Mid-SWAP sampling and public-input decoder experiments."""
import argparse
from collections import OrderedDict
from datetime import datetime, timezone
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import numpy as np
import pymatching
from . import correctness, reference

ROOT = Path(__file__).resolve().parents[2]


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args, timeout=300):
    start = time.perf_counter()
    try:
        result = subprocess.run(list(map(str, args)), capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        result = subprocess.CompletedProcess(args, 124, '', f'Process exceeded {timeout} second limit; no complete accuracy result')
    return result, time.perf_counter() - start


def checked(args):
    result, seconds = command(args)
    if result.returncode:
        raise RuntimeError(result.stderr + result.stdout)
    return seconds


def generate(binary, path, distance, rounds, loss):
    checked([binary, 'circuit', 'gen', '--code', 'surface_code', '--task', 'rotated_memory_z_midswap',
             '--distance', distance, '--rounds', rounds, '--noise', .001,
             '--operation-loss-probability', loss, '--measurement-loss-probability', loss, '--out', path])


def logical_x(text, distance):
    # Mid-SWAP initial-layout X string is the left column of data coordinates.
    coords = [op for op in reference.parse(text) if op.name == 'QUBIT_COORDS']
    support = [int(op.targets[0]) for op in coords if op.args[0] == 1 and int(op.args[1]) % 2 == 1]
    if len(support) != distance:
        raise ValueError('Unexpected initial data layout')
    return ','.join(map(str, support))


def wilson(errors, shots):
    z = 1.959963984540054
    p = errors / shots
    center = (p + z*z/(2*shots))/(1+z*z/shots)
    delta = z*np.sqrt(p*(1-p)/shots+z*z/(4*shots*shots))/(1+z*z/shots)
    return [max(0., float(center-delta)), min(1., float(center+delta))]


def score(predictions, answers):
    if predictions.shape != answers.shape or np.any(predictions > 1) or np.any(answers > 1):
        raise ValueError('Incomplete or invalid prediction rows')
    errors = int(np.count_nonzero(predictions != answers))
    return {'errors': errors, 'shots': len(answers), 'logical_error_rate': errors/len(answers),
            'wilson_95': wilson(errors, len(answers)),
            'prediction_sha256': hashlib.sha256(predictions.tobytes()).hexdigest()}


def build_matching(graph, losses):
    active = {i for loss in losses for i in graph['loss_edges'][loss]}
    scale = max(1., *(edge['weight'] for edge in graph['edges']))
    matching = pymatching.Matching()
    for i, edge in enumerate(graph['edges']):
        weight = (edge['loss_factor']*graph['mean_weight'] if i in active else edge['weight']) / scale
        kwargs = {'weight': weight, 'fault_ids': set(edge['observables']), 'merge_strategy': 'smallest-weight'}
        if edge['v'] is None:
            matching.add_boundary_edge(edge['u'], **kwargs)
        else:
            matching.add_edge(edge['u'], edge['v'], **kwargs)
    return matching


def python_decode_loop(graph, conditioned):
    # Identical shot order and at most 1024 FIFO cached patterns. RustQEC also
    # enforces a work budget; its actual builds/hits are retained for comparison.
    started = time.perf_counter()
    syndromes = np.asarray(graph['syndromes'], dtype=np.uint8)
    predictions = np.zeros(len(syndromes), dtype=np.uint8)
    cache, builds, hits = OrderedDict(), 0, 0
    for row, (syndrome, loss) in enumerate(zip(syndromes, graph['losses'], strict=True)):
        key = tuple(loss) if conditioned else ()
        if key not in cache:
            if len(cache) == 1024:
                cache.popitem(last=False)
            cache[key] = build_matching(graph, key)
            builds += 1
        else:
            hits += 1
        matching = cache[key]
        if np.any(syndrome[matching.num_detectors:]):
            raise ValueError('Unreachable fired detector')
        value = matching.decode(syndrome[:matching.num_detectors])
        predictions[row] = value[0] if len(value) else 0
    return predictions, {'decode_seconds': time.perf_counter()-started, 'graph_builds': builds, 'cache_hits': hits}


def python_decode(graph, conditioned, batch=True):
    if not batch:
        return python_decode_loop(graph, conditioned)
    started = time.perf_counter()
    syndromes = np.asarray(graph['syndromes'], dtype=np.uint8)
    if len(syndromes) != len(graph['losses']):
        raise ValueError('Incomplete loss rows')
    predictions = np.zeros(len(syndromes), dtype=np.uint8)
    groups = {}
    for row, losses in enumerate(graph['losses']):
        groups.setdefault(tuple(losses) if conditioned else (), []).append(row)
    for losses, indices in groups.items():
        matching = build_matching(graph, losses)
        rows = syndromes[indices]
        if np.any(rows[:, matching.num_detectors:]):
            raise ValueError('Unreachable fired detector')
        values = matching.decode_batch(rows[:, :matching.num_detectors])
        if values.shape[1]:
            predictions[indices] = values[:, 0]
    return predictions, {'decode_seconds': time.perf_counter()-started,
                         'graph_builds': len(groups), 'batch_calls': len(groups),
                         'execution': 'batch grouped by loss pattern' if conditioned else 'batch fixed graph'}


def export_graph(exporter, work, repetition):
    path = work/f'graph-{repetition}.json'
    checked([exporter, work/'public', path])
    return json.loads(path.read_text())


def native_decode(binary, work, decoder, repetition):
    predictions = work / f'{decoder}-{repetition}.b8'
    stats = work / f'{decoder}-{repetition}.json'
    args = [binary, 'decode', '--decoder', decoder, '--dataset', work/'public',
            '--out', predictions, '--stats-out', stats]
    if decoder == 'envelope-mle':
        args += ['--shot-timeout-ms', 500]
    result, wall = command(args, timeout=300)
    record = {'status': 'ok' if result.returncode == 0 else 'failed', 'process_wall_seconds': wall,
              'exit_code': result.returncode}
    if stats.exists():
        record['stats'] = json.loads(stats.read_text())
    if result.returncode:
        record['error'] = (result.stderr + result.stdout)[-4000:]
        return None, record
    return np.frombuffer(predictions.read_bytes(), dtype=np.uint8), record


def decoder_case(binary, exporter, work, distance, rounds, loss, shots, seed, repeats, include_mle=False, reuse=False):
    work.mkdir(parents=True, exist_ok=True)
    circuit = work/'circuit.stim'
    if not reuse:
        generate(binary, circuit, distance, rounds, loss)
        support = logical_x(circuit.read_text(), distance)
        checked([binary, 'dataset', 'export', '--circuit', circuit, '--shots', shots, '--seed', seed,
                 '--mode', 'measurements_blinded', '--logical-x-qubits', support,
                 '--public-out', work/'public', '--private-out', work/'private'])
    support = logical_x(circuit.read_text(), distance)
    public = json.loads((work/'public/manifest.json').read_text())
    private = json.loads((work/'private/manifest.json').read_text())
    assert public['dataset_id'] == private['dataset_id'] and public['circuit']['observables'] == 1
    answers = np.frombuffer((work/'private/answers.b8').read_bytes(), dtype=np.uint8)
    assert len(answers) == shots
    case = {'distance': distance, 'rounds': rounds, 'loss_probability': loss, 'pauli_probability': .001,
            'shots': shots, 'seed': seed, 'logical_x_support': support, 'dataset_id': public['dataset_id'],
            'circuit_sha256': digest(circuit), 'public_rows_sha256': digest(work/'public/shots.b8'),
            'answers_sha256': digest(work/'private/answers.b8'), 'decoders': {}}
    result, wall = command([exporter, work/'public', work/'graph.json'])
    if result.returncode:
        case['export_failure'] = result.stderr[-4000:]
        return case
    graph = json.loads((work/'graph.json').read_text())
    case['graph'] = {k: graph[k] for k in ['source','compile_seconds','transform_seconds','num_observables']}
    case['graph'].update(edges=len(graph['edges']), detectors=public['circuit']['detectors'],
                         loss_patterns=len(set(map(tuple, graph['losses']))))
    native_predictions = None
    for decoder in ['envelope-matching'] + (['envelope-mle'] if include_mle else []):
        timings, final = [], None
        for rep in range(repeats):
            predicted, record = native_decode(binary, work, decoder, rep)
            timings.append(record)
            if predicted is None:
                break
            if final is not None and not np.array_equal(predicted, final):
                raise ValueError('Native predictions changed between timing repetitions')
            final = predicted
        entry = {'status': 'ok' if all(t['status'] == 'ok' for t in timings) else 'failed', 'runs': timings}
        if entry['status'] == 'ok':
            entry.update(score(final, answers))
            entry['total_seconds'] = [r['stats']['compile_seconds']+r['stats']['decode_seconds'] for r in timings]
            if decoder == 'envelope-matching':
                native_predictions = final
        case['decoders'][decoder] = entry
    for name, conditioned in [('pymatching-fixed', False), ('pymatching-envelope', True)]:
        case['decoders'][name] = measure_python(lambda rep: export_graph(exporter, work, f'{name}-{rep}'),
                                                   conditioned, answers, repeats, native_predictions)
    if include_mle:
        case['decoders']['pymatching-fixed-loop'] = measure_python(
            lambda rep: export_graph(exporter, work, f'loop-{rep}'), False, answers, repeats, native_predictions, batch=False)
        batch_result, loop_result = [case['decoders'][key] for key in ['pymatching-fixed','pymatching-fixed-loop']]
        if batch_result['status'] == loop_result['status'] == 'ok':
            assert batch_result['prediction_sha256'] == loop_result['prediction_sha256']
    return case


def measure_python(graph_factory, conditioned, answers, repeats, native_predictions=None, batch=True):
    timings, final = [], None
    try:
        for rep in range(repeats):
            graph = graph_factory(rep)
            predicted, timing = python_decode(graph, conditioned, batch=batch)
            timing.update(compile_seconds=graph['compile_seconds'], transform_seconds=graph['transform_seconds'],
                          export_repetition=rep)
            if final is not None and not np.array_equal(predicted, final):
                raise ValueError('PyMatching predictions changed between repetitions')
            final = predicted
            timings.append(timing)
        entry = {'status':'ok', 'runs':timings, **score(final, answers)}
        entry['total_seconds'] = [r['compile_seconds']+r['transform_seconds']+r['decode_seconds'] for r in timings]
        if native_predictions is not None:
            entry['disagreements_with_native'] = int(np.count_nonzero(final != native_predictions))
            native_wrong, py_wrong = native_predictions != answers, final != answers
            entry['paired_native_only_wrong'] = int(np.count_nonzero(native_wrong & ~py_wrong))
            entry['paired_python_only_wrong'] = int(np.count_nonzero(~native_wrong & py_wrong))
        return entry
    except Exception as error:
        # Preserve completed timing runs but never score their successful prefix.
        return {'status':'failed', 'runs':timings, 'error':f'{type(error).__name__}: {error}'}


def sampling_case(binary, sampler, work, distance, shots, repeats):
    work.mkdir(parents=True, exist_ok=True)
    circuit = work/'circuit.stim'
    generate(binary, circuit, distance, distance, .003)
    checked([sampler, circuit, shots, repeats, work/'rust.json'])
    rust = json.loads((work/'rust.json').read_text())
    text = circuit.read_text()
    timings = []
    for rep in range(repeats + 1):
        start = time.perf_counter()
        rows = reference.sample(text, shots, 1700+rep)
        sample_seconds = time.perf_counter()-start
        start = time.perf_counter()
        payload = np.packbits(rows, axis=1, bitorder='little')
        packing_seconds = time.perf_counter()-start
        if rep:
            timings.append({'sample_seconds':sample_seconds, 'packing_seconds':packing_seconds,'bytes':payload.nbytes})
    return {'distance':distance, 'rounds':distance, 'shots':shots, 'pauli_probability':.001, 'loss_probability':.003,
            'circuit_sha256':digest(circuit), 'rust':rust,
            'reference':{'backend':'Python per-history lowering + Stim (correctness reference)', 'warmups':1,'records':timings}}


def cpu_model():
    if sys.platform == 'darwin':
        return subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip()
    if Path('/proc/cpuinfo').exists():
        for line in Path('/proc/cpuinfo').read_text().splitlines():
            if line.startswith('model name'):
                return line.split(':',1)[1].strip()
    return platform.processor() or 'unavailable'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--work', type=Path, default=Path('drafts/atom-loss-benchmark'))
    parser.add_argument('--out', type=Path, default=Path('site/static/data/atom-loss'))
    parser.add_argument('--shots', type=int, default=5000)
    parser.add_argument('--sampling-shots', type=int, default=256)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--stage', choices=['correctness','sampling','decoding','tradeoff','all'], default='all')
    args = parser.parse_args()
    if min(args.shots, args.sampling_shots, args.repeats) < 1:
        parser.error('shots, sampling-shots and repeats must be positive')
    binary, exporter, sampler = [ROOT/'target/release'/p for p in ['rustqec','examples/export_matching_benchmark','examples/atom_loss_sampling_benchmark']]
    out, work = args.out.resolve(), args.work.resolve()
    out.mkdir(parents=True, exist_ok=True)
    provenance = {'started_utc':datetime.now(timezone.utc).isoformat(), 'command':sys.argv,
                  'source_commit':subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
                  'working_tree_dirty': bool(subprocess.check_output(['git','status','--porcelain','--untracked-files=no'], text=True).strip()),
                  'os':platform.platform(),'cpu':cpu_model(),
                  'python':sys.version,'dependencies':{p:importlib.metadata.version(p) for p in ['stim','numpy','pymatching','matplotlib']},
                  'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),
                  'binaries':{str(p.relative_to(ROOT)):digest(p) for p in [binary,exporter,sampler]},
                  'environment':{k:os.environ.get(k) for k in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','RAYON_NUM_THREADS']},
                  'timing':'Serial processes; no explicit CPU pinning on macOS. Decode includes shared circuit compilation, public row transformation, graph construction and matching. Each repetition remeasures compilation and transformation. Excludes process startup and scoring; native decode includes buffered reads and output packing/flush, exporter transformation includes public row reads. PyMatching JSON loading is excluded. Cold caches each repetition; no steady-state claim.',
                  'sources':{str(p.relative_to(ROOT)):digest(p) for p in [*sorted((ROOT/'benchmarks/atom_loss').glob('*.py')), ROOT/'Cargo.lock', ROOT/'rustqec-cli/src/decode/benchmark.rs', ROOT/'rstim/examples/atom_loss_sampling_benchmark.rs']}}
    save(out/f'provenance-{args.stage}.json', provenance)
    snapshot_files = list(provenance['sources']) + ['rustqec-cli/Cargo.toml', 'rustqec-cli/src/lib.rs',
                     'rustqec-cli/src/decode.rs', 'rustqec-cli/examples/export_matching_benchmark.rs']
    save(out/'source-snapshot.json', {'base_commit':provenance['source_commit'],
         'description':'Exact benchmark sources at run time; overlay on base_commit.',
         'files':{name:(ROOT/name).read_text() for name in snapshot_files}})
    if args.stage in ['all','correctness']:
        result = correctness.run(binary)
        save(out/'correctness.json', result)
        assert result['status'] == 'PASS'
        from .decoder_reference import run as decoder_reference_run
        decoder_result = decoder_reference_run(binary, exporter)
        save(out/'decoder-correctness.json', decoder_result)
        assert decoder_result['status'] == 'PASS'
        print('sampling and decoding correctness PASS', flush=True)
    if args.stage in ['all','sampling']:
        results = []
        for distance in [3,5,7]:
            results.append(sampling_case(binary,sampler,work/f'sampling-d{distance}',distance,args.sampling_shots,args.repeats))
            save(out/'sampling.json', results)
            print(f'sampling d={distance} complete',flush=True)
    if args.stage in ['all','decoding']:
        results = []
        for distance in [3,5,7]:
            for loss in [.0001,.0003,.001,.003,.01]:
                case = decoder_case(binary,exporter,work/f'd{distance}-p{loss}',distance,distance,loss,args.shots,20260911,args.repeats)
                results.append(case)
                save(out/'decoding.json',results)
                print(f'decoding d={distance} p={loss}: '+', '.join(f'{k} {v.get("errors",v["status"])}' for k,v in case['decoders'].items()),flush=True)
    if args.stage in ['all','tradeoff']:
        result = decoder_case(binary,exporter,work/'tradeoff',3,2,.003,args.shots,20260912,args.repeats,include_mle=True)
        save(out/'tradeoff.json',result)
        print('tradeoff complete',flush=True)


if __name__ == '__main__':
    main()
