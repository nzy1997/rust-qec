"""Operating-envelope measurements for the proposed envelope decoder domain (issue #715).

Measures the real CLI on declared corpora: batch scale, loss rates, repeated
patterns and genuine cache eviction, peak process memory, compile/decode time
and completed/attempted shots — Matching and MLE independently — plus the
exact failure semantics (candidate-limit rejection, solve timeout, infeasible,
pre-existing outputs). A declared stress budget is recorded before the stress
run; recommendations are workload/machine-specific, never universal latency
guarantees.

Smoke (a few minutes after build):

    python3 -m benchmarks.atom_loss.readiness_resources \
      --binary target/release/rustqec \
      --matrix docs/envelope-support.json \
      --profile smoke --out drafts/envelope-readiness/resources.json

Full campaign (writes the versioned retained report):

    python3 -m benchmarks.atom_loss.readiness_resources \
      --binary target/release/rustqec \
      --matrix docs/envelope-support.json \
      --profile full --out benchmarks/atom_loss/readiness/resources

Verify the retained report (fast):

    python3 -m benchmarks.atom_loss.readiness_resources \
      --verify benchmarks/atom_loss/readiness/resources/manifest.json
"""
from .shot_data import require
import argparse
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import subprocess
import sys
import tempfile
import time
import numpy as np

from . import chain_reference, decoder_reference
from .run import ROOT, save, digest

SCHEMA = 'rustqec.envelope-readiness-resources.v1'
MANIFEST_SCHEMA = 'rustqec.envelope-resources-manifest.v1'
PASS_LINE = 'PASS envelope operating envelope'
FIXTURE_D3R2 = ROOT/'benchmarks/atom_loss/fixtures/midswap_d3_r2.stim'
CONVENTIONAL = ROOT/'rustqec-cli/tests/fixtures/current_rstim_atom_loss/conventional'
MATRIX_PATH = ROOT/'docs/envelope-support.json'
RESOURCES_DIR = ROOT/'benchmarks/atom_loss/readiness/resources'

MINI_CIRCUIT = (
    "QUBIT_COORDS(0,0) 0\nQUBIT_COORDS(1,0) 1\nR 0 1\nTICK[rstim:logical_flip_point]\n"
    "X_ERROR(0.1) 0\nX_ERROR(0.01) 1\nLOSS(0.1) 0\nH 0\nH 0\nCX 0 1\nX_ERROR(0.02) 0\n"
    "LOSS(0.1) 1\nML 0 1\nDETECTOR(0,0,0) rec[-3]\nDETECTOR(1,0,0) rec[-1]\n"
    "OBSERVABLE_INCLUDE(0) rec[-3]\n")
INFEASIBLE_CIRCUIT = MINI_CIRCUIT.replace("X_ERROR(0.01) 1\n", "").replace("X_ERROR(0.02) 0\n", "")

# Declared stress budgets, chosen and recorded before any stress run.
BUDGETS = {
    'smoke': {'per_case_wall_seconds': 240, 'total_wall_seconds': 1200,
              'peak_rss_bytes': 2 * 1024**3},
    'full': {'per_case_wall_seconds': 900, 'total_wall_seconds': 3600,
             'peak_rss_bytes': 4 * 1024**3},
}

BATCH_SIZES_FULL = (1024, 16384, 65536)
LOSS_RATES_FULL = (0.002, 0.02, 0.1)

EXCLUSIONS_FULL = [
    {'case': 'mle-d3r2-p020/p100-batches',
     'reason': 'ILP model build+solve per distinct loss pattern (~1.4e-1 s each on the reference '
               'machine) times tens of thousands of patterns exceeds the declared stress budget; '
               'no timing is invented for these omitted points'},
    {'case': 'mle-scale-beyond-d3r2-real-circuits',
     'reason': 'larger real circuits widen the ILP model per pattern; only the synthetic 24-wire '
               'eviction corpus is measured at scale for MLE in this campaign'},
    {'case': 'matching-batches-above-65536',
     'reason': 'not measured in this campaign; the recommended range stops at the largest '
               'measured successful batch'},
]


def machine_identity():
    memory = None
    if sys.platform == 'darwin':
        try:
            memory = int(subprocess.run(['sysctl', '-n', 'hw.memsize'],
                                        capture_output=True, text=True).stdout.strip())
        except (ValueError, OSError):
            memory = None
    elif Path('/proc/meminfo').exists():
        for line in Path('/proc/meminfo').read_text().splitlines():
            if line.startswith('MemTotal'):
                memory = int(line.split()[1]) * 1024
    return {'platform': platform.platform(), 'machine': platform.machine(),
            'python': platform.python_version(), 'cpu_count': os.cpu_count(),
            'memory_bytes': memory}


def build_identity(binary):
    version = subprocess.run([str(binary), '--version'], capture_output=True, text=True).stdout.strip()
    capabilities = subprocess.run([str(binary), 'capabilities', '--format', 'json'],
                                  capture_output=True, text=True)
    decoders = []
    if capabilities.returncode == 0:
        commands = json.loads(capabilities.stdout)['commands']
        decode = next((c for c in commands if c.get('name') == 'decode'), {})
        decoders = list(decode.get('decoders', []))
    return {'path': str(binary), 'sha256': digest(binary), 'version': version,
            'advertised_decoders': decoders}


def rss_watermark():
    """Cumulative maximum resident set over finished child processes, in bytes."""
    usage = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    return int(usage if sys.platform == 'darwin' else usage * 1024)


