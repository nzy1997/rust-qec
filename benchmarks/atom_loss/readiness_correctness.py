"""Bounded, independently checked correctness suite for the proposed envelope support domain (issue #714).

Every support promise in docs/envelope-support.json gets a stated evidence
level here. Expected answers come from the independent Stim oracle (private
loss histories, Stim-derived DEM and Pauli fault propagation, exact min-plus
enumeration of the validated native representation) — never from cached native
predictions. The decoder under test only sees the public
``measurements_blinded`` bundle; scoring stays private.

Smoke profile (a few minutes after build):

    python3 -m benchmarks.atom_loss.readiness_correctness \
      --binary target/release/rustqec \
      --matrix docs/envelope-support.json \
      --profile smoke \
      --out drafts/envelope-readiness/correctness.json

The full profile widens fault injection and randomized differential sampling:

    python3 -m benchmarks.atom_loss.readiness_correctness \
      --binary target/release/rustqec \
      --matrix docs/envelope-support.json \
      --profile full \
      --out drafts/envelope-readiness/correctness-full.json
"""
from .shot_data import require
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import numpy as np

from . import reference, chain_reference
from .run import ROOT, save, digest, generate

SCHEMA = 'rustqec.envelope-readiness-correctness.v1'
PASS_LINE = 'PASS envelope readiness correctness'
FIXTURE = ROOT/'benchmarks/atom_loss/fixtures/midswap_d3_r2.stim'
MATRIX_PATH = ROOT/'docs/envelope-support.json'
MAX_EXACT_STATES = 1 << 18
REQUIRED_HISTORY_CATEGORIES = (
    'no_loss', 'multiple_simultaneous_losses', 'loss_in_different_rounds', 'reset_restoring_wire')
REQUIRED_CHANNELS = ('X_ERROR', 'DEPOLARIZE1', 'DEPOLARIZE2')
INDEPENDENT_SOURCE = ('independent Stim oracle: private loss histories lowered without RustQEC code, '
                      'Stim-derived DEM and Pauli fault propagation, exact min-plus enumeration')


def sha_text(text):
    return hashlib.sha256(text.encode()).hexdigest()


def checkout_revision():
    try:
        result = subprocess.run(['git', 'rev-parse', 'HEAD'], capture_output=True,
                                text=True, cwd=ROOT, check=False)
    except OSError:
        return None
    return result.stdout.strip() or None


def binary_record(binary):
    version = subprocess.run([str(binary), '--version'], capture_output=True, text=True).stdout.strip()
    return {'path': str(binary), 'sha256': digest(binary), 'version': version}


def advertised_decoders(binary):
    result = subprocess.run([str(binary), 'capabilities', '--format', 'json'],
                            capture_output=True, text=True, check=False)
    require(result.returncode == 0, 'capabilities failed: ' + result.stderr.strip())
    commands = json.loads(result.stdout)['commands']
    decode = next((c for c in commands if c.get('name') == 'decode'), {})
    return list(decode.get('decoders', []))


def resolve_exporters(binary):
    examples = binary.parent/'examples'
    exporters = {'matching': examples/'export_matching_benchmark',
                 'oracle': examples/'export_decoder_oracle'}
    missing = [str(p) for p in exporters.values() if not p.is_file()]
    require(not missing,
            'missing exporter example binaries: ' + ', '.join(missing) +
            '; build with: cargo build --release --locked -p rustqec-cli --features benchmark-tools,ilp'
            ' --bin rustqec --example export_matching_benchmark --example export_decoder_oracle')
    return exporters


def loss_events(ops):
    """Ordered per-qubit LOSS/reset/terminal events with TICK segment indices."""
    events = defaultdict(list)
    width = 0
    segment = 0
    for position, op in enumerate(ops):
        if op.name == 'TICK':
            segment += 1
        elif op.name == 'LOSS':
            for target in op.targets:
                events[int(target)].append({'kind': 'loss', 'index': width,
                                            'segment': segment, 'position': position})
                width += 1
        elif op.name in ('MRL', 'MRZL', 'R', 'RZ'):
            for target in op.targets:
                events[int(target)].append({'kind': 'reset', 'segment': segment, 'position': position})
        elif op.name in ('ML', 'MZL'):
            for target in op.targets:
                events[int(target)].append({'kind': 'terminal', 'segment': segment, 'position': position})
    return events, width


