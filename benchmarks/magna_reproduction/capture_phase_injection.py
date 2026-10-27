#!/usr/bin/env python3
"""Regenerate bounded injection witnesses from the pinned native source."""
from pathlib import Path
from functools import reduce
from fractions import Fraction
from collections import Counter,defaultdict
import json,sys,random,hashlib
import argparse
parser=argparse.ArgumentParser(description="Capture exact pinned-native T-pair injection correctness witnesses.")
parser.add_argument("--upstream",type=Path,required=True)
parser.add_argument("--out",type=Path,required=True)
args=parser.parse_args()
if args.out.exists():parser.error("output already exists; choose a fresh path")
sys.path.insert(0,str(Path(__file__).resolve().parent))
from witness_environment import native_reference
def require(ok,detail):
 if not ok:raise RuntimeError(detail)
def parity(x):return x.bit_count()&1
def phase(x,terms,constant=0):return(constant+sum(c*parity(a&x)for a,c in terms))&7
def omega(e):return tuple(Fraction((1 if e%8<4 else -1)if j==e%4 else 0)for j in range(4))
def add(a,b):return tuple(x+y for x,y in zip(a,b))
def multiply(a,b):
 out=[Fraction(0)]*4
 for i,x in enumerate(a):
  for j,y in enumerate(b):out[(i+j)%4]+=x*y*(1 if i+j<4 else -1)
 return tuple(out)
def scalar(raw):return tuple(Fraction(c)*Fraction(2)**raw[1]for c in raw[0])
def entry_value(entries,x):
 result=omega(0);result=tuple(v*0 for v in result)
 for coeff,constraints,parities in entries:
  if all(parity(a&x)==b for a,b in constraints):result=add(result,multiply(scalar(coeff),omega(2*sum(c*parity(a&x)for a,c in parities))))
 return result
def coordinates(n,terms):
 f=lambda x:phase(x,terms);span={0};axes=[]
 for i in range(n):
  for j in range(i,n):
   a,b=1<<i,1<<j;row=0
   for k in range(n):
    c=1<<k;third=(f(a^b^c)-f(a^b)-f(a^c)-f(b^c)+f(a)+f(b)+f(c)-f(0))&7
    require(third in [0,4],'third derivative');row|=(third==4)<<k
   if row not in span:axes.append(row);span|={s^row for s in list(span)}
 r=len(axes);lifts=[next(x for x in range(1<<n)if sum(parity(a&x)<<k for k,a in enumerate(axes))==1<<j)for j in range(r)]
 values=[f(reduce(int.__xor__,(v for j,v in enumerate(lifts)if y>>j&1),0))for y in range(1<<r)]
 monomials=values[:]
 for j in range(r):
  for y in range(1<<r):
   if y>>j&1:monomials[y]=(monomials[y]-monomials[y^(1<<j)])&7
 canonical=[0]*len(values)
 for mask,c in enumerate(monomials):
  d=mask.bit_count()
  if d==1:canonical[mask]=c&1
  elif d==2:canonical[mask]=c&3
  elif d==3:canonical[mask]=c
  elif d>3:require(c==0,'noncubic monomial')
 table=[sum(c for mask,c in enumerate(canonical)if mask&y==mask)&7 for y in range(1<<r)]
 counter=Counter()
 for mask,c in enumerate(canonical):
  if c:
   require(c==1<<(mask.bit_count()-1),'canonical monomial weight')
   sub=mask
   while sub:counter[sub]+=1 if sub.bit_count()&1 else -1;sub=(sub-1)&mask
 core=[[mask,c%8]for mask,c in sorted(counter.items())if c%8]
 require(all(phase(y,core)==table[y]for y in range(1<<r)),'canonical parity reconstruction')
 return axes,lifts,table,core
def pairing(core):
 remaining=[a for a,c in core if c&1];rotations=len(remaining);span={0};groups=[]
 while len(remaining)>1:
  cosets=defaultdict(list)
  for mask in remaining:cosets[min(mask^s for s in span)].append(mask)
  remaining=[]
  for key,masks in sorted(cosets.items()):
   groups.extend([masks[j:j+2]for j in range(0,len(masks)-1,2)])
   if len(masks)&1:remaining.append(masks[-1])
  if len(remaining)<2:break
  counts=Counter(min(a^b^s for s in span)for i,a in enumerate(remaining)for b in remaining[i+1:])
  best=min(counts,key=lambda key:(-counts[key],key));span|={s^best for s in list(span)}
 if remaining:groups.append(remaining)
 rows=[reduce(int.__xor__,group,0)for group in groups];span={0}
 for row in rows:span|={s^row for s in list(span)}
 pivots=sorted({v.bit_length()-1 for v in span if v})
 independent=[next(v for v in sorted(span)if v>>p&1 and all(not(v>>j&1)for j in pivots if j!=p))for p in pivots]
 return rotations,groups,independent
