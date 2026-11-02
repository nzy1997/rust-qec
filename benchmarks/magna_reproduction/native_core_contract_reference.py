#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
# Seeded scenario loops adapted from itpyi/magna tests/core/test_native_core.py,
# commit 56847478c840395735146ef1e1ccdc3d8aedc10f (MPL-2.0).
"""Capture the exact pinned NativeTests inputs with literal dense/product oracles.

The upstream test-only native_testing.load() is never called: it builds inside
its source tree. Production contraction, norm and scalar operations cross-check
expected values. Overlap expectations come from the upstream literal dense sum.
"""

import argparse
import ast
from types import SimpleNamespace
import json
import pathlib
import random
import sys

HERE = pathlib.Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
from witness_environment import native_reference
import reference
from witness_paths import admit_destinations


def encoded(v):
    return dict(coefficients=[str(c) for c in v.coefficients], power=v.power)


def raw(v):
    w, b, c, l, u, p, f = v
    return dict(
        width=w,
        base=str(b),
        columns=list(map(str, c)),
        linear=list(l),
        upper=list(map(str, u)),
        sqrt2_power=p,
        phase=f,
    )


def literal_oracles(source):
    """Load only the two literal definitions, never the test module or its loader."""
    from tests.support.exact_scalar import ExactScalar, scalar
    path = source / "tests/core/test_native_core.py"
    tree = ast.parse(path.read_text())
    definitions = [node for node in tree.body
                   if isinstance(node, ast.FunctionDef) and node.name in {"dense", "exact"}]
    if len(definitions) != 2 or {node.name for node in definitions} != {"dense", "exact"}:
        raise reference.ReferenceError("missing unique original dense/exact oracles")
    namespace = {"ExactScalar": ExactScalar, "scalar": scalar}
    exec(compile(ast.Module(body=definitions, type_ignores=[]), str(path), "exec"), namespace)
    return SimpleNamespace(dense=namespace["dense"], exact=namespace["exact"],
                           ExactScalar=ExactScalar, scalar=scalar)


