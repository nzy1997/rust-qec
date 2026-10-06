"""Adversarial verifier tests using explicitly synthetic consistency fixtures."""
import copy
import hashlib
import json
import math
import unittest
from evidence import compact, expand
from lowering import lower, records_only, physical_width
from run import BACKENDS, BATCHES, HERE, harness_inventory, masks, sha
from verify import validate


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
    packages={name:{'version':version,'files':{'test.so':'0'*64}}
        for name,version in manifest['baseline_versions'].items()}
    result={'schema':'rstim.near-clifford-sota-results.v1','subset':True,
        'started':'2026-10-07T00:00:00+00:00','finished':'2026-10-07T00:01:00+00:00',
        'host':'synthetic unit-test data','pairs':1,'repetitions':1,'validation_shots':n,
        'manifest_sha256':sha(HERE/'manifest.json'),'harness':harness_inventory(),
        'symft_source_revision':manifest['symft_source']['commit'],'symft_source_files':{'test.cpp':'0'*64},
        'source_files':{'test.rs':'0'*64},'binary_sha256':'0'*64,
        'packages':{**packages,'clifft_environment':packages}}
    def payload(backend,shots,batch='auto',dump=False,call_shots=None):
        raw={'backend':backend,'shots':shots,'width':width,'compile_ns':100,'prepare_ns':100,
            'rng':'SmallRng/rand-0.8.7','version':'0.11.0' if backend.startswith('clifft') else '0.1.1',
            'threads':1,'batch':batch,'peak_active_width':1,'simd_backend':'scalar',
            'first_ns':[100],'warm_ns':[500000.0],'warm_totals_ns':[50_000_000],'warm_calls':[100]}
        if dump:
            raw.update(measurements=[0]*(shots*width),call_shots=call_shots)
        return raw
    defaults={b:payload(b,n,dump=True,call_shots=n) for b in BACKENDS}
    validation={'id':case['id'],'passed':True,'width':width,'physical_width':physical_width(text),
        'native_sha256':hashed(records_only(text).encode()),
        'adapted_sha256':hashed(lower(records_only(text),physical_width(text)).encode()),
        'masks':selected,'parity_counts':{b:counts[:] for b in BACKENDS},'max_difference':0,
        'threshold':2*math.sqrt(math.log(4*24*len(selected)/.001)/(2*n)),
        'payload_sha256':{case['id']+'-'+b+'-validation.json':hashed((json.dumps(p)+'\n').encode())
            for b,p in defaults.items()},'transcripts':{b:compact(p) for b,p in defaults.items()}}
    chosen={b:payload(b,n,batch='auto' if b=='rstim' else 1,dump=True,call_shots=64) for b in BACKENDS}
    tuning={'id':case['id'],'shots':64,'rstim_parity_counts':counts[:],'backends':{},
        'selected_payload_sha256':{f"{case['id']}-64-{b}-selected.json":hashed((json.dumps(p)+'\n').encode())
            for b,p in chosen.items()},'transcripts':{b:compact(p) for b,p in chosen.items()}}
    for backend in BACKENDS[1:]:
        trials=[]
        for batch in BATCHES:
            raw=payload(backend,64,batch);raw['first_ns']=[]
            for key in ['warm_ns','warm_totals_ns','warm_calls']:raw[key]*=3
            trials.append({'batch':batch,'median_ns':500000.0,'raw':raw})
        tuning['backends'][backend]={'selected_batch':1,'trials':trials,
            'selected_parity_counts':counts[:],'selected_max_difference':0}
    result.update(validation=[validation],tuning=[tuning],cases=[{'id':case['id'],'shots':64,
        'runs':[{'order':BACKENDS[:],**{b:payload(b,64,batch='auto' if b=='rstim' else 1) for b in BACKENDS}}]}])
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
        self.assertIn('terminal',validate(self.result,allow_subset=True))
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

    def test_rejects_tuning_manipulation(self):
        self.rejected(lambda r:r['tuning'][0]['backends']['symft'].update(selected_batch=64))
        self.rejected(lambda r:r['tuning'][0]['backends']['symft']['trials'][0].update(median_ns=1))
        self.rejected(lambda r:r['tuning'][0]['backends']['clifft']['trials'].pop())

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


if __name__=='__main__':unittest.main()