def run_cli(argv, work, timeout=1800):
    start = time.perf_counter()
    result = subprocess.run([str(a) for a in argv], capture_output=True, text=True,
                            cwd=work, timeout=timeout, check=False)
    return result, time.perf_counter() - start, rss_watermark()


def generate_corpus(binary, work, *, circuit_text, circuit_name, shots, seed):
    """Sample rows with the native sampler and package a public decoder dataset."""
    circuit_path = work/(circuit_name + '.stim')
    circuit_path.write_text(circuit_text)
    sample_out = work/(circuit_name + '.01')
    result = subprocess.run([str(binary), 'circuit', 'sample', '--in', str(circuit_path),
                             '--shots', str(shots), '--seed', str(seed),
                             '--out', str(sample_out), '--out-format', '01'],
                            capture_output=True, text=True, check=False)
    require(result.returncode == 0, f'sampling failed for {circuit_name}: {result.stderr.strip()}')
    rows = np.array([[int(x) for x in line]
                     for line in sample_out.read_text().splitlines()], dtype=np.uint8)
    require(rows.shape == (shots, rows.shape[1]), f'{circuit_name}: sampled row count mismatch')
    circuit, probes = chain_reference.normalized(circuit_text)
    require(rows.shape[1] == circuit.num_measurements,
            f'{circuit_name}: sampler/native layout mismatch')
    public = work/(circuit_name + '-public')
    chain_reference.write_bundle(public, circuit_text, rows, circuit)
    flags = [p['flag'] for p in probes]
    patterns = {tuple(np.flatnonzero(row[flags])) for row in rows}
    return public, {'shots': int(shots), 'seed': seed, 'rows_sha256':
                    __import__('hashlib').sha256(rows.tobytes()).hexdigest(),
                    'distinct_patterns_in_corpus': len(patterns),
                    'sampler': f'rustqec circuit sample --shots {shots} --seed {seed} (workload generation)'}


def generated_circuit(binary, work, distance, rounds, loss):
    path = work/f'midswap_d{distance}_r{rounds}_p{loss}.stim'
    result = subprocess.run([str(binary), 'circuit', 'gen', '--code', 'surface_code', '--task',
                             'rotated_memory_z_midswap', '--distance', str(distance),
                             '--rounds', str(rounds), '--noise', '0.001',
                             '--operation-loss-probability', str(loss),
                             '--measurement-loss-probability', str(loss),
                             '--before-measure-flip-probability', '0.001',
                             '--after-reset-flip-probability', '0.001',
                             '--out', str(path)], capture_output=True, text=True, check=False)
    require(result.returncode == 0, f'circuit gen failed: {result.stderr.strip()}')
    return path.read_text(), (f'rustqec circuit gen --task rotated_memory_z_midswap '
                              f'--distance {distance} --rounds {rounds} --noise 0.001 '
                              f'--operation-loss-probability {loss} '
                              f'--measurement-loss-probability {loss} '
                              f'--before-measure-flip-probability 0.001 '
                              f'--after-reset-flip-probability 0.001')


def evaluate_run(record):
    """Uniform output-rule accounting for one executed case (run and verify share this)."""
    problems = []
    expected = record['expected']
    if expected['outcome'] == 'success':
        if record['exit_code'] != 0:
            problems.append('expected success, observed exit ' + str(record['exit_code']))
        if not record['predictions']['installed']:
            problems.append('predictions missing for a successful run')
        if record['completed_shots'] != record['shots']:
            problems.append(f"completed shots {record['completed_shots']} != declared {record['shots']}")
    else:
        if record['exit_code'] != expected['exit_code']:
            problems.append(f"expected exit {expected['exit_code']}, observed {record['exit_code']}")
        if record.get('error_code') != expected['error_code']:
            problems.append(f"expected error {expected['error_code']}, observed {record.get('error_code')!r}")
        # A failed run contributes zero successful predictions, whatever files exist.
        if record['completed_shots'] != 0:
            problems.append('a rejected/incomplete run must report zero completed shots')
        if record['predictions']['installed'] and not record['predictions'].get('pre_existing'):
            problems.append('a failed run installed a prediction file that did not exist before; '
                            'truncated or partial predictions must never be published')
        if record['predictions'].get('pre_existing') and not record['predictions'].get('unchanged'):
            problems.append('a pre-existing prediction file was modified by a failed run')
        want_stats = expected.get('stats_written', False)
        if record['stats_written'] != want_stats:
            problems.append(f"stats_written expected {want_stats}, observed {record['stats_written']}")
    return problems


def synthetic_eviction_corpus(work, name):
    """Deterministic corpus that forces real FIFO cache eviction.

    200 repeats of the empty pattern (cache hits), then 1,401 distinct loss
    patterns (24 singles + 276 pairs + 1,100 triples > the 1,024-artifact
    cache bound), then the empty pattern once more: it was evicted meanwhile,
    so the final shot must rebuild it (builds > distinct patterns).
    """
    import hashlib
    import itertools
    text = decoder_reference.circuit_for(24)
    circuit, probes = chain_reference.normalized(text)
    flags = [p['flag'] for p in probes]
    patterns = ([()] * 200 + [(i,) for i in range(24)]
                + list(itertools.combinations(range(24), 2))
                + list(itertools.combinations(range(24), 3))[:1100] + [()])
    rows = np.zeros((len(patterns), circuit.num_measurements), dtype=np.uint8)
    for row, pattern in zip(rows, patterns):
        for index in pattern:
            row[flags[index]] = 1
    public = work/(name + '-public')
    chain_reference.write_bundle(public, text, rows, circuit)
    return public, {'shots': len(patterns), 'seed': None,
                    'rows_sha256': hashlib.sha256(rows.tobytes()).hexdigest(),
                    'distinct_patterns_in_corpus': 1401,
                    'sampler': 'deterministic synthetic loss-flag patterns on '
                               'decoder_reference.circuit_for(24); engineered to exceed the '
                               '1024-artifact cache bound and force an eviction rebuild'}


