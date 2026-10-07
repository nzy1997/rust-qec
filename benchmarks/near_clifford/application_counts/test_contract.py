"""Original-annotation semantic controls and resealed campaign mutations."""
import copy
import gzip
import json
from pathlib import Path
import sys
import tempfile
from common import annotations,raw_counts,require,digest
from verify import validate


def semantic_controls():
    # Eight exhaustive three-record strings: parity constraint and XOR-folded
    # repeated observable events independently have four and zero survivors/errors.
    text='M 0 1 2\nDETECTOR rec[-3] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-2]\n'
    masks=annotations(text)
    bits=[(row>>q)&1 for row in range(8) for q in range(3)]
    result=raw_counts(dict(width=3,shots=8,measurements=bits),masks)
    require(result==dict(attempted=8,accepted=4,discarded=4,logical_errors=0),'folded observable control failed')
    text='M 0\nREPEAT 3 {\nM 1\nDETECTOR rec[-1] rec[-2]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n'
    masks=annotations(text)
    require(masks==dict(width=4,detectors=[[0,1],[1,2],[2,3]],observable=[3]),'repeat record offsets differ')
    result=raw_counts(dict(width=4,shots=16,measurements=[(row>>q)&1 for row in range(16) for q in range(4)]),masks)
    require(result==dict(attempted=16,accepted=2,discarded=14,logical_errors=1),'repeat survivor control failed')
    print('PASS independent exhaustive annotation controls',flush=True)


def main():
    semantic_controls()
    if len(sys.argv)==1: return
    require(len(sys.argv)==2,'usage: test_contract.py [CAMPAIGN_DIRECTORY]')
    source=Path(sys.argv[1]);validate(source,allow_smoke=True)
    data=(source/'events.jsonl').read_bytes() if (source/'events.jsonl').exists() else gzip.decompress((source/'events.jsonl.gz').read_bytes())
    events=[json.loads(line) for line in data.splitlines()]
    header=json.loads((source/'header.json').read_text());closure=json.loads((source/'closure.json').read_text())
    def missing_timing(e,h): e.remove(next(v for v in e if v['kind']=='timing'))
    def wrong_executor(e,h): next(v for v in e if v['kind']=='timing')['result']['backend']='fake'
    def wrong_counts(e,h): next(v for v in e if v['kind']=='counts-validation' and v['backend']=='rstim')['result']['observations'][0]['accepted']+=1
    def wrong_batch(e,h): next(v for v in e if v['kind']=='timing' and v['backend']=='symft')['result']['batch']='fake'
    def optional_identity(e,h): del next(v for v in e if v['kind']=='timing' and v['backend']=='symft')['result']['loaded_files']
    def missing_capability(e,h): e.remove(next(v for v in e if v['kind']=='capability'))
    def wrong_input(e,h): next(v for v in e if v['kind']=='timing')['result']['input_sha256']='0'*64
    def zero_time(e,h): next(v for v in e if v['kind']=='timing')['result']['observations'][0]['elapsed_ns']=0
    def fabricated_rejection(e,h):
        id=next(v for v in e if v['kind']=='validation-check' and v['passed'])['id']
        e[:]=[v for v in e if v.get('id')!=id or v['kind'] not in ['validation-check','timing']]
        e.append(dict(kind='rejected',id=id,reason='counts finite witness disagreement'))
    for mutate in [missing_timing,wrong_executor,wrong_counts,wrong_batch,optional_identity,
                   missing_capability,wrong_input,zero_time,fabricated_rejection]:
        e,h,c=copy.deepcopy(events),copy.deepcopy(header),copy.deepcopy(closure);mutate(e,h)
        for index,event in enumerate(e): event['index']=index
        data=('\n'.join(json.dumps(v,separators=(',',':')) for v in e)+'\n').encode()
        c.update(events=len(e),events_sha256=digest(data))
        with tempfile.TemporaryDirectory() as temporary:
            out=Path(temporary);(out/'events.jsonl').write_bytes(data)
            (out/'header.json').write_text(json.dumps(h));(out/'closure.json').write_text(json.dumps(c))
            try: validate(out,allow_smoke=True)
            except (ValueError,KeyError,TypeError): print('PASS rejected',mutate.__name__,flush=True)
            else: raise ValueError('resealed mutation accepted: '+mutate.__name__)


if __name__=='__main__': main()