def injected_terms(r,core,groups,independent):
 pivots={v.bit_length()-1 for v in independent};free=[j for j in range(r)if j not in pivots];entries=[]
 for rhs in range(1<<len(independent)):
  shift=next(x for x in range(1<<r)if all(not(x>>j&1)for j in free)and sum(parity(row&x)<<j for j,row in enumerate(independent))==rhs)
  directions=[next(x for x in range(1<<r)if x>>j&1 and all(not(x>>k&1)for k in free if k!=j)and all(not parity(row&x)for row in independent))for j in free]
  outcomes=[parity(reduce(int.__xor__,group,0)&shift)for group in groups]
  def even(x):return(sum((c-(c&1))*parity(a&x)for a,c in core)+sum(2*parity(group[0]&x)for group,outcome in zip(groups,outcomes)if len(group)==2 and outcome==0))&7
  values=[even(shift^reduce(int.__xor__,(v for j,v in enumerate(directions)if y>>j&1),0))for y in range(1<<len(free))];constant=values[0];coeff=omega(sum(outcomes)+constant)
  polynomial=values[:]
  for j in range(len(free)):
   for y in range(len(values)):
    if y>>j&1:polynomial[y]=(polynomial[y]-polynomial[y^(1<<j)])&7
  counter=Counter()
  for j,wire in enumerate(free):
   require(polynomial[1<<j]%2==0,'even restricted linear');counter[1<<wire]+=polynomial[1<<j]//2
   for k in range(j+1,len(free)):
    q=polynomial[(1<<j)|(1<<k)];require(q in [0,4],'Clifford restricted quadratic')
    if q:counter[1<<wire]+=1;counter[1<<free[k]]+=1;counter[(1<<wire)|(1<<free[k])]-=1
  parities=[[mask,c%4]for mask,c in sorted(counter.items())if c%4]
  entries.append([[list(map(int,coeff)),0],[[row,(rhs>>j)&1]for j,row in enumerate(independent)],parities])
 return entries
def factors(ineligible=False):
 one=dict(dimension=1,core_phase_mod8=[0,1],terms=[dict(coefficient=[1,0,0,0],constraints=[[1,0]],clifford_parities=[]),dict(coefficient=[0,1,0,0],constraints=[[1,1]],clifford_parities=[])])
 two=dict(dimension=2,core_phase_mod8=[0,1,1,2],terms=[dict(coefficient=[1,0,0,0],constraints=[[3,0]],clifford_parities=[[1,1]]),dict(coefficient=[0,1,0,0],constraints=[[3,1]],clifford_parities=[])])
 if ineligible:
  two['terms']=[]
  for x,e in enumerate([0,1,1,2]):two['terms'].append(dict(coefficient=list(map(int,omega(e))),constraints=[[1,x&1],[2,x>>1]],clifford_parities=[]))
 return [one,two]
