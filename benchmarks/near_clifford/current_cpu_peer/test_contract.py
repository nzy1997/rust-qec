import base64
import collections
import copy
import hashlib
import shutil
import subprocess
import sys
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zlib

from peer_evidence import inventory, seal_preparation, verify_files, verify_preparation
from validation import compact_equal, compare, masks, replay, validate_observations
from verify import schedule


class ProtocolContracts(unittest.TestCase):
    def test_worker_bootstraps_local_helpers_in_actual_isolated_processes(self):
        worker=Path(__file__).resolve().parent/'worker.py'
        with tempfile.TemporaryDirectory() as directory:
            for flags in [[],['-O']]:
                with self.subTest(flags=flags):
                    result=subprocess.run([sys.executable,'-I','-B',*flags,str(worker),'--help'],
                                          cwd=directory,capture_output=True,timeout=10)
                    self.assertEqual(result.returncode,0,result.stderr.decode())
                    self.assertIn(b'clifft-scheduled',result.stdout)
                    self.assertIn(b'--cpu-backend',result.stdout)

    def test_preparation_requires_reviewed_main_before_cloning(self):
        import prepare
        pin = json.loads((Path(__file__).parent/'manifest.json').read_text())['symft_revision']
        self.assertEqual(prepare.PEER,pin)
        cases = [
            (pin+'\trefs/heads/main\n',True),
            ('a'*40+'\trefs/heads/symft-26-10-08\n'+pin+'\trefs/heads/main\n',True),
            (pin+'\trefs/heads/symft-26-10-08\n',False),
            (pin+'\trefs/heads/main-extra\n',False),
            ('a'*40+'\trefs/heads/main\n'+pin+'\trefs/heads/symft-26-10-08\n',False),
            (pin+'\trefs/heads/main\n'+'a'*40+'\trefs/heads/main\n',False),
        ]
        class CloneReached(RuntimeError):pass
        with tempfile.TemporaryDirectory() as directory:
            for number,(remote,valid) in enumerate(cases):
                labels = []
                def invoke(out,label,command,cwd,env,**kwargs):
                    labels.append(label)
                    if label=='peer-clone':raise CloneReached()
                    return remote.encode() if label=='peer-official-refs' else b''
                def git_output(command,**kwargs):
                    return b'' if command[1]=='status' else 'c'*40+'\n'
                with self.subTest(remote=remote), \
                     mock.patch.object(sys,'argv',['prepare.py','--out',str(Path(directory)/str(number)),'--rust-ref','b'*40,'--baseline-ref','a'*40]), \
                     mock.patch.object(sys,'version_info',(3,12)), \
                     mock.patch.object(prepare.platform,'system',return_value='Linux'), \
                     mock.patch.object(prepare.platform,'machine',return_value='x86_64'), \
                     mock.patch.object(prepare.signal,'signal'), \
                     mock.patch.object(prepare.subprocess,'check_output',side_effect=git_output), \
                     mock.patch.object(prepare,'invoke',side_effect=invoke):
                    if valid:
                        with self.assertRaises(CloneReached):prepare.main()
                    else:
                        with self.assertRaisesRegex(ValueError,'official main pin differs'):prepare.main()
                    self.assertEqual(labels,['gcc-version','g++-version','peer-official-refs']+(['peer-clone'] if valid else []))

    def test_complete_offline_bundle_relocates_and_rejects_semantic_mutations(self):
        import test_bundle_fixture as fixture
        with tempfile.TemporaryDirectory() as directory:
            original = Path(directory)/'original'
            fixture.build_bundle(original)
            moved = Path(directory)/'moved'
            shutil.copytree(original,moved);shutil.rmtree(original)
            def execute(expected, optimized=False, message=None):
                command = [sys.executable,'-I']+(['-O'] if optimized else [])+[str(Path(__file__).parent/'verify.py'),str(moved)]
                result = subprocess.run(command,capture_output=True,timeout=45)
                self.assertEqual(result.returncode,expected,result.stderr.decode())
                if message is not None:self.assertIn(message,result.stderr.decode())
            execute(0);execute(0,True)
            snapshots = {p.relative_to(moved):p.read_bytes() for p in moved.rglob('*') if p.is_file()}
            def rewrite(relative,change):
                path=moved/relative;data=json.loads(path.read_text());change(data);fixture.write(path,data)
            def worker_change(change):
                path=moved/'output/events.jsonl';events=[json.loads(line) for line in path.read_text().splitlines()]
                change(events)
                path.write_text(''.join(json.dumps(event,separators=(',',':'))+'\n' for event in events))
            def payload_change(change):
                # Change actual stdout and its compact ledger copy together;
                # resealing must reach the semantic timing check, not a digest.
                path=moved/'output/events.jsonl'
                events=[json.loads(line) for line in path.read_text().splitlines()]
                event=next(e for e in events if e['kind']=='timing')
                raw=moved/'output'/f"{event['index']:05d}.stdout"
                payload=json.loads(raw.read_text());change(payload);fixture.write(raw,payload)
                event['result']=payload
                event['stdout_sha256']=hashlib.sha256(raw.read_bytes()).hexdigest()
                path.write_text(''.join(json.dumps(e,separators=(',',':'))+'\n' for e in events))
            mutations = [
                lambda:worker_change(lambda e:e[0].update(command=['echo','fake package inspection'])),
                lambda:worker_change(lambda e:e[5].update(timed_out=True,process_status='timeout-group-killed-and-waited')),
                lambda:worker_change(lambda e:e[-5].update(shots=1)),
                lambda:rewrite('preparation/native-symft-wheel.receipt.json',lambda r:r['environment'].update(SYMFT_PY_NATIVE='0')),
                lambda:rewrite('preparation/pinned-wheel-download.receipt.json',lambda r:r.update(command=['echo','unpinned download'])),
                lambda:rewrite('preparation/rust/native-check-1.receipt.json',lambda r:r.update(command=['echo','not a test'])),
                lambda:rewrite('preparation/rust/baseline/native-build-counts.receipt.json',lambda r:r['environment'].update(RUSTFLAGS='-C target-cpu=generic')),
                lambda:rewrite('process-absence.json',lambda r:r.update(pids=[],command=['true'])),
                lambda:rewrite('output/header.json',lambda r:r['before']['harness'].pop(next(iter(r['before']['harness'])))),
                lambda:rewrite('output/frozen-selections.json',lambda r:r[0]['backends']['symft'].update(cpu_backend='legacy',batch='scalar')),
            ]
            mutations += [
                lambda:worker_change(lambda events:next(e for e in events if e['kind']=='timing' and e['backend']=='rstim')['command'].__setitem__(0,next(e for e in events if e['kind']=='timing' and e['backend']=='rstim')['command'][0].replace('/candidate/','/baseline/'))),
                lambda:rewrite('preparation/rust/candidate/native-build-counts.receipt.json',lambda r:r.update(head='a'*40)),
                lambda:rewrite('preparation/rust/native-check-2.receipt.json',lambda r:r.update(head='a'*40)),
            ]
            for number,mutation in enumerate(mutations):
                with self.subTest(mutation=number):
                    for relative,data in snapshots.items():(moved/relative).write_bytes(data)
                    mutation();fixture.reseal(moved);execute(1)
            for value in [None,'historical-probe-v1']:
                with self.subTest(timing_contract=value):
                    for relative,data in snapshots.items():(moved/relative).write_bytes(data)
                    payload_change(lambda p: p.pop('timing_contract') if value is None else p.update(timing_contract=value))
                    fixture.reseal(moved)
                    execute(1,message='matched timing contract differs')
                    execute(1,True,message='matched timing contract differs')
            pin = fixture.read(moved/'preparation/manifest.json')['symft_revision']
            invalid_refs = [
                pin+'\trefs/heads/symft-26-10-08\n',
                pin+'\trefs/heads/main-extra\n',
                'a'*40+'\trefs/heads/main\n'+pin+'\trefs/heads/symft-26-10-08\n',
                pin+'\trefs/heads/main\n'+'a'*40+'\trefs/heads/main\n',
            ]
            for remote in invalid_refs:
                with self.subTest(remote=remote):
                    for relative,data in snapshots.items():(moved/relative).write_bytes(data)
                    path = moved/'preparation/peer-official-refs.stdout'
                    path.write_text(remote)
                    rewrite('preparation/peer-official-refs.receipt.json',lambda r:r.update(stdout_sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
                    fixture.reseal(moved)
                    execute(1,message='official main pin differs')
                    execute(1,True,message='official main pin differs')
            for relative,data in snapshots.items():(moved/relative).write_bytes(data)
            for relative in ['preparation/preparation.json','preparation/manifest.json']:
                rewrite(relative,lambda r:r.update(symft_revision='a'*40))
            fixture.reseal(moved)
            execute(1,message='reviewed official main revision differs')
            execute(1,True,message='reviewed official main revision differs')

    def test_full_schedule_keeps_all_roles_and_counterbalances_each_case(self):
        manifest = json.loads((Path(__file__).parent/'manifest.json').read_text())
        selected = {(n,s):{b:{'batch':'auto',**({'cpu_backend':'compiled'} if b=='symft' else {})}
                            for b in ['clifft','clifft-scheduled','symft']}
                    for n in manifest['names'] for s in manifest['shots']}
        cases, events = schedule(manifest,selected)
        self.assertEqual(len(cases),12)
        self.assertEqual(len(events),1035)
        self.assertEqual(collections.Counter(e['kind'] for e in events),
                         {'package-inspection':4,'import-inspection':4,'host-inspection':1,
                          'tuning':102,'counts-validation':42,'raw-validation':18,'timing':864})
        for name,shots,policy in cases:
            matching = [e for e in events if e['kind']=='timing' and (e['name'],e['shots'],e['policy'])==(name,shots,policy)]
            self.assertEqual(len(matching),72)
            positions = collections.defaultdict(collections.Counter)
            for pair in range(12):
                round = [e for e in matching if e['pair']==pair]
                self.assertEqual({e['backend'] for e in round},set(manifest['roles']))
                for position,event in enumerate(round):
                    positions[event['backend']][position] += 1
            self.assertTrue(all(dict(v)=={p:2 for p in range(6)} for v in positions.values()))

    def test_seeded_clock_releases_each_temporary_result_in_its_own_span(self):
        import timing
        events = []
        class Native:
            def __del__(self): events.append('native-release')
        class Counts(dict):
            def __getitem__(self, key):
                events.append('read-'+key)
                return super().__getitem__(key)
            def __del__(self): events.append('dict-release')
        def sample(seed):
            events.append(('rng-seed',seed))
            native = Native()
            result = Counts(attempted=64,accepted=32,discarded=32,logical_errors=3)
            del native
            return result
        ticks = iter([10,40,100,145])
        def now():
            tick = next(ticks)
            events.append(('clock',tick))
            return tick
        with mock.patch.object(timing.time,'perf_counter_ns',side_effect=now):
            self.assertEqual(timing.timed_counts(sample,7),(30,64,32,32,3))
            self.assertEqual(timing.timed_counts(sample,8),(45,64,32,32,3))
        inside = ['native-release','read-attempted','read-accepted','read-discarded','read-logical_errors','dict-release']
        self.assertEqual(events,[('clock',10),('rng-seed',7),*inside,('clock',40),
                                 ('clock',100),('rng-seed',8),*inside,('clock',145)])

    def test_literal_raw_parities_include_xor_cancellation_and_all_detector_rejection(self):
        text = 'M 0 1 2\nDETECTOR rec[-1] rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-3]\nOBSERVABLE_INCLUDE(0) rec[-1]\n'
        parsed, projection = masks(text)
        self.assertEqual(parsed,dict(width=3,detectors=[[],[1]],observable=[0,2]))
        self.assertEqual(projection,'M 0 1 2\n')
        rows = [[1,0,0],[1,0,1],[0,1,1],[0,0,1]]*2048
        result = replay(dict(width=3,shots=8192,measurements=[v for row in rows for v in row]),parsed)
        self.assertEqual(result,dict(attempted=8192,accepted=6144,discarded=2048,logical_errors=4096))
        invalid = dict(width=3,shots=8192,measurements=[v for row in rows for v in row])
        invalid['measurements'][37] = True
        with self.assertRaisesRegex(ValueError,'raw bit'):
            replay(invalid,parsed)

    def test_compacted_raw_payload_requires_exact_bits_padding_and_metadata(self):
        raw = dict(shots=3,width=3,measurements=[1,0,1,1,0,0,0,1,1],backend='peer')
        encoded = dict(codec='zlib-packbits-big-v1',data=base64.b64encode(zlib.compress(bytes([0xb1,0x80]))).decode())
        compact = dict(raw,measurements=encoded)
        compact_equal(raw,compact)
        for mutant in ['padding','bits','metadata','trailing']:
            changed = copy.deepcopy(compact)
            if mutant == 'padding':
                changed['measurements']['data'] = base64.b64encode(zlib.compress(bytes([0xb1,0x81]))).decode()
            elif mutant == 'bits':
                changed['measurements']['data'] = base64.b64encode(zlib.compress(bytes([0xb0,0x80]))).decode()
            elif mutant == 'metadata':
                changed['backend'] = 'another'
            else:
                changed['measurements']['data'] = base64.b64encode(zlib.compress(bytes([0xb1,0x80]))+b'extra').decode()
            with self.subTest(mutant=mutant), self.assertRaises(ValueError):
                compact_equal(raw,changed)

    def observation(self):
        e = dict(kind='timing',shots=64,backend='rstim',policy='strict')
        o = dict(calls=1024,elapsed_ns=50_000_000,attempted=65536,accepted=32768,discarded=32768,logical_errors=71,ns_per_call=50_000_000/1024)
        p = dict(status='ok',backend='rstim',shots=64,compile_ns=32,prepare_ns=16,first_ns=8,
                 peak_rss_bytes=1024,peak_active_rank=4,arithmetic='strict',cache_reserved_bytes=1<<20,
                 timing_contract='seeded-counts-invocation-v1',
                 output_contract='all-zero raw detector postselection; raw observable 0 counts; no reference normalization',observations=[dict(o) for _ in range(7)])
        return e,p

    def test_observations_reject_short_horizons_false_integers_bad_counts_and_nan(self):
        e,p = self.observation()
        validate_observations(p,e)
        for key,value in [('calls',True),('elapsed_ns',49_999_999),('attempted',65535),('accepted',65537),('logical_errors',32769),('ns_per_call',float('nan'))]:
            changed = copy.deepcopy(p)
            changed['observations'][0][key] = value
            with self.subTest(key=key),self.assertRaises(ValueError):
                validate_observations(changed,e)
        for key,value in [('timing_contract','historical-probe-v1'),('arithmetic','fused'),('cache_reserved_bytes',64*1024**2+1),('output_contract','normalized')]:
            changed = dict(p,**{key:value})
            with self.subTest(key=key),self.assertRaises(ValueError):
                validate_observations(changed,e)

    def test_finite_bounds_reject_large_marginal_differences(self):
        a = dict(attempted=8192,accepted=4096,logical_errors=0)
        b = dict(attempted=8192,accepted=8192,logical_errors=0)
        self.assertTrue(compare(a,a,.001/(12*3*12))['passed'])
        self.assertFalse(compare(a,b,.001/(12*3*12))['passed'])

    def test_preparation_seal_is_relocatable_but_rejects_resealed_escape_and_additions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'installed-source/symft/symft').mkdir(parents=True)
            (root/'installed-source/symft/symft/native.so').write_bytes(b'native retained bytes')
            (root/'symft').mkdir()
            (root/'symft/live-environment').write_bytes(b'live venv excluded')
            seal_preparation(root,'a'*40)
            self.assertEqual(set(json.loads((root/'seal.json').read_text())['files']),{'installed-source/symft/symft/native.so'})
            self.assertEqual(verify_preparation(root),hashlib.sha256((root/'seal.json').read_bytes()).hexdigest())
            (root/'addition').write_bytes(b'new')
            with self.assertRaisesRegex(ValueError,'inventory changed'):
                verify_preparation(root)
            (root/'addition').unlink()
            (root/'installed-source/symft/symft/native.so').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'sealed bytes changed'):
                verify_preparation(root)
            with self.assertRaisesRegex(ValueError,'unsafe seal'):
                verify_files(root,{'../escape':dict(bytes=1,sha256='0'*64)})


if __name__ == '__main__':
    unittest.main()
