"""Independent integer truth tables for the pinned five phase quotient roles."""
import argparse, ast, hashlib, json
from collections import Counter
from itertools import combinations
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args()
source=(args.upstream/'tests/core/test_phase_quotient_optimization.py').read_bytes()
if hashlib.sha256(source).hexdigest()!='77c7db4efbcc00f2026a7df617fd6cfb86d1364b137f374e5f566876f2ce74fc':
 raise ValueError('pinned phase quotient source hash mismatch')
tree=ast.parse(source)
# Only the original pure mathematical oracle functions are compiled. In
# particular, importing this file never loads native_testing or builds code.
pure=ast.Module(body=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in {'phase','normalize'}],type_ignores=[])
scope={'Counter':Counter};exec(compile(pure,'original-pure-oracle','exec'),scope)
phase=scope['phase']
def row(n,terms,**extra):
 values=[phase(terms,x)for x in range(1<<n)]
 counts=[values.count(j)for j in range(8)]
 return dict(n=n,terms=terms,values=values,sum_coefficients=[counts[j]-counts[j+4]for j in range(4)],**extra)
def rank(values,n):
 size=1<<n;radical=[]
 for u in range(size):
  good=True
  for v in range(size):
   for w in range(size):
    d=sum((1 if k.bit_count()%2 else -1)*values[((u if k&1 else 0)^(v if k&2 else 0)^(w if k&4 else 0))]for k in range(8))%8
    if d:good=False
  if good:radical.append(u)
 assert len(radical).bit_count()==1
 return n-(len(radical).bit_length()-1)
small=[]
for n in range(4):
 monomials=[sum(1<<j for j in js)for d in (1,2,3)for js in combinations(range(n),d)]
 for selection in range(1<<len(monomials)):
  terms=[]
  for index,mask in enumerate(monomials):
   if not selection>>index&1:continue
   sub=mask
   while sub:
    terms.append((sub,1 if sub.bit_count()%2 else -1));sub=(sub-1)&mask
  v=row(n,terms,selection=selection);v['rank']=rank(v['values'],n);small.append(v)
assert len(small)==139
base=((1,1),(3,3),(5,5),(7,7));updates=[]
for word in range(16):
 terms=[(a,-c if word>>j&1 else c)for j,(a,c)in enumerate(base)]+[(10,2*(word%4)),(3,5),(3,-5)]
 updates.append(row(4,terms,word=word))
artifact={'schema':'rstim.phase-quotient-literals.v1','upstream_commit':'56847478c840395735146ef1e1ccdc3d8aedc10f','upstream_module_sha256':hashlib.sha256(source).hexdigest(),'producer_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Pure integer oracle; no native test helper execution, cache/ABI identity or paper claim.','small':small,'base':row(4,base),'updates':updates,'cancelled':row(4,[(7,1),(7,-1),(10,6)]),'low_rank':row(2,[(1,7),(2,3),(3,1)])}
p=args.out;p.open('x').write(json.dumps(artifact,indent=2)+'\n')
print(json.dumps({'small':len(small),'updates':len(updates),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size,'low_rank_sum':artifact['low_rank']['sum_coefficients']}))
