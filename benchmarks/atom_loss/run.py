"""Reproducible, bounded Mid-SWAP sampling and public-input decoder experiments."""
from .shot_data import require
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
from scipy.sparse import csc_matrix
from .artifacts import native_total, wilson
from . import correctness, reference
from .shot_data import validate_dataset

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


def score(predictions, answers):
    if predictions.shape != answers.shape or np.any(predictions > 1) or np.any(answers > 1):
        raise ValueError('Incomplete or invalid prediction rows')
    errors = int(np.count_nonzero(predictions != answers))
    return {'errors': errors, 'shots': len(answers), 'logical_error_rate': errors/len(answers),
            'wilson_95': wilson(errors, len(answers)),
            'prediction_sha256': hashlib.sha256(predictions.tobytes()).hexdigest()}


def prepare_matching(graph):
    """Build the invariant sparse topology once per measured adapter invocation."""
    edges = graph['edges']
    detectors, columns, observables, fault_columns = [], [], [], []
    for column, edge in enumerate(edges):
        for detector in (edge['u'], edge['v']):
            if detector is not None:
                detectors.append(detector)
                columns.append(column)
        for observable in edge['observables']:
            observables.append(observable)
            fault_columns.append(column)
    # Match the edge API's detector extent, so out-of-graph fired bits still fail.
    count = max(detectors, default=-1) + 1
    fault_count = graph.get('num_observables', max(observables, default=-1) + 1)
    checks = csc_matrix((np.ones(len(detectors), dtype=np.uint8), (detectors, columns)),
                        shape=(count, len(edges)))
    faults = csc_matrix((np.ones(len(observables), dtype=np.uint8), (observables, fault_columns)),
                        shape=(fault_count, len(edges)))
    scale = max(1., max((edge['weight'] for edge in edges), default=0.))
    base = np.array([edge['weight']/scale for edge in edges])
    conditioned = np.array([edge['loss_factor']*graph['mean_weight']/scale for edge in edges])
    return checks, faults, base, conditioned


def build_matching(graph, losses, prepared=None):
    checks, faults, base, conditioned = prepare_matching(graph) if prepared is None else prepared
    active = list({i for loss in losses for i in graph['loss_edges'][loss]})
    weights = base.copy()
    weights[active] = conditioned[active]
    return pymatching.Matching.from_check_matrix(
        checks, weights=weights, faults_matrix=faults,
        merge_strategy='smallest-weight', use_virtual_boundary_node=True)


