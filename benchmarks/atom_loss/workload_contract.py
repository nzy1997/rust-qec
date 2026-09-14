"""Bind deterministic execution metadata to public rows and the fixed run policy.

Elapsed times are checked for finite ranges and additive identities elsewhere;
neither these checks nor fresh decoder replay authenticate historical timings.
Native compiler/cache internals are range checked here and compared with current
executables by decoder_replay. Visible-pattern counts need no decoder at all.
"""
import hashlib
import json
import math
import re
import zipfile

from .shot_data import ARCHIVE, circuit_layout

SWEEP_BACKENDS = ('envelope-matching', 'pymatching-fixed', 'pymatching-envelope',
                  'envelope-matching-offline')
TRADEOFF_BACKENDS = SWEEP_BACKENDS + ('envelope-mle', 'pymatching-fixed-loop')
GRAPH_SOURCE = 'public dataset only; graph/compiler shared with RustQEC'
OFFLINE_POLICY = 'offline groups; one graph per pattern'
OFFLINE_BOUNDARY = ('JSON transport excluded; grouping, graph build, batch decode, '
                    'reorder, b8 write and flush included; no fsync')
NATIVE_COUNTERS = ('shot_count', 'attempted_shot_count', 'distinct_loss_patterns',
                   'cache_hits', 'timeout_count', 'infeasible_shot_count',
                   'circuit_compilations', 'primitive_probe_count',
                   'primitive_symptom_terms', 'loss_envelope_candidate_count',
                   'matching_graph_builds', 'mle_model_builds', 'mle_detector_rows')
NATIVE_FIELDS = set(NATIVE_COUNTERS) | {'schema_version', 'decoder', 'circuit_sha256',
                                      'compile_seconds', 'decode_seconds',
                                      'distinct_loss_patterns_exact'}


def require(condition, message):
    if not condition:
        raise ValueError('Workload report contract: '+message)


def integer(value, label, minimum=0, maximum=None):
    require(type(value) is int and value >= minimum
            and (maximum is None or value <= maximum), 'invalid '+label)
    return value


def mapping(record, fields, label):
    require(type(record) is dict and set(record) == set(fields), label+' fields')


def seconds(record):
    for name, value in record.items():
        if name.endswith('_seconds'):
            require(type(value) in (int, float) and math.isfinite(value) and value >= 0,
                    'invalid '+name)


def measurement_flags(text):
    """Index visible flags from the documented measurement/REPEAT syntax."""
    bits, detectors, observables = circuit_layout(text)  # Reject unknown syntax.
    lines = iter(text.splitlines())
    def block(nested=False):
        widths = []
        for raw in lines:
            line = raw.split('#', 1)[0].strip()
            if not line:
                continue
            if line == '}':
                require(nested, 'unexpected closing REPEAT')
                return widths
            repeat = re.fullmatch(r'REPEAT (\d+)\s*\{', line)
            if repeat:
                widths.extend(block(True)*int(repeat[1]))
                continue
            measurement = re.fullmatch(r'(M|MZ|MR|MRZ|ML|MZL|MRL|MRZL)(?:\[[^\]]*\])?\s+(.+)', line)
            if measurement:
                widths.extend([2 if measurement[1].endswith('L') else 1]
                              *len(measurement[2].split()))
        require(not nested, 'unclosed REPEAT')
        return widths
    flags, cursor = [], 0
    for width in block():
        if width == 2:
            flags.append(cursor)
        cursor += width
    require(cursor == bits, 'measurement indexing mismatch')
    return bits, detectors, observables, flags


