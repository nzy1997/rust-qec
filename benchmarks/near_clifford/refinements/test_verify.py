"""Synthetic corruption probes for the evidence validator, not physics evidence."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('refinement_verify_tests', HERE / 'verify.py')
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)
run = validator.run


def corpus():
    selected = ['rank_12', 'brick_12_193_3']
    matrix = [list(row) for row in run.matrix() if row[0] in selected]
    old = run.load(run.BASE / 'scale/run.py', 'refinement_test_inputs')
    result = dict(schema='near-clifford.refinements.v1', completed_utc='synthetic', quick=False,
        pairs=3, repetitions=3, entry_sha256=run.sha(HERE / 'run.py'),
        harness_sha256=hashlib.sha256(run.driver().encode()).hexdigest(),
        runner_sha256=run.sha(run.BASE / 'scale/run.py'), counter_names=old.COUNTERS,
        snapshot_names=old.SNAPSHOT, input_hashes={str(p.relative_to(run.ROOT)): run.sha(p) for p in [
            run.BASE / 'scale/run.py', run.BASE / 'scale/main.rs', run.BASE / 'scale/Cargo.unified.lock',
            run.BASE / 'entangled/run.py', run.BASE / 'entangled/fixtures.rs']},
        fixtures_sha256={p.name: run.sha(p) for p in (run.BASE / 'scale/fixtures').iterdir()},
        selected_matrix=matrix, matrix=matrix, selection_names=selected,
        verification_results={}, verification={}, sources={}, diagnostics={}, cases=[])
    for label in ['baseline', 'candidate']:
        meta = dict(revision=('1' if label == 'baseline' else '2') * 40, has_cache_ledger=label == 'candidate',
            lock_sha256=run.sha(run.BASE / 'scale/Cargo.unified.lock'))
        meta['diagnostic'] = meta.copy()
        result['sources'][label] = meta
        result['diagnostics'][label] = {}
        for name in selected:
            result['diagnostics'][label][name] = dict(fixture=name, semantic_verification='pass',
                counters=[0]*8, warmup_counters=[0]*8, initial=[12, 4096, 1, 10, 1],
                after_warmup=[12, 4096, 1, 10, 1], after_probe=[12, 4096, 1, 10, 1],
                cache_reservation=[1, 64*1024*1024, 1, 0] if label == 'candidate' else None)
    physics = dict(reference_fixture='brick_8_8_3', full_width_oracle=False,
        born_probability_comparisons=72, manual_executor_seed_equality=False, terminal_support_checks=3,
        reference_peak_rank=8, minimum_one_qubit_purity=0.5, status='pass', trajectories=3)
    payloads = {name: dict(fixture=name, continuation=123, circuit='synthetic circuit '+name,
        physics=physics if name.startswith('brick') else None,
        shots=[dict(m=[False]*(193 if name.startswith('brick') else 12), d=[], o=[]) for _ in range(256)])
        for name in selected}
    for label in ['baseline', 'candidate', 'baseline-diagnostic', 'candidate-diagnostic']:
        result['verification_results'][label] = copy.deepcopy(payloads)
    reseal(result)
    for index, (name, shots) in enumerate(matrix):
        runs = []
        for pair in range(3):
            values = {label: dict(fixture=name, circuit=payloads[name]['circuit'], measurements=[dict(shots=shots,
                **{mode: dict(raw_ns=[100, 105, 120], median_ns=105) for mode in validator.MODES})])
                for label in ['baseline', 'candidate']}
            values['order'] = ['baseline', 'candidate'] if (index+pair)%2 == 0 else ['candidate', 'baseline']
            runs.append(values)
        result['cases'].append(dict(fixture=name, shots=shots, runs=runs))
    return result


def reseal(result):
    for name, payload in result['verification_results']['baseline'].items():
        result['verification'][name] = dict(status='pass', output_sha256=hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest())


class EvidenceTests(unittest.TestCase):
    def test_targeted_cannot_claim_full_and_requires_all_selected_shot_counts(self):
        result = corpus()
        self.assertIn('targeted', validator.verify(result, allow_subset=True))
        with self.assertRaisesRegex(ValueError, 'targeted'): validator.verify(result)
        for key in ['selected_matrix', 'matrix', 'cases']: result[key] = result[key][:-1]
        with self.assertRaisesRegex(ValueError, 'matrix'): validator.verify(result, allow_subset=True)

    def test_resealed_physics_lies_and_missing_records_are_rejected(self):
        defects = [('full_width_oracle', True), ('reference_fixture', 'brick_12_193_3'),
                   ('born_probability_comparisons', 1), ('minimum_one_qubit_purity', 1.0),
                   ('manual_executor_seed_equality', True), ('reference_peak_rank', 193),
                   ('status', 'skipped'), ('trajectories', 0)]
        for key, value in defects:
            result = corpus()
            for rows in result['verification_results'].values(): rows['brick_12_193_3']['physics'][key] = value
            reseal(result)
            with self.subTest(key=key), self.assertRaises(ValueError): validator.verify(result, allow_subset=True)
        result = corpus()
        for rows in result['verification_results'].values(): rows['brick_12_193_3']['shots'][0]['m'].pop()
        reseal(result)
        with self.assertRaisesRegex(ValueError, 'record length'): validator.verify(result, allow_subset=True)

    def test_both_timed_circuits_must_bind_verified_circuit(self):
        result = corpus()
        for label in ['baseline', 'candidate']: result['cases'][0]['runs'][0][label]['circuit'] += '\nX 0'
        with self.assertRaisesRegex(ValueError, 'verified circuit'): validator.verify(result, allow_subset=True)

    def test_missing_pairs_medians_and_optional_over_budget_cache_are_rejected(self):
        result = corpus(); result['cases'][0]['runs'].pop()
        with self.assertRaisesRegex(ValueError, 'process pairs'): validator.verify(result, allow_subset=True)
        result = corpus(); result['cases'][0]['runs'][0]['baseline']['measurements'][0]['warm_prepared_flat']['median_ns'] = 100
        with self.assertRaisesRegex(ValueError, 'median'): validator.verify(result, allow_subset=True)
        result = corpus(); result['diagnostics']['candidate']['rank_12']['cache_reservation'][0] = 64*1024*1024+1
        with self.assertRaisesRegex(ValueError, 'over-budget'): validator.verify(result, allow_subset=True)

    def test_complete_disk_inventory_detects_dependency_and_harness_tampering(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder/'other.rs').write_text('production dependency')
            before = run.inventory_digest(run.disk_inventory(folder))
            (folder/'other.rs').write_text('changed dependency')
            self.assertNotEqual(before, run.inventory_digest(run.disk_inventory(folder)))
            expected = run.harness_inputs(folder/'harness', b'oracle')
            for name, digest in expected.items():
                self.assertEqual(set(digest), {'sha256', 'executable'}, name)
            self.assertTrue({'Cargo.lock', 'Cargo.toml', 'fixtures.rs', 'oracle.rs', 'main.rs'} <= expected.keys())

    def test_relative_binary_directory_and_resealed_lock_tampering(self):
        result = corpus()
        with tempfile.TemporaryDirectory() as temporary:
            scratch = Path(temporary)
            for label, source in result['sources'].items():
                for suffix, metadata in [('', source), ('-diagnostic', source['diagnostic'])]:
                    folder = scratch / (label + suffix)
                    for name, raw in {'rstim/src/near_clifford.rs': b'near', 'rstim/src/sim/tableau.rs': b'tableau'}.items():
                        path = folder / 'source' / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
                    harness = folder / 'harness'; harness.mkdir()
                    inputs = run.harness_inputs(harness, b'oracle')
                    (harness / 'main.rs').write_text(run.driver())
                    (harness / 'fixtures.rs').write_bytes((run.BASE / 'entangled/fixtures.rs').read_bytes())
                    (harness / 'oracle.rs').write_bytes(b'oracle')
                    (harness / 'Cargo.lock').write_bytes((run.BASE / 'scale/Cargo.unified.lock').read_bytes())
                    scale = (run.BASE / 'scale/run.py').read_text()
                    # Use the actual build template, not the verifier's expected hash.
                    start = scale.index("manifest=\'\'\'") + len("manifest=\'\'\'")
                    end = scale.index("\'\'\'+f", start)
                    (harness / 'Cargo.toml').write_text(scale[start:end] + f'rstim = {{ path = "{folder}/source/rstim" }}\n')
                    for path in (run.BASE / 'scale/fixtures').iterdir():
                        out = harness / 'fixtures' / path.name; out.parent.mkdir(exist_ok=True); out.write_bytes(path.read_bytes())
                    binary = folder / 'near-clifford-scale'; binary.write_bytes(b'synthetic binary')
                    metadata.update(binary_sha256=run.sha(binary), near_clifford_source_sha256=run.sha(folder/'source/rstim/src/near_clifford.rs'),
                        tableau_source_sha256=run.sha(folder/'source/rstim/src/sim/tableau.rs'), oracle_sha256=run.sha(harness/'oracle.rs'),
                        source_inventory_sha256=run.inventory_digest(run.disk_inventory(folder/'source')),
                        harness_inventory_sha256=run.inventory_digest(run.disk_inventory(harness)))
                    self.assertEqual(run.disk_inventory(harness), inputs)
            relative = Path(os.path.relpath(scratch))
            self.assertIn('targeted', validator.verify(result, relative, allow_subset=True))
            lock = scratch/'candidate/harness/Cargo.lock'; lock.write_text('resealed fake dependency lock')
            result['sources']['candidate']['harness_inventory_sha256'] = run.inventory_digest(run.disk_inventory(lock.parent))
            with self.assertRaisesRegex(ValueError, 'harness inputs'): validator.verify(result, relative, allow_subset=True)

    def test_diagnostic_driver_calls_sampler_accessor(self):
        self.assertIn('sampler.benchmark_cache_reservation()', run.driver())
        self.assertNotIn('rstim::near_clifford::benchmark_cache_reservation()', run.driver())


if __name__ == '__main__': unittest.main()