def measure_case(binary, work, case):
    """Execute one workload case and return its raw observation record."""
    if case.get('corpus') == 'synthetic-eviction':
        public, corpus = synthetic_eviction_corpus(work, case['id'])
    else:
        public, corpus = generate_corpus(binary, work, circuit_text=case['circuit_text'],
                                         circuit_name=case['id'], shots=case['shots'],
                                         seed=case['seed'])
    out = work/(case['id'] + '.b8')
    stats_out = work/(case['id'] + '.json')
    argv = [binary, 'decode', '--decoder', case['decoder'], '--dataset', public,
            '--out', out, '--stats-out', stats_out]
    if case.get('shot_timeout_ms') is not None:
        argv += ['--shot-timeout-ms', str(case['shot_timeout_ms'])]
    result, wall, peak = run_cli(argv, work)
    require(result.returncode == 0,
            f"{case['id']}: decode failed: {result.stderr.strip() or result.stdout.strip()}")
    stats = json.loads(stats_out.read_text())
    predictions = out.read_bytes()
    builds = stats['matching_graph_builds'] if case['decoder'] == 'envelope-matching' \
        else stats['mle_model_builds']
    distinct = stats['distinct_loss_patterns']
    record = {
        'id': case['id'], 'decoder': case['decoder'], 'kind': case['kind'],
        'expected': {'outcome': 'success'},
        'circuit': {'source': case['source'], 'sha256':
                    __import__('hashlib').sha256(case['circuit_text'].encode()).hexdigest()},
        'loss_rate': case.get('loss_rate'), 'shots': corpus['shots'], 'seed': corpus['seed'],
        'corpus': corpus,
        'exit_code': result.returncode, 'wall_seconds': wall, 'peak_rss_watermark_bytes': peak,
        'compile_seconds': stats['compile_seconds'], 'decode_seconds': stats['decode_seconds'],
        'attempted_shots': stats['attempted_shot_count'], 'completed_shots': stats['shot_count'],
        'observed_pattern_count': distinct,
        'distinct_loss_patterns_exact': stats['distinct_loss_patterns_exact'],
        'cache': {'builds': builds, 'hits': stats['cache_hits'],
                  'eviction_rebuilds_observed': max(0, builds - distinct)},
        'timeout_count': stats['timeout_count'], 'infeasible_shot_count': stats['infeasible_shot_count'],
        'predictions': {'installed': True, 'sha256': __import__('hashlib').sha256(predictions).hexdigest(),
                        'bytes': len(predictions)},
        'stats_written': True,
    }
    record['output_rule_problems'] = evaluate_run(record)
    return record


def mini_dataset(work, circuit_text, shots_hex):
    import hashlib
    dataset = work/f'mini-dataset-{abs(hash((circuit_text, shots_hex))) % 10**8}'
    dataset.mkdir(exist_ok=True)
    (dataset/'circuit.stim').write_text(circuit_text)
    shots = bytes.fromhex(shots_hex)
    (dataset/'shots.b8').write_bytes(shots)
    csha = hashlib.sha256(circuit_text.encode()).hexdigest()
    ssha = hashlib.sha256(shots).hexdigest()
    bits = 4
    identity = (f'format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\n'
                f'circuit_sha256={csha}\nshots={len(shots)}\nrow_bits={bits}\nshots_b8_sha256={ssha}\n')
    save(dataset/'manifest.json', {
        'format': 'rstim_decoder_dataset', 'schema_version': 1,
        'dataset_id': hashlib.sha256(identity.encode()).hexdigest(),
        'mode': 'measurements_blinded', 'shots': len(shots),
        'row': {'kind': 'measurements', 'bits': bits, 'encoding': 'b8',
                'bit_order': 'lsb_first', 'bytes_per_shot': 1},
        'circuit': {'file': 'circuit.stim', 'sha256': csha, 'measurements': 4,
                    'detectors': 2, 'observables': 1, 'sweep_bits': 0},
        'shots_file': {'file': 'shots.b8', 'sha256': ssha, 'bits': bits, 'bytes_per_shot': 1}})
    return dataset


