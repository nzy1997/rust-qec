"""Recompute sampling report decisions from complete observations (stdlib only).

Known probe definitions are the contract; verdicts, thresholds and summaries are
not authorities. SciPy produces low-rate intervals/Fisher p-values; this module
independently recomputes them for verification without that runtime dependency.
"""
import math
from functools import lru_cache
from .probe_specs import CASES, KNOWN, noise_specs, distribution_specs, low_specs
from .shot_data import circuit_layout


def require(condition, message):
    if not condition:
        raise ValueError('Sampling report contract: '+message)


def integer(value, minimum=0):
    require(type(value) is int and value>=minimum,'invalid integer observation')
    return value


def close(value, expected, *, tolerance=1e-12):
    require(type(value) in [int,float] and math.isfinite(value) and
            math.isclose(value,expected,rel_tol=tolerance,abs_tol=tolerance),'inconsistent derived number')


def named(records, names):
    require(isinstance(records,list) and len(records)==len(names),'incomplete case list')
    require({r['case'] for r in records}==set(names),'duplicate or unknown case')
    return {r['case']:r for r in records}


def counts(values, size, shots):
    require(isinstance(values,list) and len(values)==size,'incomplete counts')
    require(all(type(k) is int and 0<=k<=shots for k in values),'invalid counts')
    return values


def probabilities(values, size, shots):
    require(isinstance(values,list) and len(values)==size,'incomplete probabilities')
    for value in values:
        require(type(value) in [int,float] and math.isfinite(value) and 0<=value<=1,'invalid probability')
        close(value*shots,round(value*shots),tolerance=1e-10)
    close(sum(values),1.)
    return values


def marginal(joint, bits):
    return [sum(p for i,p in enumerate(joint) if i&(1<<b)) for b in range(bits)]


def checked_rates(actual, expected, tolerance):
    return all(abs(a-p)<=tolerance and (p not in [0,1] or a==p) for a,p in zip(actual,expected))


def verdict(record, passed):
    require(record['status']==('PASS' if passed else 'FAIL'),'verdict contradicts observations')
    require(passed,'failed sampling observation')


@lru_cache(maxsize=128)
def binomial_interval(n,p,tail):
    """Quantiles via normalized PMF recurrence around the mode, avoiding CDF cancellation."""
    if p==0: return [0,0]
    if p==1: return [n,n]
    if p>.5:
        lo,hi=binomial_interval(n,1-p,tail)
        return [n-hi,n-lo]
    mode=int((n+1)*p);lower=[];weight=1.;k=mode
    while k>0 and weight>1e-300:
        weight*=k/(n-k+1)*(1-p)/p;k-=1;lower.append(weight)
    weights=list(reversed(lower))+[1.];first=k;weight=1.;k=mode
    while k<n and weight>1e-300:
        weight*=(n-k)/(k+1)*p/(1-p);k+=1;weights.append(weight)
    total=math.fsum(weights)
    cumulative=0.;lo=None;hi=None
    for offset,weight in enumerate(weights):
        cumulative+=weight
        if lo is None and cumulative>=tail*total: lo=first+offset
        if cumulative>=(1-tail)*total:
            hi=first+offset;break
    require(lo is not None and hi is not None,'binomial quantile did not converge')
    return [lo,hi]


def fisher_pvalue(a,n,b,m):
    """Two-sided conditional hypergeometric test, summed in log space."""
    successes=a+b;total=n+m
    low=max(0,successes-m);high=min(n,successes)
    if low==high:return 1.
    def choose(top,k):
        return math.lgamma(top+1)-math.lgamma(k+1)-math.lgamma(top-k+1)
    denominator=choose(total,successes)
    def logp(k):return choose(n,k)+choose(m,successes-k)-denominator
    observed=logp(a)
    # lgamma subtraction at ~65k introduces ~1e-10 error for equal-probability tails.
    return min(1.,math.fsum(math.exp(value) for k in range(low,high+1)
                           if (value:=logp(k))<=observed+1e-8))


