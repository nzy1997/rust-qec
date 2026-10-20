#!/usr/bin/env python3
"""Generate immutable exact scalar witnesses from the pinned Magna reference."""

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
        rng = random.Random(20261020)

        def encode(value):
            coefficients, power = value
            return {"coefficients": [str(c) for c in coefficients], "power": power}

        vectors = []
        for _ in range(16):
            def operand():
                return (tuple(rng.getrandbits(300) * rng.choice((-1, 1)) for _ in range(4)), rng.randrange(-400, 401))
            a, b = operand(), operand()
            native = captured.native
            vectors.append({
                "left": encode(a), "right": encode(b),
                "normalized_left": encode(native.algebra_scalar("normalize", a)),
                "normalized_right": encode(native.algebra_scalar("normalize", b)),
                "sum": encode(native.algebra_scalar("add", a, b)),
                "product": encode(native.algebra_scalar("multiply", a, b)),
                "conjugate_left": encode(native.algebra_scalar("conjugate", a)),
            })
        data = {"schema": "rmagna.scalar-reference.v1", "upstream_commit": captured.manifest["upstream"]["commit"],
                "reference_manifest_sha256": captured.manifest_sha256,
                "native_sha256": captured.native_sha256, "seed": 20261020, "vectors": vectors}
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