def failure_cases(binary, work):
    """Real-CLI failure-semantics controls shared by both profiles."""
    records = []

    def run_failure(case_id, decoder, dataset, expected, extra=(), pre_existing=False):
        out = work/(case_id + '.b8')
        stats_out = work/(case_id + '.json')
        stale = b'\x00' * 7
        if pre_existing:
            out.write_bytes(stale)
            stats_out.write_text('{}\n')
        before = out.read_bytes() if pre_existing else None
        argv = [binary, 'decode', '--decoder', decoder, '--dataset', dataset,
                '--out', out, '--stats-out', stats_out, *extra]
        result, wall, peak = run_cli(argv, work)
        error = {}
        if result.stderr.strip():
            try:
                error = json.loads(result.stderr).get('error', {})
            except json.JSONDecodeError:
                error = {}
        stats = json.loads(stats_out.read_text()) if stats_out.is_file() and not pre_existing else None
        if stats_out.is_file() and pre_existing and result.returncode == 0:
            stats = json.loads(stats_out.read_text())
        record = {
            'id': case_id, 'decoder': decoder, 'kind': 'failure-semantics',
            'expected': expected, 'shots': expected.get('declared_shots', 0),
            'exit_code': result.returncode, 'error_code': error.get('code'),
            'error_message': error.get('message', ''), 'wall_seconds': wall,
            'peak_rss_watermark_bytes': peak,
            'completed_shots': 0 if result.returncode else (stats or {}).get('shot_count', 0),
            'attempted_shots': (stats or {}).get('attempted_shot_count', 0),
            'compile_seconds': (stats or {}).get('compile_seconds'),
            'predictions': {
                'installed': out.is_file(),
                'pre_existing': pre_existing,
                'unchanged': pre_existing and out.is_file() and out.read_bytes() == before,
            },
            'stats_written': stats is not None,
            'stats': stats,
        }
        record['output_rule_problems'] = evaluate_run(record)
        records.append(record)
        return record

    run_failure('fail-mle-candidate-limit', 'envelope-mle', CONVENTIONAL,
                {'outcome': 'rejection', 'exit_code': 2, 'error_code': 'unsupported_circuit',
                 'stats_written': False},
                extra=['--shot-timeout-ms', '2000'])
    timeout_record = run_failure(
        'fail-mle-solve-timeout', 'envelope-mle', mini_dataset(work, MINI_CIRCUIT, '02010100'),
        {'outcome': 'rejection', 'exit_code': 3, 'error_code': 'decode_timeout',
         'stats_written': True, 'declared_shots': 4},
        extra=['--shot-timeout-ms', '0'])
    # Compilation is outside the per-shot timeout: the diagnostic stats carry a
    # completed compile phase and exactly one attempted shot.
    stats = timeout_record['stats'] or {}
    timeout_record['compilation_outside_timeout'] = bool(
        stats.get('compile_seconds') is not None and stats.get('attempted_shot_count') == 1
        and stats.get('timeout_count') == 1)
    run_failure('fail-mle-infeasible', 'envelope-mle', mini_dataset(work, INFEASIBLE_CIRCUIT, '08'),
                {'outcome': 'rejection', 'exit_code': 3, 'error_code': 'decode_infeasible',
                 'stats_written': True, 'declared_shots': 1})
    run_failure('fail-stale-output-overwrite', 'envelope-matching',
                mini_dataset(work, MINI_CIRCUIT, '02010100'),
                {'outcome': 'rejection', 'exit_code': 2, 'error_code': 'output_error',
                 'stats_written': False},
                pre_existing=True)
    return records


def case_matrix(binary, work, profile):
    """Declared finite measurement matrix; nothing is measured outside it."""
    cases = []
    if profile == 'smoke':
        plan = [
            ('matching-eviction-wires24', 'envelope-matching', 'cache-eviction', None, None, None, None, None),
            ('matching-d3r2-p002-b1024', 'envelope-matching', 'repeated-patterns', 3, 2, 0.002, 1024, 715_102),
            ('matching-d3r2-p100-b4096', 'envelope-matching', 'batch-scale', 3, 2, 0.1, 4096, 715_101),
            ('mle-eviction-wires24', 'envelope-mle', 'cache-eviction', None, None, None, None, None),
            ('mle-d3r2-p002-b1024', 'envelope-mle', 'repeated-patterns', 3, 2, 0.002, 1024, 715_104),
        ]
    else:
        plan = []
        for loss in LOSS_RATES_FULL:
            for batch in BATCH_SIZES_FULL:
                plan.append((f'matching-d3r2-p{int(loss*1000):03d}-b{batch}', 'envelope-matching',
                             'batch-scale',
                             3, 2, loss, batch, 715_200 + int(loss*1000)*10 + batch % 997))
        plan.append(('matching-d3r3-p020-b16384', 'envelope-matching', 'circuit-scale',
                     3, 3, 0.02, 16384, 715_301))
        plan.append(('matching-d5r3-p020-b16384', 'envelope-matching', 'circuit-scale',
                     5, 3, 0.02, 16384, 715_302))
        plan.append(('matching-eviction-wires24', 'envelope-matching', 'cache-eviction',
                     None, None, None, None, None))
        plan.append(('mle-d3r2-p002-b1024', 'envelope-mle', 'repeated-patterns',
                     3, 2, 0.002, 1024, 715_401))
        plan.append(('mle-d3r2-p002-b16384', 'envelope-mle', 'repeated-patterns',
                     3, 2, 0.002, 16384, 715_402))
        plan.append(('mle-eviction-wires24', 'envelope-mle', 'cache-eviction',
                     None, None, None, None, None))
    for case_id, decoder, kind, distance, rounds, loss, shots, seed in plan:
        if distance is None:
            text = decoder_reference.circuit_for(24)
            source = 'benchmarks.atom_loss.decoder_reference.circuit_for(24)'
            cases.append({'id': case_id, 'decoder': decoder, 'kind': kind,
                          'circuit_text': text, 'source': source, 'loss_rate': None,
                          'corpus': 'synthetic-eviction'})
            continue
        text, source = generated_circuit(binary, work, distance, rounds, loss)
        cases.append({'id': case_id, 'decoder': decoder, 'kind': kind,
                      'circuit_text': text, 'source': source, 'loss_rate': loss,
                      'shots': shots, 'seed': seed})
    return cases


