#!/usr/bin/env python3
"""Capture bounded native-coordinate selection witnesses from the pinned source.

Computational one-/two-T factors make optional injection ineligible. Literal
third differences and original-domain truth tables check coordinates before
resource terms are recorded. These are correctness witnesses, not timings.
"""
from pathlib import Path
import argparse
from functools import reduce
import hashlib
import json
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
from witness_environment import native_reference

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--upstream", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
if args.out.exists():
    parser.error("output already exists; choose a new witness path")
def parity(x):return bin(x).count('1')&1
def value(x,constant,terms):return (constant+sum(c*parity(a&x)for a,c in terms))&7
def literal_coordinates(n,terms):
    f=lambda x:value(x,0,terms)
    original=[];pivots={}
    for i in range(n):
        for j in range(i,n):
            a,b=1<<i,1<<j;row=0
            for k in range(n):
                c=1<<k;third=(f(a^b^c)-f(a^b)-f(a^c)-f(b^c)+f(a)+f(b)+f(c)-f(0))&7
                assert third in [0,4]
                if third:row|=c
            reduced=row
            while reduced:
                pivot=reduced.bit_length()-1
                if pivot in pivots:reduced^=pivots[pivot]
                else:pivots[pivot]=reduced;original.append(row);break
    r=len(original);lifts=[]
    for j in range(r):
        lifts.append(next(x for x in range(1<<n)if sum(parity(a&x)<<k for k,a in enumerate(original))==1<<j))
    section=lambda y:reduce(int.__xor__,(lift for j,lift in enumerate(lifts)if y>>j&1),0)
    coefficients=[f(section(y))for y in range(1<<r)]
    for bit in range(r):
        for word in range(1<<r):
            if word>>bit&1:coefficients[word]=(coefficients[word]-coefficients[word^(1<<bit)])&7
    for word in range(1<<r):
        degree=bin(word).count('1')
        if degree==0:coefficients[word]=0
        elif degree==1:coefficients[word]&=1
        elif degree==2:coefficients[word]&=3
        elif degree>3:assert coefficients[word]==0
    table=[sum(c for mask,c in enumerate(coefficients)if mask&y==mask)&7 for y in range(1<<r)]
    return original,lifts,table
def computational(dimension,table,cancellation=False):
    terms=[]
    for x,phase in enumerate(table):
        coefficient=[0]*4;coefficient[phase%4]=1 if phase<4 else -1
        terms.append(dict(coefficient=coefficient,constraints=[[1<<j,(x>>j)&1]for j in range(dimension)],clifford_parities=[]))
    if cancellation:
        terms.extend([dict(coefficient=[c,0,0,0],constraints=[],clifford_parities=[])for c in [1,-1]])
    return dict(dimension=dimension,core_phase_mod8=table,terms=terms)
factor1=computational(1,[0,1]);factor2=computational(2,[0,1,1,2])
cases=[]
for n in range(7):
    terms=[[1<<j,1]for j in range(n)]
    for database in ['absent','empty','one','two','both','direct']:
        cases.append(dict(name=f'independent-{n}-{database}',n=n,constant=0,terms=terms,database=database))
for name,n,terms in [('coupled',4,[[3,1],[5,1],[7,1]]),('radical',5,[[3,1],[5,1],[6,1]]),('ccz',4,[[1,1],[2,1],[4,1],[3,7],[5,7],[6,7],[7,1]])]:
    for database in ['absent','empty','one','two','both','direct']:cases.append(dict(name=name+'-'+database,n=n,constant=5,terms=terms+[[9,2]],database=database))
with native_reference(args.upstream)as reference:
    results=[]
    for case in cases:
        n=case['n'];axes,lifts,table=literal_coordinates(n,case['terms']);mode=case['database'];rank=len(axes)
        entries=[]
        if mode in ['one','both','direct']:entries.append(factor1)
        if mode in ['two','both','direct']:entries.append(factor2)
        if mode=='direct':
            entries=[entry for entry in entries if entry['dimension']!=rank or entry['core_phase_mod8']!=table]
            entries.append(computational(rank,table,cancellation=True))
        database=None if mode=='absent'else dict(schema_version=1,entries=entries)
        tensor=dict(n=n,linear=[0]*n,upper=[0]*n,constant=case['constant'],terms=case['terms'],power=0,boundary=[[1<<j,0]for j in range(n)],output_count=n)
        out=reference.native.phase_decompose(tensor,dict(max_quotient_rank=8,max_terms=4096,max_pair_contractions=1<<20,max_table_bytes=64<<20),database,False)
        resource=out[1]
        assert list(resource['axes'])==axes and list(resource['section_lifts'])==lifts and list(resource['core_phase_mod8'])==table,case['name']
        direct=any(entry['dimension']==rank and entry['core_phase_mod8']==table for entry in entries)
        positive=table==[bin(x).count('1')%8 for x in range(1<<rank)]
        factors=all(any(entry['dimension']==min(2,rank-offset)and entry['core_phase_mod8']==[bin(x).count('1')for x in range(1<<min(2,rank-offset))]for entry in entries)for offset in range(0,rank,2))
        route='catalogue'if direct else 'positive_t_product'if database is not None and positive and factors else 'walsh'
        assert resource['database_hit']==(route!='walsh')and resource['database_product']==(route=='positive_t_product'),case['name']
        assert len(factor2['terms'])!=2
        results.append(dict(case=case,literal=dict(axes=axes,lifts=lifts,table=table),database=database,route=route,resource=resource))
    result=dict(schema='rstim.phase-selection-native-witness.v1',created_ns=time.time_ns(),passed=True,source_commit='56847478c840395735146ef1e1ccdc3d8aedc10f',native_sha256=reference.native_sha256,manifest_sha256=reference.manifest_sha256,optional_injection_ineligible_reason='All databases either lack a required one-/two-T entry or use four computational two-T indicators; the native two-entry parity-partition eligibility condition is false.',cases=results)

fixture = dict(
    schema="rstim.phase-selection-native.v1",
    upstream_commit=result["source_commit"],
    native_binary_sha256=result["native_sha256"],
    manifest_sha256=result["manifest_sha256"],
    generator_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    optional_injection="ineligible: computational two-T factors have four indicators",
    absorb=False,
    native_limits=dict(max_quotient_rank=8, max_terms=4096,
                       max_pair_contractions=1 << 20, max_table_bytes=64 << 20),
    cases=[dict(name=row["case"]["name"], n=row["case"]["n"],
                constant=row["case"]["constant"], terms=row["case"]["terms"],
                database=row["case"]["database"], route=row["route"],
                axes=row["literal"]["axes"], lifts=row["literal"]["lifts"],
                table=row["literal"]["table"],
                exact_entries=row["resource"]["exact_entries"])
           for row in result["cases"]],
)
with args.out.open("x", encoding="utf-8") as stream:
    json.dump(fixture, stream, indent=2, allow_nan=False)
    stream.write("\n")
print(json.dumps(dict(cases=len(fixture["cases"]), output=str(args.out),
                     native_sha256=fixture["native_binary_sha256"])))
