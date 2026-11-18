"""Reweight the same full encoded-input draw at all three physical rates."""

from fractions import Fraction
from functools import lru_cache
from itertools import product
from pathlib import Path

from cultivation_statistics import ONE, ZERO, add, as_float, divide, multiply, real_field, scale, sign
from remaining_contracts import read_json, require
import reference


HERE = Path(__file__).resolve().parent
FIXTURE = HERE/'paper/logical-residuals.json'
WIDTH, PATCHES = 9, 15
P0, NONIDENTITY = Fraction(1, 100), Fraction(1, 5)
RATES = (Fraction(1, 1000), Fraction(3, 1000), Fraction(1, 100))


def atom(rate, weight, width):
    return (rate/3)**weight * (1-rate)**(width-weight)


class ResidualWeights:
    def __init__(self):
        fixture = read_json(FIXTURE)
        self.counts = tuple(map(tuple, fixture['counts_IXZY']))
        self.x, self.z = tuple(fixture['x_residual']), tuple(fixture['z_residual'])
        require(len(self.x) == len(self.z) == 1 << WIDTH
                and len(self.counts) == 4 and all(len(row) == WIDTH+1 for row in self.counts)
                and sum(map(sum, self.counts)) == 4**WIDTH, 'invalid encoded-input residual fixture')
        self.identity_mass = self.classes(P0)[0]
        self.classify = lru_cache(maxsize=1 << 14)(self.classify)

    def classes(self, rate):
        return tuple(sum(count * atom(rate, weight, WIDTH) for weight, count in enumerate(row))
                     for row in self.counts)

    def q_eff(self, rate):
        identity, x, z, y = self.classes(rate)
        require(identity+x+z+y == 1, 'logical residual distribution does not normalize')
        return x/2 + z + y/2

    def classify(self, word):
        require(type(word) is int and 0 <= word < 4**WIDTH, 'invalid encoded-input physical Pauli word')
        x = sum(((word >> (2*j)) & 1) << j for j in range(WIDTH))
        z = sum(((word >> (2*j+1)) & 1) << j for j in range(WIDTH))
        return (x | z).bit_count(), bool(self.x[x] or self.z[z])

    def weight(self, rate, physical_weight, nonidentity):
        return ((rate/P0)**physical_weight * ((1-rate)/(1-P0))**(WIDTH*PATCHES-physical_weight)
                * (self.identity_mass/(1-NONIDENTITY))**(PATCHES-nonidentity)
                * ((1-self.identity_mass)/NONIDENTITY)**nonidentity)

    def verify_proposal(self, reported):
        require(reported['pattern_encoding'] == 'interleaved_ixzy_low_wire_first'
                and reported['p0'] == str(P0) and reported['nonidentity'] == str(NONIDENTITY)
                and reported['classifier_sha256'] == reference.digest(FIXTURE)
                and reported['patches'] == PATCHES and reported['active_wires_per_patch'] == WIDTH
                and reported['categories'] == PATCHES*WIDTH,
                'encoded-input proposal fixture or site count changed')
        require(type(reported['identity_mass_numerator']) is str and type(reported['identity_mass_denominator']) is str
                and reported['identity_mass_denominator'] == str(300**WIDTH)
                and Fraction(int(reported['identity_mass_numerator']), 300**WIDTH) == self.identity_mass,
                'encoded-input proposal normalization differs')


def histogram(payload, model):
    require(payload['status'] == 'complete' and payload['failure'] is None
            and payload['schema'] == 'rmagna.paper-logical-distillation-run.v1'
            and payload['history_sampler'] == 'exact_marginal' and payload['history_weight'] == '1',
            'logical distillation draw did not complete')
    model.verify_proposal(payload['proposal'])
    start = payload['configuration']['start']
    count = payload['configuration']['shots']
    expected_checks = (payload['configuration']['scope'], start) == ('precision', 0)
    checks = payload['prescribed_fault_checks']
    require(isinstance(checks, list) and len(checks) == (4 if expected_checks else 0),
            'missing or unexpected logical distillation prescribed fault checks')
    for patches, row in enumerate(checks):
        require(row['index'] == (1 << 32)-2-patches
                and row['patterns'] == [10]*patches + [0]*(PATCHES-patches)
                and type(row['physical_weight']) is int and row['physical_weight'] == 2*patches
                and type(row['nonidentity']) is int and row['nonidentity'] == patches
                and real_field(row['acceptance']) == (ONE if patches in (0, 3) else ZERO)
                and real_field(row['error']) == (ONE if patches == 3 else ZERO),
                'logical distillation weight-six fault control differs')
    require(type(count) is int and count > 0 and type(payload['completed']) is int
            and len(payload['rows']) == payload['completed'] == count, 'incomplete logical distillation range')
    moments = {}
    for index, row in enumerate(payload['rows'], start=start):
        patterns = row['patterns']
        require(type(row['index']) is int and row['index'] == index
                and isinstance(patterns, list) and len(patterns) == PATCHES,
                'missing logical distillation draw or encoded input')
        classified = [model.classify(word) for word in patterns]
        key = tuple(map(sum, zip(*classified)))
        require(all(type(row[field]) is int for field in ('physical_weight', 'nonidentity'))
                and key == (row['physical_weight'], row['nonidentity']),
                'logical distillation physical fault weights differ')
        acceptance, error = real_field(row['acceptance']), real_field(row['error'])
        require(sign(error) >= 0 and sign(add(acceptance, scale(error, -1))) >= 0
                and sign(add(ONE, scale(acceptance, -1))) >= 0,
                'logical distillation mass violates 0 <= E <= A <= 1')
        observation = key, acceptance, error
        moments[observation] = moments.get(observation, 0) + 1
    return moments


