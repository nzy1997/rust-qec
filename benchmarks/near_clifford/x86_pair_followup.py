"""Retain actual measured x86 binaries/assembly, then run same-binary A/A controls.

Assembly extraction finishes before controls begin. This adds no instrumentation
to measured code and never substitutes A/A noise estimates for source effects.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

import rust_pair_scout as scout


def retain_binaries(campaign, out):
    header, closure = scout.verify_closed_campaign(campaign)
    if header["schema"] != "exploratory.avx2-pair-ablation.v1":
        raise ValueError("production extraction requires the original source-pair campaign")
    roots = {"baseline": scout.ROOT / "drafts/rust-pair-scout-baseline", "candidate": scout.ROOT}
    identities = {role: scout.identity(root) for role, root in roots.items()}
    if identities != closure["identities_after"]:
        raise ValueError("current binaries/source no longer match measured campaign")
    out.mkdir(parents=True, exist_ok=False)
    retained = {}
    for role, root in roots.items():
        binary = scout.probe(root) / "target/release/counts-path-ablation"
        target = out / (role + ".bin")
        shutil.copyfile(binary, target)
        if scout.digest(target) != identities[role]["binary"]:
            raise ValueError("copied production binary digest mismatch")
        outputs = {}
        for label, command in [("assembly", ["objdump", "-drwC", str(target)]), ("symbols", ["nm", "-SC", "--size-sort", str(target)])]:
            path = out / (role + "." + label + ".txt")
            with path.open("wb") as log:
                subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=300)
            outputs[label] = {"path": path.name, "sha256": scout.digest(path), "command": command}
        retained[role] = {"path": target.name, "sha256": scout.digest(target), "outputs": outputs}
    after = {role: scout.identity(root) for role, root in roots.items()}
    if after != identities:
        raise ValueError("source/binary changed during extraction")
    receipt = dict(scope="Actual previously measured public-API probe binaries; extraction has no valid timing", measured_header_sha256=scout.digest(campaign / "header.json"), measured_closure_sha256=scout.digest(campaign / "closure.json"), identities=identities, retained=retained)
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--closed-campaign", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    retain_binaries(args.closed_campaign, args.out / "production-binaries")
    roots = {"baseline": scout.ROOT, "candidate": scout.ROOT}
    scout.collect(roots, args.out / "same-binary-warm", same_binary_control=True)
    scout.collect(roots, args.out / "same-binary-confirmation", same_binary_control=True)