def capture(upstream):
    upstream = pathlib.Path(upstream).resolve()
    with native_reference(upstream) as captured:
        source = upstream.resolve()
        sys.path.insert(0, str(source))
        m = literal_oracles(source)
        native = captured.native

        def witness(k):
            found = native.contract(k)
            expected = m.dense(k)
            actual = m.dense(found)
            if actual != expected:
                raise RuntimeError(
                    "native contraction disagrees with upstream literal oracle"
                )
            expected_norm = sum(
                (v.conjugate() * v for v in expected.values()), m.ExactScalar()
            )
            found_norm = (
                m.ExactScalar()
                if found is None
                else m.exact(native.algebra_ket("norm", found))
            )
            if expected_norm != found_norm:
                raise RuntimeError("native norm differs")
            return dict(
                input=raw(k),
                rank=None if found is None else len(found[2]),
                norm=encoded(expected_norm),
                amplitudes=[
                    dict(output=str(y), value=encoded(v))
                    for y, v in sorted(expected.items())
                ],
            )

        groups = {}
        rng = random.Random(26091461)
        rows = []
        for n in range(9):
            for trial in range(18):
                width = rng.randrange(5)
                k = (
                    width,
                    rng.randrange(1 << width),
                    tuple((rng.randrange(1 << width) for j in range(n))),
                    tuple((rng.randrange(4) for j in range(n))),
                    tuple(
                        (rng.randrange(1 << n) & ~((1 << j + 1) - 1) for j in range(n))
                    ),
                    rng.randrange(-15, 3),
                    rng.randrange(8),
                )
                rows.append(dict(paths=n, trial=trial, **witness(k)))
        groups["contractions"] = (26091461, rows)
        rng = random.Random(26091462)
        rows = []
        skipped = []
        for trial in range(100):
            ks = []
            for side in range(2):
                n = rng.randrange(6)
                k = (
                    3,
                    rng.randrange(8),
                    tuple((rng.randrange(8) for j in range(n))),
                    tuple((rng.randrange(4) for j in range(n))),
                    tuple(
                        (rng.randrange(1 << n) & ~((1 << j + 1) - 1) for j in range(n))
                    ),
                    rng.randrange(-10, 10),
                    rng.randrange(8),
                )
                found = native.contract(k)
                if found is None:
                    if m.dense(k):
                        raise RuntimeError("native zero contraction disagrees with literal sum")
                    skipped.append(dict(trial=trial, side=side, input=raw(k)))
                    break
                ks.append(found)
            if len(ks) != 2:
                continue
            da, db = map(m.dense, ks)
            expected = sum(
                (v.conjugate() * db.get(y, m.ExactScalar()) for y, v in da.items()),
                m.ExactScalar(),
            )
            rows.append(
                dict(
                    trial=trial,
                    left=raw(ks[0]),
                    right=raw(ks[1]),
                    value=encoded(expected),
                )
            )
        groups["overlaps"] = (26091462, rows)
        wide = []
        for n in (63, 64, 65, 127, 128, 129, 257):
            for l in (0, 1, 2, 3):
                k = (0, 0, (0,) * n, (l,) * n, (0,) * n, 0, 3)
                unit = m.ExactScalar((1, 0, 0, 0)) + m.scalar((1, 0), 2 * l, 0)
                expected = m.scalar((1, 0), 3, 0)
                for _ in range(n):
                    expected = expected * unit
                found = native.contract(k)
                observed = (
                    m.ExactScalar()
                    if found is None
                    else m.scalar((1, 0), found[6], found[5])
                )
                expected_norm = expected.conjugate() * expected
                found_norm = (
                    m.ExactScalar()
                    if found is None
                    else m.exact(native.algebra_ket("norm", found))
                )
                if found_norm != expected_norm:
                    raise RuntimeError("wide Gaussian norm differs")
                row = dict(
                    input=raw(k),
                    rank=None if found is None else len(found[2]),
                    norm=encoded(expected_norm),
                    amplitudes=(
                        []
                        if not expected
                        else [dict(output="0", value=encoded(expected))]
                    ),
                )
                if observed != expected:
                    raise RuntimeError("wide Gaussian product differs")
                wide.append(
                    dict(
                        paths=n,
                        linear_phase=l,
                        expected_product=encoded(expected),
                        **row
                    )
                )
        groups["wide_gaussian"] = (None, wide)
        big = []
        for trial in range(100):
            x = m.ExactScalar(
                tuple((rng.randrange(-(1 << 300), 1 << 300) for j in range(4))),
                rng.randrange(-200, 200),
            )
            y = m.ExactScalar(
                tuple((rng.randrange(-(1 << 300), 1 << 300) for j in range(4))),
                rng.randrange(-200, 200),
            )
            left = (x.coefficients, x.power)
            right = (y.coefficients, y.power)
            add = m.exact(native.algebra_scalar("add", left, right))
            multiply = m.exact(native.algebra_scalar("multiply", left, right))
            if add != x + y or multiply != x * y:
                raise RuntimeError(
                    "native scalar arithmetic differs from upstream literal oracle"
                )
            big.append(
                dict(
                    trial=trial,
                    left=encoded(x),
                    right=encoded(y),
                    sum=encoded(add),
                    product=encoded(multiply),
                )
            )
        groups["large_scalars_0"] = (26091462, big[:50])
        groups["large_scalars_1"] = (26091462, big[50:])
        rng = random.Random(26091463)
        packed = []
        for width in (0, 1, 63, 64, 65, 127, 128, 129, 257):
            for trial in range(10):
                n = 6
                cols = [rng.randrange(1 << width) for j in range(n)]
                cols[4] = cols[0] ^ cols[1]
                cols[5] = 0
                k = (
                    width,
                    rng.randrange(1 << width),
                    tuple(cols),
                    tuple((rng.randrange(4) for j in range(n))),
                    tuple(
                        (rng.randrange(1 << n) & ~((1 << j + 1) - 1) for j in range(n))
                    ),
                    -4,
                    5,
                )
                packed.append(dict(trial=trial, **witness(k)))
        groups["packed"] = (26091463, packed)
        license_text = (source / "LICENSE").read_text()
        common = dict(
            schema="rmagna.native-core-contract.v1",
            upstream_commit=captured.manifest["upstream"]["commit"],
            source_module="tests/core/test_native_core.py",
            source_sha256=captured.manifest["upstream"]["files"][
                "tests/core/test_native_core.py"
            ],
            native_sha256=captured.native_sha256,
            reference_manifest_sha256=captured.manifest_sha256,
            expected_kind="upstream literal dense/product oracle; production native contraction/norm/scalar cross-checks",
        )
    return common, groups, license_text, skipped


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    a = parser.parse_args()
    try:
        a.out, = admit_destinations([a.out], a.upstream)
    except ValueError as error:
        parser.error(str(error))
    common, groups, license_text, skipped = capture(a.upstream.resolve())
    a.out.mkdir()
    (a.out / "LICENSE").write_text(license_text)
    (a.out / "NOTICE.md").write_text(
        "# Pinned Magna native-core scenarios\n\n"
        "Source: https://github.com/itpyi/magna/blob/"
        "56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_native_core.py\n\n"
        "The original seeded scenario definitions are adapted from Magna under "
        "MPL-2.0; the complete upstream license is included in LICENSE. "
        "The fixture values are generated by native_core_contract_reference.py "
        "using those inputs and the upstream literal dense/product oracles. "
        "The independent Rust assertion implementation is separately authored.\n"
    )
    files = []
    for family, (seed, rows) in groups.items():
        path = a.out / (family + ".json")
        value = dict(common, family=family, seed=seed, vectors=rows)
        if family == "overlaps":
            value["skipped_zero_contractions"] = skipped
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        files.append(
            dict(
                file=path.name,
                sha256=reference.digest(path),
                bytes=path.stat().st_size,
                vectors=len(rows),
            )
        )
    (a.out / "receipt.json").write_text(
        json.dumps(dict(common, files=files), indent=2, sort_keys=True) + "\n"
    )
    print(json.dumps(files, indent=2))


if __name__ == "__main__":
    main()
