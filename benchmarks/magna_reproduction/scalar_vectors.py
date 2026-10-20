#!/usr/bin/env python3
"""Generate immutable exact scalar witnesses from the pinned Magna reference."""

import argparse
import hashlib
import json
from pathlib import Path
import random
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    # Installed beside reference.py when integrated into the workspace.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import reference

    manifest = reference.load_manifest()
    source = args.upstream.resolve()
    reference.verify_source(source, manifest)
    sys.path.insert(0, str(source / "core"))
    from magna import _native

    binary = Path(_native.__file__).resolve()
    if not binary.is_relative_to(source / "core/magna"):
        raise ValueError("native extension is outside the pinned source tree")
    binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    rng = random.Random(20261020)

    def encode(value):
        coefficients, power = value
        return {"coefficients": [str(c) for c in coefficients], "power": power}

    vectors = []
    for _ in range(16):
        def operand():
            return (tuple(rng.getrandbits(300) * rng.choice((-1, 1)) for _ in range(4)), rng.randrange(-400, 401))
        a, b = operand(), operand()
        vectors.append({
            "left": encode(a), "right": encode(b),
            "normalized_left": encode(_native.algebra_scalar("normalize", a)),
            "normalized_right": encode(_native.algebra_scalar("normalize", b)),
            "sum": encode(_native.algebra_scalar("add", a, b)),
            "product": encode(_native.algebra_scalar("multiply", a, b)),
            "conjugate_left": encode(_native.algebra_scalar("conjugate", a)),
        })
    reference.verify_source(source, manifest)
    if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_hash:
        raise ValueError("native extension changed during witness generation")
    data = {"schema": "rmagna.scalar-reference.v1", "upstream_commit": manifest["upstream"]["commit"],
            "reference_manifest_sha256": reference.digest(reference.MANIFEST),
            "native_sha256": binary_hash, "seed": 20261020, "vectors": vectors}
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(data, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
