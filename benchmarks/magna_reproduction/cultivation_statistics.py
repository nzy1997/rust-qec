"""Exact joint moments and the two independent-batch cultivation estimator."""
from fractions import Fraction
from functools import lru_cache
import math
import re


ZERO = (Fraction(0), Fraction(0))
ONE = (Fraction(1), Fraction(0))
WEIGHT_FIELDS = ('occurrences', 'tail_candidates', 'passing_candidates', 'passes')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def add(left, right):
    return left[0] + right[0], left[1] + right[1]


def scale(value, scalar):
    return value[0] * scalar, value[1] * scalar


def multiply(left, right):
    a, b = left
    c, d = right
    return a*c + 2*b*d, a*d + b*c


def divide(left, right):
    c, d = right
    return scale(multiply(left, (c, -d)), 1/(c*c - 2*d*d))


def sign(value):
    a, b = value
    if a == 0:
        return (b > 0) - (b < 0)
    if b == 0 or (a > 0) == (b > 0):
        return (a > 0) - (a < 0)
    return ((a > 0) - (a < 0)) * ((a*a > 2*b*b) - (a*a < 2*b*b))


def real_field(values):
    require(isinstance(values, list) and len(values) == 4 and all(
        isinstance(value, str) and re.fullmatch(r'-?[0-9]{1,256}(/[1-9][0-9]{0,255})?', value)
        for value in values), 'invalid exact cultivation coefficients')
    coefficients = [Fraction(value) for value in values]
    require([str(value) for value in coefficients] == values, 'noncanonical cultivation coefficients')
    a, b, c, d = coefficients
    require(c == 0 and b == -d, 'cultivation joint contribution is not real')
    # omega - omega^3 = sqrt(2), in the positive pi/4 convention.
    return a, b


def as_float(value):
    a, b = value
    if a*b < 0:
        # Rationalize near cancellation before rounding to double precision.
        return float(a*a - 2*b*b)/(float(a)-math.sqrt(2)*float(b))
    return float(a) + math.sqrt(2)*float(b)


@lru_cache(maxsize=4096)
def possible_tail_products(channels, length):
    if length == 0:
        return frozenset({1})
    if length == 1:
        return frozenset(channels)
    seen, products = set(), set()
    for count in channels:
        products.update(previous*count for previous in seen)
        seen.add(count)
    return frozenset(products)


def weight_key(value, channels, closing, offset):
    require(isinstance(value, dict) and set(value) == set(WEIGHT_FIELDS),
            'invalid cultivation weight descriptor fields')
    w, m, k, passes = (value[name] for name in WEIGHT_FIELDS)
    require(type(w) is int and 0 <= w <= len(channels) and type(m) is int
            and type(k) is int and type(passes) is bool, 'invalid cultivation weight descriptor types')
    if closing == 0:
        require((m, k, passes) == (0, 0, False), 'physical sample has a closing weight')
    else:
        length = min(w, closing)
        # The selected tail must leave room for w-length earlier occupied sites.
        require(m in possible_tail_products(channels[w-length:], length) and 0 <= k <= m
                and (not passes or k > 0) and (passes or k < m), 'invalid cultivation closing normalizer')
        if w == 0:
            require(k == int(not offset), 'empty cultivation assignment has the wrong offset parity')
    return w, m, k, passes