def verify_sampler(report):
    close(report['familywise_alpha_bound'],1.2e-6)
    require(report['negative_skipped_gate_mutation_rejected'] is True and
            report['unsupported_reference_operation_rejected'] is True,'missing semantic controls')
    small=named(report['cases'],CASES)
    for name,text in CASES.items():
        case=small[name];n=integer(case['shots_per_sampler'],32768)
        bits=circuit_layout(text)[0];size=1<<bits
        require(set(case['histogram_counts'])=={'rust','reference'},'missing histogram backend')
        hist=[counts(case['histogram_counts'][key],size,n) for key in ['rust','reference']]
        require(all(sum(h)==n for h in hist),'histogram does not cover all shots')
        tolerance=2*math.sqrt(math.log(4*64*len(CASES)/5e-7)/(2*n))
        delta=max(abs(a-b)/n for a,b in zip(*hist))
        known=True
        if name in KNOWN:
            target=sum(bit<<i for i,bit in enumerate(KNOWN[name]))
            known=all(h[target]==n for h in hist)
        close(case['max_bin_difference'],delta);close(case['tolerance'],tolerance)
        require(case['known_answer_pass'] is known,'known answer verdict mismatch')
        verdict(case,delta<=tolerance and known)
    analytic=report['analytic_noise_controls'];specs=list(noise_specs());noise=named(analytic['cases'],[s[0] for s in specs])
    close(analytic['familywise_alpha_bound'],5e-7)
    for name,text,columns,expected,channel in specs:
        case=noise[name];n=integer(case['shots_per_sampler'],32768)
        require(case['channel']==channel,'wrong channel')
        tolerance=math.sqrt(math.log(4*len(specs)/4e-7)/(2*n))
        close(case['expected_probability'],expected);close(case['tolerance'],tolerance)
        rates=[case[k] for k in ['rust_probability','reference_probability']]
        for p in rates:probabilities([p,1-p],2,n)
        verdict(case,checked_rates(rates,[expected]*2,tolerance))
    distribution=analytic['distribution_probes'];specs=distribution_specs()
    records=named(distribution['cases'],[s['name'] for s in specs])
    events=sum(len(s['expected'])+len(s['columns']) for s in specs)
    close(distribution['familywise_alpha_bound'],1e-7)
    for spec in specs:
        case=records[spec['name']];n=integer(case['shots_per_sampler'],32768)
        expected=spec['expected'];bits=len(spec['columns']);marg=marginal(expected,bits)
        require(case['channel']==spec['channel'] and case['expected_joint']==expected and case['expected_marginals']==marg,'wrong distribution specification')
        tolerance=math.sqrt(math.log(4*events/1e-7)/(2*n));close(case['tolerance'],tolerance)
        passed=True
        for backend in ['rust','reference']:
            obs=case[backend];joint=probabilities(obs['joint'],len(expected),n)
            require(len(obs['marginals'])==bits,'missing marginal')
            for a,b in zip(obs['marginals'],marginal(joint,bits)):close(a,b)
            passed &= checked_rates(joint,expected,tolerance) and checked_rates(obs['marginals'],marg,tolerance)
        verdict(case,passed)
    low=report['low_probability_controls'];unit=low['analytic'];specs=low_specs()
    records=named(unit['cases'],[s[0] for s in specs]);events=sum(len(p)+len(cols)+1 for _,_,cols,p in specs)
    close(low['familywise_alpha_bound'],2e-7);close(unit['familywise_alpha_bound'],1e-7)
    for name,text,columns,expected in specs:
        case=records[name];primitive=.001 if not name.startswith('LOSS') else float(name[5:])
        n=integer(case['shots_per_sampler'],max(262144,math.ceil(26/primitive)) if name.startswith('LOSS') else 262144)
        close(case['primitive_probability'],primitive)
        ps=expected+marginal(expected,len(columns))+[1-expected[0]]
        require(case['expected_probabilities']==ps,'wrong low-rate probabilities')
        intervals=[binomial_interval(n,p,1e-7/(4*events)) for p in ps]
        require(case['accepted_counts']==intervals,'wrong low-rate acceptance interval')
        require(set(case['counts'])=={'rust','reference'},'missing low-rate backend')
        passed=True
        for values in case['counts'].values():
            values=counts(values,len(ps),n);joint=values[:len(expected)]
            require(sum(joint)==n,'low-rate histogram incomplete')
            require(values[len(expected):-1]==marginal(joint,len(columns)) and values[-1]==n-joint[0],'low-rate derived counts inconsistent')
            passed &= all(lo<=k<=hi for k,(lo,hi) in zip(values,intervals))
        verdict(case,passed)
    actual=low['real_circuit'];n=integer(actual['shots_per_sampler'],65536)
    require(actual['mode']=='measurements_blinded' and actual['pauli_probability']==.001 and actual['loss_probability']==.003,'wrong real-circuit configuration')
    require(actual['scoring_key_check']=={'status':'PASS','checked_shots':n},'missing scoring key check')
    close(actual['familywise_alpha_bound'],1e-7)
    comparison=actual['comparison']
    all_names=[f'measurement_{i}' for i in range(50)]+[f'detector_{i}' for i in range(16)]+['observable']+[f'adjacent_detector_joint_{i}' for i in range(15)]
    expected_names={f'input_{mask}/{group}/{event}' for mask in [0,1] for group,names in [('all',all_names),('no_visible_loss',[f'detector_{i}' for i in range(16)])] for event in names}
    rows=comparison['events'];require(len(rows)==len(expected_names) and {e['event'] for e in rows}==expected_names,'incomplete real-circuit events')
    threshold=1e-7/len(rows);close(comparison['threshold'],threshold,tolerance=1e-16)
    totals={};failed=[]
    for e in rows:
        a,native,b,ref=[integer(e[k]) for k in ['rust_events','rust_shots','reference_events','reference_shots']]
        require(native>0 and ref>0 and a<=native and b<=ref,'invalid Fisher counts')
        mask,group,_=e['event'].split('/');key=(mask,group)
        require(totals.setdefault(key,(native,ref))==(native,ref),'inconsistent stratum denominators')
        p=fisher_pvalue(a,native,b,ref)
        close(e['pvalue'],p,tolerance=2e-8)
        if p<threshold:failed.append(e['event'])
    for backend in [0,1]:
        require(sum(totals[(f'input_{m}','all')][backend] for m in [0,1])==n,'missing logical-input shots')
        require(all(totals[(f'input_{m}','no_visible_loss')][backend]<=totals[(f'input_{m}','all')][backend] for m in [0,1]),'invalid conditioned denominator')
    require(comparison['failed_events']==failed,'stale Fisher verdict')
    verdict(comparison,not failed)
    # Top-level PASS cannot override any rejected/missing constituent observation.
    for r in [actual,unit,low,distribution,analytic,report]:verdict(r,True)

    def mutations(records, expected, valid_cases, field='failed_cases'):
        require(set(records)==set(expected),'missing mutation family')
        for record in records.values():
            failures=record[field]
            require(record['rejected'] is True and isinstance(failures,list) and bool(failures)
                    and len(set(failures))==len(failures) and set(failures)<=set(valid_cases),'missing or invalid mutation witness')
    channels=['X_ERROR','Y_ERROR','Z_ERROR','DEPOLARIZE1','DEPOLARIZE2']
    mutations(analytic['channel_deletion_mutations'],channels,[s[0] for s in noise_specs()])
    mutations(distribution['channel_replacement_mutations'],
              ['DEPOLARIZE2_ix_only','DEPOLARIZE2_xi_only','DEPOLARIZE2_independent_x','DEPOLARIZE1_x_only','DEPOLARIZE1_z_only'],
              [s['name'] for s in distribution_specs()])
    mutations(unit['low_probability_deletion_mutations'],['pauli','loss','both'],[s[0] for s in low_specs()])
    mutations(actual['low_probability_deletion_mutations'],['pauli','loss','both'],expected_names,'failed_events')
    for record in actual['low_probability_deletion_mutations'].values():integer(record['removed_instructions'],1)
