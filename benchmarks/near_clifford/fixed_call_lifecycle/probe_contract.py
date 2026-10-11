"""Hardened local smoke replay; measurements are UNCACHED REFERENCE ONLY."""
from pathlib import Path
import hashlib,json,sys
def unique(pairs):
 d={}
 for k,v in pairs:
  if k in d:raise ValueError('duplicate key '+k)
  d[k]=v
 return d
def reject(v):raise ValueError('nonfinite JSON '+v)
def read(p):return json.loads(p.read_text(),object_pairs_hook=unique,parse_constant=reject)
def require(condition):
 if not condition:raise ValueError('smoke contract rejected')
def integer(v):return type(v) is int and v>=0
def check(d,shots,policy,cache,mode,input_sha):
 require(set(d)==set('schema timing_contract action input_sha256 arithmetic cache_factor shots compile_ns prepare_ns teardown_ns final_compile_prepare_sampling_teardown_ns cache_reserved_final peak_active_rank calls prefixes'.split()))
 require(d['schema']=='rstim.fixed-call-lifecycle.draft.v1' and d['timing_contract']=='fixed-seeded-counts-prefix-v1')
 require(type(d['shots']) is int)
 require(d['action']==mode and d['input_sha256']==input_sha and d['arithmetic']==policy and d['cache_factor']==cache and d['shots']==shots)
 require(all(integer(d[k]) for k in ['compile_ns','prepare_ns','teardown_ns','final_compile_prepare_sampling_teardown_ns','cache_reserved_final','peak_active_rank']))
 rows=d['calls'];maximum=8 if mode=='validate' else 256;require(len(rows)==maximum)
 keys=set('call_index seed elapsed_ns attempted accepted discarded logical_errors cache_reserved_after'.split())
 if mode=='validate':keys|={'rng_tail','measurements'}
 for i,r in enumerate(rows):
  require(set(r)==keys and all(integer(r[k]) for k in keys-{'rng_tail','measurements'}))
  require(r['call_index']==i and r['seed']==1739+i and r['attempted']==shots and r['logical_errors']<=r['accepted']<=shots and r['discarded']==shots-r['accepted'])
  if cache=='off':require(r['cache_reserved_after']==0)
  if mode=='validate':
   require(len(r['rng_tail'])==16 and all(integer(v) and v<2**64 for v in r['rng_tail']))
   require(len(r['measurements'])==shots and all(isinstance(v,list) and all(type(bit) is bool for bit in v) for v in r['measurements']))
 horizons=[1,2,4,8] if mode=='validate' else [1,2,4,8,16,32,64,128,256]
 require([v['calls'] for v in d['prefixes']]==horizons)
 for v in d['prefixes']:
  require(set(v)=={'calls','sampling_ns','compile_prepare_sampling_ns'} and all(integer(x) for x in v.values()))
  total=sum(r['elapsed_ns'] for r in rows[:v['calls']]);require(v['sampling_ns']==total and v['compile_prepare_sampling_ns']==d['compile_ns']+d['prepare_ns']+total)
 require(d['cache_reserved_final']==rows[-1]['cache_reserved_after'])
 require(d['final_compile_prepare_sampling_teardown_ns']==d['compile_ns']+d['prepare_ns']+sum(r['elapsed_ns'] for r in rows)+d['teardown_ns'])
def main():
 p=Path(sys.argv[1]);input_sha=hashlib.sha256(Path(sys.argv[2]).read_bytes()).hexdigest();positive=0
 expected=set()
 for shots in [1,64,1024]:
  for policy in ['strict','fused']:
   pair=[]
   for cache in ['default','off']:
    name=f'validate-{shots}-{policy}-{cache}.stdout';expected.add(name);d=read(p/name);check(d,shots,policy,cache,'validate',input_sha);positive+=sum(v['logical_errors'] for v in d['calls']);pair.append(d)
   for a,b in zip(pair[0]['calls'],pair[1]['calls']):
    fields=['seed','attempted','accepted','discarded','logical_errors','rng_tail','measurements'];require({k:a[k] for k in fields}=={k:b[k] for k in fields})
 require({v.name for v in p.glob('validate-*.stdout')}==expected and positive>0)
 check(read(p/'bench.stdout'),1,'strict','default','bench',input_sha)
 print('PASS strict identities/seeds/indices/counts/sums/schema; measurement parity is reference-only; local debug smoke only')
if __name__=='__main__':main()
