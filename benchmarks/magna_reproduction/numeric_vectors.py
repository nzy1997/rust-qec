"""Independent high-precision decimal witnesses for numeric field exports."""

import argparse
from decimal import Decimal, localcontext
import json
import math
import random
import struct


def inputs():
    rng = random.Random(888)
    pairs = [
        (1, 1, 0, 1),
        (0, 1, 1, 1),
        (1, 3, -1, 7),
        (-7, 11, 13, 17),
        (10**400 + 1, 3, -(10**400), 5),
        (1, 10**400, 0, 1),
    ]
    a, b = 1, 0
    for _ in range(128):
        a, b = a + 2 * b, a + b
    pairs.extend([(a, 1, -b, 1), (a, 1, b, 1)])
    for _ in range(72):
        pairs.append((
            rng.randrange(-(10**50), 10**50),
            rng.randrange(1, 10**40),
            rng.randrange(-(10**50), 10**50),
            rng.randrange(1, 10**40),
        ))
    return pairs


def bits(value):
    return f'{struct.unpack(">Q", struct.pack(">d", value))[0]:016x}'


def export(value):
    if not value:
        return {"mantissa_bits": bits(0.0), "exponent": 0, "binary64": bits(0.0)}
    absolute = abs(value)
    exponent = math.floor(absolute.adjusted() * math.log2(10))
    while absolute < Decimal(2)**exponent:
        exponent -= 1
    while absolute >= Decimal(2)**(exponent + 1):
        exponent += 1
    mantissa = float(value / Decimal(2)**exponent)
    if abs(mantissa) == 2:
        mantissa /= 2
        exponent += 1
    direct = float(value)
    if math.isinf(direct):
        binary64 = "overflow-" if value < 0 else "overflow+"
    elif direct == 0:
        binary64 = "underflow-" if value < 0 else "underflow+"
    else:
        binary64 = bits(direct)
    return {"mantissa_bits": bits(mantissa), "exponent": exponent, "binary64": binary64}


def generate(precision):
    rows = []
    with localcontext() as context:
        context.prec = precision
        root = Decimal(2).sqrt()
        for a, ad, b, bd in inputs():
            value = Decimal(a) / ad + Decimal(b) / bd * root
            rows.append({
                "a": str(a), "ad": str(ad), "b": str(b), "bd": str(bd),
                "real": export(value),
                "sqrt": export(value.sqrt()) if value >= 0 else None,
            })
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    rows = generate(1200)
    if rows != generate(2400):
        raise RuntimeError("decimal witnesses changed between 1200 and 2400 digits")
    with open(args.out, "w", encoding="utf-8") as output:
        json.dump(rows, output, indent=2)
        output.write("\n")
    print(len(rows), "stable independent decimal witnesses")


if __name__ == "__main__":
    main()
