"""Offline replay of fixed prefixes and all finite role/cache controls.

Native preparation identity and receipt closure are separate gates. This draft
validator cannot admit native performance or source-to-binary provenance.
"""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))

from pathlib import Path
import math, statistics, sys
from probe_contract import read, require, check
from schedule import INPUTS, CELLS, HORIZONS, ROLES, schedule

def verify(directory):
    directory = Path(directory)
    manifest = read(directory / "streams.json")
    expected = schedule("validate") + schedule("bench")
    require(set(manifest) == {"schema", "events"})
    require(manifest["schema"] == "rstim.fixed-call-streams.draft.v1")
    require(manifest["events"] == expected)
    require({p.name for p in directory.glob("*.stdout")} == {e["id"] + ".stdout" for e in expected})
    finite, timing = {}, {}
    for event in expected:
        name, shots, policy, factor = event["cell"]
        data = read(directory / (event["id"] + ".stdout"))
        check(data, shots, policy, factor, event["action"], INPUTS[name])
        key = (name, shots, policy, factor, event["role"])
        if event["action"] == "validate":
            finite[key] = data
        else:
            timing[(event["round"], *key)] = data
    # Each action validates its own counts and RNG continuation in Rust.
    # These retained measurements are UNCACHED REFERENCE ONLY.
    fields = ("call_index", "seed", "attempted", "accepted", "discarded", "logical_errors", "rng_tail", "measurements")
    for name, shots, policy, factor in CELLS:
        reference = finite[(name, shots, policy, "off", "baseline")]["calls"]
        for role in ROLES:
            rows = finite[(name, shots, policy, factor, role)]["calls"]
            require([{k:r[k] for k in fields} for r in rows] == [{k:r[k] for k in fields} for r in reference])
    # Timed counts are retained and must agree across sources and cache factors.
    count_fields = ("call_index", "seed", "attempted", "accepted", "discarded", "logical_errors")
    for round_index in range(12):
        for name, shots, policy, factor in CELLS:
            reference = timing[(round_index, name, shots, policy, "off", "baseline")]["calls"]
            for role in ROLES:
                rows = timing[(round_index, name, shots, policy, factor, role)]["calls"]
                require([{k:r[k] for k in count_fields} for r in rows] == [{k:r[k] for k in count_fields} for r in reference])
    result = []
    for cell in CELLS:
        for prefix_index, horizon in enumerate(HORIZONS):
            record = {"cell": list(cell), "calls": horizon}
            for metric in ("sampling_ns", "compile_prepare_sampling_ns"):
                ratios, nulls = [], []
                for round_index in range(12):
                    values = {role: timing[(round_index, *cell, role)]["prefixes"][prefix_index][metric] for role in ROLES}
                    require(all(value > 0 for value in values.values()))
                    ratios.append(values["baseline"] / values["candidate"])
                    nulls.append(values["baseline"] / values["control"])
                record[metric] = {"paired_ratios": ratios, "paired_geometric_mean": math.exp(statistics.mean(map(math.log, ratios))),
                    "identical_binary_paired_ratios": nulls, "identical_binary_geometric_mean": math.exp(statistics.mean(map(math.log, nulls)))}
            result.append(record)
    return {"scope": "Offline stream contract only; native identity/receipts, positive native guard and independent replication required separately",
        "validation_workers": 72, "timing_workers": 864, "comparisons": result,
        "statistics": "Predeclared paired log-ratio means, 24 cells x 9 horizons x 2 metrics and AA controls; descriptive only, no significance or universal winner claim",
        "teardown": "Only 256-call teardown measured; never extrapolated to shorter prefixes"}

if __name__ == "__main__":
    import json
    print(json.dumps(verify(sys.argv[1]), indent=2))