def public_workload(read):
    text = read('public/circuit.stim')
    bits, detectors, observables, flags = measurement_flags(text.decode())
    support = []
    for line in text.decode().splitlines():
        coordinate = re.fullmatch(r'\s*QUBIT_COORDS\(([^)]+)\)\s+(\d+)\s*', line)
        if coordinate:
            xy = [float(value) for value in coordinate[1].split(',')]
            if len(xy) >= 2 and xy[0] == 1 and xy[1] % 2 == 1:
                support.append(coordinate[2])
    manifest = json.loads(read('public/manifest.json'))
    shots = integer(manifest['shots'], 'public shots', 1)
    stride = (bits+7)//8
    rows = read('public/shots.b8')
    require(bits > 0 and len(rows) == stride*shots, 'public row dimensions')
    require(set(observables) == {0}, 'one observable required')
    patterns = {tuple((rows[row*stride+bit//8] >> (bit%8)) & 1 for bit in flags)
                for row in range(shots)}
    return {'shots': shots, 'detectors': detectors, 'num_observables': 1,
            'loss_patterns': len(patterns), 'circuit_sha256': hashlib.sha256(text).hexdigest(),
            'logical_x_support': ','.join(support)}


def native_stats(stats, name, workload):
    mapping(stats, NATIVE_FIELDS, name+' native stats')
    seconds(stats)
    n, patterns = workload['shots'], workload['loss_patterns']
    for key in NATIVE_COUNTERS:
        integer(stats[key], name+'/'+key)
    require(stats['schema_version'] == 'rustqec.decode-stats.v1' and stats['decoder'] == name,
            'native decoder/schema')
    require(stats['circuit_sha256'] == workload['circuit_sha256'], 'native circuit identity')
    require(stats['shot_count'] == stats['attempted_shot_count'] == n, 'native shot count')
    require(stats['timeout_count'] == stats['infeasible_shot_count'] == 0, 'incomplete native run')
    require(stats['circuit_compilations'] == 1, 'native circuit compilations')
    require(stats['primitive_probe_count'] > 0 and stats['primitive_symptom_terms'] > 0,
            'missing compiler probes')
    integer(stats['distinct_loss_patterns'], 'native distinct patterns', 1, n)
    require(type(stats['distinct_loss_patterns_exact']) is bool, 'native exact-count flag')
    # Approximate counters can lie on either side of the exact cardinality.
    if stats['distinct_loss_patterns_exact']:
        require(stats['distinct_loss_patterns'] == patterns, 'native exact pattern count')
    builds = stats['matching_graph_builds'] if name == 'envelope-matching' else stats['mle_model_builds']
    integer(builds, 'native builds', patterns, n)
    integer(stats['cache_hits'], 'native cache hits', 0, n)
    require(builds + stats['cache_hits'] == n, 'native build/cache accounting')
    if name == 'envelope-matching':
        require(stats['mle_model_builds'] == stats['mle_detector_rows'] == stats['loss_envelope_candidate_count'] == 0,
                'matching run contains MLE counters')
    else:
        require(stats['matching_graph_builds'] == 0
                and stats['mle_detector_rows'] == workload['detectors']
                and stats['loss_envelope_candidate_count'] > 0, 'MLE model counters')


def verify_case(case, workload, *, tradeoff=False):
    names = TRADEOFF_BACKENDS if tradeoff else SWEEP_BACKENDS
    require(type(case['decoders']) is dict and set(case['decoders']) == set(names), 'backend inventory')
    require(case['timing_order'] == [list(names[rep:]+names[:rep]) for rep in range(3)],
            'fixed serial timing order')
    n, patterns = workload['shots'], workload['loss_patterns']
    require(type(case['shots']) is int and case['shots'] == n == 5000, 'case shots')
    integer(case['distance'], 'case distance', 3, 7)
    integer(case['rounds'], 'case rounds', 1)
    require(case['distance'] in (3, 5, 7), 'fixed workload distance')
    require((case['distance'], case['rounds'], case['loss_probability']) == (3, 2, .003)
            if tradeoff else case['rounds'] == case['distance'], 'fixed workload dimensions')
    require(case['circuit_sha256'] == workload['circuit_sha256'], 'case circuit identity')
    require(case['logical_x_support'] == workload['logical_x_support']
            and len(workload['logical_x_support'].split(',')) == case['distance'], 'logical-X support')
    graph = case['graph']
    mapping(graph, ('source', 'compile_seconds', 'transform_seconds', 'num_observables',
                    'edges', 'detectors', 'loss_patterns'), 'graph metadata')
    seconds(graph)
    require(graph['source'] == GRAPH_SOURCE, 'graph source')
    integer(graph['edges'], 'graph edges', 1)
    for key in ('num_observables', 'detectors', 'loss_patterns'):
        integer(graph[key], 'graph '+key, 1)
        require(graph[key] == workload[key], 'graph '+key+' differs from public rows')
    for name, result in case['decoders'].items():
        require(result['status'] == 'ok' and type(result['shots']) is int and result['shots'] == n,
                name+' incomplete result')
        require(type(result['runs']) is list and len(result['runs']) == 3, name+' repetitions')
        for rep, run in enumerate(result['runs']):
            seconds(run)
            if name.startswith('pymatching'):
                common = {'compile_seconds', 'transform_seconds', 'decode_seconds', 'write_seconds',
                          'export_repetition', 'graph_builds'}
                if name.endswith('-loop'):
                    mapping(run, common | {'cache_hits'}, name+' run')
                    integer(run['cache_hits'], name+' cache hits')
                    require(run['cache_hits'] == n-1, 'fixed loop cache accounting')
                    expected = 1
                else:
                    mapping(run, common | {'topology_seconds', 'preprocess_seconds', 'graph_build_seconds',
                            'matching_seconds', 'output_seconds', 'adapter_overhead_seconds', 'batch_calls',
                            'graph_api', 'execution'}, name+' run')
                    expected = patterns if name == 'pymatching-envelope' else 1
                    integer(run['batch_calls'], name+' batch calls', 1)
                    require(run['batch_calls'] == expected, name+' batch calls differ from public rows/policy')
                    require(run['graph_api'] == 'from_check_matrix', 'Python graph API')
                    require(run['execution'] == ('batch grouped by loss pattern' if name == 'pymatching-envelope'
                                                 else 'batch fixed graph'), 'Python execution policy')
                    require(run['output_seconds'] >= run['write_seconds'], 'Python output/write accounting')
                integer(run['graph_builds'], name+' graph builds', 1)
                require(run['graph_builds'] == expected, name+' graph builds differ from public rows/policy')
                integer(run['export_repetition'], name+' export repetition')
                require(run['export_repetition'] == rep, 'Python export repetition')
                require(run['decode_seconds'] >= run['write_seconds'], 'Python decode/write accounting')
            else:
                fields = {'status', 'exit_code', 'process_wall_seconds', 'stats'}
                if name == 'envelope-matching-offline':
                    fields |= {'batch', 'transform_seconds'}
                mapping(run, fields, name+' run')
                require(run['status'] == 'ok' and type(run['exit_code']) is int and run['exit_code'] == 0,
                        'native run status/exit code')
                if name != 'envelope-matching-offline':
                    native_stats(run['stats'], name, workload)
                    continue
                batch, stats = run['batch'], run['stats']
                mapping(batch, ('decode_seconds', 'graph_build_seconds', 'matching_seconds', 'write_seconds',
                                'batch_overhead_seconds', 'graph_builds', 'shots', 'policy', 'boundary'), 'offline batch')
                mapping(stats, ('compile_seconds', 'decode_seconds', 'attempted_shot_count', 'timeout_count',
                                'infeasible_shot_count', 'matching_graph_builds', 'cache_hits'), 'offline stats')
                seconds(batch); seconds(stats)
                for record, key in [(batch, 'shots'), (batch, 'graph_builds'),
                                    (stats, 'attempted_shot_count'), (stats, 'matching_graph_builds'),
                                    (stats, 'cache_hits'), (stats, 'timeout_count'), (stats, 'infeasible_shot_count')]:
                    integer(record[key], 'offline '+key)
                require(batch['shots'] == stats['attempted_shot_count'] == n, 'offline shots')
                require(batch['graph_builds'] == stats['matching_graph_builds'] == patterns, 'offline graph count')
                require(stats['cache_hits'] == stats['timeout_count'] == stats['infeasible_shot_count'] == 0,
                        'offline cache/failures')
                require(batch['policy'] == OFFLINE_POLICY and batch['boundary'] == OFFLINE_BOUNDARY,
                        'offline policy/boundary')


def verify_workloads(root):
    """Validate all original measured workflows before trusting derived tables."""
    try:
        cases = json.loads((root/'decoding.json').read_text())
        tradeoff = json.loads((root/'tradeoff.json').read_text())
        with zipfile.ZipFile(root/ARCHIVE) as archive:
            for case in cases+[tradeoff]:
                label = 'tradeoff' if case is tradeoff else f"d{case['distance']}-p{case['loss_probability']}"
                workload = public_workload(lambda name: archive.read(label+'/'+name))
                verify_case(case, workload, tradeoff=case is tradeoff)
    except (KeyError, TypeError, AttributeError) as error:
        raise ValueError('Workload report contract: missing or malformed observation') from error
