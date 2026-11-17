"""Full and flux-matched TQD ratios with paired physical-draw uncertainty."""

from collections import Counter
from fractions import Fraction
import math

from cultivation_statistics import ZERO, ONE, add, as_float, divide, multiply, real_field, scale, sign
from remaining_contracts import require
from tqd_author_approximation import NESTED_DRAWS, fault_identity, target_z_infidelity
from tqd_fault_map import COLORS


RNG_PROTOCOL = 'splitmix64-addressed-u40-seed-u23-index-role-v1'
WARMUP = (1 << 23)-1
MASK64 = (1 << 64)-1
REFERENCE_NETWORKS = {'fidelity': (4,10), 'acceptance': (2,5), 'sampling_capped': (1,0)}


def fault_stream(seed, index, rate, sites):
    # Python reference for the Rust address split and uniform exact Bernoulli.
    gamma = 0x9e3779b97f4a7c15
    mask = MASK64
    def permutation(value):
        value = ((value ^ (value >> 30))*0xbf58476d1ce4e5b9) & mask
        value = ((value ^ (value >> 27))*0x94d049bb133111eb) & mask
        return value ^ (value >> 31)
    state = permutation((((seed << 24) | (index << 1))+gamma) & mask)
    cutoff = mask-mask%rate.denominator
    selected = []
    for site in range(sites):
        state = (state+gamma) & mask
        word = permutation(state)
        while word >= cutoff:
            state = (state+gamma) & mask
            word = permutation(state)
        if word%rate.denominator < rate.numerator:
            selected.append(site)
    return selected


def verify_readout_witness(payload, entries):
    config = payload['configuration']
    witness = payload['readout_witness']
    required = config['start'] == 0 and config['seed'] == 202610080000+config['size']*100000
    if not required:
        require(witness is None, 'unexpected TQD reported-readout witness')
        return
    require(witness is not None and witness['index'] == WARMUP-1, 'missing TQD reported-readout witness')
    index = witness['fault']
    require(type(index) is int and 0 <= index < len(entries), 'invalid TQD readout witness fault')
    match = entries[index]
    require((match['kind'],match['color'],match['edge']) == ('X','b',2) and 'record' in match,
            'wrong TQD reported-readout fault')
    before, after = witness['unflipped'], witness['flipped']
    require(all(row['name'] == 'jit_b_0' and row['call'] == 0
                and isinstance(row['inputs'],list) and len(row['inputs']) > 0
                and all(type(bit) is bool for bit in row['inputs']) for row in (before,after))
            and len(before['inputs']) == len(after['inputs'])
            and before['inputs'][0] != after['inputs'][0]
            and before['inputs'][1:] == after['inputs'][1:],
            'TQD decoder did not receive the flipped reported bit')


def historical_networks(read, size):
    folder = f'raw/tqd/timing/tqd{size}'
    result = read(f'{folder}/result.json')
    require(result['status'] == 'complete' and len(result['trials']) == 3, 'invalid deposited TQD resource trials')
    for trial in result['trials']:
        compilation = trial['resources']['compilation']
        require(compilation['status'] == 'complete', 'deposited TQD reference was incomplete')
        families = compilation['query_families']
        measured = {}
        for family in ('protocol','target'):
            for reference in families[family]:
                purpose = reference['purpose']
                require(purpose not in measured and purpose in REFERENCE_NETWORKS,
                        'invalid deposited TQD reference family')
                terms, coordinates = REFERENCE_NETWORKS[purpose]
                require((reference['reference_terms'],reference['reference_phase_coordinates'],
                         reference['direct_term_bound'],reference['stable_pairing_term_bound'])
                        == (terms,coordinates,terms,terms), 'deposited TQD reference network resources differ')
                measured[purpose] = dict(terms=terms, phase_coordinates=coordinates)
        require(measured.keys() == REFERENCE_NETWORKS.keys(), 'missing deposited TQD reference network')
    return dict(trials=len(result['trials']), references=measured)


