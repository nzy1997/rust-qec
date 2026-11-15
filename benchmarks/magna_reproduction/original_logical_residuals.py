#!/usr/bin/env python3
"""Capture the finite d3 residual classifier and verify deposited proposal data."""
# SPDX-License-Identifier: MPL-2.0
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import zipfile

import reference
from witness_paths import admit_destinations


def require(condition, message):
    if not condition:
        raise ValueError(message)


def capture(upstream, paper_archive):
    upstream, paper_archive = Path(upstream).resolve(), Path(paper_archive).resolve()
    manifest = reference.load_manifest()
    before = reference.verify_source(upstream, manifest)
    archive_hash = reference.digest(paper_archive)
    require(archive_hash == manifest["paper"]["archive_sha256"], "paper archive changed")
    paths = ["raw/logical-distillation/encoded-input/metadata.json",
             "raw/logical-distillation/encoded-input/result.json"]
    paths += [f"raw/cultivation/{case}/failure-p0.001/summary.json" for case in
              ("color3", "color5", "rp2-d3-T", "rp2-d5-T")]
    sources = {}
    with zipfile.ZipFile(paper_archive) as archive:
        for name in paths:
            raw = archive.read("magna-data-deposit/"+name)
            require(hashlib.sha256(raw).hexdigest() == manifest["paper"]["files"][name],
                    f"deposited file changed: {name}")
            sources[name] = json.loads(raw)
    require(not any(n == "decoder" or n.startswith("decoder.") for n in sys.modules),
            "fresh Python process required")
    old_path, old_cache = sys.path[:], sys.pycache_prefix
    with tempfile.TemporaryDirectory(prefix="magna-residual-cache-") as cache:
        sys.pycache_prefix = cache
        sys.path.insert(0, str(upstream/"examples/logical-distillation"))
        try:
            import numpy
            import pymatching
            import stim
            from decoder.patch import Patch
            from decoder.decoder import matching
            patch = Patch(3)

            def mask(values):
                return sum(1 << q for q in values)

            def residuals(checks, logical, basis):
                checks, logical = list(map(mask, checks)), mask(logical)
                graph = matching(3, basis, 1, False)
                result = []
                for error in range(512):
                    # The original decoder is given only its reported syndrome.
                    syndrome = [(error & row).bit_count() % 2 for row in checks]
                    correction = graph.decode(syndrome)
                    correction = sum(int(bit) << q for q, bit in enumerate(correction))
                    remaining = error ^ correction
                    require(all((remaining & row).bit_count() % 2 == 0 for row in checks),
                            "matching correction leaves a nonzero syndrome")
                    result.append((remaining & logical).bit_count() % 2)
                return result

            x = residuals(patch.zchecks, patch.lz, "Z")
            z = residuals(patch.xchecks, patch.lx, "X")
            counts = [[0]*10 for _ in range(4)]
            for a in range(512):
                for b in range(512):
                    counts[x[a]+2*z[b]][(a | b).bit_count()] += 1
            versions = dict(python=sys.version, numpy=numpy.__version__,
                            pymatching=pymatching.__version__, stim=stim.__version__)
        finally:
            sys.path[:], sys.pycache_prefix = old_path, old_cache
    metadata = sources[paths[0]]
    require(counts == metadata["pauli_weight_counts"], "enumeration differs from deposited IXZY counts")
    metadata = {key: metadata[key] for key in
                ("arguments", "proposal", "pauli_weight_counts", "effective_q_leading", "output_error_leading")}
    exact_rows = [dict(p=row["p"], exact=row["exact"]["exact"],
                       effective_q_exact=row["exact"]["effective_q_exact"])
                  for row in sources[paths[1]]["rows"]]
    cultivation = {name.split("/")[2]: {key: sources[name][key] for key in
                    ("rate", "proposal", "noise_categories")} for name in paths[2:]}
    require(reference.verify_source(upstream, manifest) == before, "source changed during capture")
    require(reference.digest(paper_archive) == archive_hash, "archive changed during capture")
    source_names = ["examples/logical-distillation/decoder/patch.py",
                    "examples/logical-distillation/decoder/decoder.py"]
    files = manifest["upstream"]["files"] | manifest["upstream"].get("decoder_files", {})
    return dict(schema="rmagna.original-logical-residuals.v1", upstream_commit=manifest["upstream"]["commit"],
        paper=manifest["paper"]["arxiv"], paper_archive_sha256=archive_hash,
        reference_sha256=reference.digest(reference.MANIFEST), producer_sha256=reference.digest(Path(__file__)),
        source_sha256={name: files[name] for name in source_names},
        paper_source_sha256={name: manifest["paper"]["files"][name] for name in paths},
        versions=versions, geometry=dict(xchecks=patch.xchecks, zchecks=patch.zchecks, lx=patch.lx, lz=patch.lz),
        x_residual=x, z_residual=z, counts_IXZY=counts, patterns=4**9,
        matches_historical_counts=True, metadata=metadata, exact_rows=exact_rows, cultivation=cultivation)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--paper-archive", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream,
            protected_roots=[reference.HERE, args.paper_archive.parent])
        result = capture(args.upstream, args.paper_archive)
        with destination.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, separators=(",", ":"), allow_nan=False)
            stream.write("\n")
    except (ValueError, OSError, KeyError, zipfile.BadZipFile) as error:
        parser.error(str(error))
    print("classified all262144 physical Pauli patterns; IXZY weight counts match deposited data")


if __name__ == "__main__":
    main()
