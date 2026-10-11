"""Finite semantics, actual benchmark schemas and process lifecycle regressions."""
import collections
import copy
import importlib.util
import json
import hashlib
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('cdf_pair',HERE/'run.py')
pilot=importlib.util.module_from_spec(spec);spec.loader.exec_module(pilot)
outer_spec=importlib.util.spec_from_file_location('cdf_outer',HERE/'run_retained.py')
outer=importlib.util.module_from_spec(outer_spec);outer_spec.loader.exec_module(outer)

prepare_spec=importlib.util.spec_from_file_location('cdf_prepare',HERE/'prepare.py')
prepare=importlib.util.module_from_spec(prepare_spec);prepare_spec.loader.exec_module(prepare)
evidence=prepare.evidence
verify_spec=importlib.util.spec_from_file_location('cdf_verify',HERE/'verify.py')
verifier=importlib.util.module_from_spec(verify_spec);verify_spec.loader.exec_module(verifier)
fixture_spec=importlib.util.spec_from_file_location('cdf_bundle_fixture',HERE/'test_bundle_fixture.py')
fixture=importlib.util.module_from_spec(fixture_spec);fixture_spec.loader.exec_module(fixture)

class ContractTests(unittest.TestCase):
    def test_complete_standalone_bundle_relocates_and_rejects_execution_binding_corruption(self):
        """Real verifier subprocesses, synthetic bytes; no timings or native builds."""
        import shutil
        with tempfile.TemporaryDirectory() as name:
            root=Path(name)/'original';fixture.build_bundle(root)
            moved=Path(name)/'moved';shutil.copytree(root,moved);shutil.rmtree(root)
            def execute(expected, optimized=False):
                command=[sys.executable,'-I']+(['-O'] if optimized else [])+[str(HERE/'verify.py'),str(moved)]
                result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
                self.assertEqual(result.returncode,expected,result.stderr.decode())
            execute(0);execute(0,True)
            original={path.relative_to(moved):path.read_bytes() for path in moved.rglob('*') if path.is_file()}
            def rewrite(path, mutate):
                data=json.loads(path.read_text());mutate(data)
                path.write_text(json.dumps(data,separators=(',',':'))+'\n')
            def reseal():
                prep=moved/'preparation';(prep/'seal.json').unlink();evidence.seal_preparation(prep,'c'*40)
                digest=evidence.sha(prep/'seal.json')
                rewrite(moved/'output/header.json',lambda d:d.update(preparation_seal_sha256=digest))
                rewrite(moved/'output/closure.json',lambda d:d.update(preparation_seal_after=digest,events_sha256=evidence.sha(moved/'output/events.jsonl')))
                rewrite(moved/'control/closure.json',lambda d:d.update(preparation_seal_before=digest,preparation_seal_after=digest))
                (moved/'original-seal.json').write_text(json.dumps(dict(scope='SYNTHETIC TEST ONLY',files=verifier.files(moved)))+'\n')
            def wrong_worker():
                ledger=moved/'output/events.jsonl';rows=[json.loads(line) for line in ledger.read_text().splitlines()]
                rows[0]['command']=['/wrong/native-probe']
                ledger.write_text(''.join(json.dumps(row,separators=(',',':'))+'\n' for row in rows))
            def wrong_candidate_log():
                log=moved/'preparation/native-check-2.log'
                log.write_text('test result: ok. 1 passed; 0 failed; 0 ignored;\n')
                rewrite(log.with_suffix('.receipt.json'),lambda d:d.update(log_sha256=evidence.sha(log)))
            def append_candidate_record(record):
                log=moved/'preparation/native-check-2.log'
                log.write_text(log.read_text()+record+'\n')
                rewrite(log.with_suffix('.receipt.json'),lambda d:d.update(log_sha256=evidence.sha(log)))
            mutations=[
                wrong_worker,
                lambda:rewrite(moved/'output/00000.config.json',lambda d:d.update(cache_bytes=0)),
                lambda:rewrite(moved/'preparation/baseline/native-build-counts.receipt.json',lambda d:d.update(command=['echo','not a build'])),
                lambda:rewrite(moved/'preparation/native-check-1.receipt.json',lambda d:d.update(command=['echo','not a test'])),
                lambda:rewrite(moved/'preparation/native-check-1.receipt.json',lambda d:d['environment'].update(RUSTFLAGS='-C target-cpu=generic')),
                lambda:rewrite(moved/'preparation/native-check-2.receipt.json',lambda d:d.update(command=['echo','not candidate tests'])),
                lambda:rewrite(moved/'preparation/native-check-2.receipt.json',lambda d:d.update(exit_code=17)),
                lambda:rewrite(moved/'preparation/native-check-2.receipt.json',lambda d:d['environment'].update(RUSTFLAGS='-C target-cpu=generic')),
                lambda:rewrite(moved/'preparation/native-check-3.receipt.json',lambda d:d.update(command=['echo','not layout test'])),
                lambda:rewrite(moved/'preparation/native-check-3.receipt.json',lambda d:d.update(exit_code=17)),
                lambda:rewrite(moved/'preparation/native-check-3.receipt.json',lambda d:d['environment'].update(RUSTFLAGS='-C target-cpu=generic')),
                wrong_candidate_log,
                lambda:append_candidate_record('test near_clifford::compiled::coherent_packet::diagonal_projection_offset_tests::large_diagonal_projection_preserves_frozen_plane_bits_for_masks_and_pivots ... ignored'),
                lambda:append_candidate_record('test near_clifford::compiled::coherent_packet::diagonal_projection_offset_tests::large_diagonal_projection_preserves_frozen_plane_bits_for_masks_and_pivots ... FAILED'),
                lambda:append_candidate_record('test result: FAILED. 0 passed; 1 failed; 0 ignored;'),
                lambda:append_candidate_record('test extra ... '),
                lambda:append_candidate_record('test result: '),
                lambda:append_candidate_record('test broken'),
                lambda:append_candidate_record('test result:'),
                lambda:rewrite(moved/'preparation/preparation.json',lambda d:d.update(compiler='rustc unexpected')),
                lambda:rewrite(moved/'process-absence.json',lambda d:d.update(pids=[],command=['true'])),
                lambda:rewrite(moved/'control/preparation-process-absence.json',lambda d:d.update(pids=[32169])),
                lambda:rewrite(moved/'output/summary.json',lambda d:d[0].update(median_ratio=2)),
            ]
            for mutation in mutations:
                with self.subTest(mutation=mutations.index(mutation)):
                    for relative,data in original.items():(moved/relative).write_bytes(data)
                    mutation();reseal();execute(1)

    def test_historical_projection_stdout_cannot_qualify_a_real_packet_candidate(self):
        """Historical native output is retained, but cannot qualify a different candidate."""
        path = HERE/'schema-fixtures/native-projection-tests.stdout.json'
        provenance = json.loads((HERE/'schema-fixtures/native-projection-tests.provenance.json').read_text())
        raw = json.loads(path.read_text())['stdout'].encode()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), provenance['stdout_sha256'])
        self.assertEqual(provenance['native_test_exit_code'], 0)
        self.assertEqual(provenance['workflow_conclusion'], 'failure')
        for validate in [prepare.require_candidate_tests, verifier.require_candidate_tests]:
            with self.assertRaises(ValueError):
                validate(raw.decode())
            # The original mistaken namespace must remain rejected, even with
            # the same successful test count and unchanged stdout scaffolding.
            wrong = raw.decode().replace('near_clifford::compiled::coherent_packet::', 'near_clifford::coherent_packet::')
            with self.assertRaises(ValueError):
                validate(wrong)

    def test_historical_real_packet_stdout_cannot_qualify_cdf_materialized_candidate(self):
        """Historical real-packet native names cannot qualify this materialized-input candidate."""
        path = HERE/'schema-fixtures/local-real-packet-tests.stdout.json'
        provenance = json.loads((HERE/'schema-fixtures/local-real-packet-tests.provenance.json').read_text())
        raw = json.loads(path.read_text())['stdout'].encode()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), provenance['stdout_sha256'])
        self.assertEqual(len(raw), provenance['stdout_bytes'])
        self.assertEqual(provenance['test_exit_code'], 0)
        self.assertEqual(provenance['build_profile'], 'debug')
        self.assertFalse(provenance['native_flags_used'])
        for validate in [prepare.require_candidate_tests, verifier.require_candidate_tests]:
            with self.assertRaises(ValueError):
                validate(raw.decode())
            wrong = raw.decode().replace('near_clifford::compiled::real_coherent_packet::', 'near_clifford::real_coherent_packet::')
            with self.assertRaises(ValueError):
                validate(wrong)

    def test_actual_local_cdf_materialized_stdout_binds_every_mounted_candidate_test(self):
        """Actual Rust output checks names; its local debug provenance is explicit."""
        path = HERE/'schema-fixtures/local-cdf-materialized-tests.stdout.json'
        provenance = json.loads((HERE/'schema-fixtures/local-cdf-materialized-tests.provenance.json').read_text())
        raw = json.loads(path.read_text())['stdout'].encode()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), provenance['stdout_sha256'])
        self.assertEqual(len(raw), provenance['stdout_bytes'])
        self.assertEqual(provenance['test_exit_code'], 0)
        self.assertEqual(provenance['build_profile'], 'debug')
        self.assertFalse(provenance['native_flags_used'])
        for validate in [prepare.require_candidate_tests, verifier.require_candidate_tests]:
            validate(raw.decode())
            wrong = raw.decode().replace('near_clifford::compiled::cdf_materialized_projection_tests::', 'near_clifford::cdf_materialized_projection_tests::')
            with self.assertRaises(ValueError):
                validate(wrong)

    def test_candidate_preflight_requires_every_named_test_and_successful_summary(self):
        names = ['near_clifford::compiled::cdf_materialized_projection_tests::probability_miss_reuses_only_the_unmodified_input_for_both_projection_branches', 'near_clifford::compiled::cdf_materialized_projection_tests::cached_first_branch_keeps_buffer_untouched_and_missing_second_branch_loads_input', 'near_clifford::compiled::cdf_materialized_projection_tests::projection_error_invalidates_loaded_input_and_preserves_the_old_error_state', 'near_clifford::compiled::cdf_materialized_projection_tests::default_mixed_calls_match_old_reload_state_records_counts_and_rng']
        for layout in [False, True]:
            selected = ['near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage'] if layout else names
            count = len(selected)
            good = ''.join('test '+name+' ... ok\n' for name in selected)+f'test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 288 filtered out\n'
            for validate in [prepare.require_candidate_tests, verifier.require_candidate_tests]:
                validate(good, layout=layout)
                for bad in [good.replace(selected[0], 'different_test'),good.replace(f'{count} passed','0 passed'),good.replace('0 ignored','1 ignored'),good.replace('0 failed','1 failed'),good+good,good.replace(' ... ok',' ... ignored',1),good.split('test result:')[0],
                            good+'test '+selected[0]+' ... ignored\n', good+'test '+selected[0]+' ... FAILED\n',
                            good+'test result: FAILED. 0 passed; 1 failed; 0 ignored;\n',good+'test unrelated::test ... ok\n',
                            good.replace('0 measured','1 measured'),good+'test extra ... \n',good+'test result: \n',good+'test broken\n',good+'test result:\n']:
                    with self.subTest(validator=validate.__module__,layout=layout,log=bad):
                        with self.assertRaises(ValueError):validate(bad, layout=layout)

    def test_preparation_changes_only_rstim_dependency_and_rejects_zero_tests(self):
        original = (HERE.parent/'diagnostics/Cargo.toml').read_text()
        changed = prepare.probe_manifest(original, Path('/tmp/source path/rstim'))
        self.assertIn('path = "main.rs"', changed)
        self.assertEqual(changed.replace('path = "/tmp/source path/rstim"', 'path = "../../../rstim"'), original)
        with self.assertRaises(ValueError): prepare.probe_manifest('path = "main.rs"', Path('/tmp/rstim'))
        with self.assertRaises(ValueError): prepare.probe_manifest(original + original, Path('/tmp/rstim'))
        evidence.require_executed_tests('test result: ok. 4 passed; 0 failed')
        for log in ['test result: ok. 0 passed; 0 failed', 'error: compile failed']:
            with self.assertRaises(ValueError): evidence.require_executed_tests(log)

    def test_native_preparation_failure_retains_actual_wait_and_log(self):
        with tempfile.TemporaryDirectory() as name:
            log = Path(name)/'native.log'
            with self.assertRaisesRegex(ValueError, 'receipt retained'):
                prepare.invoke([sys.executable,'-I','-c','print("build failed");raise SystemExit(17)'],
                               None, os.environ.copy(), log, {})
            receipt = json.loads(log.with_suffix('.receipt.json').read_text())
            self.assertEqual(receipt['exit_code'],17)
            self.assertTrue(receipt['child_waited'])
            self.assertFalse(receipt['timed_out'])
            self.assertEqual(receipt['log_sha256'],evidence.sha(log))
            self.assertIn('build failed',log.read_text())
            self.assertEqual(subprocess.run(['ps','-p',str(receipt['child_pid']),'-o','pid='],capture_output=True).returncode,1)

    def test_preparation_seal_relocates_and_detects_additions_and_changed_bytes(self):
        import shutil
        with tempfile.TemporaryDirectory() as name:
            root=Path(name)/'original';root.mkdir();(root/'binary.bin').write_bytes(b'actual executable fixture')
            evidence.seal_preparation(root,'a'*40)
            moved=Path(name)/'moved';shutil.copytree(root,moved)
            self.assertEqual(evidence.verify_preparation(root),evidence.verify_preparation(moved))
            (moved/'extra.log').write_text('unsealed addition')
            with self.assertRaisesRegex(ValueError,'inventory'):evidence.verify_preparation(moved)
            (moved/'extra.log').unlink();(moved/'binary.bin').write_bytes(b'changed')
            with self.assertRaises(ValueError):evidence.verify_preparation(moved)
            with self.assertRaises(ValueError):evidence.verify_files(root,{'../escape':dict(bytes=0,sha256='bad')})

    def test_fixed_independent_ledger_schedule_and_invalid_adaptive_observations(self):
        manifest=json.loads((HERE/'manifest.json').read_text())
        cases,schedule=verifier.schedule(manifest)
        self.assertEqual(len(schedule),720)
        self.assertEqual(schedule[0],('baseline','structural',('depth-32',1,'strict'),'dump',None,'structured'))
        self.assertEqual(schedule[1],('baseline','structural',('depth-32',1,'strict'),'dump',None,'flat'))
        self.assertEqual(sum(row[3]=='bench' for row in schedule),648)
        data=dict(elapsed_ns=50_000_000,calls=2,ns_per_call=25_000_000)
        pilot.validate_observations([data]*7)
        for changed in [dict(data,calls=True),dict(data,elapsed_ns=True),dict(data,ns_per_call=float('nan')),dict(data,elapsed_ns=49_999_999),dict(data,ns_per_call=1)]:
            with self.assertRaises(RuntimeError):pilot.validate_observations([changed]*7)

    def test_fixed_manifest_inputs_cardinality_and_each_role_position_twice(self):
        manifest=json.loads((HERE/'manifest.json').read_text())
        cases=manifest['structural_cases']+manifest['application_counts_cases']
        self.assertEqual(len(cases),36);self.assertEqual(len({tuple(c) for c in cases}),36)
        self.assertEqual(manifest['expected_timing'],36*6*3)
        self.assertEqual(manifest['expected_observations'],36*6*3*7)
        structural={(c[0],c[2]) for c in manifest['structural_cases']}
        finite=len(structural)*4+len(manifest['application_counts_cases'])*2
        self.assertEqual(finite,72);self.assertEqual(manifest['expected_events'],finite+648)
        self.assertEqual({p.name:pilot.sha(p) for p in (HERE/'fixtures').glob('*.stim')},manifest['inputs'])
        for ordinal in range(36):
            positions=[]
            for pair in range(6):
                roles=['baseline','candidate','control'];shift=(pair+ordinal)%3
                roles=roles[shift:]+roles[:shift]
                if (pair+ordinal)%2:roles.reverse()
                positions.append(roles.index('candidate'))
            self.assertEqual(collections.Counter(positions),{0:2,1:2,2:2})

    def test_independent_original_annotation_replay_and_wrong_population(self):
        masks=pilot.semantics.annotations('M 0 1 2\nDETECTOR rec[-3] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n')
        payload=dict(measurements=[0,0,0,0,1,1,1,0,1,1,1,1],shots=4,width=3)
        self.assertEqual(pilot.semantics.raw_counts(payload,masks),dict(attempted=4,accepted=2,discarded=2,logical_errors=1))
        with self.assertRaises(ValueError):pilot.semantics.raw_counts(dict(payload,shots=5),masks)
        with self.assertRaises(ValueError):pilot.semantics.raw_counts(dict(payload,measurements=[2]+payload['measurements'][1:]),masks)

    def finite_payload(self):
        observation=dict(elapsed_ns=1000000,calls=128,ns_per_call=1000000/128,attempted=8192,accepted=8192,discarded=0,logical_errors=0)
        return dict(status='ok',backend='rstim',input_sha256='input',arithmetic='strict',shots=8192,call_shots=64,width=3,measurements=[0]*(8192*3),observations=[observation],exact_native_counts_rng=True,output_contract=pilot.semantics.CONTRACT,compile_ns=1,prepare_ns=0,first_ns=0,peak_active_rank=1,peak_rss_bytes=1,cache_reserved_bytes=0)

    def test_counts_finite_schema_and_independent_replay_reject_bad_payloads(self):
        data=self.finite_payload();event=dict(exit_code=0,timed_out=False,result=data)
        text='M 0 1 2\nDETECTOR rec[-3] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n';case=['synthetic',64,'strict']
        pilot.validate_counts(event,case,'input',text,True)
        for mutate in [lambda d:d.update(call_shots=1),lambda d:d.update(arithmetic='fused'),lambda d:d.update(shots=8191),lambda d:d.update(exact_native_counts_rng=1),lambda d:d['measurements'].__setitem__(0,2),lambda d:d['observations'][0].update(logical_errors=1)]:
            altered=copy.deepcopy(data);mutate(altered)
            with self.assertRaises((ValueError,RuntimeError)):pilot.validate_counts(dict(event,result=altered),case,'input',text,True)
        for bad in [dict(event,exit_code=17),dict(event,timed_out=True)]:
            with self.assertRaises(RuntimeError):pilot.validate_counts(bad,case,'input',text,True)

    def test_real_counts_bench_schema_has_no_dump_only_call_shots(self):
        data=json.loads((HERE/'schema-fixtures/counts.json').read_text())
        binding=json.loads((HERE/'schema-fixtures/provenance.json').read_text())['counts']
        self.assertNotIn('call_shots',data)
        case=binding['case'];event=dict(exit_code=0,timed_out=False,result=data)
        pilot.validate_counts(event,case,data['input_sha256'],'')
        for mutate in [lambda d:d.update(shots=2),lambda d:d.update(arithmetic='fused'),lambda d:d['observations'][0].update(attempted=0)]:
            altered=copy.deepcopy(data);mutate(altered)
            with self.assertRaises((ValueError,RuntimeError)):pilot.validate_counts(dict(event,result=altered),case,data['input_sha256'],'')

    def test_real_structural_bench_uses_config_and_rejects_other_route(self):
        data=json.loads((HERE/'schema-fixtures/structural.json').read_text());binding=json.loads((HERE/'schema-fixtures/provenance.json').read_text())['structural'];case=binding['case']
        self.assertNotIn('call_kind',data);self.assertNotIn('call_shots',data)
        data['config']['circuit']=str(HERE/'fixtures'/(case[0]+'.stim'))
        event=dict(exit_code=0,timed_out=False,result=data);pilot.validate_structural_timing(event,case,data['input_sha256'])
        for mutate in [lambda d:d['config'].update(shots=2),lambda d:d['config'].update(call_kind='structured'),lambda d:d['config'].update(action='dump'),lambda d:d['config'].update(cache_bytes=0)]:
            altered=copy.deepcopy(data);mutate(altered)
            with self.assertRaises(RuntimeError):pilot.validate_structural_timing(dict(event,result=altered),case,data['input_sha256'])

    def test_structural_129rows_16literal_words_and_transport_are_required(self):
        flat=dict(status='ok',input_sha256='input',arithmetic='strict',call_kind='flat',call_shots=129,shots=129,width=3,measurements=[0]*(129*3),continuation=list(range(16)))
        scalar=dict(flat,call_kind='structured',call_shots=1)
        a=dict(exit_code=0,timed_out=False,result=scalar);b=dict(a,result=flat);case=['synthetic',1,'strict']
        pilot.validate_structural_pair(a,b,case,'input')
        for mutate in [lambda d:d.update(status='failed'),lambda d:d.update(shots=0),lambda d:d.update(continuation=[]),lambda d:d['continuation'].__setitem__(0,True),lambda d:d['measurements'].__setitem__(0,2)]:
            altered=copy.deepcopy(flat);mutate(altered)
            with self.assertRaises(RuntimeError):pilot.validate_structural_pair(a,dict(b,result=altered),case,'input')
        for bad in [dict(b,exit_code=17),dict(b,timed_out=True)]:
            with self.assertRaises(RuntimeError):pilot.validate_structural_pair(a,bad,case,'input')

    def test_valid_json_from_failed_or_timed_out_child_is_retained_and_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            recorder=pilot.Recorder(Path(name))
            for command,timeout in [('import sys;print(\'{"status":"ok"}\');sys.exit(17)',3),('import time;print(\'{"status":"ok"}\',flush=True);time.sleep(5)',.05)]:
                event,data=recorder.invoke([sys.executable,'-I','-c',command],'synthetic',timeout=timeout)
                self.assertEqual(data,{'status':'ok'});self.assertTrue(event['child_waited']);self.assertFalse(pilot.timing_transport_ok(event))
                self.assertEqual(subprocess.run(['ps','-p',str(event['child_pid']),'-o','pid='],capture_output=True).returncode,1)

    def test_cancelled_producer_reaps_worker_in_separate_session(self):
        with tempfile.TemporaryDirectory() as name:
            output=Path(name);script=output/'producer.py'
            script.write_text("import importlib.util,signal,sys;from pathlib import Path\n"+"s=importlib.util.spec_from_file_location('p',"+repr(str(HERE/'run.py'))+");p=importlib.util.module_from_spec(s);s.loader.exec_module(p)\n"+"signal.signal(signal.SIGTERM,p.cancel);p.Recorder(Path("+repr(name)+")).invoke([sys.executable,'-I','-c','import time;time.sleep(30)'],'cancel')\n")
            with (output/'producer.log').open('wb') as log:
                child=subprocess.Popen([sys.executable,'-I',str(script)],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
                try:
                    active=output/'active-child.json';deadline=time.monotonic()+10
                    while not active.exists() and time.monotonic()<deadline:time.sleep(.01)
                    self.assertTrue(active.exists());worker=json.loads(active.read_text())['child_pid']
                    cleanup=outer.stop_producer(child,active,grace=3)
                    self.assertNotEqual(child.returncode,0);self.assertFalse(active.exists())
                    event=json.loads((output/'events.jsonl').read_text());self.assertEqual(event['child_pid'],worker);self.assertTrue(event['child_waited']);self.assertEqual(event['exit_code'],-9);self.assertEqual(event['cancellation'],'CampaignCancelled');self.assertEqual(cleanup[0]['exit_code'],1)
                finally:
                    if child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.wait()
if __name__=='__main__':unittest.main()