def python_decode_loop(graph, conditioned):
    # Identical shot order and at most 1024 FIFO cached patterns. RustQEC also
    # enforces a work budget; its actual builds/hits are retained for comparison.
    started = time.perf_counter()
    prepared = prepare_matching(graph)
    syndromes = np.asarray(graph['syndromes'], dtype=np.uint8)
    predictions = np.zeros(len(syndromes), dtype=np.uint8)
    cache, builds, hits = OrderedDict(), 0, 0
    for row, (syndrome, loss) in enumerate(zip(syndromes, graph['losses'], strict=True)):
        key = tuple(loss) if conditioned else ()
        if key not in cache:
            if len(cache) == 1024:
                cache.popitem(last=False)
            cache[key] = build_matching(graph, key, prepared)
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
    prepared = prepare_matching(graph)
    topology_seconds = time.perf_counter()-started
    syndromes = np.asarray(graph['syndromes'], dtype=np.uint8)
    if len(syndromes) != len(graph['losses']):
        raise ValueError('Incomplete loss rows')
    predictions = np.zeros(len(syndromes), dtype=np.uint8)
    groups = {}
    for row, losses in enumerate(graph['losses']):
        groups.setdefault(tuple(losses) if conditioned else (), []).append(row)
    preprocessing = time.perf_counter()-started-topology_seconds
    graph_seconds = matching_seconds = output_seconds = 0.
    for losses, indices in groups.items():
        stage = time.perf_counter()
        matching = build_matching(graph, losses, prepared)
        graph_seconds += time.perf_counter()-stage
        stage = time.perf_counter()
        rows = syndromes[indices]
        if np.any(rows[:, matching.num_detectors:]):
            raise ValueError('Unreachable fired detector')
        preprocessing += time.perf_counter()-stage
        stage = time.perf_counter()
        values = matching.decode_batch(rows[:, :matching.num_detectors])
        matching_seconds += time.perf_counter()-stage
        stage = time.perf_counter()
        if values.shape[1]:
            predictions[indices] = values[:, 0]
        output_seconds += time.perf_counter()-stage
    elapsed = time.perf_counter()-started
    return predictions, {'decode_seconds': elapsed,
                         'topology_seconds': topology_seconds, 'preprocess_seconds': preprocessing, 'graph_build_seconds': graph_seconds,
                         'matching_seconds': matching_seconds, 'output_seconds': output_seconds,
                         'adapter_overhead_seconds': elapsed-topology_seconds-preprocessing-graph_seconds-matching_seconds-output_seconds,
                         'graph_builds': len(groups), 'batch_calls': len(groups),
                         'graph_api': 'from_check_matrix',
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


def measure_offline(exporter, work, rep, answers):
    name='envelope-matching-offline'
    graph=export_graph(exporter,work,f'{name}-{rep}')
    predictions=work/f'{name}-{rep}.b8';stats=work/f'{name}-{rep}.json'
    result,wall=command([exporter.parent/'offline_matching_benchmark',
                        work/f'graph-{name}-{rep}.json',predictions,stats])
    if result.returncode:
        return {'status':'failed','runs':[{'exit_code':result.returncode,'error':result.stderr}]}
    batch=json.loads(stats.read_text())
    record={'status':'ok','exit_code':0,'process_wall_seconds':wall,'batch':batch,
        'transform_seconds':graph['transform_seconds'],
        'stats':{'compile_seconds':graph['compile_seconds'],
                 'decode_seconds':graph['transform_seconds']+batch['decode_seconds'],
                 'attempted_shot_count':batch['shots'],'timeout_count':0,'infeasible_shot_count':0,
                 'matching_graph_builds':batch['graph_builds'],'cache_hits':0}}
    predicted=np.frombuffer(predictions.read_bytes(),dtype=np.uint8)
    return {'status':'ok','runs':[record],'total_seconds':[native_total(record)],**score(predicted,answers)}


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
    require((public['dataset_id'] == private['dataset_id'] and public['circuit']['observables'] == 1), "run: public['dataset_id'] == private['dataset_id'] and public['circuit']['observables'] == 1")
    answers = np.frombuffer(validate_dataset(lambda name:(work/name).read_bytes()), dtype=np.uint8)
    require((len(answers) == shots), 'run: len(answers) == shots')
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
    names = ['envelope-matching', 'pymatching-fixed', 'pymatching-envelope', 'envelope-matching-offline']
    if include_mle:
        names += ['envelope-mle', 'pymatching-fixed-loop']
    entries = {name: [] for name in names}
    case['timing_order'] = []
    for rep in range(repeats):
        order = names[rep % len(names):] + names[:rep % len(names)]
        case['timing_order'].append(order)
        for name in order:
            if name == 'envelope-matching-offline':
                entry = measure_offline(exporter, work, rep, answers)
            elif name.startswith('pymatching'):
                entry = measure_python(
                    lambda _: export_graph(exporter, work, f'{name}-{rep}'),
                    name == 'pymatching-envelope', answers, 1,
                    batch=not name.endswith('-loop'), repetition_offset=rep,
                    prediction_path=work/f'{name}-{rep}.b8')
            else:
                predicted, record = native_decode(binary, work, name, rep)
                entry = {'status': record['status'], 'runs': [record]}
                if predicted is not None:
                    entry.update(score(predicted, answers))
                    entry['total_seconds'] = [native_total(record)]
            entries[name].append(entry)
    for name, repetitions in entries.items():
        runs = [run for entry in repetitions for run in entry['runs']]
        if any(entry['status'] != 'ok' for entry in repetitions):
            case['decoders'][name] = {'status': 'failed', 'runs': runs,
                                     'error': 'Incomplete repetition; no prefix accuracy'}
            continue
        if len({entry['prediction_sha256'] for entry in repetitions}) != 1:
            raise ValueError(f'{name} predictions changed between timing repetitions')
        case['decoders'][name] = {**repetitions[0], 'runs': runs,
            'total_seconds': [entry['total_seconds'][0] for entry in repetitions]}
    native = case['decoders']['envelope-matching']
    offline = case['decoders']['envelope-matching-offline']
    require(offline['status']=='ok' and native['status']=='ok' and offline['prediction_sha256']==native['prediction_sha256'], 'Offline/streaming predictions differ')
    if native['status'] == 'ok':
        native_predictions = np.frombuffer((work/'envelope-matching-0.b8').read_bytes(), dtype=np.uint8)
        for name, entry in case['decoders'].items():
            if not name.startswith('pymatching') or entry['status'] != 'ok':
                continue
            predicted = np.frombuffer((work/f'{name}-0.b8').read_bytes(), dtype=np.uint8)
            native_wrong, py_wrong = native_predictions != answers, predicted != answers
            entry.update(disagreements_with_native=int(np.count_nonzero(predicted != native_predictions)),
                         paired_native_only_wrong=int(np.count_nonzero(native_wrong & ~py_wrong)),
                         paired_python_only_wrong=int(np.count_nonzero(~native_wrong & py_wrong)))
    if include_mle:
        batch_result, loop_result = [case['decoders'][key] for key in ['pymatching-fixed','pymatching-fixed-loop']]
        if batch_result['status'] == loop_result['status'] == 'ok':
            require((batch_result['prediction_sha256'] == loop_result['prediction_sha256']), "run: batch_result['prediction_sha256'] == loop_result['prediction_sha256']")
    return case


def measure_python(graph_factory, conditioned, answers, repeats, native_predictions=None, batch=True, repetition_offset=0, prediction_path=None):
    timings, final = [], None
    try:
        for rep in range(repeats):
            graph = graph_factory(rep)
            predicted, timing = python_decode(graph, conditioned, batch=batch)
            timing.update(compile_seconds=graph['compile_seconds'], transform_seconds=graph['transform_seconds'],
                          export_repetition=rep+repetition_offset)
            if final is not None and not np.array_equal(predicted, final):
                raise ValueError('PyMatching predictions changed between repetitions')
            final = predicted
            if prediction_path is not None:
                write_started = time.perf_counter()
                with prediction_path.open('wb') as stream:
                    stream.write(predicted.tobytes())
                    stream.flush()
                timing['write_seconds'] = time.perf_counter()-write_started
                timing['decode_seconds'] += timing['write_seconds']
                if 'output_seconds' in timing:
                    timing['output_seconds'] += timing['write_seconds']
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
                  'python':sys.version,'dependencies':{p:importlib.metadata.version(p) for p in ['stim','numpy','pymatching','scipy','matplotlib']},
                  'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),
                  'binaries':{str(p.relative_to(ROOT)):digest(p) for p in [binary,exporter,sampler,exporter.parent/'export_decoder_oracle']},
                  'environment':{k:os.environ.get(k) for k in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','RAYON_NUM_THREADS']},
                  'timing':'Serial processes; no explicit CPU pinning on macOS. Decode includes shared circuit compilation, public row transformation, graph construction and matching. Each repetition remeasures compilation and transformation. Excludes process startup and scoring; native decode includes buffered reads and output packing/flush, exporter transformation includes public row reads. PyMatching JSON loading is excluded. Cold caches each repetition; no steady-state claim.',
                  'sources':{str(p.relative_to(ROOT)):digest(p) for p in [*sorted((ROOT/'benchmarks/atom_loss').glob('*.py')), *sorted((ROOT/'benchmarks/atom_loss/fixtures').glob('*.stim')), ROOT/'Cargo.lock', ROOT/'rustqec-cli/src/decode/benchmark.rs', ROOT/'rstim/examples/atom_loss_sampling_benchmark.rs']}}
    save(out/f'provenance-{args.stage}.json', provenance)
    snapshot_files = list(provenance['sources']) + ['rustqec-cli/Cargo.toml', 'rustqec-cli/src/lib.rs',
                     'rustqec-cli/src/decode.rs', 'rustqec-cli/examples/export_matching_benchmark.rs',
                     'rustqec-cli/examples/export_decoder_oracle.rs']
    save(out/'source-snapshot.json', {'base_commit':provenance['source_commit'],
         'description':'Exact benchmark sources at run time; overlay on base_commit.',
         'files':{name:(ROOT/name).read_text() for name in snapshot_files}})
    if args.stage in ['all','correctness']:
        result = correctness.run(binary)
        save(out/'correctness.json', result)
        require((result['status'] == 'PASS'), "run: result['status'] == 'PASS'")
        from .decoder_reference import run as decoder_reference_run
        decoder_result = decoder_reference_run(binary, exporter)
        save(out/'decoder-correctness.json', decoder_result)
        require((decoder_result['status'] == 'PASS'), "run: decoder_result['status'] == 'PASS'")
        from .chain_reference import run as chain_reference_run
        chain_result = chain_reference_run(binary, exporter)
        save(out/'chain-correctness.json', chain_result)
        require((chain_result['status'] == 'PASS'), "run: chain_result['status'] == 'PASS'")
        print('sampling, matching and real-circuit chain correctness PASS', flush=True)
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
