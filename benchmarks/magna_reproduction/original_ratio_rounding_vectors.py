#!/usr/bin/env python3
"""Capture complete pinned ratio-rounding loops with a prebuilt read-only oracle.

The portable fixture contains integer inputs and IEEE mantissa bits. Runtime,
binary, build and generator identities belong to the separate receipt. Neither
capture nor import invokes the original build-capable native_testing loader.
"""
from __future__ import annotations

import argparse
import ast
from fractions import Fraction
import hashlib
import importlib.util
import json
import math
from pathlib import Path
from random import Random
import stat
import struct
import sys
import sysconfig

import reference


COMMIT = "56847478c840395735146ef1e1ccdc3d8aedc10f"
MANIFEST_SHA256 = "c49203a586f5a451e3183b435793df693dd486857356aed8826df5dc723e5454"
RATIO_METHOD = "test_noise_numeric_fastpaths.NoiseNumericFastpathTests.test_rational_ratios_against_python_fraction_rounding"
LARGE_METHOD = "test_weight_fast_paths.WeightFastPathTests.test_large_rational_rounding_and_extreme_exponents"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def identity(path):
    path = Path(path).resolve()
    return {"path": str(path), "sha256": reference.digest(path), "bytes": path.stat().st_size}


def bits(value):
    return f'{struct.unpack(">Q", struct.pack(">d", value))[0]:016x}'


def ratio_inputs():
    """The complete original Random(20260927) call order, including both powers."""
    rng = Random(20260927)
    rows = []
    for width in (1, 16, 52, 53, 54, 128, 257):
        for _ in range(50):
            n = rng.randrange(1, 1 << width) * rng.choice((-1, 1))
            d = rng.randrange(1, 1 << width)
            a, b = rng.randrange(-500000, 500001), rng.randrange(-500000, 500001)
            rows.append({"n": str(n), "d": str(d), "a": a, "b": b})
    return rows


def large_inputs():
    """All 210 random pairs and the original 27 midpoint/neighbor pairs."""
    rng = Random(5641)
    rows = [
        {"n": str(rng.getrandbits(n) or 1), "d": str(rng.getrandbits(d) or 1),
         "kind": "random", "n_bits": n, "d_bits": d}
        for n in (54, 63, 64, 65, 127, 1024, 5000)
        for d in (3, 53, 64, 65, 129, 3000) for _ in range(5)
    ]
    for width in (54, 100, 1024):
        for q in (2**52, 2**52 + 1, 2**53 - 1):
            for offset in (-1, 0, 1):
                rows.append({"n": str((2*q + 1)*2**width + offset),
                             "d": str(2**(width + 54)), "kind": "midpoint",
                             "bits": width, "q": str(q), "offset": offset})
    return rows


def expected_ratio(n, d, power):
    mantissa, exponent = math.frexp(float(Fraction(n, d)))
    return {"mantissa_bits": bits(mantissa), "exponent": exponent + power}


def expected_large(n, d, sign, power):
    exponent = n.bit_length() - d.bit_length() + 1
    normalized = Fraction(n, d) * (Fraction(1, 2**exponent) if exponent >= 0 else 2**-exponent)
    mantissa, shift = math.frexp(float(normalized))
    return {"mantissa_bits": bits(sign * mantissa), "exponent": exponent + shift + power}


def check_native(oracle, pairs, expected):
    actual = oracle.file_scalar_ratios(pairs)
    require(len(actual) == len(expected), "native ratio result count differs from the original loop")
    for index, (value, wanted) in enumerate(zip(actual, expected)):
        require(isinstance(value, (tuple, list)) and len(value) == 2,
                f"native ratio {index} must contain mantissa and exponent")
        mantissa, exponent = value
        require(type(mantissa) is float and math.isfinite(mantissa)
                and 0.5 <= abs(mantissa) < 1 and type(exponent) is int,
                f"native ratio {index} is not normalized scientific binary64")
        require({"mantissa_bits": bits(mantissa), "exponent": exponent} == wanted,
                f"native ratio {index} disagrees with independent Fraction/frexp rounding")


