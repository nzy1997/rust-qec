"""Analytic channel checks: known probabilities, independent of either sampler."""
import math
import tempfile
from pathlib import Path
import numpy as np
from . import reference, channel_probes

from .probe_specs import noise_specs as cases


def evaluate(binary, sampler, shots=32768):
    specifications=list(cases())
    # One-sample two-sided Hoeffding bound, unioned over BOTH samplers and all events.
    tolerance=math.sqrt(math.log(4*len(specifications)/4e-7)/(2*shots))
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
        observations=evaluate(binary,defective,shots)
        failed=[r['case'] for r in observations if r['status']=='FAIL']
        mutations[channel]={'rejected':bool(failed),'failed_cases':failed,
                            'observations':observations}
    distributions = channel_probes.run(binary, sampler, shots)
    return {'distribution_probes':distributions, 'status':'PASS' if all(r['status']=='PASS' for r in records) and all(m['rejected'] for m in mutations.values()) and distributions['status']=='PASS' else 'FAIL',
            'method':'analytic single-bit/parity probabilities; one-sample bounds; live, absent, reset-restored and pre-loss controls',
            'familywise_alpha_bound':5e-7,'cases':records,'channel_deletion_mutations':mutations}