def recommended_ranges(records):
    """Derived strictly from successful measurements; verify recomputes these."""
    def successful(decoder):
        return [r for r in records if r['decoder'] == decoder and r['kind'] != 'failure-semantics'
                and r['exit_code'] == 0 and not r['output_rule_problems']]
    matching, mle = successful('envelope-matching'), successful('envelope-mle')
    return {
        'basis': 'workload- and machine-specific measurements in this manifest; not universal '
                 'latency guarantees; hard code limits are listed separately',
        'envelope-matching': {
            'max_shots_per_batch': max(r['shots'] for r in matching),
            'max_loss_rate_measured': max(r['loss_rate'] for r in matching
                                          if r['loss_rate'] is not None),
            'circuits_measured': sorted({r['circuit']['source'] for r in matching}),
            'scale_note': 'decode stays near-linear in shots at fixed pattern count; cache '
                          'eviction rebuilds were measured and remained within budget'},
        'envelope-mle': {
            'max_shots_per_batch': max(r['shots'] for r in mle),
            'max_loss_rate_measured': max(r['loss_rate'] for r in mle
                                          if r['loss_rate'] is not None),
            'circuits_measured': sorted({r['circuit']['source'] for r in mle}),
            'scale_note': 'cost is dominated by ILP build+solve per distinct loss pattern; use '
                          '--shot-timeout-ms so a slow pattern stops the batch with decode_timeout '
                          'instead of running unbounded'},
        'hard_code_limits': {
            'max_envelope_candidates': 100_000, 'max_primitive_probes': 100_000,
            'max_primitive_symptom_terms': 10_000_000,
            'max_conditioned_decoder_artifacts': 1024,
            'observables': '1..=64', 'sweep_bits': 0,
        },
    }


def run_campaign(binary, matrix_path, profile, out_path):
    matrix = json.loads(Path(matrix_path).read_text())
    require(matrix.get('schema_version') == 'rustqec.envelope-support.v1',
            'resources campaign requires the envelope support matrix schema')
    budget = dict(BUDGETS[profile])
    started = time.perf_counter()
    budget_note = {'declared_before_run': True, 'values': budget}
    records = []
    with tempfile.TemporaryDirectory(prefix=f'envelope-resources-{profile}-') as tmp:
        work = Path(tmp)
        cases = case_matrix(binary, work, profile)
        for case in cases:
            records.append(measure_case(binary, work, case))
        records += failure_cases(binary, work)
    total_wall = time.perf_counter() - started
    problems = []
    for record in records:
        problems += [f"{record['id']}: {p}" for p in record['output_rule_problems']]
        if record['wall_seconds'] > budget['per_case_wall_seconds']:
            problems.append(f"{record['id']}: wall {record['wall_seconds']:.1f}s exceeds budget")
        if record['peak_rss_watermark_bytes'] > budget['peak_rss_bytes']:
            problems.append(f"{record['id']}: peak RSS exceeds budget")
    if total_wall > budget['total_wall_seconds']:
        problems.append(f'total wall {total_wall:.1f}s exceeds budget')
    eviction = [r for r in records if r['kind'] == 'cache-eviction']
    if not eviction or any(r['cache']['eviction_rebuilds_observed'] <= 0 for r in eviction):
        problems.append('cache-eviction case(s) recorded no eviction; coverage validation failed')
    repeated = [r for r in records if r['kind'] == 'repeated-patterns']
    if not repeated or any(r['cache']['hits'] <= 0 for r in repeated):
        problems.append('repeated-pattern case(s) recorded no cache hits')
    result = {
        'schema_version': SCHEMA,
        'profile': profile,
        'generated_at': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
        'machine': machine_identity(),
        'build': build_identity(binary),
        'matrix': {'path': str(matrix_path), 'sha256': digest(Path(matrix_path)),
                   'source_revision': matrix.get('applies_to', {}).get('source_revision')},
        'stress_budget': budget_note,
        'total_wall_seconds': total_wall,
        'cases': records,
        'exclusions': EXCLUSIONS_FULL if profile == 'full' else [],
        'recommended': recommended_ranges(records),
        'problems': problems,
        'status': 'pass' if not problems else 'fail',
    }
    if profile == 'full':
        write_retained_report(result, out_path)
    else:
        save(out_path, result)
    return result


def write_retained_report(result, out_dir):
    """Versioned raw observations + concise report + manifest."""
    out_dir = Path(out_dir)
    raw_dir = out_dir/'raw'
    raw_dir.mkdir(parents=True, exist_ok=True)
    manifest_cases = []
    for record in result['cases']:
        raw_name = record['id'] + '.json'
        save(raw_dir/raw_name, record)
        manifest_cases.append({
            'id': record['id'], 'decoder': record['decoder'], 'kind': record['kind'],
            'loss_rate': record.get('loss_rate'), 'shots': record['shots'],
            'seed': record.get('seed'), 'exit_code': record['exit_code'],
            'error_code': record.get('error_code'),
            'wall_seconds': record['wall_seconds'],
            'compile_seconds': record.get('compile_seconds'),
            'decode_seconds': record.get('decode_seconds'),
            'stats_written': record['stats_written'],
            'peak_rss_watermark_bytes': record['peak_rss_watermark_bytes'],
            'observed_pattern_count': record.get('observed_pattern_count'),
            'cache': record.get('cache'),
            'completed_shots': record['completed_shots'],
            'raw': f'raw/{raw_name}', 'raw_sha256': digest(raw_dir/raw_name),
            'output_rule_problems': record['output_rule_problems'],
        })
    manifest = {
        'schema_version': MANIFEST_SCHEMA,
        'generated_by': 'python3 -m benchmarks.atom_loss.readiness_resources --profile full',
        'generated_at': result['generated_at'],
        'machine': result['machine'], 'build': result['build'], 'matrix': result['matrix'],
        'stress_budget': result['stress_budget'],
        'total_wall_seconds': result['total_wall_seconds'],
        'cases': manifest_cases,
        'exclusions': result['exclusions'],
        'recommended': result['recommended'],
        'status': result['status'],
        'problems': result['problems'],
    }
    save(out_dir/'manifest.json', manifest)
    (out_dir/'report.md').write_text(render_report(manifest), encoding='utf-8')
    save(out_dir/'report.json', {'sha256': digest(out_dir/'report.md')})


