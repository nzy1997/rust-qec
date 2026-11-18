"""Exact joint and paired moments for code-switching contribution receipts."""

from fractions import Fraction
import math

from cultivation_statistics import ZERO, add, as_float, divide, multiply, occurrence_ratio, real_field, scale, sign
from remaining_contracts import require


def code_switch_histogram(payload):
    require(payload['schema'] == 'rmagna.paper-code-switch-run.v1'
            and payload['status'] == 'complete' and payload['failure'] is None,
            'code-switching draw range did not complete')
    config = payload['configuration']
    count, sites = payload['completed'], payload['original_sites']
    require(type(count) is int and 2 <= count == config['shots']
            and type(sites) is int and sites > 0, 'invalid code-switching draw or site count')
    rows = payload['histogram']
    require(isinstance(rows, list) and len(rows) <= config['max_histogram_entries'],
            'code-switching histogram exceeds declared bound')
    observations = {}
    draws = 0
    for row in rows:
        require(isinstance(row, dict) and row.keys() == {'occurrences', 'acceptance', 'error', 'count'},
                'invalid code-switching histogram fields')
        occurrences, multiplicity = row['occurrences'], row['count']
        require(type(occurrences) is int and 0 <= occurrences <= sites
                and type(multiplicity) is int and 0 < multiplicity <= count,
                'invalid code-switching histogram width or multiplicity')
        acceptance, error = real_field(row['acceptance']), real_field(row['error'])
        require(sign(error) >= 0 and sign(add(acceptance, scale(error, -1))) >= 0,
                'code-switching mass violates 0 <= E <= A')
        observation = occurrences, acceptance, error
        require(observation not in observations, 'duplicate code-switching histogram bin')
        observations[observation] = multiplicity
        draws += multiplicity
    require(draws == count, 'code-switching histogram does not cover completed draws')
    return observations


def code_switch_estimators(payload):
    config = payload['configuration']
    p, q = (Fraction(config[key]) for key in ('physical_rate', 'proposal_rate'))
    return code_switch_moments(code_switch_histogram(payload), p, q, payload['original_sites'])


def code_switch_moments(observations, p, q, sites):
    count = sum(observations.values())
    require(count >= 2 and 0 < p < 1 and 0 < q < 1, 'invalid code-switching rates or total draw count')
    totals = {}
    positive = 0
    for (occurrences, acceptance, error), multiplicity in observations.items():
        positive += multiplicity if error != ZERO else 0
        # Bin by occurrence before introducing the potentially large exact P/Q.
        values = (acceptance, error, multiply(acceptance, acceptance),
                  multiply(acceptance, error), multiply(error, error))
        group = totals.setdefault(occurrences, [ZERO] * len(values))
        for index, value in enumerate(values):
            group[index] = add(group[index], scale(value, multiplicity))
    weighted = [ZERO] * 5
    for occurrences, group in totals.items():
        likelihood = occurrence_ratio(p, q, sites, occurrences)
        for index, value in enumerate(group):
            weighted[index] = add(weighted[index], scale(value, likelihood if index < 2 else likelihood**2))
    sum_a, sum_e, aa, ae, ee = weighted
    acceptance, error = (scale(total, Fraction(1, count)) for total in (sum_a, sum_e))
    require(sign(acceptance) > 0, 'code-switching acceptance is unresolved')

    def mean_covariance(total, left, right):
        centered = add(total, scale(multiply(left, right), -Fraction(1, count)))
        return scale(centered, Fraction(1, count * (count - 1)))

    variance_a = mean_covariance(aa, sum_a, sum_a)
    covariance = mean_covariance(ae, sum_a, sum_e)
    variance_e = mean_covariance(ee, sum_e, sum_e)
    ratio = divide(error, acceptance)
    residual = add(add(variance_e, multiply(multiply(ratio, ratio), variance_a)),
                   scale(multiply(ratio, covariance), -2))
    variance_ratio = divide(residual, multiply(acceptance, acceptance))
    require(all(sign(variance) >= 0 for variance in (variance_a, variance_e, variance_ratio)),
            'invalid exact code-switching variance')
    statistics = dict(acceptance=acceptance, accepted_error=error, conditional_error=ratio,
                      variance_acceptance=variance_a, covariance=covariance,
                      variance_accepted_error=variance_e, variance_conditional_error=variance_ratio)
    return dict(completed=count, positive_error_draws=positive, precision_resolved=positive > 0,
                exact_encoding='hexadecimal_rational_coefficients',
                exact={name: [[hex(coefficient.numerator), hex(coefficient.denominator)]
                              for coefficient in value] for name, value in statistics.items()},
                estimates={name: as_float(value) for name, value in statistics.items()},
                conditional_standard_error=math.sqrt(as_float(variance_ratio)) if positive > 0 else None)


def verify_code_switch_decoder(payload, pinned):
    """Check every observed recovery syndrome against the pinned Steane checks."""
    family = payload['configuration']['family']
    require(family in ('dk', 'wg-S', 'wg-T'), 'unknown code-switching decoder family')
    rows = payload['decoder_histogram']
    timing = payload['timings']
    decoder = payload['decoder']
    if family != 'dk':
        require(rows == [] and decoder is None and payload['decoder_transcript_sha256'] is None
                and timing['decoder_attempted'] == timing['decoder_completed'] == timing['decoder_seconds'] == 0,
                'WG ran an external decoder')
        return dict(calls=0, nonzero_input_calls=0)
    paths = pinned['upstream']['files']
    prefix = 'examples/dk/'
    expected_sources = {name: paths[prefix + name] for name in ('decoder/__init__.py', 'decoder/decoder.py')}
    require(decoder['module_sources'] == expected_sources
            and decoder['controllers_sha256'] == paths[prefix + 'controllers.json']
            and decoder['bindings']['steane_css']['import_name'] == 'decoder.decoder:decode',
            'DK decoder worker sources differ')
    require(isinstance(rows, list) and len(rows) <= 64, 'invalid DK syndrome inventory')
    # The two sets of three MPPs in the pinned recovery have the same support.
    checks = ({0, 1, 5, 6}, {1, 2, 3, 6}, {3, 4, 5, 6})
    corrections = {tuple(wire in check for check in checks): wire for wire in range(7)}
    counts, observed = 0, set()
    nonzero = 0
    for row in rows:
        inputs, outputs, count = row['inputs'], row['outputs'], row['count']
        require(isinstance(inputs, list) and len(inputs) == 6 and all(type(bit) is bool for bit in inputs)
                and isinstance(outputs, list) and len(outputs) == 14 and all(type(bit) is bool for bit in outputs)
                and type(count) is int and 0 < count <= payload['completed'],
                'invalid DK syndrome or response')
        key = tuple(inputs)
        require(key not in observed, 'duplicate DK syndrome')
        observed.add(key)
        expected = [False] * 14
        for start, offset in ((0, 0), (3, 7)):
            syndrome = key[start:start + 3]
            if any(syndrome):
                expected[offset + corrections[syndrome]] = True
        require(outputs == expected, 'DK decoder response differs from measured syndrome')
        counts += count
        nonzero += count if any(inputs) else 0
    require(counts == timing['decoder_attempted'] == timing['decoder_completed'] == payload['completed'],
            'missing DK decoder calls')
    return dict(calls=counts, nonzero_input_calls=nonzero)