def paired_rows(physical, author, expected_sources):
    config = physical['configuration']
    n = config['shots']
    require(physical['schema'] == 'rmagna.paper-tqd-physical-run.v1'
            and physical['status'] == 'complete' and physical['failure'] is None
            and type(physical['completed']) is int and physical['completed'] == n
            and type(physical['active_sites']) is int and physical['active_sites'] > 0
            and len(physical['rows']) == n and physical['rng_protocol'] == RNG_PROTOCOL,
            'TQD physical draw range is incomplete')
    require(author['schema'] == 'rmagna.paper-tqd-author-approximation.v1'
            and author['size'] == config['size'] and author['noise_seed'] == config['seed']
            and author['depth'] == physical['charge_depth']-1
            and author['nested_draws'] == NESTED_DRAWS
            and author['author_source_sha256'] == expected_sources
            and author['logical_z_infidelity'] == str(target_z_infidelity())
            and author['fault_map_sha256'] == physical['fault_map_sha256']
            and len(author['rows']) == n, 'incomplete or unpinned TQD author run')
    rate = Fraction(config['p'])
    require(0 < rate < 1 and type(config['seed']) is int and 0 <= config['seed'] < 1 << 40,
            'invalid TQD physical sampling policy')
    result = []
    for index, (quantum, approximate) in enumerate(zip(physical['rows'],author['rows'],strict=True),start=config['start']):
        selected = quantum['faults']
        require(type(quantum['index']) is int and quantum['index'] == approximate['index'] == index
                and isinstance(selected,list) and all(type(site) is int and 0 <= site < physical['active_sites'] for site in selected)
                and selected == sorted(set(selected)) and fault_identity(selected) == approximate['fault_sha256'],
                'TQD quantum and author faults are unpaired')
        require(selected == fault_stream(config['seed'],index,rate,physical['active_sites']),
                'TQD fault assignment differs from the exact physical RNG')
        accepted = real_field(quantum['accepted'])
        error = real_field(quantum['error'])
        weight = real_field(quantum['history_weight'])
        require(sign(error) >= 0 and sign(add(accepted,scale(error,-1))) >= 0
                and sign(add(weight,scale(accepted,-1))) >= 0, 'TQD physical mass violates 0 <= E <= A <= H')
        flux, failures = approximate['flux_success'], approximate['flux_failures']
        require(type(flux) is bool and failures.keys() == set(COLORS)
                and all(type(value) is bool for value in failures.values()) and flux is not any(failures.values()),
                'invalid author flux sector')
        charge = quantum['odd_terminal_charge']
        require(charge.keys() == set(COLORS) and all(type(value) is bool for value in charge.values()),
                'invalid TQD actual reported charge parity')
        nested = approximate['nested_logical_errors']
        approximation = ZERO
        if flux:
            require(approximate['nested_attempted'] == NESTED_DRAWS and type(nested) is int and 0 <= nested <= NESTED_DRAWS,
                    'incomplete author nested draws')
            infidelity = target_z_infidelity()*Fraction(nested,NESTED_DRAWS)
            require(approximate['matched_infidelity'] == str(infidelity), 'TQD author approximation normalization changed')
            approximation = (infidelity,Fraction(0))
        else:
            require(approximate['nested_attempted'] == 0 and nested is None and approximate['matched_infidelity'] is None,
                    'TQD flagged flux sector entered the matched estimate')
        result.append((accepted,error,flux,approximation,any(charge.values())))
    return result


def summarize(rows):
    n = len(rows)
    require(n > 1, 'TQD statistics require at least two physical draws')
    full, physical, author = [], [], []
    flux, odd_total, odd_matched = 0, 0, 0
    for a,e,matched,approximate,odd in rows:
        full.append((e,a))
        physical.append((e,a) if matched else (ZERO,ZERO))
        author.append((approximate,ONE) if matched else (ZERO,ZERO))
        flux += int(matched)
        odd_total += int(odd)
        odd_matched += int(odd and matched)

    def ratio_and_influence(values, label):
        numer, denom = ZERO, ZERO
        for numerator, denominator in values:
            numer = add(numer,numerator)
            denom = add(denom,denominator)
        require(sign(denom) > 0, f'unresolved TQD {label} denominator')
        ratio = divide(numer,denom)
        unit = divide((Fraction(n),Fraction(0)),denom)
        influence = [multiply(add(num,scale(multiply(ratio,den),-1)),unit) for num,den in values]
        return ratio,influence

    def estimate(mean, influence, zero_means_unresolved):
        squares = ZERO
        for value,multiplicity in Counter(influence).items():
            squares = add(squares,scale(multiply(value,value),multiplicity))
        variance = scale(squares,Fraction(1,n*(n-1)))
        require(sign(variance) >= 0, 'invalid TQD paired variance')
        zero_observed = zero_means_unresolved and mean == ZERO
        return dict(mean=as_float(mean), standard_error=None if zero_observed else math.sqrt(as_float(variance)),
                    zero_observed=zero_observed,
                    exact_mean=[[hex(value.numerator),hex(value.denominator)] for value in mean],
                    exact_variance=[[hex(value.numerator),hex(value.denominator)] for value in variance])

    a, ua = ratio_and_influence(full,'full')
    b, ub = ratio_and_influence(physical,'matched physical')
    c, uc = ratio_and_influence(author,'matched author')
    delta = [add(left,scale(right,-1)) for left,right in zip(ub,uc,strict=True)]
    return dict(draws=n, flux_success=flux, odd_after_all=odd_total, odd_after_unflagged=odd_matched,
                full=estimate(a,ua,True), matched_physical=estimate(b,ub,True), matched_author=estimate(c,uc,True),
                paired_difference=estimate(add(b,scale(c,-1)),delta,False))