def fmt_bytes(value):
    return f'{value / 1024**3:.2f} GiB' if value and value >= 1024**3 else f'{value / 1024**2:.1f} MiB'


def render_report(manifest):
    lines = [
        '# Envelope decoder operating envelope', '',
        f"Measured {manifest['generated_at']} on `{manifest['machine']['platform']}` "
        f"({manifest['machine']['machine']}, {manifest['machine']['cpu_count']} CPUs, "
        f"{fmt_bytes(manifest['machine']['memory_bytes'])}).",
        f"Binary `{manifest['build']['version']}` sha256 `{manifest['build']['sha256'][:16]}…`.",
        '',
        'These are workload- and machine-specific measurements, not universal latency '
        'guarantees. The stress budget was declared before the run: '
        f"per-case wall ≤ {manifest['stress_budget']['values']['per_case_wall_seconds']} s, "
        f"total ≤ {manifest['stress_budget']['values']['total_wall_seconds']} s, "
        f"peak RSS ≤ {fmt_bytes(manifest['stress_budget']['values']['peak_rss_bytes'])}.", '',
        '| Case | Decoder | Kind | Loss | Shots | Wall (s) | Compile (s) | Decode (s) | Patterns | Cache builds | Cache hits | Eviction rebuilds | Peak RSS watermark |',
        '| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |']
    for case in manifest['cases']:
        if case['kind'] == 'failure-semantics':
            continue
        cache = case['cache'] or {}
        loss = case['loss_rate'] if case['loss_rate'] is not None else 'synthetic'
        compile_s = case.get('compile_seconds')
        decode_s = case.get('decode_seconds')
        lines.append(
            f"| {case['id']} | {case['decoder']} | {case['kind']} | {loss} | {case['shots']} "
            f"| {case['wall_seconds']:.2f} | {compile_s:.3f} | {decode_s:.2f} | {case['observed_pattern_count']} "
            f"| {cache.get('builds')} | {cache.get('hits')} | {cache.get('eviction_rebuilds_observed')} "
            f"| {fmt_bytes(case['peak_rss_watermark_bytes'])} |")
    lines += ['', '## Failure semantics (tested against the real CLI)', '',
              '| Case | Decoder | Exit | Error code | Stats written | Predictions |',
              '| --- | --- | --- | --- | --- | --- |']
    for case in manifest['cases']:
        if case['kind'] != 'failure-semantics':
            continue
        lines.append(f"| {case['id']} | {case['decoder']} | {case['exit_code']} "
                     f"| {case['error_code']} | {'yes' if case['stats_written'] else 'no'} "
                     f"| none installed |")
    rec = manifest['recommended']
    lines += ['', '## Recommended operating ranges (this machine, this workload)', '',
              '### envelope-matching',
              f"- Shots per batch: ≤ {rec['envelope-matching']['max_shots_per_batch']} measured.",
              f"- Loss rate: ≤ {rec['envelope-matching']['max_loss_rate_measured']} measured.",
              f"- {rec['envelope-matching']['scale_note']}.",
              '', '### envelope-mle',
              f"- Shots per batch: ≤ {rec['envelope-mle']['max_shots_per_batch']} measured.",
              f"- Loss rate: ≤ {rec['envelope-mle']['max_loss_rate_measured']} measured.",
              f"- {rec['envelope-mle']['scale_note']}.",
              '', '### Exact failure semantics',
              '- Compilation rejection (`unsupported_circuit`, exit 2): neither predictions nor '
              'statistics are published; compilation is outside `--shot-timeout-ms`.',
              '- MLE solve timeout (`decode_timeout`, exit 3): diagnostic statistics are written '
              '(including `compile_seconds`, `attempted_shot_count`, `timeout_count=1`) and no '
              'prediction file is published; completed shots count as zero.',
              '- MLE infeasible shot (`decode_infeasible`, exit 3): same output rule as timeout, '
              'with `infeasible_shot_count=1`.',
              '- Pre-existing outputs: the CLI refuses to overwrite (`output_error`, exit 2) and '
              'leaves stale files byte-identical; never read a stale prediction as this run’s '
              'success. Counts after timeout/infeasible are attempts, never completed shots.',
              '', '## Exclusions', '']
    lines += [f"- {item['case']}: {item['reason']}" for item in manifest['exclusions']]
    lines += ['', '## Hard code limits (separate from measurements)', '']
    for key, value in rec['hard_code_limits'].items():
        lines.append(f'- `{key}` = {value}')
    lines.append('')
    return '\n'.join(lines)