def targeted_histories(text):
    """Named private loss histories exercising the required coverage categories."""
    ops = reference.parse(text)
    events, width = loss_events(ops)
    histories = []

    def record(name, indices, category):
        history = np.zeros(width, dtype=bool)
        for index in indices:
            history[index] = True
        histories.append({'name': name, 'indices': sorted(indices),
                          'category': category, 'history': history})

    record('no_loss', [], 'no_loss')
    if width:
        record('single_early', [0], 'single_loss')
        record('single_middle', [width//2], 'single_loss')
        record('single_late', [width - 1], 'single_loss')
    by_segment = defaultdict(dict)
    for qubit, qevents in events.items():
        for event in qevents:
            if event['kind'] == 'loss':
                by_segment[event['segment']].setdefault(qubit, event['index'])
    for segment in sorted(by_segment):
        chosen = sorted(by_segment[segment].values())
        if len(chosen) >= 3:
            record('multi_simultaneous', chosen[:3], 'multiple_simultaneous_losses')
            break
    rounds_done = reset_done = False
    for qubit, qevents in sorted(events.items()):
        losses = [e for e in qevents if e['kind'] == 'loss']
        resets = [e for e in qevents if e['kind'] == 'reset']
        if not rounds_done and len({e['segment'] for e in losses}) >= 2:
            record(f'different_rounds_q{qubit}', [losses[0]['index'], losses[-1]['index']],
                   'loss_in_different_rounds')
            rounds_done = True
        if not reset_done:
            for first in losses:
                mid = [r for r in resets if r['position'] > first['position']]
                later = [b for b in losses if mid and b['position'] > mid[0]['position']]
                if mid:
                    indices = [first['index']] + ([later[0]['index']] if later else [])
                    record(f'reset_restore_q{qubit}', indices, 'reset_restoring_wire')
                    reset_done = True
                    break
    return histories


def single_fault_rows(lowered):
    """Every single Pauli fault at every noise instruction of a lowered circuit."""
    rows = []
    channels = set()
    for position, instruction in enumerate(lowered):
        if instruction.name not in ['X_ERROR', 'Y_ERROR', 'Z_ERROR', 'DEPOLARIZE1', 'DEPOLARIZE2']:
            continue
        channels.add(instruction.name)
        targets = [t.value for t in instruction.targets_copy()]
        if instruction.name == 'DEPOLARIZE2':
            faults = [[(a, q), (b, r)] for q, r in zip(targets[::2], targets[1::2])
                      for a in 'IXYZ' for b in 'IXYZ' if a + b != 'II']
        else:
            paulis = 'XYZ' if instruction.name == 'DEPOLARIZE1' else instruction.name[0]
            faults = [[(pauli, q)] for q in targets for pauli in paulis]
        for fault in faults:
            injected = lowered[:position].without_noise()
            for pauli, qubit in fault:
                if pauli != 'I':
                    injected.append(pauli, [qubit])
            injected += lowered[position + 1:].without_noise()
            rows.append(injected.reference_sample())
    return rows, channels


def randomized_rows(ops, width, count, seed, loss_rate=0.15):
    """Reproducible randomized differential rows with a recorded seed."""
    rng = np.random.default_rng(seed)
    rows = []
    for _ in range(count):
        history = rng.random(width) < loss_rate
        lowered = reference.lower(ops, history)
        rows.append(lowered.without_noise().reference_sample())
    return rows


def build_rows(text, *, seed, fault_histories=('no_loss',), random_histories=8):
    """Targeted-history witnesses plus injected faults and randomized rows."""
    ops = reference.parse(text)
    _, width = loss_events(ops)
    histories = targeted_histories(text)
    rows, channels, row_provenance = [], set(), []
    targeted_rows = {}
    for entry in histories:
        lowered = reference.lower(ops, entry['history'])
        noiseless = lowered.without_noise().reference_sample()
        rows.append(noiseless)
        targeted_rows[entry['name']] = noiseless
        if entry['name'] in fault_histories:
            faulted, used = single_fault_rows(lowered)
            rows += faulted
            channels |= used
    rows += randomized_rows(ops, width, random_histories, seed)
    merged = np.unique(np.array(rows, dtype=np.uint8), axis=0)
    exercised = {name: any(np.array_equal(row, kept) for kept in merged)
                 for name, row in targeted_rows.items()}
    return merged, histories, exercised, channels


def native_costs_per_pattern(model, candidates, patterns, count):
    indices = np.arange(1 << (count + 1))
    base = chain_reference.costs(
        [(chain_reference.native_mask(e, count), e['weight']) for e in model['independent_effects']],
        count + 1)
    per_pattern = {}
    for pattern in set(patterns):
        values = base.copy()
        for i in pattern:
            values = np.minimum.reduce([values[indices ^ candidate] for candidate in candidates[i]])
        per_pattern[pattern] = values
    return per_pattern


def matching_costs_per_pattern(edges, loss_edges, patterns, count):
    mean = np.mean([e['weight'] for e in edges])
    per_pattern = {}
    for pattern in set(patterns):
        active = set().union(*(loss_edges[i] for i in pattern)) if pattern else set()
        per_pattern[pattern] = chain_reference.costs(
            [(e['mask'], e['factor']*mean if i in active else e['weight'])
             for i, e in enumerate(edges)], count + 1)
    return per_pattern


def check_predictions(predictions, choices):
    """Predictions must stay inside the independently derived allowed optima."""
    require(len(predictions) == len(choices)
            and all(type(v) is int and v in (0, 1) for v in predictions),
            'Incomplete or invalid decoder predictions')
    rejected = [i for i, (value, answers) in enumerate(zip(predictions, choices, strict=True))
                if value not in answers]
    singleton = [i for i, answers in enumerate(choices) if len(answers) == 1]
    require(bool(singleton), 'Oracle has no unique optimum witness')
    flipped = predictions.copy()
    flipped[singleton[0]] ^= 1
    half = len(choices)//2
    return {'predictions': predictions, 'checked_rows': len(choices), 'rejected_rows': rejected,
            'unique_optimum_rows': len(singleton), 'allowed_tie_rows': len(choices) - len(singleton),
            'flipped_prediction_rejected': flipped[singleton[0]] not in choices[singleton[0]],
            'placeholder_invariance': predictions[:half] == predictions[half:],
            'prediction_sha256': hashlib.sha256(bytes(predictions)).hexdigest()}


def decode_public(binary, decoder, public, work, timeout=300):
    out = work/(decoder + '.b8')
    result = subprocess.run([str(binary), 'decode', '--decoder', decoder, '--dataset', str(public),
                             '--out', str(out), '--stats-out', str(work/(decoder + '.json'))],
                            capture_output=True, text=True, timeout=timeout, check=False)
    require(result.returncode == 0,
            f'{decoder} decode failed: {result.stderr.strip() or result.stdout.strip()}')
    return list(out.read_bytes())


def oracle_case(name, text, *, binary, exporters, work, seed, source,
                end_to_end=True, fault_histories=('no_loss',), random_histories=8):
    """One independently checked case; returns its evidence record."""
    record = {'name': name, 'source': source, 'circuit_sha256': sha_text(text),
              'expected_answer_source': INDEPENDENT_SOURCE if end_to_end else
              'independent Stim compiler-output validation only (no end-to-end enumeration)',
              'evidence_level': 'independent-end-to-end' if end_to_end else 'compiler-output-only',
              'seed': seed}
    case_work = work/name
    case_work.mkdir()
    circuit, probes, effects, edges, candidates, loss_edges, probe_count = \
        chain_reference.independent_model(text, require_shape=None)
    count = circuit.num_detectors
    record['detectors'] = count
    record['observables'] = circuit.num_observables
    rows, histories, exercised, channels = build_rows(
        text, seed=seed, fault_histories=fault_histories, random_histories=random_histories)
    record['histories'] = [{'name': h['name'], 'category': h['category'],
                            'active_loss_indices': h['indices'],
                            'exercised': exercised[h['name']]} for h in histories]
    record['noise_channels_exercised'] = sorted(channels)
    require(all(exercised.values()),
            f'{name}: targeted histories dropped by deduplication: '
            + ', '.join(k for k, v in exercised.items() if not v))
    flags = [p['flag'] for p in probes]
    alternate = rows.copy()
    for flag in flags:
        alternate[alternate[:, flag] == 1, flag + 1] ^= 1
    full_rows = np.concatenate([rows, alternate])
    canonical = full_rows.copy()
    for flag in flags:
        canonical[canonical[:, flag] == 1, flag + 1] = 1
    detections = circuit.compile_m2d_converter().convert(
        measurements=canonical.astype(bool), separate_observables=True)[0]
    patterns = [tuple(np.flatnonzero(row[flags])) for row in full_rows]
    syndromes = [sum(int(v) << i for i, v in enumerate(row)) for row in detections]
    record['rows'] = len(full_rows)
    record['distinct_rows_before_placeholders'] = len(rows)
    record['placeholder_pairs'] = len(rows)
    record['patterns'] = len(set(patterns))
    record['measurement_sha256'] = hashlib.sha256(full_rows.tobytes()).hexdigest()
    chain_reference.write_bundle(case_work/'public', text, full_rows, circuit)
    public = case_work/'public'
    manifest = json.loads((public/'manifest.json').read_text())
    require(manifest['mode'] == 'measurements_blinded' and not (public/'answers.b8').exists(),
            f'{name}: public bundle must stay blinded (public/private scoring separation)')
    graph_path = case_work/'graph.json'
    subprocess.run([str(exporters['matching']), str(public), str(graph_path)],
                   check=True, capture_output=True)
    graph = json.loads(graph_path.read_text())
    require(graph['syndromes'] == detections.astype(int).tolist(),
            f'{name}: independent m2d mismatch')
    require(graph['losses'] == [list(p) for p in patterns],
            f'{name}: independent visible-loss mapping mismatch')
    model_path = case_work/'models.json'
    subprocess.run([str(exporters['oracle']), str(public), str(model_path)],
                   check=True, capture_output=True)
    model = json.loads(model_path.read_text())
    chain_reference.validate_model(model, effects, candidates, count)
    record['compiler_output_check'] = 'pass'
    record['independent_effects'] = len(effects)
    record['independent_graph_edges'] = len(edges)
    record['stim_pauli_probes'] = probe_count
    if not end_to_end:
        record['exclusion'] = (f'exact enumeration over 2^{count + 1} detector/logical states is '
                               'infeasible for this case; compiler output is independently validated '
                               'but end-to-end replay is a separate evidence level and is not claimed')
        return record
    require(count + 1 <= 18, f'{name}: exact enumeration budget exceeded')
    matching_values = matching_costs_per_pattern(edges, loss_edges, patterns, count)
    mle_values = native_costs_per_pattern(model, candidates, patterns, count)
    expected = {
        'envelope-matching': [chain_reference.allowed(matching_values[p], s, count)
                              for p, s in zip(patterns, syndromes)],
        'envelope-mle': [chain_reference.allowed(mle_values[p], s, count)
                         for p, s in zip(patterns, syndromes)],
    }
    record['allowed_answers'] = {name_: [sorted(a) for a in choices]
                                 for name_, choices in expected.items()}
    backends = {}
    mismatches = []
    for decoder, choices in expected.items():
        predictions = decode_public(binary, decoder, public, case_work)
        outcome = check_predictions(predictions, choices)
        backends[decoder] = outcome
        mismatches += [{'decoder': decoder, 'row': row, 'predicted': predictions[row],
                        'allowed': sorted(choices[row])} for row in outcome['rejected_rows']]
        require(outcome['flipped_prediction_rejected'],
                f'{name}: flipped known-answer control was not rejected by the oracle')
        require(outcome['placeholder_invariance'],
                f'{name}: placeholder bits changed a {decoder} prediction')
    record['backends'] = backends
    record['mismatches'] = mismatches
    record['status'] = 'pass' if not mismatches else 'fail'
    record['public_private_separation'] = True
    return record


def generate_circuit(binary, work, distance, rounds, loss, pauli=0.001):
    path = work/f'midswap_d{distance}_r{rounds}.stim'
    generate(binary, path, distance, rounds, loss, pauli)
    # Exercise the X_ERROR channel as well: --noise only sets depolarization.
    from .run import checked
    checked([binary, 'circuit', 'gen', '--code', 'surface_code', '--task',
             'rotated_memory_z_midswap', '--distance', distance, '--rounds', rounds,
             '--noise', pauli, '--operation-loss-probability', loss,
             '--measurement-loss-probability', loss,
             '--before-measure-flip-probability', pauli,
             '--after-reset-flip-probability', pauli, '--out', path])
    return path.read_text(), (f'rustqec circuit gen --code surface_code --task '
                              f'rotated_memory_z_midswap --distance {distance} --rounds {rounds} '
                              f'--noise {pauli} --operation-loss-probability {loss} '
                              f'--measurement-loss-probability {loss} '
                              f'--before-measure-flip-probability {pauli} '
                              f'--after-reset-flip-probability {pauli}')


PROFILES = {
    'smoke': {'random_histories': 8, 'fault_histories': ('no_loss',)},
    'full': {'random_histories': 64,
             'fault_histories': ('no_loss', 'multi_simultaneous', 'different_rounds')},
}


def run_suite(binary, matrix_path, profile, out_path=None):
    require(profile in PROFILES, f'unknown profile: {profile}')
    params = PROFILES[profile]
    matrix = json.loads(Path(matrix_path).read_text())
    require(matrix.get('schema_version') == 'rustqec.envelope-support.v1',
            'readiness correctness requires the envelope support matrix schema')
    decoders = set(matrix['decoders'])
    missing = decoders - set(advertised_decoders(binary))
    require(not missing,
            'binary does not advertise matrix decoder(s): ' + ', '.join(sorted(missing)))
    exporters = resolve_exporters(binary)
    cases = []
    exclusions = []
    with tempfile.TemporaryDirectory(prefix='envelope-readiness-correctness-') as tmp:
        work = Path(tmp)
        fault_histories = tuple(h for h in params['fault_histories'])
        fixture_text = FIXTURE.read_text()
        cases.append(oracle_case(
            'midswap-d3-r2-fixture', fixture_text, binary=binary, exporters=exporters,
            work=work, seed=714_001, source=str(FIXTURE.relative_to(ROOT)),
            fault_histories=fault_histories,
            random_histories=params['random_histories']))
        for distance, rounds, seed in ((3, 1, 714_002),):
            text, command_text = generate_circuit(binary, work, distance, rounds, 0.01)
            cases.append(oracle_case(
                f'midswap-d{distance}-r{rounds}-generated', text, binary=binary,
                exporters=exporters, work=work, seed=seed, source=command_text,
                fault_histories=fault_histories,
                random_histories=max(4, params['random_histories']//2)))
        text, command_text = generate_circuit(binary, work, 3, 3, 0.01)
        case = oracle_case(
            'midswap-d3-r3-generated', text, binary=binary, exporters=exporters,
            work=work, seed=714_003, source=command_text, end_to_end=False,
            fault_histories=(), random_histories=4)
        cases.append(case)
        exclusions.append({'case': case['name'], 'reason': case['exclusion']})
    exclusions.append({
        'case': 'conventional-family-end-to-end',
        'reason': 'conventional circuits are isolated checked examples in the support matrix; '
                  'envelope-mle rejects the pinned fixture at the candidate limit, so no '
                  'conventional MLE end-to-end domain is claimed'})
    coverage = defaultdict(int)
    channels = set()
    points = []
    mismatches = []
    for case in cases:
        for history in case['histories']:
            if history['exercised']:
                coverage[history['category']] += 1
        channels |= set(case['noise_channels_exercised'])
        points.append({'case': case['name'], 'detectors': case['detectors'],
                       'evidence_level': case['evidence_level']})
        coverage['placeholder_pairs'] += case['placeholder_pairs']
        coverage['rows'] += case['rows']
        if case['evidence_level'] == 'independent-end-to-end':
            for outcome in case['backends'].values():
                coverage['allowed_tie_rows'] += outcome['allowed_tie_rows']
                coverage['checked_rows'] += outcome['checked_rows']
        mismatches += case.get('mismatches', [])
    coverage_report = {
        'history_categories': {category: coverage[category]
                               for category in REQUIRED_HISTORY_CATEGORIES},
        'single_loss_histories': coverage['single_loss'],
        'noise_channels': sorted(channels),
        'circuit_points': points,
        'placeholder_pairs': coverage['placeholder_pairs'],
        'total_rows': coverage['rows'],
        'checked_rows': coverage['checked_rows'],
        'allowed_tie_rows': coverage['allowed_tie_rows'],
        'randomized_differential_seeds': [case['seed'] for case in cases],
    }
    problems = validate_coverage(coverage_report, cases)
    problems += [f"mismatch: {m}" for m in mismatches]
    status = 'pass' if not problems else 'fail'
    result = {
        'schema_version': SCHEMA,
        'profile': profile,
        'checkout_revision': checkout_revision(),
        'binary': binary_record(binary),
        'matrix': {'path': str(matrix_path), 'sha256': digest(Path(matrix_path)),
                   'source_revision': matrix.get('applies_to', {}).get('source_revision')},
        'method': ('Independent Stim lowering of private loss histories; Stim-derived DEM, '
                   'primitive Pauli propagation and loss candidates validated against native '
                   'compiler output; exact min-plus enumeration of allowed optima per decoder '
                   'objective. Matching is checked against its minimum-weight objective with '
                   'explicit allowed ties; MLE against its fault-configuration objective. '
                   'Neither check claims logical-class Bayes optimality nor that every noisy '
                   'shot must decode correctly.'),
        'cases': cases,
        'coverage': coverage_report,
        'exclusions': exclusions,
        'mismatches': mismatches,
        'problems': problems,
        'status': status,
    }
    if out_path is not None:
        save(out_path, result)
    return result


def validate_coverage(coverage, cases):
    problems = []
    for category, count in coverage['history_categories'].items():
        if count <= 0:
            problems.append(f'missing required loss-history coverage: {category}')
    missing_channels = [c for c in REQUIRED_CHANNELS if c not in coverage['noise_channels']]
    if missing_channels:
        problems.append('missing required noise-channel coverage: ' + ', '.join(missing_channels))
    if len(coverage['circuit_points']) < 2:
        problems.append('missing required circuit size/round coverage')
    if coverage['placeholder_pairs'] <= 0:
        problems.append('missing placeholder-invariance coverage')
    end_to_end = [c for c in cases if c['evidence_level'] == 'independent-end-to-end']
    if not end_to_end:
        problems.append('no independent end-to-end case ran')
    for case in cases:
        source = case.get('expected_answer_source', '')
        if case['evidence_level'] == 'independent-end-to-end' and not source.startswith('independent'):
            problems.append(f"{case['name']}: expected answers are not independently derived ({source!r}); "
                            'cached native predictions cannot satisfy the independent checks')
        if case.get('status') == 'fail':
            problems.append(f"{case['name']}: case failed")
    return problems


def self_test(binary):
    """Prove the runner rejects corrupted oracle inputs, flipped answers and removed coverage."""
    exporters = resolve_exporters(binary)
    matrix = json.loads(MATRIX_PATH.read_text())
    mini = next(c for c in matrix['controls'] if c['id'] == 'mini-circuit-known-answer')
    text = mini['dataset']['circuit']
    observations = []

    with tempfile.TemporaryDirectory(prefix='envelope-correctness-selftest-') as tmp:
        work = Path(tmp)
        case = oracle_case('self-test-mini', text, binary=binary, exporters=exporters,
                           work=work, seed=714_900, source='matrix inline control',
                           random_histories=2)
        require(case['status'] == 'pass', 'self-test baseline case must pass')

        # Tamper 1: corrupt the compiled loss-to-edge mapping and a Pauli effect.
        model = json.loads((work/'self-test-mini'/'models.json').read_text())
        circuit, probes, effects, edges, candidates, loss_edges, _ = \
            chain_reference.independent_model(text, require_shape=None)
        count = circuit.num_detectors
        rejected = []
        for name, mutate in (
                ('pauli_effect_weight', lambda m: m['independent_effects'][0].__setitem__('weight', m['independent_effects'][0]['weight'] + .25)),
                ('loss_candidate_mapping', lambda m: next(es for es in m['loss_candidates'] if len(es) > 1).pop())):
            defective = json.loads(json.dumps(model))
            mutate(defective)
            try:
                chain_reference.validate_model(defective, effects, candidates, count)
                rejected.append(False)
            except ValueError:
                rejected.append(True)
        first = all(rejected)
        observations.append({'mutation': 'corrupted-compiled-mapping-or-pauli-effect', 'rejected': first})

        # Tamper 2: flip a logical prediction on a known-answer row.
        outcome = case['backends']['envelope-mle']
        flipped = outcome['predictions'].copy()
        unique_row = next(i for i, answers in enumerate(case['allowed_answers']['envelope-mle'])
                          if len(answers) == 1)
        flipped[unique_row] ^= 1
        checked = check_predictions(flipped, [set(a) for a in case['allowed_answers']['envelope-mle']])
        second = bool(checked['rejected_rows'])
        observations.append({'mutation': 'flipped-known-answer-prediction', 'rejected': second,
                             'rejected_rows': checked['rejected_rows']})

        # Tamper 3: remove the multi-loss coverage cases.
        coverage = {'history_categories': {'no_loss': 1, 'multiple_simultaneous_losses': 0,
                                           'loss_in_different_rounds': 1, 'reset_restoring_wire': 1},
                    'noise_channels': list(REQUIRED_CHANNELS),
                    'circuit_points': [{'case': 'a'}, {'case': 'b'}],
                    'placeholder_pairs': 1}
        third = any('multiple_simultaneous_losses' in p
                    for p in validate_coverage(coverage, []))
        observations.append({'mutation': 'multi-loss-coverage-removed', 'rejected': third})

        # Tamper 4: cached native predictions as both expected and observed.
        fake_case = {'name': 'cached', 'evidence_level': 'independent-end-to-end',
                     'expected_answer_source': 'native-cli prediction cache',
                     'status': 'pass', 'histories': [], 'noise_channels_exercised': [],
                     'placeholder_pairs': 1, 'detectors': 2}
        fourth = any('cached native predictions' in p or 'not independently derived' in p
                     for p in validate_coverage(coverage_ok(), [fake_case]))
        observations.append({'mutation': 'cached-native-predictions-as-expected', 'rejected': fourth})

    passed = all(o['rejected'] for o in observations)
    print(json.dumps({'self_test_mutations': observations}, indent=2))
    if passed:
        print('PASS envelope readiness correctness self-test')
        return 0
    print('FAIL envelope readiness correctness self-test: a mutation was not rejected', file=sys.stderr)
    return 1


def coverage_ok():
    return {'history_categories': {category: 1 for category in REQUIRED_HISTORY_CATEGORIES},
            'noise_channels': list(REQUIRED_CHANNELS),
            'circuit_points': [{'case': 'a'}, {'case': 'b'}],
            'placeholder_pairs': 1}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/release/rustqec')
    parser.add_argument('--matrix', type=Path, default=MATRIX_PATH)
    parser.add_argument('--profile', default='smoke', choices=sorted(PROFILES))
    parser.add_argument('--out', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    binary = args.binary.resolve()
    require(binary.is_file(), f'missing binary: {binary}')
    if args.self_test:
        raise SystemExit(self_test(binary))
    result = run_suite(binary, args.matrix, args.profile, args.out)
    if result['status'] == 'pass':
        checked = result['coverage']['checked_rows']
        print(f"{PASS_LINE} cases={len(result['cases'])} rows={result['coverage']['total_rows']} "
              f"checked_rows={checked}")
        raise SystemExit(0)
    print('FAIL envelope readiness correctness', file=sys.stderr)
    for problem in result['problems']:
        print('  - ' + str(problem), file=sys.stderr)
    raise SystemExit(1)


if __name__ == '__main__':
    main()