def portable_values(oracle, source_hashes):
    small, large = ratio_inputs(), large_inputs()
    require(len(small) == 350 and len(large) == 237,
            "complete original ratio input inventory changed")
    pairs, expected = [], []
    for row in small:
        n, d = int(row["n"]), int(row["d"])
        pairs.append((((n, 0, 0, 0), row["a"]), ((d, 0, 0, 0), row["b"])))
        value = expected_ratio(n, d, row["a"] - row["b"])
        row["expected"] = value
        expected.append(value)
    check_native(oracle, pairs, expected)
    for row in large:
        n, d = int(row["n"]), int(row["d"])
        variants = []
        for sign, power in ((1, 0), (-1, -99999), (1, 99999)):
            value = expected_large(n, d, sign, power)
            check_native(oracle, ((((sign*n, 0, 0, 0), power), ((d, 0, 0, 0), 0)),), [value])
            variants.append({"sign": sign, "power": power, "expected": value})
        row["variants"] = variants
    return {"schema": "rstim.magna-original-ratio-rounding.v1", "upstream_commit": COMMIT,
            "reference_sha256": MANIFEST_SHA256, "test_source_sha256": source_hashes,
            "rational_ratios": {"method": RATIO_METHOD, "seed": 20260927, "cases": small},
            "large_rounding": {"method": LARGE_METHOD, "seed": 5641, "cases": large}}


def source_digest(upstream, version, suffix):
    """Reproduce only the loader's source digest, without importing its code."""
    loader = upstream / "tests/support/native_testing.py"
    tree = ast.parse(loader.read_text(encoding="utf-8"))
    flags = ast.literal_eval(next(node.value for node in tree.body if isinstance(node, ast.Assign)
                                 and any(isinstance(t, ast.Name) and t.id == "FLAGS" for t in node.targets)))
    native, oracles = upstream / "core/magna/native", upstream / "tests/native/python"
    units = [oracles / "testing_module.cpp", *sorted(p for p in native.rglob("*.cpp")
                                                    if p.parent.name != "bindings")]
    sources = sorted({*units, *native.rglob("*.hpp"), *oracles.glob("*.hpp")})
    digest = hashlib.sha256(" ".join([*flags, version, suffix]).encode())
    hashes = {}
    for path in sources:
        name = path.relative_to(upstream).as_posix()
        data = path.read_bytes()
        digest.update(name.encode())
        digest.update(data)
        hashes[name] = hashlib.sha256(data).hexdigest()
    return digest.hexdigest()[:16], flags, hashes


def verify_helper(upstream, helper, record):
    suffix = sysconfig.get_config_var("EXT_SUFFIX")
    digest, flags, hashes = source_digest(upstream, sys.version, suffix)
    require(helper.name == "_native_testing" + suffix and helper.parent.name == digest,
            "prebuilt test helper name/source digest does not match the current Python ABI")
    require(helper.is_file() and not stat.S_IMODE(helper.stat().st_mode) & 0o222,
            "prebuilt test helper must be an existing read-only regular file")
    require(Path(record["helper"]["path"]).resolve() == helper
            and reference.digest(helper) == record["helper"]["sha256"],
            "prebuilt test helper differs from its admitted binary identity")
    require(record["recorded_python_version"] == sys.version
            and reference.digest(Path(sys.executable).resolve()) == record["runtime"]["sha256"],
            "run with the pinned Python executable/version from the helper admission")
    require(record["loader_FLAGS"] == flags
            and {name: row["sha256"] for name, row in record["source_cpp_hpp"].items()} == hashes,
            "prebuilt helper source/build identity differs from the pinned reference")
    for row in record["shared_libraries"]:
        require(reference.digest(Path(row["resolved"])) == row["sha256"],
                "pinned Python shared library changed")
    return {"source_digest": digest, "source_sha256": hashes, "flags": flags}