def baseline_count(r,table,mode):
 if mode=='direct':return len(table)
 if mode not in ['absent','empty','one']and table==[x.bit_count()%8 for x in range(1<<r)]:return 1<<((r+1)//2)if mode!='ineligible'else 1<<r
 if mode=='one'and r==1:return 2
 return sum(any(sum((1 if parity(x&y)==0 else -1)*omega(e)[j]for y,e in enumerate(table))for j in range(4))for x in range(1<<r))
cases=[];rng=random.Random(947)
for n in range(3,7):
 for trial in range(12):
  terms=[[mask,2*rng.randrange(4)+1]for mask in range(1,1<<n)if rng.randrange(4)==0]
  cases.append(dict(name=f'random-{n}-{trial}',n=n+1,constant=trial%8,terms=terms+[[1<<n,2]],mode='eligible'))
for n in range(0,7):
 for mode in ['eligible','absent','empty','one','ineligible','direct']:
  cases.append(dict(name=f'positive-{n}-{mode}',n=n+1,constant=3,terms=[[1<<j,1]for j in range(n)]+[[1<<n,2]],mode=mode))
ccz=[[mask,1 if mask.bit_count()&1 else 7]for mask in range(1,8)]
cases.extend([dict(name='ccz-'+mode,n=5,constant=5,terms=ccz+[[9,2]],mode=mode)for mode in ['eligible','absent','ineligible','direct']])

for index,triples in enumerate([[(0,1,2),(0,3,4),(1,3,5)],[(0,1,3),(1,2,4),(0,2,5)],[(0,1,2),(0,3,4),(1,2,5),(2,4,5)]]):
 terms=[]
 for triple in triples:
  for subset in range(1,8):terms.append([sum(1<<triple[j]for j in range(3)if subset>>j&1),1 if subset.bit_count()&1 else 7])
 for mode in ['eligible','absent']:cases.append(dict(name=f'cubic-zero-{index}-{mode}',n=7,constant=6,terms=terms+[[64,2]],mode=mode))

results=[];selected=0;reduction64=0;dependent=0;walshzeros=0
with native_reference(args.upstream)as reference:
 for case in cases:
  n=case['n'];axes,lifts,table,core=coordinates(n,case['terms']);r=len(axes);mode=case['mode'];entries=[]if mode in ['absent','empty']else factors(mode=='ineligible')[:1 if mode=='one'else 2]
  if mode=='direct':
   direct=[]
   for x,e in enumerate(table):direct.append(dict(coefficient=list(map(int,omega(e))),constraints=[[1<<j,(x>>j)&1]for j in range(r)],clifford_parities=[]))
   entries=[e for e in entries if e['dimension']!=r or e['core_phase_mod8']!=table]+[dict(dimension=r,core_phase_mod8=table,terms=direct)]
  database=None if mode=='absent'else dict(schema_version=1,entries=entries)
  tensor=dict(n=n,linear=[0]*n,upper=[0]*n,constant=case['constant'],terms=case['terms'],power=0,boundary=[[1<<j,0]for j in range(n)],output_count=n)
  resource=reference.native.phase_decompose(tensor,dict(max_quotient_rank=8,max_terms=4096,max_pair_contractions=1<<24,max_table_bytes=64<<20),database,False)[1]
  require(list(resource['axes'])==axes and list(resource['section_lifts'])==lifts and list(resource['core_phase_mod8'])==table,case['name']+' coordinates')
  baseline=baseline_count(r,table,mode)
  # Rank one/two direct hits use their supplied actual factor lengths.
  direct=next((entry for entry in entries if entry['dimension']==r and entry['core_phase_mod8']==table),None)
  if direct:baseline=len(direct['terms'])
  eligible=mode in ['eligible','direct']and len(entries)==2 and all(entry['dimension']in[1,2]for entry in entries)
  # A direct core certificate can coexist with the two partition factors.
  eligible=mode in ['eligible','direct']and all(any(e['dimension']==d and len(e['terms'])==2 and e['terms'][0]['constraints']==[[mask,0]]and e['terms'][1]['constraints']==[[mask,1]]for e in entries)for d,mask in [(1,1),(2,3)])
  rotations,groups,independent=pairing(core)if eligible and r>1 and baseline>1 else(0,[],[])
  candidate=1<<len(independent)if rotations else 0;chosen=bool(candidate and candidate<baseline)
  require(len(resource['exact_entries'])==(candidate if chosen else baseline),case['name']+' selected count')
  if chosen:
   predicted=injected_terms(r,core,groups,independent)
   require(json.loads(json.dumps(resource['exact_entries']))==predicted,case['name']+' exact Native entries')
   selected+=1;reduction64+=baseline==64 and candidate==8;dependent+=len(groups)>len(independent)
  for y,e in enumerate(table):require(entry_value(resource['exact_entries'],y)==omega(e),case['name']+' exact core reconstruction')
  walshzeros+=mode=='absent'and baseline<len(table)
  original=[];labels=[];sums=[]
  for update in range(4):
   values=[(0,0,0,0)for _ in range(8)];total=(0,0,0,0)
   for x in range(1<<n):
    q=sum(((j+update)%4)*((x>>j)&1)for j in range(n))+2*sum(((j+k)%2)*((x>>j)&1)*((x>>k)&1)for j in range(n)for k in range(j+1,n))
    e=phase(x,case['terms'],case['constant'])+update+2*update*parity(x)+2*q;v=multiply((Fraction(1,4),Fraction(1,4),Fraction(1,4),Fraction(-1,4)),omega(e));label=1
    for j in range(n):
     if x>>j&1:label^=[1,3,1,0][j%4]
    total=add(total,v);values[label]=add(values[label],v)
   sums.append([[str(x.numerator),str(x.denominator)]for x in total]);labels.append([[[str(x.numerator),str(x.denominator)]for x in v]for v in values])
  results.append(dict(case=case,axes=axes,lifts=lifts,table=table,core=core,database=database,original_terms=baseline,rotations=rotations,groups=groups,constraint_rows=independent,candidate_terms=candidate,selected=chosen,resource=resource,expected_sums=sums,expected_labels=labels))
 require(selected>0,'selected coverage');require(reduction64>0,'64-to-8 coverage');require(dependent>0,'dependent constraints coverage');require(walshzeros>0,'genuine Walsh zero coverage')
 result=dict(schema='rstim.phase-injection-native.v1',passed=True,upstream_commit='56847478c840395735146ef1e1ccdc3d8aedc10f',native_sha256=reference.native_sha256,manifest_sha256=reference.manifest_sha256,cases=results,counts=dict(cases=len(results),selected=selected,reductions64to8=reduction64,dependent=dependent,walsh_zero_cases=walshzeros),scope='Finite truth tables, deterministic finite-span pairing and exact restricted Clifford witnesses; correctness examples, not a performance sample.')
 result['generator_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
 with args.out.open('x')as stream:json.dump(result,stream,indent=2);stream.write('\n')
 print(json.dumps(result['counts']))