def moments(observations, model):
    count = sum(observations.values())
    require(count >= 2, 'logical distillation statistics need at least two draws')
    groups = {}
    positives = 0
    minimum_error_weight = None
    for (key, acceptance, error), multiplicity in observations.items():
        if error != ZERO:
            positives += multiplicity
            minimum_error_weight = key[0] if minimum_error_weight is None else min(minimum_error_weight, key[0])
        # The paper estimates acceptance as 1 - mean(W * (H - A)).
        # Exact Born history sampling makes H = 1 for each independent draw.
        rejection = add(ONE, scale(acceptance, -1))
        values = (rejection, error, multiply(rejection, rejection), multiply(rejection, error), multiply(error, error))
        group = groups.setdefault(key, [ZERO] * len(values))
        for index, value in enumerate(values):
            group[index] = add(group[index], scale(value, multiplicity))

    size = len(RATES)
    sum_r, sum_e = [ZERO]*size, [ZERO]*size
    aa, ae, ee = ([[ZERO]*size for _ in RATES] for _ in range(3))
    for key, (r, e, r2, rx, e2) in groups.items():
        factors = [model.weight(rate, *key) for rate in RATES]
        for index, weight in enumerate(factors):
            sum_r[index] = add(sum_r[index], scale(r, weight))
            sum_e[index] = add(sum_e[index], scale(e, weight))
        for i,j in product(range(size), repeat=2):
            pair = factors[i]*factors[j]
            aa[i][j] = add(aa[i][j], scale(r2, pair))
            ae[i][j] = add(ae[i][j], scale(rx, pair))
            ee[i][j] = add(ee[i][j], scale(e2, pair))

    mean_a = [add(ONE, scale(value, -Fraction(1, count))) for value in sum_r]
    mean_e = [scale(value, Fraction(1, count)) for value in sum_e]
    require(all(sign(value) > 0 for value in mean_a), 'logical distillation acceptance unresolved')
    ratio = [divide(error, acceptance) for acceptance, error in zip(mean_a, mean_e, strict=True)]
    covariances = {}
    for name, raw, left, right in (('acceptance', aa, sum_r, sum_r),
                                   ('acceptance_error', ae, sum_r, sum_e),
                                   ('accepted_error', ee, sum_e, sum_e)):
        # Rejection has the same variance as acceptance and the opposite
        # covariance with accepted error.
        orientation = -1 if name == 'acceptance_error' else 1
        covariances[name] = [
            [scale(add(total, scale(multiply(left[i], right[j]), -Fraction(1, count))),
                   Fraction(orientation, count*(count-1))) for j, total in enumerate(row)]
            for i, row in enumerate(raw)
        ]
    ca, cae, ce = (covariances[name] for name in ('acceptance', 'acceptance_error', 'accepted_error'))
    conditional = []
    for i in range(size):
        row = []
        for j in range(size):
            terms = (ce[i][j], multiply(multiply(ratio[i], ratio[j]), ca[i][j]),
                     scale(multiply(ratio[i], cae[i][j]), -1), scale(multiply(ratio[j], cae[j][i]), -1))
            numerator = ZERO
            for term in terms:
                numerator = add(numerator, term)
            row.append(divide(numerator, multiply(mean_a[i], mean_a[j])))
        conditional.append(row)
    covariances['conditional_error'] = conditional
    require(all(sign(matrix[i][i]) >= 0 for name, matrix in covariances.items()
                if name != 'acceptance_error' for i in range(size)), 'invalid encoded-input covariance')

    def encode(value):
        return [[hex(coefficient.numerator), hex(coefficient.denominator)] for coefficient in value]

    rows = [dict(p=str(rate), q_eff=str(model.q_eff(rate)), acceptance=encode(mean_a[i]),
                 accepted_error=encode(mean_e[i]), conditional_error=encode(ratio[i]),
                 acceptance_float=as_float(mean_a[i]), conditional_error_float=as_float(ratio[i]))
            for i, rate in enumerate(RATES)]
    return dict(completed=count, positive_error_draws=positives, minimum_error_weight=minimum_error_weight,
                precision_resolved=positives > 0, acceptance_estimator='one_minus_weighted_rejection',
                exact_encoding='hexadecimal_rational_coefficients',
                rows=rows, covariance={name: [[encode(value) for value in row] for row in matrix]
                                       for name, matrix in covariances.items()})