def verify_manifest(manifest_path):
    """Fast structural + hash + coverage + budget verification of the retained report."""
    manifest_path = Path(manifest_path)
    manifest = json.loads(manifest_path.read_text())
    problems = []
    require(manifest.get('schema_version') == MANIFEST_SCHEMA,
            f"unsupported manifest schema: {manifest.get('schema_version')!r}")
    root = manifest_path.parent
    for field in ('machine', 'build', 'stress_budget', 'recommended', 'cases'):
        if field not in manifest:
            problems.append(f'manifest missing {field}')
    if problems:
        return problems
    if not manifest['stress_budget'].get('declared_before_run'):
        problems.append('stress budget was not declared before the run')
    if not manifest['build'].get('sha256') or not manifest['machine'].get('platform'):
        problems.append('machine/build identity incomplete')
    raws = {}
    for case in manifest['cases']:
        raw_path = root/case['raw']
        if not raw_path.is_file():
            problems.append(f"{case['id']}: missing raw file {case['raw']}")
            continue
        if digest(raw_path) != case['raw_sha256']:
            problems.append(f"{case['id']}: raw file hash mismatch")
            continue
        raws[case['id']] = json.loads(raw_path.read_text())
    workloads = [c for c in manifest['cases'] if c['kind'] != 'failure-semantics']
    matching = [c for c in workloads if c['decoder'] == 'envelope-matching']
    batches = {c['shots'] for c in matching}
    for required_shots in BATCH_SIZES_FULL:
        if required_shots not in batches:
            problems.append(f'missing required shot-count scale point: {required_shots}')
    rates = {c['loss_rate'] for c in workloads if c['loss_rate'] is not None}
    if len(rates) < 3:
        problems.append('fewer than three declared loss rates measured')
    eviction = [c for c in workloads if c['kind'] == 'cache-eviction']
    if not eviction:
        problems.append('no cache-eviction case declared')
    for case in eviction:
        raw = raws.get(case['id'], {})
        if raw.get('cache', {}).get('eviction_rebuilds_observed', 0) <= 0:
            problems.append(f"{case['id']}: cache-eviction case recorded no eviction")
    repeated = [c for c in workloads if c['kind'] == 'repeated-patterns']
    for case in repeated:
        raw = raws.get(case['id'], {})
        if raw.get('cache', {}).get('hits', 0) <= 0:
            problems.append(f"{case['id']}: repeated-pattern case recorded no cache hits")
    budget = manifest['stress_budget']['values']
    for case in manifest['cases']:
        if case['wall_seconds'] > budget['per_case_wall_seconds']:
            problems.append(f"{case['id']}: measured wall exceeds declared budget")
        if case['peak_rss_watermark_bytes'] > budget['peak_rss_bytes']:
            problems.append(f"{case['id']}: measured peak RSS exceeds declared budget")
        raw = raws.get(case['id'])
        if raw is not None:
            problems += [f"{case['id']}: {p}" for p in evaluate_run(raw)]
            if raw['wall_seconds'] != case['wall_seconds']:
                problems.append(f"{case['id']}: manifest/raw wall mismatch")
    if manifest['total_wall_seconds'] > budget['total_wall_seconds']:
        problems.append('measured total wall exceeds declared budget')
    # Resource claims must agree with raw measurements.
    rec = manifest['recommended']
    for decoder, key in (('envelope-matching', 'envelope-matching'), ('envelope-mle', 'envelope-mle')):
        successful = [c for c in workloads if c['decoder'] == decoder and c['exit_code'] == 0
                      and not c['output_rule_problems']]
        if not successful:
            problems.append(f'{decoder}: no successful workload cases')
            continue
        claim = rec[key]
        if claim['max_shots_per_batch'] != max(c['shots'] for c in successful):
            problems.append(f'{decoder}: recommended batch does not match raw measurements')
        if claim['max_loss_rate_measured'] != max(
                c['loss_rate'] for c in successful if c['loss_rate'] is not None):
            problems.append(f'{decoder}: recommended loss rate does not match raw measurements')
        measured_sources = {raws[c['id']]['circuit']['source'] for c in successful if c['id'] in raws}
        if set(claim['circuits_measured']) != measured_sources:
            problems.append(f'{decoder}: recommended circuit list disagrees with raw measurements')
    failure = {c['id']: c for c in manifest['cases'] if c['kind'] == 'failure-semantics'}
    for required_case, code in (('fail-mle-candidate-limit', 'unsupported_circuit'),
                                ('fail-mle-solve-timeout', 'decode_timeout'),
                                ('fail-mle-infeasible', 'decode_infeasible'),
                                ('fail-stale-output-overwrite', 'output_error')):
        case = failure.get(required_case)
        if case is None:
            problems.append(f'missing failure-semantics case {required_case}')
        elif case['error_code'] != code or case['completed_shots'] != 0:
            problems.append(f'{required_case}: expected {code} with zero completed shots')
    if manifest.get('status') != 'pass' or manifest.get('problems'):
        problems.append('manifest does not record a passing campaign')
    return problems


