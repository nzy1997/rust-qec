#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Observe complete original proposal, precision, estimator and noise methods.

The fixture records explicit range/bit protocols, not a Rust implementation of
Python Random. Nonfinite Native float exports use tagged strict-JSON objects.
"""
from pathlib import Path
import argparse
import importlib.util
import json
import math
import random
import sys
import struct
import unittest
from decimal import Decimal
from fractions import Fraction
import reference
from witness_environment import native_reference
from witness_paths import admit_destinations
OriginalRandom = random.Random

class LoggedRandom(OriginalRandom):
    instances = []

    def __init__(self, seed):
        super().__init__(seed)
        self.seed_value = seed
        self.events = []
        self.depth = 0
        self.instances.append(self)

    def randrange(self, *args, **kwargs):
        self.depth += 1
        try:
            value = super().randrange(*args, **kwargs)
        finally:
            self.depth -= 1
        self.events.append(dict(kind='range', args=args, value=value))
        return value

    def getrandbits(self, count):
        value = super().getrandbits(count)
        if not self.depth:
            self.events.append(dict(kind='bits', args=[count], value=value))
        return value

def serial(v):
    if isinstance(v, (Decimal, Fraction)):
        return str(v)
    if isinstance(v, dict):
        return {k: serial(x) for k, x in v.items()}
    if isinstance(v, (tuple, list)):
        return [serial(x) for x in v]
    if isinstance(v, float) and (not math.isfinite(v)):
        return {'native_float': 'positive_infinity' if v > 0 else 'negative_infinity' if v < 0 else 'nan'}
    return v

def frexp_decimal(text):
    value = Fraction(Decimal(text))
    if not value:
        return {'mantissa_bits': '0000000000000000', 'exponent': 0}
    exponent = abs(value.numerator).bit_length() - value.denominator.bit_length()
    scaled = value / Fraction(2) ** exponent
    mantissa, shift = math.frexp(float(scaled))
    return {'mantissa_bits': f"{struct.unpack('>Q', struct.pack('>d', mantissa))[0]:016x}", 'exponent': exponent + shift}

def capture(upstream, helper):
    upstream, helper = (Path(upstream).resolve(), Path(helper).resolve())
    helper_hash = reference.digest(helper)
    LoggedRandom.instances.clear()
    if '_native_testing' in sys.modules:
        raise ValueError('fresh Python process required')
    with native_reference(upstream) as cap:
        spec = importlib.util.spec_from_file_location('_native_testing', helper)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        sys.modules['_native_testing'] = mod
        sys.path[:0] = [str(upstream), str(upstream / 'tests/core')]
        import test_native_proposals as proposals
        import test_simulator_optimization as optimization
        from magna import _native
        from magna.operations import IndependentProposal, AffineProposal, NoiseModel
        observations = {'independent': [], 'affine': [], 'decimals': [], 'estimators': [], 'noise': []}
        old_independent, old_affine = (IndependentProposal.draw, AffineProposal.draw)
        old_decimal, old_estimate, old_noise = (_native.scalar_decimal_value, proposals.estimate, NoiseModel.draw)

        def draw_independent(self, *args, **kwargs):
            result = old_independent(self, *args, **kwargs)
            observations['independent'].append(dict(true=result.true, q=str(result.proposal_probability)))
            return result

        def draw_affine(self, *args, **kwargs):
            result = old_affine(self, *args, **kwargs)
            observations['affine'].append(dict(true=result.true, q=str(result.proposal_probability)))
            return result

        def decimal(value, *args):
            result = old_decimal(value, *args)
            observations['decimals'].append(dict(coefficients=[str(c) for c in value[0]], power=value[1], weight=None if not args else str(Fraction(*args[0])), decimal=result))
            return result

        def estimate(rows, score):
            values = [dict(weight=str(row.weight_decimal()), acceptance=str(row.a), diagnostic=str(row.d)) for row in rows]
            result = old_estimate(rows, score)
            observations['estimators'].append(dict(rows=values, result=serial(result)))
            return result

        def noise(self, *args, **kwargs):
            result = old_noise(self, *args, **kwargs)
            observations['noise'].append(dict(kwargs=kwargs, result=serial(result)))
            return result
        IndependentProposal.draw, AffineProposal.draw = (draw_independent, draw_affine)
        _native.scalar_decimal_value, proposals.estimate, NoiseModel.draw = (decimal, estimate, noise)
        proposals.Random = LoggedRandom
        random.Random = LoggedRandom
        ids = ['test_native_proposals.NativeProposalTests.test_exact_independent_and_affine_rng_streams', 'test_native_proposals.NativeProposalTests.test_real_decimal_and_weight_preserve_precision_and_rounding', 'test_native_proposals.NativeProposalTests.test_estimator_matches_independent_decimal_operations', 'test_noise_numeric_fastpaths.NoiseNumericFastpathTests.test_finite_noise_thresholds_and_general_conversion_in_native_module', 'test_simulator_optimization.OptimizationTests.test_unbiased_noise_preserves_exact_rng_stream']
        try:
            result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames(ids))
        finally:
            IndependentProposal.draw, AffineProposal.draw = (old_independent, old_affine)
            _native.scalar_decimal_value, proposals.estimate, NoiseModel.draw = (old_decimal, old_estimate, old_noise)
            proposals.Random = OriginalRandom
            random.Random = OriginalRandom
        if not (result.wasSuccessful() and result.testsRun == 5 and (not result.skipped)):
            raise ValueError('original numeric witness validation failed')
        if not [len(observations[k]) for k in ['independent', 'affine', 'decimals', 'estimators', 'noise']] == [30, 30, 70, 6, 140]:
            raise ValueError({k: len(v) for k, v in observations.items()})
        for row in observations['decimals']:
            row['rounded_decimal_frexp'] = frexp_decimal(row['decimal'])
        streams = [dict(seed=r.seed_value, events=r.events, state_repr=repr(r.getstate())) for r in LoggedRandom.instances]
        if not len(streams) == 7:
            raise ValueError(len(streams))
        out = dict(schema='rmagna.original-numeric-probe.v1', upstream_commit=cap.manifest['upstream']['commit'], reference_manifest_sha256=cap.manifest_sha256, native_frontend_sha256=cap.native_sha256, native_helper_sha256=helper_hash, producer_sha256=reference.digest(Path(__file__)), source_sha256={f: cap.manifest['upstream']['files'][f] for f in ['tests/core/test_native_proposals.py', 'tests/core/test_noise_numeric_fastpaths.py', 'tests/native/rational_random.cpp', 'tests/core/test_simulator_optimization.py', 'tests/support/decimal_estimator.py']}, methods=ids, original_methods_passed=5, streams=streams, observations=observations)
        document = out
    if reference.digest(helper) != helper_hash:
        raise ValueError('identified helper changed during capture')
    return document

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--native-helper', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream, protected_roots=[reference.HERE, args.native_helper.parent])
        result = capture(args.upstream, args.native_helper)
        with destination.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, separators=(',', ':'), allow_nan=False)
            stream.write('\n')
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print('all five unchanged original methods passed:60 proposals,70 scalar conversions,6 estimator families,140 noise draws')
if __name__ == '__main__':
    main()