def capture(upstream, helper, helper_record):
    upstream, helper = Path(upstream).resolve(), Path(helper).resolve()
    require(reference.digest(reference.MANIFEST) == MANIFEST_SHA256,
            "frozen reference manifest changed")
    manifest = reference.load_manifest()
    require(manifest["upstream"]["commit"] == COMMIT, "wrong pinned reference commit")
    before = reference.verify_source(upstream, manifest)
    record = json.loads(Path(helper_record).read_text(encoding="utf-8"))
    helper_proof = verify_helper(upstream, helper, record)
    helper_identity, producer = identity(helper), identity(__file__)
    require(not any(n == "magna" or n.startswith("magna.") or n == "_native_testing"
                    for n in sys.modules), "run capture in a fresh Python process")
    previous_path, previous_bytecode = sys.path[:], sys.dont_write_bytecode
    native_identity = None
    try:
        sys.dont_write_bytecode = True
        sys.path[:0] = [str(upstream / "core"), str(upstream)]
        from magna import _native
        native_identity = identity(_native.__file__)
        require(Path(native_identity["path"]).is_relative_to(upstream / "core/magna"),
                "production native extension is outside the verified source")
        spec = importlib.util.spec_from_file_location("_native_testing", helper)
        oracle = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(oracle)
        sys.modules["_native_testing"] = oracle
        require(Path(oracle.__file__).resolve() == helper,
                "loaded native test oracle differs from the admitted input")
        paths = ("tests/core/test_noise_numeric_fastpaths.py", "tests/core/test_weight_fast_paths.py")
        payload = portable_values(oracle, {name: manifest["upstream"]["files"][name] for name in paths})
        receipt = {"schema": "rstim.magna-original-ratio-rounding-receipt.v1", "passed": True,
                   "reference_sha256": MANIFEST_SHA256, "upstream_commit": COMMIT,
                   "producer": producer, "python": {"version": sys.version,
                   "executable": identity(Path(sys.executable).resolve()),
                   "extension_suffix": sysconfig.get_config_var("EXT_SUFFIX")},
                   "production": native_identity, "helper": helper_identity,
                   "helper_admission": identity(helper_record), "helper_build": helper_proof,
                   "counts": {"rational_cases": 350, "large_pairs": 237,
                              "large_comparisons": 711, "midpoint_pairs": 27},
                   "scope": "Existing source/ABI/hash-bound test oracle loaded read-only; no builder invoked. Independent Fraction/frexp values compared bitwise. Host/build identities excluded from portable fixture."}
        return payload, receipt
    finally:
        sys.path[:] = previous_path
        sys.dont_write_bytecode = previous_bytecode
        require(reference.verify_source(upstream, manifest) == before,
                "reference source changed during capture, including its failure path")
        require(identity(helper) == helper_identity and identity(__file__) == producer,
                "helper or generator changed during capture")
        if native_identity is not None:
            require(identity(native_identity["path"]) == native_identity,
                    "production native extension changed during capture")


def admit_destinations(outputs, upstream, helper, helper_record):
    lexical = [Path(p).absolute() for p in outputs]
    require(all(not p.exists() and not p.is_symlink() for p in lexical),
            "witness and receipt destinations must be fresh, including existing links")
    resolved = [p.resolve() for p in lexical]
    require(len(set(resolved)) == len(resolved), "witness and receipt destinations must be distinct")
    record = json.loads(Path(helper_record).read_text(encoding="utf-8"))
    roots = [Path(upstream).resolve(), Path(__file__).resolve().parents[2] / "rmagna",
             Path(record["source_root"]).resolve(), Path(helper).resolve().parent]
    require(all(not p.is_relative_to(root) for p in resolved for root in roots),
            "witness destinations must be outside upstream, helper and Rust source-input trees")
    return resolved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--helper", type=Path, required=True)
    parser.add_argument("--helper-record", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    output, receipt_path = admit_destinations([args.out, args.receipt], args.upstream,
                                             args.helper, args.helper_record)
    payload, receipt = capture(args.upstream, args.helper, args.helper_record)
    data = (json.dumps(payload, indent=2) + "\n").encode()
    receipt["fixture"] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    with output.open("xb") as stream:
        stream.write(data)
    with receipt_path.open("x", encoding="utf-8") as stream:
        json.dump(receipt, stream, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": True, **receipt["counts"], "fixture": receipt["fixture"]}))


if __name__ == "__main__":
    main()