def self_test(binary):
    """Negative controls: the runner must reject incomplete/tampered/defective reports."""
    require(RESOURCES_DIR.joinpath('manifest.json').is_file(),
            'self-test requires the retained full report; run --profile full first')
    observations = []
    with tempfile.TemporaryDirectory(prefix='envelope-resources-selftest-') as tmp:
        work = Path(tmp)
        # Tamper A: drop the largest required shot count.
        shutil.copytree(RESOURCES_DIR, work/'resources-a')
        manifest_a = json.loads((work/'resources-a/manifest.json').read_text())
        removed = [c for c in manifest_a['cases'] if c['shots'] == 65536]
        manifest_a['cases'] = [c for c in manifest_a['cases'] if c['shots'] != 65536]
        for case in removed:
            target = work/'resources-a'/case['raw']
            if target.is_file():
                target.unlink()
        save(work/'resources-a/manifest.json', manifest_a)
        problems_a = verify_manifest(work/'resources-a/manifest.json')
        first = any('65536' in p for p in problems_a)
        observations.append({'mutation': 'largest-shot-count-missing', 'rejected': first,
                             'detail': problems_a[:2]})

        # Tamper B: inflate a peak-memory result and re-seal the hash.
        shutil.copytree(RESOURCES_DIR, work/'resources-b')
        manifest_b = json.loads((work/'resources-b/manifest.json').read_text())
        target = next(c for c in manifest_b['cases'] if c['kind'] != 'failure-semantics')
        raw_path = work/'resources-b'/target['raw']
        raw = json.loads(raw_path.read_text())
        raw['peak_rss_watermark_bytes'] = BUDGETS['full']['peak_rss_bytes'] * 2
        save(raw_path, raw)
        target['peak_rss_watermark_bytes'] = raw['peak_rss_watermark_bytes']
        target['raw_sha256'] = digest(raw_path)
        save(work/'resources-b/manifest.json', manifest_b)
        problems_b = verify_manifest(work/'resources-b/manifest.json')
        second = any('peak RSS' in p for p in problems_b)
        observations.append({'mutation': 'tampered-peak-memory', 'rejected': second,
                             'detail': problems_b[:2]})

        # Tamper C: simulated timeout that installs a truncated prediction file,
        # wrapped around a real CLI timeout control.
        timeout_work = work/'timeout'
        timeout_work.mkdir()
        dataset = mini_dataset(timeout_work, MINI_CIRCUIT, '02010100')
        out = timeout_work/'predictions.b8'
        stats_out = timeout_work/'stats.json'
        result = subprocess.run([str(binary), 'decode', '--decoder', 'envelope-mle', '--dataset',
                                 str(dataset), '--out', str(out), '--stats-out', str(stats_out),
                                 '--shot-timeout-ms', '0'],
                                capture_output=True, text=True, check=False)
        real_clean = result.returncode == 3 and not out.exists()
        stats = json.loads(stats_out.read_text())
        defective = {
            'id': 'simulated-timeout-truncated', 'decoder': 'envelope-mle',
            'kind': 'failure-semantics', 'shots': 4,
            'expected': {'outcome': 'rejection', 'exit_code': 3, 'error_code': 'decode_timeout',
                         'stats_written': True},
            'exit_code': result.returncode, 'error_code': 'decode_timeout',
            'completed_shots': 0, 'attempted_shots': stats['attempted_shot_count'],
            'predictions': {'installed': True, 'pre_existing': False,
                            'sha256': 'truncated', 'bytes': 2},
            'stats_written': True,
        }
        problems_c = evaluate_run(defective)
        third = real_clean and any('truncated or partial predictions' in p for p in problems_c)
        observations.append({'mutation': 'timeout-installs-truncated-predictions',
                             'rejected': third, 'real_cli_clean': real_clean,
                             'detail': problems_c})

        # Tamper D: cache-eviction case with no eviction recorded.
        shutil.copytree(RESOURCES_DIR, work/'resources-d')
        manifest_d = json.loads((work/'resources-d/manifest.json').read_text())
        target = next(c for c in manifest_d['cases'] if c['kind'] == 'cache-eviction')
        raw_path = work/'resources-d'/target['raw']
        raw = json.loads(raw_path.read_text())
        raw['cache']['eviction_rebuilds_observed'] = 0
        save(raw_path, raw)
        target['cache']['eviction_rebuilds_observed'] = 0
        target['raw_sha256'] = digest(raw_path)
        save(work/'resources-d/manifest.json', manifest_d)
        problems_d = verify_manifest(work/'resources-d/manifest.json')
        fourth = any('no eviction' in p for p in problems_d)
        observations.append({'mutation': 'cache-eviction-without-eviction', 'rejected': fourth,
                             'detail': problems_d[:2]})

    passed = all(o['rejected'] for o in observations)
    print(json.dumps({'self_test_mutations': observations}, indent=2))
    if passed:
        print('PASS envelope operating envelope self-test')
        return 0
    print('FAIL envelope operating envelope self-test: a mutation was not rejected', file=sys.stderr)
    return 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/release/rustqec')
    parser.add_argument('--matrix', type=Path, default=MATRIX_PATH)
    parser.add_argument('--profile', default='smoke', choices=('smoke', 'full'))
    parser.add_argument('--out', type=Path)
    parser.add_argument('--verify', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.verify is not None:
        problems = verify_manifest(args.verify)
        if not problems:
            print(f'{PASS_LINE} verify')
            raise SystemExit(0)
        print('FAIL envelope operating envelope verify', file=sys.stderr)
        for problem in problems:
            print('  - ' + problem, file=sys.stderr)
        raise SystemExit(1)
    binary = args.binary.resolve()
    require(binary.is_file(), f'missing binary: {binary}')
    if args.self_test:
        raise SystemExit(self_test(binary))
    require(args.out is not None, '--out is required for a campaign run')
    result = run_campaign(binary, args.matrix, args.profile, args.out)
    if result['status'] == 'pass':
        print(f"{PASS_LINE} profile={args.profile} cases={len(result['cases'])} "
              f"wall={result['total_wall_seconds']:.1f}s")
        raise SystemExit(0)
    print('FAIL envelope operating envelope', file=sys.stderr)
    for problem in result['problems']:
        print('  - ' + problem, file=sys.stderr)
    raise SystemExit(1)


if __name__ == '__main__':
    main()
