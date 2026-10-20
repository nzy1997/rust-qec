#!/usr/bin/env python3
"""Generate prescribed affine-ket witnesses from the pinned native reference."""

import argparse
import json
from pathlib import Path
import random
import sys


def encode_scalar(value):
    coefficients, power = value
    return {"coefficients": [str(c) for c in coefficients], "power": power}


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
    _native = captured.native
    rng = random.Random(20261021)

    def scaled_phase(phase, power):
        phase %= 8
        c = [0] * 4
        c[phase % 4] = 1 if phase < 4 else -1
        value = (tuple(c), power // 2)
        if power % 2:
            value = _native.algebra_scalar("multiply", value, ((0, 1, 0, -1), 0))
        return encode_scalar(_native.algebra_scalar("normalize", value))

    vectors = []
    for width in (0, 1, 3, 5, 65, 129):
        for paths in (0, 3, 6, 8):
            columns = [rng.getrandbits(width) for _ in range(paths)]
            if paths >= 3:
                columns[-1] = columns[0] ^ columns[1]
                columns[-2] = 0
            base = rng.getrandbits(width)
            linear = tuple(rng.randrange(4) for _ in range(paths))
            upper = tuple(rng.getrandbits(paths) & ~((1 << (j + 1)) - 1) for j in range(paths))
            power, phase = -paths - 2, rng.randrange(8)
            raw = (width, base, tuple(columns), linear, upper, power, phase)
            contracted = _native.contract(raw)
            amplitudes = []
            norm = ((0, 0, 0, 0), 0)
            if contracted is not None:
                _, output_base, output_columns, output_linear, output_upper, output_power, output_phase = contracted
                norm = _native.algebra_ket("norm", contracted)
                for x in range(1 << len(output_columns)):
                    y, q = output_base, 0
                    for j, column in enumerate(output_columns):
                        if x >> j & 1:
                            y ^= column
                            q += output_linear[j] + 2 * (output_upper[j] & x).bit_count()
                    amplitudes.append({"output": str(y), "value": scaled_phase(output_phase + 2 * (q % 4), output_power)})
            vectors.append({"width": width, "base": str(base), "columns": [str(c) for c in columns],
                            "linear": linear, "upper": [str(row) for row in upper], "sqrt2_power": power,
                            "phase": phase, "amplitudes": sorted(amplitudes, key=lambda row: int(row["output"])),
                            "norm": encode_scalar(norm)})
    return {"schema": "rmagna.stabilizer-reference.v1", "upstream_commit": captured.manifest["upstream"]["commit"],
            "reference_manifest_sha256": captured.manifest_sha256, "native_sha256": captured.native_sha256,
            "seed": 20261021, "vectors": vectors}


if __name__ == "__main__":
    main()
