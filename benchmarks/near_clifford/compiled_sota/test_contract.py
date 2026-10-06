"""Adversarial verifier tests using explicitly synthetic consistency fixtures."""
import copy
import hashlib
import json
import math
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from evidence import compact, expand
from projection import records_only, physical_width
from run import BACKENDS, BATCHES, HERE, RSTIM_API, batches, bind_input, bind_peer, capture_identities, capture_packages, default_batch, environment_summary, harness_inventory, masks, sha
from verify import validate
from worker import prepare


def hashed(content):
    return hashlib.sha256(content).hexdigest()


def freeze(result):
    result['frozen_tuning_sha256']=hashed((json.dumps(result['tuning'],indent=2)+'\n').encode())


def fixture():
    # These zero transcripts are test data, never retained benchmark evidence.
    manifest=json.loads((HERE/'manifest.json').read_text())
    case=manifest['cases'][0]; text=(HERE/case['file']).read_text()
    width=sum(len(line.split())-1 for line in text.splitlines() if line.split() and
        line.split()[0].split('(')[0] in ['M','MX','MY','MR','MPP'])
    n=1024; selected=[list(m) for m in masks(text,width)]; counts=[0]*len(selected)
    native_hash=hashed(records_only(text).encode('utf-8'))
    identities={name:{name:{'path':f'/synthetic/site-packages/{name}/__init__.py','sha256':'0'*64},
        name+suffix:{'path':f'/synthetic/site-packages/{name}/{suffix[1:]}.so','sha256':'0'*64}}
        for name,suffix in [('clifft','._clifft_core'),('symft','._native')]}
    packages={name:{'version':version,'files':{f['path']:f['sha256'] for f in identities[name].values()}
        if name in identities else {'/synthetic/site-packages/numpy/__init__.py':'0'*64}}
        for name,version in manifest['baseline_versions'].items()}
    result={'schema':'rstim.near-clifford-compiled-sota-results.v1','rstim_api':RSTIM_API,
        'input_contract':'identical native records_only circuit for every backend','subset':True,
        'started':'2026-10-07T00:00:00+00:00','finished':'2026-10-07T00:01:00+00:00',
        'host':'synthetic unit-test data','pairs':1,'repetitions':1,'validation_shots':n,
        'manifest_sha256':sha(HERE/'manifest.json'),'harness':harness_inventory(),
        'symft_source_revision':manifest['symft_source']['commit'],'symft_source_files':{'test.cpp':'0'*64},
        'source_files':{'test.rs':'0'*64},'binary_sha256':'0'*64,
        'packages':{**packages,'clifft_environment':packages},'peer_loaded_files':identities}
    summary=environment_summary(result['packages'],result['symft_source_files'],
        result['symft_source_revision'],identities)
    result['environment_guard']={'before':dict(summary),'after':dict(summary)}
    def payload(backend,shots,batch='auto',dump=False,call_shots=None):
        raw={'backend':backend,'shots':shots,'width':width,'compile_ns':100,'prepare_ns':100,
            'input_sha256':native_hash,
            'api':RSTIM_API,'peak_active_rank':1,'cache_reserved_bytes':1024,'continuation':739,
            'rng':'SmallRng/rand-0.8.7','version':'0.11.0' if backend.startswith('clifft') else '0.1.1',
            'threads':1,'batch':batch,'peak_active_width':1,'simd_backend':'scalar',
            'batch_enabled':batch!='scalar','isolated':True,
            'loaded_files':copy.deepcopy(identities.get('symft' if backend=='symft' else 'clifft')),
            'first_ns':[100],'warm_ns':[500000.0],'warm_totals_ns':[50_000_000],'warm_calls':[100]}
        if dump:
            raw.update(measurements=[0]*(shots*width),call_shots=call_shots)
        return raw
    defaults={b:payload(b,n,batch=default_batch(b),dump=True,call_shots=n) for b in BACKENDS}
    validation={'id':case['id'],'passed':True,'width':width,'physical_width':physical_width(text),
        'native_sha256':hashed(records_only(text).encode()),
        'adapted_sha256':hashed(records_only(text).encode()),
        'masks':selected,'parity_counts':{b:counts[:] for b in BACKENDS},'max_difference':0,
        'threshold':2*math.sqrt(math.log(4*24*len(selected)/.001)/(2*n)),
        'payload_sha256':{case['id']+'-'+b+'-validation.json':hashed((json.dumps(p)+'\n').encode())
            for b,p in defaults.items()},'transcripts':{b:compact(p) for b,p in defaults.items()}}
    def selected_batch(backend):
        return 'scalar' if backend=='symft' else 'auto' if backend=='rstim' else 1
    chosen={b:payload(b,n,batch=selected_batch(b),dump=True,call_shots=64) for b in BACKENDS}
    tuning={'id':case['id'],'shots':64,'rstim_parity_counts':counts[:],'backends':{},
        'selected_payload_sha256':{f"{case['id']}-64-{b}-selected.json":hashed((json.dumps(p)+'\n').encode())
            for b,p in chosen.items()},'transcripts':{b:compact(p) for b,p in chosen.items()}}
    for backend in BACKENDS[1:]:
        trials=[]
        for batch in batches(backend):
            raw=payload(backend,64,batch);raw['first_ns']=[]
            for key in ['warm_ns','warm_totals_ns','warm_calls']:raw[key]*=3
            trials.append({'batch':batch,'median_ns':500000.0,'raw':raw})
        tuning['backends'][backend]={'selected_batch':selected_batch(backend),'trials':trials,
            'selected_parity_counts':counts[:],'selected_max_difference':0}
    result.update(validation=[validation],tuning=[tuning],cases=[{'id':case['id'],'shots':64,
        'runs':[{'order':BACKENDS[:],**{b:payload(b,64,batch=selected_batch(b)) for b in BACKENDS}}]}])
    freeze(result)
    return result


