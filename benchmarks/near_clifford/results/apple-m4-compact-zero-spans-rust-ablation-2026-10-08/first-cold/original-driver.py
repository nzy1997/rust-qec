import hashlib,json,subprocess,time,statistics,sys
from pathlib import Path
roots={'baseline':Path('/Users/nzy/.codex/worktrees/near-clifford-four-coefficients/rstim/drafts/compact-zero-spans-master-baseline'),'candidate':Path('/Users/nzy/.codex/worktrees/near-clifford-four-coefficients/rstim')}
out=roots['candidate']/'drafts/compact-zero-spans-master-cold-comparison';out.mkdir(exist_ok=False)
def probe(root):
 return root/'drafts'/'compact-zero-spans-cold-probe'
names=['msc_d3_inject_cultivate_p1e-3','msc_d5_inject_cultivate_p1e-3','pure_surface_d7_r7_p1e-3','pure_surface_d9_r9_p1e-3']
cases=[(name,shots,policy) for name in names for shots in [1,64,1024] for policy in ['strict','fused']]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def identity(root):
 paths=[root/'benchmarks/near_clifford/application_counts/fixtures'/f'{name}.stim' for name in names]+[probe(root)/f'{name}.masks.json' for name in names]+list((root/'rstim/src').rglob('*.rs'))+[root/'Cargo.toml',root/'Cargo.lock',root/'rstim/Cargo.toml',probe(root)/'main.rs',probe(root)/'Cargo.toml',probe(root)/'Cargo.lock']
 return {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'dirty':subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True),'sources':{str(p.relative_to(root)):digest(p) for p in paths},'binary':digest(probe(root)/'target/release/counts-path-ablation'),'driver_sha256':digest(Path(__file__)),'environment':{k:__import__('os').environ.get(k) for k in ['RUSTFLAGS','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS']}}
expected_heads={'baseline':'e1453a7b634275d3055ef64b2f4a7b33d3995870','candidate':'d989531d837fe874a77ac05745f5d77ea5639d52'}
before={key:identity(root) for key,root in roots.items()}
for role,info in before.items():
 if info['head']!=expected_heads[role] or info['dirty']!='':raise ValueError('unexpected cold source')
retained={}
for role,root in roots.items():
 directory=out/'probe'/role;directory.mkdir(parents=True)
 entries={}
 for name in ['Cargo.toml','Cargo.lock','main.rs']+[n+'.masks.json' for n in names]+['build.log']:
  original=probe(root)/name;snapshot=directory/name;__import__('shutil').copyfile(original,snapshot)
  if digest(original)!=digest(snapshot):raise ValueError('cold retained bytes changed')
  entries[name]={'path':str(snapshot.relative_to(out)),'sha256':digest(snapshot)}
 retained[role]=entries
__import__('shutil').copyfile(Path(__file__),out/'original-driver.py')
header={'schema':'exploratory.strict-coefficient-pairs-cold.v1','started':time.time(),'identities':before,'retained':retained,'pairs':5,'observations_per_process':32,'minimum_cold_observation_ns':None,'scope':'Same-host exact master e145 versus compact zero spans d989; fresh compilation and fresh sampler for every fixed seed, compile/prepare/first-call phase times separately retained, seed setup and output destruction excluded; no 50ms minimum per cold observation; no peers or SOTA claim','cases':cases,'rustc':subprocess.check_output(['rustc','-Vv'],text=True),'host':subprocess.check_output(['uname','-a'],text=True),'environment':{key:__import__('os').environ.get(key) for key in ['RUSTFLAGS','CARGO_PROFILE_RELEASE_OPT_LEVEL','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS']}}
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
 for role in roots:
  v=execute(role,case,'validate')
  if v.get('status')!='ok' or v.get('seeds')!=4 or v.get('exact_records_counts_rng') is not True or v.get('native_counts_rng') is not True:raise ValueError('cold source prevalidation failed')
print('All 48 cold-probe four-seed validations passed',flush=True)
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
if before!=after:raise ValueError('source/binary changed during measurement')
for role,entries in retained.items():
 for name,entry in entries.items():
  if digest(out/entry['path'])!=entry['sha256'] or digest(probe(roots[role])/name)!=entry['sha256']:raise ValueError('cold retained input/log changed')
if digest(out/'original-driver.py')!=before['candidate']['driver_sha256']:raise ValueError('cold retained driver changed')
binary_dir=out/'production-binaries';binary_dir.mkdir()
for role,root in roots.items():
 target=binary_dir/(role+'.bin');__import__('shutil').copyfile(probe(root)/'target/release/counts-path-ablation',target)
 if digest(target)!=before[role]['binary']:raise ValueError('cold archived binary changed')
(binary_dir/'receipt.json').write_text(json.dumps({'identities':before},indent=2)+'\n')
summary=[]
for case in cases:
 for phase in ['compile_ns','prepare_ns','first_ns','phase_sum_ns']:
  m=measurements[str(case)];baseline=[o[phase] for o in m['baseline']];candidate=[o[phase] for o in m['candidate']];ratios=[a/b for a,b in zip(baseline,candidate)]
  summary.append({'case':case,'phase':phase,'baseline_ns':statistics.median(baseline),'candidate_ns':statistics.median(candidate),'speedup':statistics.median(baseline)/statistics.median(candidate),'paired_range':[min(ratios),max(ratios)]})
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
(out/'closure.json').write_text(json.dumps({'finished':time.time(),'events':len(events),'events_sha256':digest(out/'events.jsonl'),'identities_after':after},indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
