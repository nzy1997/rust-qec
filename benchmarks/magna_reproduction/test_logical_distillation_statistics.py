"""Independent exact checks for encoded-input likelihood and covariance."""

from fractions import Fraction
from itertools import product
import unittest

from cultivation_statistics import ONE, ZERO
from logical_distillation_statistics import PATCHES, RATES, ResidualWeights, moments


def as_rational(values):
    [num, den], irrational = values
    if irrational != ['0x0', '0x1']:
        raise ValueError('expected real rational fixture result')
    return Fraction(int(num, 16), int(den, 16))


class LogicalDistillationStatisticsTests(unittest.TestCase):
    def test_rejection_estimator_carries_shared_draws_across_rates(self):
        model = ResidualWeights()
        groups = [((6, 3), ONE, ONE), ((4, 2), ZERO, ZERO), ((2, 1), ZERO, ZERO), ((0, 0), ONE, ZERO)]
        report = moments(dict.fromkeys(groups, 1), model)
        draws = [[(1 - model.weight(rate, *key)*(1-a[0]), model.weight(rate, *key)*e[0])
                  for key,a,e in groups] for rate in RATES]
        n = len(groups)
        influences = []
        for point, sample in zip(report['rows'], draws, strict=True):
            a = sum(x for x,_ in sample)/n
            e = sum(y for _,y in sample)/n
            self.assertEqual(as_rational(point['acceptance']), a)
            self.assertEqual(as_rational(point['conditional_error']), e/a)
            influences.append([(y - e/a*x)/a for x,y in sample])
        for i,j in product(range(len(RATES)), repeat=2):
            expected = sum(x*y for x,y in zip(influences[i], influences[j], strict=True))/(n*(n-1))
            self.assertEqual(as_rational(report['covariance']['conditional_error'][i][j]), expected)
        self.assertEqual(report['minimum_error_weight'], 6)

    def test_patch_words_are_classified_before_likelihood(self):
        model = ResidualWeights()
        self.assertEqual(model.classify(10), (2, True))
        self.assertEqual(model.classify(0), (0, False))
        self.assertEqual(model.classify(1536), (2, False))
        self.assertEqual(model.q_eff(Fraction(1, 1000)), Fraction(2128055613755249, 177978515625000000000))
        self.assertEqual(model.weight(Fraction(1, 100), 0, 0),
                         (model.identity_mass/Fraction(4, 5))**PATCHES)


if __name__ == '__main__':
    unittest.main()
