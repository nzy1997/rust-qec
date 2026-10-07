"""Adversarial resealing tests against an actual retained campaign."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
from verify import validate


def main():
    if len(sys.argv)!=2:
        raise ValueError('usage: test_contract.py CAMPAIGN_DIRECTORY')
    source=Path(sys.argv[1])
    original=(source/'events.jsonl').read_bytes() if (source/'events.jsonl').exists() else gzip.decompress((source/'events.jsonl.gz').read_bytes())
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
    def pre_harness_source(e,h):
        h['source_revision']=h['manifest']['baseline']
        h['sources']={key:value for key,value in h['sources'].items() if '/diagnostics/' not in key}
    mutations=[missing_timing,changed_timing,missing_mask,wrong_policy,changed_input,
               dropped_cell,forged_import,truncated_warm,wrong_batch,missing_tuning,
               empty_sources,forged_environment,optional_identity,pre_harness_source]
    if any(v['kind']=='lifetime' for v in events):
        mutations += [wrong_lifetime,missing_peer_lifetime,empty_peer_calls,empty_lifetime_digest,duplicate_lifetime]
    for mutate in mutations:
        with tempfile.TemporaryDirectory() as temporary:
            out=Path(temporary)
            shutil.copytree(source/'circuits',out/'circuits')
            e,h,c=copy.deepcopy(events),copy.deepcopy(header),copy.deepcopy(closure)
            mutate(e,h)
            if mutate in [empty_sources,pre_harness_source]: c['sources_after']=h['sources']
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
