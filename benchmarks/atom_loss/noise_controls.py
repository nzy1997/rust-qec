"""Analytic channel checks: known probabilities, independent of either sampler."""
import math
import tempfile
from pathlib import Path
import numpy as np
from . import reference

P = .17


def cases():
    # Observed event is a value bit, except DEPOLARIZE2 where it is Z parity.
    for name, probability, basis in [('X_ERROR',P,''),('Y_ERROR',P,''),
                                     ('Z_ERROR',P,'H 0\n'),('DEPOLARIZE1',2*P/3,''),
                                     ('DEPOLARIZE2',8*P/15,'')]:
        target='0 1' if name=='DEPOLARIZE2' else '0'
        noise=f'{name}({P}) {target}\n'
        for state in ['alive','lost','restored']:
            before='LOSS(1) 0\n' if state!='alive' else ''
            if state=='restored': before+='R 0\n'
            text='R 0 1\n'+before+basis+noise+basis+'ML 0 1'
            # For a lost wire inspect the surviving wire: noise touching loss must skip.
            columns=[3] if state=='lost' else ([1,3] if name=='DEPOLARIZE2' else [1])
            yield f'{name}_{state}',text,columns,0. if state=='lost' else probability,name
    # Nonzero channel before loss must still affect its surviving partner.
    yield 'DEPOLARIZE2_before_loss','R 0 1\nDEPOLARIZE2(0.17) 0 1\nLOSS(1) 0\nML 0 1',[3],8*P/15,'DEPOLARIZE2'


def evaluate(binary, sampler, shots=32768):
    specifications=list(cases())
    # One-sample two-sided Hoeffding bound, unioned over BOTH samplers and all events.
    tolerance=math.sqrt(math.log(4*len(specifications)/5e-7)/(2*shots))
    records=[]
    with tempfile.TemporaryDirectory(prefix='analytic-noise-') as tmp:
        for name,text,columns,expected,channel in specifications:
            native=sampler(binary,text,shots,991,Path(tmp))
            independent=reference.sample(text,shots,773)
            rates=[float(np.logical_xor.reduce(rows[:,columns],axis=1).mean()) for rows in [native,independent]]
            passed=all(abs(rate-expected)<=tolerance for rate in rates)
            # Zero-probability controls are exact and must not admit a single event.
            if expected==0: passed=all(rate==0 for rate in rates)
            records.append({'case':name,'channel':channel,'expected_probability':expected,
                            'rust_probability':rates[0],'reference_probability':rates[1],
                            'shots_per_sampler':shots,'tolerance':tolerance,'status':'PASS' if passed else 'FAIL'})
    return records


def run(binary, sampler, shots=32768):
    records=evaluate(binary,sampler,shots)
    mutations={}
    for channel in ['X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2']:
        def defective(binary,text,shots,seed,work):
            altered='\n'.join(line for line in text.splitlines() if not line.startswith(channel+'('))
            return sampler(binary,altered,shots,seed,work)
        # Execute the same acceptance test with a real defective native input.
        failed=[r['case'] for r in evaluate(binary,defective,shots) if r['status']=='FAIL']
        mutations[channel]={'rejected':bool(failed),'failed_cases':failed}
    return {'status':'PASS' if all(r['status']=='PASS' for r in records) and all(m['rejected'] for m in mutations.values()) else 'FAIL',
            'method':'analytic single-bit/parity probabilities; one-sample bounds; live, absent, reset-restored and pre-loss controls',
            'familywise_alpha_bound':5e-7,'cases':records,'channel_deletion_mutations':mutations}