class ContractTest(unittest.TestCase):
    def setUp(self):
        self.result=fixture()

    def rejected(self,mutate):
        candidate=copy.deepcopy(self.result);mutate(candidate);freeze(candidate)
        with self.assertRaises((ValueError,KeyError,TypeError)):
            validate(candidate,allow_subset=True)

    def test_valid_consistency_fixture_and_lossless_transcript(self):
        report=validate(self.result,allow_subset=True)
        self.assertIn('terminal',report)
        self.assertIn(RSTIM_API,report)
        self.assertIn('identical native',report)
        self.assertIn('Compile ms | Prepare ms | First ms',report)
        self.assertIn('Cache reserved bytes',report)
        self.assertIn('Fastest peer',report)
        sample={'shots':2,'width':3,'measurements':[1,0,1,0,1,1]}
        self.assertEqual(expand(compact(sample)),sample)

    def test_subset_cannot_be_publication(self):
        with self.assertRaises(ValueError):validate(self.result)
        self.rejected(lambda r:r.update(subset=False))

    def test_rejects_invalid_timing_normalization_and_process_pairing(self):
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim']['warm_ns'].__setitem__(0,1))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim']['warm_totals_ns'].__setitem__(0,1))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['order'].reverse())
        self.rejected(lambda r:r.update(pairs=2))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].update(first_ns=[]))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].update(prepare_ns=0))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].update(compile_ns=0))

    def test_rejects_tuning_manipulation(self):
        self.rejected(lambda r:r['tuning'][0]['backends']['symft'].update(selected_batch=64))
        self.rejected(lambda r:r['tuning'][0]['backends']['symft']['trials'][0].update(median_ns=1))
        self.rejected(lambda r:r['tuning'][0]['backends']['clifft']['trials'].pop())

    def test_symft_scalar_is_a_distinct_public_default_candidate(self):
        self.assertEqual(batches('clifft'),BATCHES)
        self.assertEqual(batches('clifft-scheduled'),BATCHES)
        self.assertEqual(batches('symft'),['scalar']+BATCHES)
        self.assertEqual(default_batch('symft'),'scalar')
        circuit=Mock()
        circuit.compile_sampler.return_value=SimpleNamespace(max_active_qubits=1,num_measurements=1)
        with patch.dict(sys.modules,{'symft':SimpleNamespace(Circuit=Mock(return_value=circuit))}):
            for batch, expected in [('scalar',{'batch':False,'batch_size':0}),
                    (1,{'batch':True,'batch_size':1}),('auto',{'batch':True,'batch_size':0})]:
                prepare('symft','M 0\n',batch)
                circuit.compile_sampler.assert_called_with(**expected)
        self.rejected(lambda r:r['tuning'][0]['backends']['symft']['trials'].pop(0))
        self.rejected(lambda r:r['tuning'][0]['backends']['clifft']['trials'].insert(0,
            copy.deepcopy(r['tuning'][0]['backends']['symft']['trials'][0])))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['symft'].update(batch_enabled=True))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['symft'].update(batch_enabled=0))
        self.rejected(lambda r:r['tuning'][0]['backends']['symft']['trials'][0]['raw'].update(batch_enabled=True))

    def test_rejects_missing_changed_or_unbound_peer_environment(self):
        self.rejected(lambda r:r.pop('environment_guard'))
        self.rejected(lambda r:r['environment_guard'].pop('after'))
        self.rejected(lambda r:r['environment_guard']['after'].update(packages_sha256='0'*64))
        self.rejected(lambda r:r['environment_guard']['before'].update(symft_source_sha256='0'*64))
        self.rejected(lambda r:r['packages']['symft']['files'].pop('/synthetic/site-packages/symft/_native.so'))
        self.rejected(lambda r:r['peer_loaded_files'].pop('clifft'))
        for backend in BACKENDS[1:]:
            self.rejected(lambda r,b=backend:r['cases'][0]['runs'][0][b].update(isolated=False))
            self.rejected(lambda r,b=backend:r['cases'][0]['runs'][0][b].pop('loaded_files'))
            self.rejected(lambda r,b=backend:r['cases'][0]['runs'][0][b]['loaded_files'].pop(
                'symft._native' if b=='symft' else 'clifft._clifft_core'))
            self.rejected(lambda r,b=backend:r['tuning'][0]['backends'][b]['trials'][0]['raw'].update(isolated=False))
        actual=self.result['cases'][0]['runs'][0]['symft']
        self.assertEqual(bind_peer(actual,'symft',self.result['packages'],self.result['peer_loaded_files']),actual)
        shadow=copy.deepcopy(actual);shadow['loaded_files']['symft']['path']='/shadow/symft/__init__.py'
        with self.assertRaises(ValueError):bind_peer(shadow,'symft',self.result['packages'],self.result['peer_loaded_files'])
        changed=copy.deepcopy(actual);changed['loaded_files']['symft._native']['sha256']='1'*64
        with self.assertRaises(ValueError):bind_peer(changed,'symft',self.result['packages'],self.result['peer_loaded_files'])

    def test_inspection_subprocesses_require_isolated_python(self):
        with patch('run.invoke',return_value={}) as invoke:
            capture_packages('/synthetic/python','/synthetic/symft-python')
            capture_identities('/synthetic/python','/synthetic/symft-python')
        self.assertEqual(len(invoke.call_args_list),4)
        for call in invoke.call_args_list:self.assertEqual(call.args[0][1],'-I')

    def test_rejects_scalar_or_identity_transcript_forgery_after_rehash(self):
        def altered(r,backend,selected,mutate):
            owner=r['tuning'][0] if selected else r['validation'][0]
            payload=expand(owner['transcripts'][backend]);mutate(payload)
            owner['transcripts'][backend]=compact(payload)
            hashes=owner['selected_payload_sha256' if selected else 'payload_sha256']
            name=f'terminal-64-{backend}-selected.json' if selected else f'terminal-{backend}-validation.json'
            hashes[name]=hashed((json.dumps(payload)+'\n').encode())
        for selected in [False,True]:
            for mutate in [lambda p:p.update(batch='auto',batch_enabled=True),
                    lambda p:p.update(batch_enabled=True),lambda p:p.pop('batch_enabled')]:
                self.rejected(lambda r,s=selected,m=mutate:altered(r,'symft',s,m))
            if selected:
                self.rejected(lambda r:altered(r,'symft',True,lambda p:p.update(call_shots=1024)))
            for backend in BACKENDS[1:]:
                for mutate in [lambda p:p.update(isolated=False),lambda p:p.pop('loaded_files'),
                        lambda p:p['loaded_files'].clear()]:
                    self.rejected(lambda r,b=backend,s=selected,m=mutate:altered(r,b,s,m))

    def test_rejects_summaries_that_disagree_with_raw_transcripts(self):
        def false_counts(r):
            for counts in r['validation'][0]['parity_counts'].values():counts[0]=1
        self.rejected(false_counts)
        def false_selected(r):
            r['tuning'][0]['rstim_parity_counts'][0]=1
            for choice in r['tuning'][0]['backends'].values():choice['selected_parity_counts'][0]=1
        self.rejected(false_selected)
        self.rejected(lambda r:r['tuning'][0]['transcripts']['symft'].update(call_shots=1024))

    def test_rejects_easier_inputs_and_missing_provenance(self):
        self.rejected(lambda r:r['validation'][0].update(native_sha256='0'*64,adapted_sha256='0'*64))
        self.rejected(lambda r:r['harness'].pop('main.rs'))
        self.rejected(lambda r:r['validation'][0].update(threshold=1))
        self.rejected(lambda r:r['cases'].clear())
        self.rejected(lambda r:r.pop('finished'))

    def test_rejects_transcript_corruption_and_nonbinary_bits(self):
        self.rejected(lambda r:r['validation'][0]['transcripts']['rstim']['measurements'].update(data='!!!!'))
        with self.assertRaises(ValueError):compact({'shots':1,'width':1,'measurements':[.5]})

    def test_rejects_legacy_api_schema_and_result_identity(self):
        self.rejected(lambda r:r.update(schema='rstim.near-clifford-sota-results.v1'))
        self.rejected(lambda r:r.update(rstim_api='NearCliffordExecutor'))
        self.rejected(lambda r:r.pop('rstim_api'))
        self.rejected(lambda r:r.update(input_contract='lowered circuit'))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].update(api='NearCliffordExecutor'))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].pop('api'))

    def test_rejects_invalid_compiled_resource_metadata(self):
        for field, values in [('peak_active_rank',[-1,17,True,1.0]),
                ('cache_reserved_bytes',[-1,64*1024*1024+1,True,1.0])]:
            for value in values:
                self.rejected(lambda r,f=field,v=value:r['cases'][0]['runs'][0]['rstim'].update({f:v}))
        self.rejected(lambda r:r['cases'][0]['runs'][0]['rstim'].update(peak_active_rank=2))

    def test_rejects_legacy_dump_even_with_recomputed_transcript_hash(self):
        def altered(r, selected, mutate):
            owner=r['tuning'][0] if selected else r['validation'][0]
            payload=expand(owner['transcripts']['rstim']);mutate(payload)
            owner['transcripts']['rstim']=compact(payload)
            hashes=owner['selected_payload_sha256' if selected else 'payload_sha256']
            name='terminal-64-rstim-selected.json' if selected else 'terminal-rstim-validation.json'
            hashes[name]=hashed((json.dumps(payload)+'\n').encode())
        for selected in [False,True]:
            for mutate in [lambda p:p.update(api='NearCliffordExecutor'),lambda p:p.pop('api'),
                    lambda p:p.update(backend='clifft'),lambda p:p.update(rng='other'),
                    lambda p:p.update(cache_reserved_bytes=-1),lambda p:p.update(continuation=2**64)]:
                self.rejected(lambda r,s=selected,m=mutate:altered(r,s,m))
        self.rejected(lambda r:altered(r,True,lambda p:p.update(peak_active_rank=2)))

    def test_consumed_native_input_is_bound_in_all_evidence_paths(self):
        expected=self.result['validation'][0]['native_sha256']
        self.assertEqual(bind_input({'input_sha256':expected},expected),{'input_sha256':expected})
        for wrong in [None,'0'*64,'not-a-digest']:
            with self.assertRaises(ValueError):bind_input({'input_sha256':wrong},expected)
        with self.assertRaises(ValueError):bind_input({},expected)
        for backend in BACKENDS:
            for mutate in [lambda p:p.pop('input_sha256'),lambda p:p.update(input_sha256='0'*64)]:
                self.rejected(lambda r,b=backend,m=mutate:m(r['cases'][0]['runs'][0][b]))
                for selected in [False,True]:
                    def altered(r,b=backend,m=mutate,s=selected):
                        owner=r['tuning'][0] if s else r['validation'][0]
                        payload=expand(owner['transcripts'][b]);m(payload)
                        owner['transcripts'][b]=compact(payload)
                        hashes=owner['selected_payload_sha256' if s else 'payload_sha256']
                        name=f'terminal-64-{b}-selected.json' if s else f'terminal-{b}-validation.json'
                        # Rehash the raw transcript so rejection specifically checks
                        # the consumed-input binding, not stale transcript metadata.
                        hashes[name]=hashed((json.dumps(payload)+'\n').encode())
                    self.rejected(altered)
                if backend != 'rstim':
                    self.rejected(lambda r,b=backend,m=mutate:
                        m(r['tuning'][0]['backends'][b]['trials'][0]['raw']))

    def test_native_projection_preserves_mpp_noisy_measurement_and_width(self):
        text='# native instrument\nH 0\nMPP !Y0*X2 Z1\nMX(0.37) !2\nMR(1) 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n'
        projected=records_only(text)
        self.assertIn('MPP !Y0*X2 Z1\n',projected)
        self.assertIn('MX(0.37) !2\nMR(1) 1\n',projected)
        self.assertNotIn('DETECTOR',projected)
        self.assertNotIn('OBSERVABLE_INCLUDE',projected)
        self.assertEqual(physical_width(projected),physical_width(text))
        self.assertEqual(projected,records_only(projected))
        manifest=json.loads((HERE/'manifest.json').read_text())
        for case in manifest['cases']:
            original=(HERE/case['file']).read_text()
            native=records_only(original)
            self.assertEqual(physical_width(original),physical_width(native))
            self.assertEqual(sha(HERE/case['file']),case['sha256'])

    def test_observable_masks_merge_includes_and_cancel_repeated_records(self):
        text=('M 0 1 2 3 4 5\n'
            'OBSERVABLE_INCLUDE(0) rec[-6] rec[-4]\n'
            'OBSERVABLE_INCLUDE(0) rec[-4] rec[-1]\n'
            'OBSERVABLE_INCLUDE(1) rec[-5] rec[-2]\n'
            'OBSERVABLE_INCLUDE(1) rec[-5] rec[-2]\n')
        selected=masks(text,6)
        self.assertIn((0,5),selected)
        self.assertIn((),selected)
        # Preserve the component checks alongside each complete observable.
        self.assertIn((0,2),selected)
        self.assertIn((2,5),selected)
        self.assertIn((1,4),selected)
        self.assertNotIn((0,2,5),selected)

    def test_cultivation_masks_include_complete_logical_observable(self):
        for name,width,expected in [('msc3',21,(8,9,12,13,14)),
                ('msc5',112,tuple(range(83,94)))]:
            with self.subTest(fixture=name):
                text=(HERE/'fixtures'/f'{name}.stim').read_text()
                self.assertIn(expected,masks(text,width))

    def test_git_source_verification_requires_complete_inventory_and_content(self):
        # Mock Git objects, without reading or modifying the evolving production
        # tree. This is synthetic contract data, not a campaign artifact.
        result=copy.deepcopy(self.result)
        result['source_revision']='a'*40
        names=['Cargo.toml','Cargo.lock','rstim/Cargo.toml','rstim/src/test.rs']
        content=b'synthetic Git source bytes'
        result['source_files']={name:hashed(content) for name in names}
        def git(command,**kwargs):
            if command[1]=='ls-tree':return '\n'.join(names)+'\n'
            if command[1]=='show':return content
            raise ValueError('unexpected synthetic Git request')
        with patch('verify.subprocess.check_output',side_effect=git):
            validate(result,allow_subset=True,git_sources=True)
            for mutate in [lambda r:r['source_files'].pop('rstim/src/test.rs'),
                    lambda r:r['source_files'].update({'rstim/src/extra.rs':hashed(content)}),
                    lambda r:r['source_files'].update({'Cargo.lock':'0'*64})]:
                candidate=copy.deepcopy(result);mutate(candidate)
                with self.assertRaises(ValueError):
                    validate(candidate,allow_subset=True,git_sources=True)


if __name__=='__main__':unittest.main()
