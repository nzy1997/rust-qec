import hashlib,json,subprocess,time,statistics,sys
from pathlib import Path
roots={'baseline':Path('/Users/nzy/.codex/worktrees/near-clifford-packet-rejection/rstim'),'candidate':Path('/Users/nzy/.codex/worktrees/near-clifford-noise-lazy-final/rstim')}
out=roots['candidate']/'drafts/intern-cold-comparison';out.mkdir(exist_ok=False)
def probe(root):
 return root/'drafts'/'intern-cold-ablation'
names=['msc_d3_inject_cultivate_p1e-3','msc_d5_inject_cultivate_p1e-3','pure_surface_d7_r7_p1e-3','pure_surface_d9_r9_p1e-3']
cases=[(name,shots,policy) for name in names for shots in [1,64,1024] for policy in ['strict','fused']]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def identity(root):
 paths=[root/'benchmarks/near_clifford/application_counts/fixtures'/f'{name}.stim' for name in names]+[probe(root)/f'{name}.masks.json' for name in names]+list((root/'rstim/src').rglob('*.rs'))+[root/'Cargo.toml',root/'Cargo.lock',root/'rstim/Cargo.toml',probe(root)/'main.rs',probe(root)/'Cargo.toml',probe(root)/'Cargo.lock']
 return {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'dirty':subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True),'sources':{str(p.relative_to(root)):digest(p) for p in paths},'binary':digest(probe(root)/'target/release/counts-path-ablation'),'driver_sha256':digest(Path(__file__))}
before={key:identity(root) for key,root in roots.items()}
header={'schema':'exploratory.coefficient-intern-cold.v1','started':time.time(),'identities':before,'pairs':5,'observations_per_process':32,'minimum_cold_observation_ns':None,'scope':'Same-host compact counts replay source versus admission-only exact-bit state interning; fresh compilation and fresh sampler for every fixed seed, compile/prepare/first-call phase times separately retained, seed setup and output destruction excluded; no 50ms minimum per cold observation; no peers or SOTA claim','cases':cases,'rustc':subprocess.check_output(['rustc','-Vv'],text=True),'host':subprocess.check_output(['uname','-a'],text=True),'environment':{key:__import__('os').environ.get(key) for key in ['RUSTFLAGS','CARGO_PROFILE_RELEASE_OPT_LEVEL','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','RAYON_NUM_THREADS']}}
(out/'header.json').write_text(json.dumps(header,indent=2))
events=[]
def execute(key,case,action):
 name,shots,policy=case;root=roots[key];base=probe(root)
 args=[str(base/'target/release/counts-path-ablation'),str(root/'benchmarks/near_clifford/application_counts/fixtures'/f'{name}.stim'),str(base/f'{name}.masks.json'),str(shots),policy,'native',action,'7']
 result=json.loads(subprocess.check_output(args,cwd=root,text=True,timeout=180))
 event={'index':len(events),'route':key,'case':case,'action':action,'result':result};events.append(event)
 with (out/'events.jsonl').open('a') as f:f.write(json.dumps(event,separators=(',',':'))+'\n')
 return result
measurements={str(case):{key:[] for key in roots} for case in cases}
for round_index in range(5):
 rotated=cases[round_index:]+cases[:round_index]
 if round_index%2:rotated.reverse()
 for index,case in enumerate(rotated):
  order=list(roots)
  if (index+round_index)%2:order.reverse()
  outputs={}
  for key in order:
   result=execute(key,case,'cold');obs=result['observations'];outputs[key]=obs
   if len(obs)!=32 or [o['seed'] for o in obs]!=list(range(739,771)):raise ValueError('cold seed coverage')
   for o in obs:
    if any(not isinstance(o[f],int) or o[f]<0 for f in ['compile_ns','prepare_ns','first_ns','attempted','accepted','logical_errors']):raise ValueError('cold values')
    if o['attempted']!=case[1] or not 0<=o['logical_errors']<=o['accepted']<=o['attempted'] or len(o['continuation'])!=16:raise ValueError('cold semantics')
   values={phase:statistics.median(o[phase] for o in obs) for phase in ['compile_ns','prepare_ns','first_ns']}
   values['phase_sum_ns']=statistics.median(o['compile_ns']+o['prepare_ns']+o['first_ns'] for o in obs)
   measurements[str(case)][key].append(values)
  for left,right in zip(outputs['baseline'],outputs['candidate']):
   if any(left[f]!=right[f] for f in ['seed','attempted','accepted','logical_errors','continuation']):raise ValueError(('cross-source cold counts/RNG mismatch',case,left['seed']))
 print('round',round_index+1,'complete',flush=True)
after={key:identity(root) for key,root in roots.items()}
assert before==after,'source/binary changed during measurement'
summary=[]
for case in cases:
 for phase in ['compile_ns','prepare_ns','first_ns','phase_sum_ns']:
  m=measurements[str(case)];baseline=[o[phase] for o in m['baseline']];candidate=[o[phase] for o in m['candidate']];ratios=[a/b for a,b in zip(baseline,candidate)]
  summary.append({'case':case,'phase':phase,'baseline_ns':statistics.median(baseline),'candidate_ns':statistics.median(candidate),'speedup':statistics.median(baseline)/statistics.median(candidate),'paired_range':[min(ratios),max(ratios)]})
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
(out/'closure.json').write_text(json.dumps({'finished':time.time(),'events':len(events),'events_sha256':digest(out/'events.jsonl'),'identities_after':after},indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
