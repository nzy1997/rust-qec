"""Capture and verify small matched work and exact-value comparisons."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys

from paper_probe_identity import source_identity, source_inputs
from remaining_contracts import read_json, reference, require


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "rmagna.paper-ablation-capture.v1"
PRODUCER = "rmagna/examples/paper_ablation_run.rs"
PHYSICAL = b"H 0 1\nCX 1 2\nT 2\nCX 1 2\nH 1\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nPOSTSELECT det[0]"
TARGET = b"I 0"
HISTORY_DRAWS = 64
MASK = (1 << 64)-1
LANE_SEED = 2026100801
CUTOFFS = (0, 1, 1 << 63, 0x5555555555555555, MASK-1, MASK)
LIVE = (MASK, 0xa55a5aa5f00f6996)
MODES = {
    "baseline": ("FixedTemplate", "SharedCommutingLayer", "ExactMarginal"),
    "cold": ("ColdPrefix", "SharedCommutingLayer", "ExactMarginal"),
    "parity_copies": ("FixedTemplate", "ParityCopies", "ExactMarginal"),
    "affine": ("FixedTemplate", "SharedCommutingLayer", "AffineSupport"),
}
HALF = dict(coefficients=["1", "0", "0", "0"], power=-1)
ZERO = dict(coefficients=["0", "0", "0", "0"], power=0)
JOINT_ERROR = dict(coefficients=["2", "-1", "0", "1"], power=-3)


def sha(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def nonnegative(value):
    return type(value) is int and value >= 0


def source_digest(inputs):
    require({PRODUCER, "Cargo.lock", "rmagna/build.rs"} <= inputs.keys(),
            "ablation producer or lockfile is missing from the source inventory")
    digest = hashlib.sha256()
    for name, value in sorted(inputs.items()):
        require(sha(value) and "\0" not in name and "\n" not in name, "invalid ablation source file binding")
        digest.update(name.encode()+b"\0"+value.encode()+b"\n")
    return digest.hexdigest()


def verify_histories(rows):
    require([row["name"] for row in rows] == list(MODES), "ablation routes changed or are incomplete")
    counters = ("clifford_gates", "path_variables", "phase_ancillas", "shared_phase_coordinates")
    observed = {}
    for cell in rows:
        name = cell["name"]
        require(tuple(cell[key] for key in ("causal_strategy", "compiler", "history_sampler")) == MODES[name],
                f"{name} ablation changes more than its declared factor")
        require(type(cell["reference_compilations"]) is int and cell["reference_compilations"] == 1
                and all(nonnegative(cell[key]) for key in counters), "invalid ablation compilation measurements")
        require(len(cell["rows"]) == HISTORY_DRAWS, "incomplete ablation history tape")
        reports = Counter()
        work = Counter()
        for index, row in enumerate(cell["rows"]):
            tape = "0000000000000000" if index % 2 == 0 else "c000000000000001"
            bit = row["reported"]
            require(type(row["index"]) is int and row["index"] == index and row["tape"] == tape
                    and type(bit) is bool and row["callback"] == [bit] and type(row["callback"][0]) is bool,
                    f"{name} ablation tape or actual reported callback differs")
            require(row["history"] == HALF and row["proposal"] == (HALF if name == "affine" else None)
                    and row["accepted"] == (ZERO if bit else HALF)
                    and row["error"] == (ZERO if bit else JOINT_ERROR),
                    f"{name} ablation exact branch masses changed")
            for key in ("history_queries", "network_compilations", "history_words", "noise_words"):
                require(nonnegative(row[key]), "invalid ablation history work count")
                work[key] += row[key]
            # The single reported bit needs one query. The cold route builds
            # that prefix and the terminal network for each replay.
            require(row["noise_words"] == 0 and row["history_words"] == row["history_queries"] == 1
                    and row["network_compilations"] == (2 if name == "cold" else 0),
                    "ablation work or entropy route changed")
            reports[bit] += 1
        require(reports == Counter({False: HISTORY_DRAWS//2, True: HISTORY_DRAWS//2}),
                "sampler pair lost a branch with the same physical mass")
        observed[name] = dict(strategy=dict(zip(("causal", "compiler", "history"), MODES[name], strict=True)),
                              **{key:cell[key] for key in counters}, **work, draws=len(cell["rows"]))
    require(observed["baseline"]["phase_ancillas"] < observed["parity_copies"]["phase_ancillas"]
            and observed["baseline"]["shared_phase_coordinates"] > observed["parity_copies"]["shared_phase_coordinates"],
            "shared versus parity-copy fixture did not exercise different compiler representations")
    return observed


def splitmix_words(seed):
    state = seed
    while True:
        state = (state+0x9e3779b97f4a7c15)&MASK
        word = state
        word = ((word ^ (word >> 30))*0xbf58476d1ce4e5b9)&MASK
        word = ((word ^ (word >> 27))*0x94d049bb133111eb)&MASK
        yield word ^ (word >> 31)


def determining_prefix(word, cutoff):
    """Count inspected high bits using scalar interval bounds."""
    for bits in range(65):
        trailing = 64-bits
        lo = (word >> trailing) << trailing
        hi = lo+(1 << trailing)-1
        if lo >= cutoff or hi < cutoff:
            return bits
    raise ValueError("finite occurrence interval never determined its comparison")


def verify_occurrence(rows):
    require(len(rows) == len(CUTOFFS)*len(LIVE), "incomplete finite-occurrence tapes")
    stream = splitmix_words(LANE_SEED)
    totals = Counter()
    groups = []
    for index, row in enumerate(rows):
        cutoff_index, mask_index = divmod(index,len(LIVE))
        cutoff, live = CUTOFFS[cutoff_index], LIVE[mask_index]
        words = [next(stream) for _ in range(64)]
        words[:4] = [0, (cutoff-1)&MASK, cutoff, MASK]
        require(row["cutoff_index"] == cutoff_index and row["cutoff"] == f"{cutoff:016x}"
                and row["live"] == f"{live:016x}" and row["words"] == [f"{word:016x}" for word in words],
                "finite-occurrence input stream changed")
        lanes = [(i,word) for i,word in enumerate(words) if live >> i & 1]
        answer = sum(1 << i for i,word in lanes if word < cutoff)
        require(row["scalar"] == row["packed"] == f"{answer:016x}",
                "packed and scalar occurrence masks differ from the independent oracle")
        scalar = len(lanes) if cutoff else 0
        packed = max(determining_prefix(word,cutoff) for _,word in lanes)
        require(nonnegative(row["scalar_entropy_words"]) and nonnegative(row["packed_entropy_planes"])
                and (row["scalar_entropy_words"],row["packed_entropy_planes"]) == (scalar,packed),
                "finite-occurrence entropy count changed")
        totals.update(active_lanes=len(lanes), scalar_entropy_words=scalar, packed_entropy_planes=packed)
        groups.append(dict(cutoff=row["cutoff"], active_lanes=len(lanes),
                           scalar_entropy_words=scalar, packed_entropy_planes=packed))
    return dict(groups=len(groups), by_tape=groups, **totals)


def check_capture(receipt):
    require(receipt["schema"] == SCHEMA, "invalid ablation capture schema")
    run = receipt["run"]
    sha_source = source_digest(receipt["source_inputs"])
    require(run["schema"] == "rmagna.paper-ablation-run.v1" and run["status"] == "complete"
            and run["rust_source_sha256"] == sha_source and sha(run["executable_sha256"])
            and run["physical_sha256"] == hashlib.sha256(PHYSICAL).hexdigest()
            and run["target_sha256"] == hashlib.sha256(TARGET).hexdigest()
            and run["lane_seed"] == LANE_SEED, "ablation source, fixture, or seed identity differs")
    return dict(
        status="PASS", host=receipt["host"], rustc=receipt["rustc"], build=run["build"],
        rust_source_sha256=sha_source, executable_sha256=run["executable_sha256"],
        lock_sha256=receipt["source_inputs"]["Cargo.lock"],
        circuit=dict(scope="two equally probable reported histories of a small controlled phase circuit",
                     modes=verify_histories(run["histories"])),
        finite_occurrence=dict(scope="matched finite cutoffs and tapes with bit planes prepared outside the count",
                               **verify_occurrence(run["finite_occurrence"])),
        measurement_scope="bounded operation counts for these fixtures",
    )


def capture(executable, out):
    proc = subprocess.run([executable.resolve(strict=True)], cwd=ROOT, timeout=60, capture_output=True, text=True, check=True)
    run = json.loads(proc.stdout)
    require(run["rust_source_sha256"] == source_identity(ROOT)
            and run["executable_sha256"] == reference.digest(executable), "ablation binary or compiled source changed")
    receipt = dict(schema=SCHEMA, run=run, source_inputs=source_inputs(ROOT),
                   host=dict(platform=platform.platform(), processor=platform.processor()),
                   rustc=subprocess.run(["rustc","--version"],capture_output=True,text=True,check=True).stdout.strip())
    summary = check_capture(receipt)
    with out.open("x") as handle:
        json.dump(receipt,handle,sort_keys=True,indent=2)
        handle.write("\n")
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--runner", type=Path, help="capture with this release example")
    args = parser.parse_args()
    try:
        result = capture(args.runner,args.results) if args.runner else check_capture(read_json(args.results))
        print(json.dumps(result,sort_keys=True))
        return 0
    except (ValueError, KeyError, OSError, TypeError, subprocess.SubprocessError) as error:
        print(f"paper ablation: FAIL: {error}",file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
