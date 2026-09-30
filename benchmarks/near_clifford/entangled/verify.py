"""Fail closed on incomplete/tampered entangled campaigns and diagnostic evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from run import HERE, SCALE, ROOT, MATRIX, driver, sha, load, adapt_oracle
import math

INHERITED = load(SCALE/'run.py', 'entangled_verify_scale')

MODES = {'unprepared','cold_prepared_structured','first_prepared_structured',
         'first_prepared_flat','warm_prepared_structured','warm_prepared_flat'}

def require(condition,message):
    if not condition: raise ValueError(message)


def verify(r,binaries=None,git_sources=False):
    require(r.get('schema')=='near-clifford.entangled.v1','unknown schema')
    require(r.get('completed_utc') and not r['quick'],'incomplete/smoke campaign')
    require(r['pairs']==r['repetitions']==3,'not three paired processes/repetitions')
    require(r['matrix']==[list(t) for t in MATRIX],'matrix mismatch')
    require([(c['fixture'],c['shots']) for c in r['cases']]==MATRIX,'missing/duplicate configurations')
    require(r['entangled_inputs']=={n:sha(HERE/n) for n in ['run.py','report.py','fixtures.rs']},'entangled driver hash differs')
    require(r['scale_inputs']=={n:sha(SCALE/n) for n in ['run.py','main.rs','Cargo.unified.lock']},'scale inputs differ')
    require(r['harness_sha256']==hashlib.sha256(driver().encode()).hexdigest(),'adapted driver hash differs')
    require(r['runner_sha256']==sha(SCALE/'run.py'),'runner hash differs')
    require(r['fixtures_sha256']=={p.name:sha(p) for p in (SCALE/'fixtures').iterdir()},'historic fixtures differ')
    require(r['counter_names']==INHERITED.COUNTERS and r['snapshot_names']==INHERITED.SNAPSHOT,'diagnostic definitions differ')
    require(set(r['sources'])==set(r['diagnostics'])=={'baseline','candidate'},'unexpected source/diagnostic labels')
    require(set(r['verification_results'])=={'baseline','candidate','baseline-diagnostic','candidate-diagnostic'},'unexpected verification labels')
    require(r['sources']['baseline']['revision']!=r['sources']['candidate']['revision'],'identical source revisions')
    names={n for n,_ in MATRIX}
    require(set(r['verification'])==names,'missing semantic checks')
    for label in ['baseline','candidate']:
        require(set(r['diagnostics'][label])==names,'missing diagnostics')
        require(set(r['verification_results'][label])==names,'missing verification payloads')
        require(set(r['verification_results'][label+'-diagnostic'])==names,'missing diagnostic semantics')
        source=r['sources'][label]
        require(source['lock_sha256']==sha(SCALE/'Cargo.unified.lock'),'wrong lock')
        diagnostic=source.get('diagnostic',{})
        require(diagnostic.get('revision')==source['revision'] and diagnostic.get('lock_sha256')==source['lock_sha256']
            and diagnostic.get('oracle_sha256')==source['oracle_sha256'] and diagnostic.get('oracle_source_sha256')==source['oracle_source_sha256'],'missing/mismatched diagnostic source binding')
        for metadata in [source,diagnostic]:
            for key in ['binary_sha256','lock_sha256','near_clifford_source_sha256','oracle_sha256','oracle_source_sha256']:
                digest=metadata.get(key,'')
                require(len(digest)==64 and all(ch in '0123456789abcdef' for ch in digest),'missing/invalid source digest')
        if git_sources:
            for file,key in [('rstim/src/near_clifford.rs','near_clifford_source_sha256'),
                             ('rstim/tests/support/near_clifford_oracle.rs','oracle_source_sha256')]:
                raw=subprocess.check_output(['git','show',source['revision']+':'+file],cwd=ROOT)
                require(hashlib.sha256(raw).hexdigest()==source[key],'source/oracle differs')
                if key=='oracle_source_sha256':
                    require(hashlib.sha256(adapt_oracle(raw)).hexdigest()==source['oracle_sha256'],'adapted oracle differs')
                if key=='near_clifford_source_sha256':
                    overlay=INHERITED.instrument(raw.decode().strip()+'\n')
                    require(hashlib.sha256(overlay.encode()).hexdigest()==diagnostic[key],'diagnostic overlay differs')
        if binaries:
            require(sha(binaries/label/'near-clifford-scale')==source['binary_sha256'],'timing binary differs')
            require(sha(binaries/(label+'-diagnostic')/'near-clifford-scale')==source['diagnostic']['binary_sha256'],'diagnostic binary differs')
        for name in names:
            payload=r['verification_results'][label][name]
            require(payload==r['verification_results']['baseline'][name],'cross-revision outputs/RNG/physics differ')
            require(payload==r['verification_results'][label+'-diagnostic'][name],'overlay changed semantics')
            check=r['verification'][name]
            require(check['status']=='pass' and check['output_sha256']==hashlib.sha256(json.dumps(payload,sort_keys=True).encode()).hexdigest(),'semantic hash differs')
            require(payload['fixture']==name and len(payload['shots'])==16,'wrong verified fixture/shot count')
            require(type(payload['continuation']) is int and 0<=payload['continuation']<2**64,'invalid RNG continuation')
            physics=payload['physics']
            family,rank,width,depth=name.split('_');rank,width,depth=map(int,[rank,width,depth])
            exact=width<=16
            full_measurements=width if family!='rounds' else width-1+depth
            for shot in payload['shots']:
                require(len(shot['m'])==full_measurements and all(type(b) is bool for b in shot['m']) and shot['d']==shot['o']==[],'invalid verified shot records')
            reference=name if exact else f'{family}_{min(rank,8)}_{8 if family=="brick" else 9}_{depth}'
            _,rr,rw,rd=reference.split('_');rr,rw,rd=map(int,[rr,rw,rd])
            measurements=rw if family!='rounds' else rw-1+rd
            require(physics['reference_fixture']==reference and physics['full_width_oracle'] is exact,'incorrect oracle coverage disclosure')
            require(physics['born_probability_comparisons']==9*measurements,'incorrect oracle comparison count')
            require(physics['manual_executor_seed_equality'] is (family=='rounds') and physics['terminal_support_checks']==(0 if family=='rounds' else 3),'incorrect terminal/manual validation coverage')
            require(type(physics['reference_peak_rank']) is int and rr<=physics['reference_peak_rank']<=rw,'invalid reference rank')
            require(math.isfinite(physics['minimum_one_qubit_purity']) and 0.5-1e-10<=physics['minimum_one_qubit_purity']<0.99,'invalid entanglement witness')
            require(physics['status']=='pass' and physics['trajectories']==3 and physics['born_probability_comparisons']>0 and physics['minimum_one_qubit_purity']<0.99,'oracle/entanglement check missing')
            diag=r['diagnostics'][label][name]
            require(diag['fixture']==name and diag['semantic_verification']=='pass','diagnostic validation failed or mislabelled')
            for key in ['counters','warmup_counters']:
                values=diag.get(key,[])
                require(len(values)==8 and all(type(n) is int and n>=0 for n in values),'missing/invalid counters')
            for key in ['initial','after_warmup','after_probe']:
                require(len(diag[key])==5 and all(type(n) is int and n>=0 for n in diag[key]),'missing/invalid snapshot')
                require(diag[key][0]<=16 and diag[key][1]==1<<diag[key][0] and diag[key][4] in [0,1],'invalid rank/coefficient/plan snapshot')
                require(diag[key][2]<=diag[key][3],'cache exceeded node budget')
                require(diag[key][4]==int(family!='rounds'),'incorrect terminal planning flag')
    for case_index,c in enumerate(r['cases']):
        require(len(c['runs'])==3,'missing pairs')
        for i,t in enumerate(c['runs']):
            expected=['baseline','candidate'] if (case_index+i)%2==0 else ['candidate','baseline']
            require(t['order']==expected,'missing alternating paired order')
            for label in ['baseline','candidate']:
                require(t[label]['fixture']==c['fixture'] and t[label]['circuit']==r['verification_results'][label][c['fixture']]['circuit'],'timed circuit differs from verified circuit')
                require(len(t[label]['measurements'])==1,'unexpected shot modes')
                m=t[label]['measurements'][0]
                require(m['shots']==c['shots'] and set(m)-{'shots'}==MODES,'missing mode/count')
                for mode in MODES:
                    v=m[mode];raw=v['raw_ns']
                    require(len(raw)==3 and all(isinstance(n,int) and n>=0 for n in raw),'invalid repetitions')
                    require(v['median_ns']==sorted(raw)[1],'incorrect median')
    return f'PASS: {len(MATRIX)} entangled configurations, {len(names)} cross-revision physics/output/RNG checks, {2*len(names)} diagnostic checks'


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('results',type=Path)
    p.add_argument('--binaries',type=Path);p.add_argument('--git-sources',action='store_true');a=p.parse_args()
    print(verify(json.loads(a.results.read_text()),a.binaries,a.git_sources))

if __name__=='__main__':main()