def histogram(payload):
    law = payload['law']
    channels = tuple(law['conditional_channel_counts'])
    closing = payload['configuration']['closing_sites']
    offset = law['flow']['offset']
    result = {}
    rows = payload['histogram']
    require(isinstance(rows, list) and len(rows) <= payload['configuration']['max_histogram_entries'],
            'cultivation histogram exceeds its configured bound')
    for row in rows:
        require(isinstance(row, dict) and set(row) == {'weight', 'accepted', 'error', 'count'},
                'invalid cultivation histogram row')
        key = weight_key(row['weight'], channels, closing, offset)
        accepted, error = real_field(row['accepted']), real_field(row['error'])
        require(sign(error) >= 0 and sign(add(accepted, scale(error, -1))) >= 0
                and sign(add(ONE, scale(accepted, -1))) >= 0, 'cultivation mass outside 0 <= E <= A <= 1')
        if closing and not key[-1]:
            require(accepted == ZERO, 'cultivation assignment failed an exact constraint but has positive acceptance')
        observation = key, accepted, error
        require(observation not in result and type(row['count']) is int and row['count'] > 0,
                'duplicate or nonpositive cultivation histogram row')
        result[observation] = row['count']
    require(sum(result.values()) == payload['completed'], 'cultivation histogram count differs from executed draws')
    return result


@lru_cache(maxsize=512)
def occurrence_ratio(p, q, sites, occurrences):
    return (p/q)**occurrences * ((1-p)/(1-q))**(sites-occurrences)


@lru_cache(maxsize=16384)
def likelihood(p, q, sites, key):
    w, m, k, passes = key
    if p == q:
        return Fraction(1)
    correction = Fraction(1) if k == 0 else (Fraction(20*k, k+19*m) if passes else Fraction(20))
    return occurrence_ratio(p, q, sites, w)*correction


def moments(entries, p, q, sites, component):
    count = sum(entries.values())
    require(count >= 2, 'cultivation statistics need at least two draws')
    totals = {}
    for (key, accepted, error), multiplicity in entries.items():
        value = accepted if component == 'acceptance' else error
        current = totals.setdefault(key, [ZERO, ZERO, 0])
        current[0] = add(current[0], scale(value, multiplicity))
        current[1] = add(current[1], scale(multiply(value, value), multiplicity))
        current[2] += multiplicity
    total, squared = ZERO, ZERO
    sum_weight, sum_weight_sq = Fraction(0), Fraction(0)
    for key, (linear, quadratic, multiplicity) in totals.items():
        weight = likelihood(p, q, sites, key)
        total = add(total, scale(linear, weight))
        squared = add(squared, scale(quadratic, weight*weight))
        sum_weight += weight*multiplicity
        sum_weight_sq += weight*weight*multiplicity
    variance = scale(add(squared, scale(multiply(total, total), Fraction(-1, count))),
                     Fraction(1, count*(count-1)))
    require(sign(variance) >= 0 and sum_weight > 0 and sum_weight**2 <= count*sum_weight_sq,
            'invalid cultivation exact moments')
    return dict(count=count, mean=scale(total, Fraction(1, count)), variance=variance,
                positive=sum(multiplicity for (_, a, e), multiplicity in entries.items()
                             if (a if component == 'acceptance' else e) != ZERO),
                mean_weight=sum_weight/count, ess=sum_weight**2/sum_weight_sq)


def two_estimator(acceptance, failure):
    a, e = acceptance['mean'], failure['mean']
    require(sign(a) > 0, 'cultivation physical acceptance is unresolved')
    ratio = divide(e, a)
    ratio_variance = divide(add(failure['variance'], multiply(multiply(ratio, ratio), acceptance['variance'])),
                            multiply(a, a))
    require(sign(ratio_variance) >= 0, 'negative independent cultivation variance')
    resolved = failure['positive'] > 0
    return dict(acceptance=as_float(a), accepted_failure=as_float(e), conditional_error=as_float(ratio) if resolved else None,
                acceptance_variance=as_float(acceptance['variance']),
                accepted_failure_variance=as_float(failure['variance']),
                conditional_error_variance=as_float(ratio_variance) if resolved else None,
                conditional_error_se=math.sqrt(as_float(ratio_variance)) if resolved else None,
                acceptance_draws=acceptance['count'], failure_draws=failure['count'],
                positive_failure_draws=failure['positive'], failure_ess=float(failure['ess']),
                failure_mean_weight=float(failure['mean_weight']), precision_resolved=resolved)
