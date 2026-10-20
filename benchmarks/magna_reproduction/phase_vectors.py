#!/usr/bin/env python3
"""Capture exact closed phase sums from the pinned native implementation."""

import argparse
import json
from pathlib import Path
import random
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from witness_environment import native_reference

    with native_reference(args.upstream) as captured:
        data = generate(captured)
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


def generate(captured):
    rng = random.Random(20261022)
    vectors = []
    for width in (0, 1, 3, 5, 7, 65, 129):
        for trial in range(4):
            axes = [rng.getrandbits(width) for _ in range(3)]
            terms = []
            for _ in range(12):
                axis = 0
                for a in rng.sample(axes, rng.randrange(4)):
                    axis ^= a
                terms.append((axis, rng.randrange(8)))
            linear = tuple(rng.randrange(4) for _ in range(width))
            upper = tuple(rng.getrandbits(width) & ~((1 << (j + 1)) - 1)
                          for j in range(width))
            constant, power = rng.randrange(8), rng.randrange(-8, 4)
            payload = dict(n=width, linear=linear, upper=upper, terms=tuple(terms),
                           constant=constant, power=power, boundary=(),
                           output_count=0, environment=())
            _, resource, states, norm = captured.native.phase_decompose(payload, {}, None, False)
            _, absorbed_resource, _, absorbed_norm = captured.native.phase_decompose(payload, {}, None, True)
            if absorbed_norm != norm:
                raise RuntimeError("native radical absorption changed the closed network norm")
            native = captured.native
            total = ((0, 0, 0, 0), 0)
            for coefficient, ket in states:
                ket = native.contract(ket)
                if ket is None:
                    continue
                boundary_width, base, columns, _, _, ket_power, ket_phase = ket
                if boundary_width != 0 or base != 0 or columns:
                    raise RuntimeError("closed reference contraction retained boundary paths")
                c = [0] * 4
                ket_phase %= 8
                c[ket_phase % 4] = 1 if ket_phase < 4 else -1
                amplitude = (tuple(c), ket_power // 2)
                if ket_power % 2:
                    amplitude = native.algebra_scalar("multiply", amplitude, ((0, 1, 0, -1), 0))
                total = native.algebra_scalar("add", total, native.algebra_scalar("multiply", coefficient, amplitude))
            conjugate = native.algebra_scalar("conjugate", total)
            if native.algebra_scalar("multiply", conjugate, total) != norm:
                raise RuntimeError("closed reference amplitude disagrees with its native norm")
            coefficients, exponent = total
            vectors.append(dict(width=width, trial=trial, terms=[(str(a), c) for a, c in terms],
                                linear=linear, upper=[str(a) for a in upper], constant=constant,
                                sqrt2_power=power, quotient_rank=resource["quotient_rank"],
                                absorbed_rank=absorbed_resource["quotient_rank"],
                                result=dict(coefficients=[str(c) for c in coefficients],
                                                              power=exponent)))
    return dict(schema="rmagna.phase-reference.v1", seed=20261022,
                upstream_commit=captured.manifest["upstream"]["commit"],
                reference_manifest_sha256=captured.manifest_sha256,
                native_sha256=captured.native_sha256, vectors=vectors)


if __name__ == "__main__":
    main()
