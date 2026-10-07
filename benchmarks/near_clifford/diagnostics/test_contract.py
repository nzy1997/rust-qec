"""Adversarial resealing tests against an actual retained campaign."""
import copy
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
from verify import validate
from evidence_io import read_event_bytes


def main():
    if len(sys.argv)!=2:
        raise ValueError('usage: test_contract.py CAMPAIGN_DIRECTORY')
    source=Path(sys.argv[1])
    original=read_event_bytes(source)
    events=[json.loads(line) for line in original.splitlines()]
    header=json.loads((source/'header.json').read_text())
    closure=json.loads((source/'closure.json').read_text())
    validate(source,allow_smoke=True)
    def missing_timing(e,h):
        index=next(i for i,v in enumerate(e) if v['kind']=='timing')
        del e[index]
    def changed_timing(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='rstim')
        v['result']['warm'][0]['elapsed_ns']=1
    def missing_mask(e,h):
        v=next(v for v in e if v['kind']=='validation')
        del v['masks'][0]
    def wrong_policy(e,h):
        v=next(v for v in e if v['kind']=='validation')
        v['result']['arithmetic']='unknown'
    def changed_input(e,h):
        v=next(v for v in e if v['kind']=='validation')
        v['result']['input_sha256']='0'*64
    def dropped_cell(e,h):
        id=h['manifest']['selected_cells'].pop()
        e[:]=[v for v in e if v.get('id')!=id]
    def forged_import(e,h):
        v=next(v for v in e if v['kind']=='peer-validation')
        next(iter(v['result']['loaded_files'].values()))['sha256']='0'*64
    def truncated_warm(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='symft')
        v['result']['warm_totals_ns']=[]
        v['result']['warm_calls']=[]
        v['result']['warm_ns']=[-1]*len(v['result']['warm_ns'])
    def wrong_batch(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='symft')
        v['result']['batch']='unvalidated'
    def missing_tuning(e,h):
        e[:]=[v for v in e if v['kind']!='peer-tuning']
    def empty_sources(e,h):
        h['sources']={}
    def forged_environment(e,h):
        h['symft_sources']={'fake':'0'*64}
    def wrong_lifetime(e,h):
        v=next(v for v in e if v['kind']=='lifetime')
        v['result']['arithmetic']='unknown'
        v['result']['cache_bytes']=-1
    def missing_peer_lifetime(e,h):
        index=next(i for i,v in enumerate(e) if v['kind']=='peer-lifetime')
        del e[index]
    def empty_peer_calls(e,h):
        v=next(v for v in e if v['kind']=='peer-lifetime')
        row=v['result']['histories'][0]
        row.update(calls=[],sampling_ns=0,phase_sum_ns=row['compile_ns']+row['prepare_ns'])
    def optional_identity(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='symft')
        del v['result']['loaded_files']
        del v['result']['isolated']
    def empty_lifetime_digest(e,h):
        for v in e:
            if v['kind']=='lifetime':
                for row in v['result']['histories']:
                    row.update(outputs_sha256='',continuation=[])
    def duplicate_lifetime(e,h):
        v=next(v for v in e if v['kind']=='peer-lifetime')
        e.append(copy.deepcopy(v))
    def wrong_peer_executor(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='clifft')
        v['result']['backend']='clifft-scheduled'
    def wrong_tuning_executor(e,h):
        v=next(v for v in e if v['kind']=='peer-tuning' and v['backend']=='clifft')
        v['trials'][0]['result']['backend']='clifft-scheduled'
    def duplicate_lifecycle_witness(e,h):
        v=next(v for v in e if v['kind']=='lifetime-peer-validation')
        e.insert(e.index(v)+1,copy.deepcopy(v))
    def pre_harness_source(e,h):
        h['source_revision']=h['manifest']['baseline']
        h['sources']={key:value for key,value in h['sources'].items() if '/diagnostics/' not in key}
    def wrong_declared_order(e,h):
        v=next(v for v in e if v['kind']=='timing')
        v['order']=v['order'][::-1]
    def wrong_emission_order(e,h):
        indices=[i for i,v in enumerate(e) if v['kind']=='timing'][:2]
        a,b=indices;e[a],e[b]=e[b],e[a]
    def pair_out_of_range(e,h):
        v=copy.deepcopy(next(v for v in e if v['kind']=='timing'))
        v['pair']=h['manifest']['pairs'];e.append(v)
    def boolean_pair(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['pair']==0)
        v['pair']=False
    def unknown_timing_backend(e,h):
        v=copy.deepcopy(next(v for v in e if v['kind']=='timing'))
        v.update(backend='unknown',result={'status':'error'});e.append(v)
    def unselected_timing(e,h):
        v=copy.deepcopy(next(v for v in e if v['kind']=='timing'))
        v.update(id=next(c['id'] for c in h['manifest']['cells'] if c['id'] not in h['manifest']['selected_cells']),
                 result={'status':'error'});e.append(v)
    def missing_cold(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='rstim')
        del v['result']['cold']
    def negative_cold_first(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='rstim')
        v['result']['cold'][0]['first_ns']=-1
    def negative_cold_prepare(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='rstim')
        v['result']['cold'][0]['prepare_ns']=-1
    def excessive_cold_reservation(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='rstim')
        v['result']['cold'][0]['cache_reserved_bytes']=v['result']['cache_bytes']+1
    def negative_peer_compile(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='symft')
        v['result']['compile_ns']=-1
    def truncated_peer_first(e,h):
        v=next(v for v in e if v['kind']=='timing' and v['backend']=='symft')
        v['result']['first_ns'].pop()
    def wrong_lifetime_emission(e,h):
        indices=[i for i,v in enumerate(e) if v['kind'] in ['lifetime','peer-lifetime']][:2]
        a,b=indices;e[a],e[b]=e[b],e[a]
    def failed_lifetime_envelope(e,h):
        v=copy.deepcopy(next(v for v in e if v['kind']=='lifetime'))
        v.update(id='msc5/lifetime/unknown/c0/strict',result={'status':'error'});e.append(v)
    def negative_lifetime_compile(e,h):
        v=next(v for v in e if v['kind']=='lifetime')
        row=v['result']['histories'][0]
        row['compile_ns']=-1
        row['phase_sum_ns']=row['compile_ns']+row['prepare_ns']+row['sampling_ns']
    def negative_lifetime_prepare(e,h):
        v=next(v for v in e if v['kind']=='lifetime')
        row=v['result']['histories'][0]
        row['prepare_ns']=-1
        row['phase_sum_ns']=row['compile_ns']+row['prepare_ns']+row['sampling_ns']
    def excessive_lifetime_reservation(e,h):
        v=next(v for v in e if v['kind']=='lifetime')
        v['result']['histories'][0]['calls'][0]['reserved_before']=v['result']['cache_bytes']+1
    def excessive_lifetime_top_reservation(e,h):
        v=next(v for v in e if v['kind']=='lifetime')
        v['result']['cache_reserved_bytes']=v['result']['cache_bytes']+1
    def negative_peer_lifetime_compile(e,h):
        v=next(v for v in e if v['kind']=='peer-lifetime')
        row=v['result']['histories'][0]
        row['compile_ns']=-1
        row['phase_sum_ns']=row['compile_ns']+row['prepare_ns']+row['sampling_ns']
    mutations=[missing_timing,changed_timing,missing_mask,wrong_policy,changed_input,
               dropped_cell,forged_import,truncated_warm,wrong_batch,missing_tuning,
               empty_sources,forged_environment,optional_identity,pre_harness_source,
               wrong_peer_executor,wrong_tuning_executor,wrong_declared_order,wrong_emission_order,
               pair_out_of_range,boolean_pair,unknown_timing_backend,
               missing_cold,negative_cold_first,negative_cold_prepare,excessive_cold_reservation,
               negative_peer_compile,truncated_peer_first]
    if any(c['id'] not in header['manifest']['selected_cells'] for c in header['manifest']['cells']):
        mutations.append(unselected_timing)
    if any(v['kind']=='lifetime' for v in events):
        mutations += [wrong_lifetime,missing_peer_lifetime,empty_peer_calls,empty_lifetime_digest,
                      duplicate_lifetime,duplicate_lifecycle_witness,wrong_lifetime_emission,
                      failed_lifetime_envelope,negative_lifetime_compile,negative_lifetime_prepare,
                      excessive_lifetime_reservation,excessive_lifetime_top_reservation,
                      negative_peer_lifetime_compile]
    def missing_helper_inventory(e,h):
        del h['sources']['benchmarks/near_clifford/evidence_io.py']
    if 'benchmarks/near_clifford/evidence_io.py' in header['sources']:
        mutations.append(missing_helper_inventory)
    for mutate in mutations:
        with tempfile.TemporaryDirectory() as temporary:
            out=Path(temporary)
            shutil.copytree(source/'circuits',out/'circuits')
            e,h,c=copy.deepcopy(events),copy.deepcopy(header),copy.deepcopy(closure)
            mutate(e,h)
            if mutate in [empty_sources,pre_harness_source,missing_helper_inventory]: c['sources_after']=h['sources']
            for index,v in enumerate(e): v['index']=index
            data=('\n'.join(json.dumps(v,separators=(',',':')) for v in e)+'\n').encode()
            c.update(events=len(e),events_sha256=hashlib.sha256(data).hexdigest())
            (out/'events.jsonl').write_bytes(data)
            (out/'header.json').write_text(json.dumps(h))
            (out/'closure.json').write_text(json.dumps(c))
            try:
                validate(out,allow_smoke=True)
            except (ValueError,KeyError,TypeError):
                print('PASS rejected',mutate.__name__)
            else:
                raise ValueError('resealed mutation accepted: '+mutate.__name__)


if __name__=='__main__': main()
