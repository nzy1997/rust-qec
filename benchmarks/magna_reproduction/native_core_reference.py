#!/usr/bin/env python3
"""Capture the pinned NativeTests inputs and independent exact expected values.

Run with the pinned Python environment. This uses production contract/norm/scalar
bindings, never the internal test-only overlap/scalar extension or its builder.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import importlib
import json
import random
import sys
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def scalar_data(value):
    return {"coefficients": [str(c) for c in value.coefficients], "power": value.power}


def ket_data(value):
    if value is None:
        return None
    width, base, columns, linear, upper, power, phase = value
    return {"width": width, "base": str(base), "columns": [str(c) for c in columns],
            "linear": list(linear), "upper": [str(c) for c in upper],
            "power": power, "phase": phase}


def capture(upstream, manifest_path):
    # The standalone manifest is supplied explicitly; it remains the repository's
    # existing pinned reference contract, including every source identity.
    require(digest(manifest_path) == "c49203a586f5a451e3183b435793df693dd486857356aed8826df5dc723e5454", "changed pinned reference manifest")
    manifest = json.loads(manifest_path.read_text())
    require(manifest["upstream"]["commit"] == "56847478c840395735146ef1e1ccdc3d8aedc10f", "wrong reference commit")
    def verify():
        identities = {}
        for name, expected in manifest["upstream"]["files"].items():
            path = upstream / name
            require(not any(p.is_symlink() for p in [path, *path.parents] if p != upstream.parent), "symlink in source")
            require(digest(path) == expected, f"changed pinned source: {name}")
            identities[name] = expected
        return identities
    before = verify()
    sys.path[:0] = [str(upstream / "core"), str(upstream)]
    from tests.support.exact_scalar import ExactScalar, scalar
    from magna import _native
    native_path = Path(_native.__file__).resolve()
    require(native_path.is_relative_to(upstream.resolve()), "native module is outside frozen source")
    native_sha = digest(native_path)
    # Execute only the literal dense oracle definition. In particular, do not
    # import native_testing or invoke the optional test extension's load().
    module_path = upstream / "tests/core/test_native_core.py"
    tree = ast.parse(module_path.read_text())
    definitions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "dense"]
    require(len(definitions) == 1, "missing unique original dense oracle")
    namespace = {"ExactScalar": ExactScalar, "scalar": scalar}
    exec(compile(ast.Module(body=definitions, type_ignores=[]), str(module_path), "exec"), namespace)
    dense = namespace["dense"]
    def norm(amplitudes):
        return sum((v.conjugate() * v for v in amplitudes.values()), ExactScalar())
    def exact_native(value):
        return ExactScalar(tuple(value[0]), value[1])
    def record(raw):
        expected = dense(raw)
        contracted = _native.contract(raw)
        require(dense(contracted) == expected, "original contraction disagrees with dense sum")
        expected_norm = norm(expected)
        if contracted is not None:
            require(exact_native(_native.algebra_ket("norm", contracted)) == expected_norm, "original norm disagrees")
        # For large boundaries, enumerate only the raw image (at most 256 paths),
        # never all 2^width basis words. Include one exact zero outside support.
        width, base, columns, *_ = raw
        image = {base}
        for column in columns:
            image |= {word ^ column for word in image}
        probe = next((word for word in range(len(image) + 1) if word < (1 << width) and word not in image), None)
        words = set(range(1 << width)) if width <= 4 else image | ({probe} if probe is not None else set())
        return {"raw": ket_data(raw), "zero": contracted is None,
                "amplitudes": [{"word": str(word), "value": scalar_data(expected.get(word, ExactScalar()))}
                               for word in sorted(words)], "norm": scalar_data(expected_norm)}
    contractions = []
    rng = random.Random(26091461)
    for n in range(9):
        for _ in range(18):
            width = rng.randrange(5)
            raw = (width, rng.randrange(1 << width), tuple(rng.randrange(1 << width) for _ in range(n)),
                   tuple(rng.randrange(4) for _ in range(n)),
                   tuple(rng.randrange(1 << n) & ~((1 << (j + 1)) - 1) for j in range(n)),
                   rng.randrange(-15, 3), rng.randrange(8))
            contractions.append(record(raw))
    overlaps, skips = [], []
    rng = random.Random(26091462)
    for attempt in range(100):
        raw_inputs, contracted = [], []
        for side in range(2):
            n = rng.randrange(6)
            raw = (3, rng.randrange(8), tuple(rng.randrange(8) for _ in range(n)),
                   tuple(rng.randrange(4) for _ in range(n)),
                   tuple(rng.randrange(1 << n) & ~((1 << (j + 1)) - 1) for j in range(n)),
                   rng.randrange(-10, 10), rng.randrange(8))
            raw_inputs.append(raw)
            value = _native.contract(raw)
            require(dense(value) == dense(raw), "overlap input contraction disagrees")
            if value is None:
                skips.append({"attempt": attempt, "zero_side": side,
                              "attempted_inputs": [ket_data(k) for k in raw_inputs]})
                break
            contracted.append(value)
        if len(contracted) != 2:
            continue
        a, b = map(dense, contracted)
        expected = sum((v.conjugate() * b.get(y, ExactScalar()) for y, v in a.items()), ExactScalar())
        overlaps.append({"attempt": attempt, "raw": [ket_data(k) for k in raw_inputs],
                         "contracted": [ket_data(k) for k in contracted], "expected": scalar_data(expected)})
    gauss = []
    for n in (63, 64, 65, 127, 128, 129, 257):
        for linear in (0, 1, 2, 3):
            raw = (0, 0, (0,) * n, (linear,) * n, (0,) * n, 0, 3)
            found = _native.contract(raw)
            actual = ExactScalar() if found is None else scalar((1, 0), found[6], found[5])
            unit = ExactScalar((1, 0, 0, 0)) + scalar((1, 0), 2 * linear, 0)
            expected = scalar((1, 0), 3, 0)
            for _ in range(n):
                expected = expected * unit
            require(actual == expected, "original Gauss sum disagrees with finite closed product")
            gauss.append({"paths": n, "linear": linear, "expected": scalar_data(expected)})
    scalars = []
    # Deliberately continue the overlap RNG, including its original early breaks.
    for _ in range(100):
        a = ExactScalar(tuple(rng.randrange(-(1 << 300), 1 << 300) for _ in range(4)), rng.randrange(-200, 200))
        b = ExactScalar(tuple(rng.randrange(-(1 << 300), 1 << 300) for _ in range(4)), rng.randrange(-200, 200))
        require(exact_native(_native.algebra_scalar("add", (a.coefficients, a.power), (b.coefficients, b.power))) == a + b, "native scalar addition disagrees")
        require(exact_native(_native.algebra_scalar("multiply", (a.coefficients, a.power), (b.coefficients, b.power))) == a * b, "native scalar multiplication disagrees")
        scalars.append({"a": scalar_data(a), "b": scalar_data(b), "sum": scalar_data(a + b), "product": scalar_data(a * b)})
    boundaries = []
    rng = random.Random(26091463)
    for width in (0, 1, 63, 64, 65, 127, 128, 129, 257):
        for _ in range(10):
            n = 6
            columns = [rng.randrange(1 << width) for _ in range(n)]
            columns[4] = columns[0] ^ columns[1]
            columns[5] = 0
            raw = (width, rng.randrange(1 << width), tuple(columns), tuple(rng.randrange(4) for _ in range(n)),
                   tuple(rng.randrange(1 << n) & ~((1 << (j + 1)) - 1) for j in range(n)), -4, 5)
            boundaries.append(record(raw))
    malformed = []
    for role, raw in zip(("column_outside_width", "quadratic_diagonal", "negative_base", "dimension_mismatch"),
                         ((1, 0, (2,), (0,), (0,), 0, 0), (1, 0, (1,), (0,), (1,), 0, 0),
                          (1, -1, (), (), (), 0, 0), (1, 0, (1,), (), (), 0, 0))):
        try:
            _native.contract(raw)
        except ValueError:
            malformed.append({"role": role, "raw": ket_data(raw), "native_exception": "ValueError"})
        else:
            raise ValueError(f"original malformed input was accepted: {role}")
    require([len(x) for x in (contractions, overlaps, skips, gauss, scalars, boundaries, malformed)] == [162, 92, 8, 28, 100, 90, 4], "original RNG inventory changed")
    require(verify() == before and digest(native_path) == native_sha, "original source or native changed")
    return {"schema": "rstim.magna-native-core.v1", "commit": manifest["upstream"]["commit"],
            "reference_sha256": digest(manifest_path), "test_source_sha256": digest(module_path),
            "producer_sha256": digest(__file__), "native_sha256": native_sha,
            "seeds": [26091461, 26091462, 26091463],
            "oracle": "Original independent dense sums and finite closed products; production contraction/norm/scalar cross-checks. Internal test-only overlap/scalar helpers were not called.",
            "contractions": contractions, "overlaps": overlaps, "zero_overlap_skips": skips,
            "gauss": gauss, "scalars": scalars, "boundaries": boundaries, "malformed": malformed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        value = capture(args.upstream.resolve(), args.manifest.resolve())
        with args.out.open("x", encoding="utf-8") as output:
            json.dump(value, output, indent=2, allow_nan=False)
            output.write("\n")
        print(json.dumps({"passed": True, "sha256": digest(args.out), "counts": {k: len(value[k]) for k in ("contractions", "overlaps", "zero_overlap_skips", "gauss", "scalars", "boundaries", "malformed")}}))
        return 0
    except (OSError, ValueError, ImportError) as error:
        print(f"FAIL native core capture: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
