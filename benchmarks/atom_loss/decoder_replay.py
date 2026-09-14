"""Re-decode every archived corpus using current code; never replay old timings."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import zipfile
import numpy as np
from .run import ROOT, checked, export_graph, native_decode, python_decode, python_decode_loop, score
from .shot_data import cases_from, validate_dataset, rescore, rescore_seeds



def without_timings(record):
    return {key:value for key,value in record.items() if not key.endswith('_seconds')}


def verify_work_observations(case, observations, graph_metadata, label):
    """Compare reproducible work counters, never replay historical durations."""
    def same(actual,expected,context):
        # Preserve JSON scalar types: bool/float cannot substitute for integer counters.
        if json.dumps(actual,sort_keys=True) != json.dumps(expected,sort_keys=True):
            raise ValueError('Current workload metadata differs from archive: '+context)
    same(graph_metadata,without_timings(case['graph']),label+'/graph')
    for name,current in observations.items():
        for rep,run in enumerate(case['decoders'][name]['runs']):
            if name=='envelope-matching-offline':
                historical=without_timings(run['batch'])
            elif name.startswith('pymatching'):
                historical=without_timings(run)
                historical.pop('export_repetition',None)
            else:
                historical=without_timings(run['stats'])
            same(current,historical,f'{label}/{name}/run{rep}')


def decode_current(work, binary, exporter, names):
    """Only public files exist in work; private keys are loaded after decoding."""
    graph = export_graph(exporter, work, 'replay')
    predictions = {}; observations = {}
    for name in names:
        if name in {'envelope-matching', 'envelope-mle'}:
            pred, record = native_decode(binary, work, name, 'replay')
            if record['status'] != 'ok':
                raise ValueError('Current decoder failed: '+name+' '+str(record))
            observations[name] = without_timings(record['stats'])
        elif name == 'envelope-matching-offline':
            path = work/(name+'.b8')
            checked([exporter.parent/'offline_matching_benchmark', work/'graph-replay.json', path, work/'offline-stats.json'])
            pred = np.frombuffer(path.read_bytes(), dtype=np.uint8)
            observations[name] = without_timings(json.loads((work/'offline-stats.json').read_text()))
        elif name == 'pymatching-fixed-loop':
            pred, stats = python_decode_loop(graph, False)
            observations[name] = without_timings(stats)
        elif name in {'pymatching-fixed', 'pymatching-envelope'}:
            pred, stats = python_decode(graph, name == 'pymatching-envelope')
            observations[name] = without_timings(stats)
        else:
            raise ValueError('Unknown comparator: '+name)
        predictions[name] = pred
    graph_metadata = {key:graph[key] for key in ['source','num_observables']}
    manifest = json.loads((work/'public/manifest.json').read_text())
    graph_metadata.update(edges=len(graph['edges']), detectors=manifest['circuit']['detectors'],
                          loss_patterns=len(set(map(tuple,graph['losses']))))
    return predictions, observations, graph_metadata


def replay_case(z, label, case, work, binary, exporter, seeded):
    public = work/'public'
    public.mkdir(parents=True)
    for name in ['manifest.json', 'circuit.stim', 'shots.b8']:
        (public/name).write_bytes(z.read(f'{label}/public/{name}'))
    predictions, observations, graph_metadata = decode_current(work, binary, exporter, case['decoders'])
    if not seeded:
        verify_work_observations(case, observations, graph_metadata, label)
    # Scoring keys and historical predictions have not been given to a decoder.
    answers = np.frombuffer(validate_dataset(lambda n: z.read(f'{label}/{n}'), case), dtype=np.uint8)
    native = predictions['envelope-matching']
    for name, pred in predictions.items():
        result = score(pred, answers)
        expected = case['decoders'][name]
        for field in ['prediction_sha256', 'errors', 'shots', 'logical_error_rate', 'wilson_95']:
            if result[field] != expected[field]:
                raise ValueError(f'Current decoder differs from archive: {label}/{name}/{field}')
        members = [name+'.b8'] if seeded else [f'{name}-{rep}.b8' for rep in range(3)]
        for member in members:
            if pred.tobytes() != z.read(label+'/'+member):
                raise ValueError(f'Current prediction bytes differ: {label}/{member}')
        if name != 'envelope-matching':
            paired = {'native_only_wrong': int(np.count_nonzero((native != answers) & (pred == answers))),
                      'other_only_wrong': int(np.count_nonzero((native == answers) & (pred != answers)))}
            if seeded:
                historical = case['paired'].get(name)
            elif 'paired_native_only_wrong' in expected:
                historical = {'native_only_wrong': expected['paired_native_only_wrong'],
                              'other_only_wrong': expected['paired_python_only_wrong']}
            else:
                historical = None  # Some native comparators have no archived paired field.
            if historical is not None and historical != paired:
                raise ValueError(f'Current paired errors differ: {label}/{name}')
    return len(predictions)


def replay(archive, binary, exporter):
    seeded = archive.name == 'accuracy-seeds.zip'
    # Enforce the complete case/backend inventory before running anything.
    (rescore_seeds if seeded else rescore)(archive)
    counts = []
    with zipfile.ZipFile(archive) as z, tempfile.TemporaryDirectory() as tmp:
        cases = ([(f"{c['setting']}-s{c['seed']}", c) for c in json.loads(z.read('accuracy-seeds.json'))['cases']]
                 if seeded else cases_from(z.read))
        for label, case in cases:
            count = replay_case(z, label, case, Path(tmp)/label, binary, exporter, seeded)
            counts.append(count)
            print(f'{label}: {count} current decoders match all archived repetitions and scores', flush=True)
    return {'corpora': len(counts), 'decoder_cases': sum(counts)}


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--root', type=Path, default=ROOT/'site/static/data/atom-loss')
    p.add_argument('--binary', type=Path, default=ROOT/'target/release/rustqec')
    p.add_argument('--exporter', type=Path, default=ROOT/'target/release/examples/export_matching_benchmark')
    a = p.parse_args()
    from .verify import verify
    verify(a.root)
    for name in ['shot-data-v1.zip', 'accuracy-seeds.zip']:
        print('PASS', name, replay(a.root/name, a.binary, a.exporter), flush=True)
