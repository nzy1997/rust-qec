import hashlib,json,subprocess,time,statistics,sys
from pathlib import Path
roots={'baseline':Path('/Users/nzy/.codex/worktrees/near-clifford-noise-lazy-final/rstim'),'candidate':Path('/Users/nzy/.codex/worktrees/near-clifford-packet-rejection/rstim')}
out=roots['candidate']/'drafts/compact-replay-confirmation';out.mkdir(exist_ok=False)
def probe(root):
 return root/'drafts'/('compact-replay-ablation' if root==roots['candidate'] else 'recorded-tail-ablation')
names=['msc_d3_inject_cultivate_p1e-3','msc_d5_inject_cultivate_p1e-3','pure_surface_d7_r7_p1e-3','pure_surface_d9_r9_p1e-3']
cases=[(name,shots,policy) for name in names for shots in [1,64,1024] for policy in ['strict','fused']]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def identity(root):
 paths=[root/'benchmarks/near_clifford/application_counts/fixtures'/f'{name}.stim' for name in names]+[probe(root)/f'{name}.masks.json' for name in names]+list((root/'rstim/src').rglob('*.rs'))+[root/'Cargo.toml',root/'Cargo.lock',root/'rstim/Cargo.toml',probe(root)/'main.rs',probe(root)/'Cargo.toml',probe(root)/'Cargo.lock']
 return {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'dirty':subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True),'sources':{str(p.relative_to(root)):digest(p) for p in paths},'binary':digest(probe(root)/'target/release/counts-path-ablation'),'driver_sha256':digest(Path(__file__))}
before={key:identity(root) for key,root in roots.items()}
header={'schema':'exploratory.compact-replay-ablation.v1','started':time.time(),'identities':before,'pairs':5,'observations_per_process':7,'minimum_warm_observation_ns':50000000,'scope':'Same-host recorded rejection-tail source versus lazy compact scalar counts replay; warm-only paired scout; first-call preparation excluded by explicit warmups; complete source/binary/driver identity closure; no peers or SOTA claim','cases':cases,'rustc':subprocess.check_output(['rustc','-Vv'],text=True),'host':subprocess.check_output(['uname','-a'],text=True),'environment':{key:__import__('os').environ.get(key) for key in ['RUSTFLAGS','CARGO_PROFILE_RELEASE_OPT_LEVEL','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','RAYON_NUM_THREADS']}}
(out/'header.json').write_text(json.dumps(header,indent=2))
events=[]
def execute(key,case,action):
 name,shots,policy=case;root=roots[key];base=probe(root)
 args=[str(base/'target/release/counts-path-ablation'),str(root/'benchmarks/near_clifford/application_counts/fixtures'/f'{name}.stim'),str(base/f'{name}.masks.json'),str(shots),policy,'native',action,'7']
 result=json.loads(subprocess.check_output(args,cwd=root,text=True,timeout=180))
 event={'index':len(events),'route':key,'case':case,'action':action,'result':result};events.append(event)
 with (out/'events.jsonl').open('a') as f:f.write(json.dumps(event,separators=(',',':'))+'\n')
 return result
for case in cases:
 for key in roots: execute(key,case,'validate')
print('validation complete',flush=True)
measurements={str(case):{key:[] for key in roots} for case in cases}
for round_index in range(5):
 rotated=cases[round_index:]+cases[:round_index]
 if round_index%2:rotated.reverse()
 for index,case in enumerate(rotated):
  order=list(roots)
  if (index+round_index)%2:order.reverse()
  for key in order:
   result=execute(key,case,'bench');obs=result['observations']
   assert len(obs)==7 and all(o['elapsed_ns']>=50000000 and o['calls']>0 and o['ns_per_call']==o['elapsed_ns']/o['calls'] for o in obs)
   measurements[str(case)][key].append(statistics.median(o['ns_per_call'] for o in obs))
 print('round',round_index+1,'complete',flush=True)
after={key:identity(root) for key,root in roots.items()}
assert before==after,'source/binary changed during measurement'
summary=[]
for case in cases:
 m=measurements[str(case)];ratios=[a/b for a,b in zip(m['baseline'],m['candidate'])]
 summary.append({'case':case,'baseline_ns':statistics.median(m['baseline']),'candidate_ns':statistics.median(m['candidate']),'speedup':statistics.median(m['baseline'])/statistics.median(m['candidate']),'paired_range':[min(ratios),max(ratios)]})
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
(out/'closure.json').write_text(json.dumps({'finished':time.time(),'events':len(events),'events_sha256':digest(out/'events.jsonl'),'identities_after':after},indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
