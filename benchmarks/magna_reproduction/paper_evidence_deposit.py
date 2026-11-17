"""Verify the deposited BH reference curves, original trials and Table I overview."""

from fractions import Fraction
import hashlib
import math
from statistics import median
from zipfile import ZipFile

from remaining_contracts import read_json, reference, require


OVERVIEW = "data/macstudio-overview.json"
BH_TIMING = "data/bh-k100.json"
BH_PRECISION = "data/bh-curves.json"
BH_SIZES = (2, 4, 6, 10, 20, 40, 60, 80, 100)


def runs(k):
    return 10 if k in (40, 60) else 3


def members(pinned):
    files = pinned["paper"]["files"]
    selected = [OVERVIEW, BH_TIMING, BH_PRECISION, "data/dimension-comparison.json", "data/tqd-terminal-timing.json"]
    for k in BH_SIZES:
        for variant in ("T", "S"):
            prefix = f"raw/overview/bh{k}-{variant}"
            selected.extend([prefix+"/result.json", *(f"{prefix}/draw-batch-{i}.json" for i in range(runs(k)))])
    return {name: files[name] for name in selected}


def extract(root, archive, pinned, archive_sha256):
    require(reference.digest(archive) == archive_sha256, "paper deposit bytes changed")
    admissible = members(pinned)
    # Verify before exposing new files to the bundle. These small members are
    # the additional reference material that the campaign captures do not keep.
    with ZipFile(archive) as source:
        for name, expected in admissible.items():
            entry = source.read("magna-data-deposit/"+name)
            require(hashlib.sha256(entry).hexdigest() == expected, f"paper deposit member changed: {name}")
            path = reference.relative_file(root, name)
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.exists():
                require(path.read_bytes() == entry, f"existing paper deposit member changed: {name}")
            else:
                with path.open("xb") as handle:
                    handle.write(entry)


def check(root, pinned, bh):
    reference.verify_files(root, members(pinned))

    def read(name):
        return read_json(reference.relative_file(root, name))

    overview = read(OVERVIEW)
    require(overview["kernel_commit"] == pinned["paper"]["kernel_commit"], "historical overview kernel differs")
    observed = {row["k"]: row for row in read(BH_TIMING)}
    require(tuple(sorted(observed)) == BH_SIZES, "published BH timing scale differs")
    timing = []
    for k in BH_SIZES:
        for variant in ("T", "S"):
            family = f"bh{k}-{variant}"
            prefix = "raw/overview/"+family
            capture = read(prefix+"/result.json")
            published = observed[k]["cases"][variant]
            shots, repeats = capture["config"]["shots"], runs(k)
            published_trials = [{key: value for key, value in trial.items() if key != "resources"}
                                for trial in capture["trials"]]
            require(shots == 2048 and capture["config"]["repeats"] == repeats and capture["status"] == "complete"
                    and len(capture["trials"]) == repeats and published["trials"] == published_trials,
                    f"deposited BH timing schedule changed for {family}")
            prep, sampling = [], []
            for index, trial in enumerate(capture["trials"]):
                raw = read(f"{prefix}/draw-batch-{index}.json")
                require(raw["status"] == "complete" and raw["requested"] == raw["completed"] == trial["shots"] == shots
                        and raw["failed"] == raw["not_attempted"] == trial["decoder_calls"] == 0
                        and raw["seed"] == 2026100800+index
                        and raw["timings"] == trial["timings"] and raw["estimates"] == trial["estimates"]
                        and raw["standard_errors"] == trial["standard_errors"] and raw["resources"] == trial["resources"],
                        f"deposited BH timing trial changed for {family}")
                compiled = raw["resources"]["compilation"]
                queries = [query for group in compiled["query_families"].values() for query in group]
                require(compiled["status"] == "complete" and {q["purpose"] for q in queries} == {"acceptance", "fidelity"}
                        and all(q["reference_terms"] == 1 for q in queries), f"deposited BH query terms changed for {family}")
                prep.append(raw["timings"]["preparation_seconds"])
                sampling.append(raw["timings"]["sampling_seconds"]/shots)
            overview_row = overview["rows"]["overview-"+family]
            preparation = median(prep)
            seconds = median(sampling)
            require(math.isclose(preparation*1000, overview_row["prepare_ms"], rel_tol=1e-12)
                    and math.isclose(seconds*1000, overview_row["sample_ms"], rel_tol=1e-12)
                    and overview_row["ranks"] == dict(acceptance=1,fidelity=1)
                    and overview_row["shots"] == shots and overview_row["repeats"] == repeats,
                    f"deposited BH overview does not regenerate for {family}")
            timing.append(dict(family=family, repetitions=repeats, preparation_seconds_median=preparation,
                               preparation_seconds_range=[min(prep),max(prep)], sampling_seconds_median=seconds,
                               sampling_seconds_range=[min(sampling),max(sampling)]))

    precision = read(BH_PRECISION)
    require(len(precision) == len(bh["precision"]), "deposited BH curve point count differs")
    for old, current in zip(precision, bh["precision"], strict=True):
        case = current["case"]
        require(old["k"] == case["k"] and Fraction(str(old["p"])) == Fraction(case["p"])
                and old["shots"] == case["shots"] and old["seed"] == case["seed"]
                and len(old["trials"]) == 1, "deposited BH precision addresses differ")
        for key in ("acceptance", "conditional_error"):
            require(math.isclose(old["reference"][key], float(Fraction(current["exact"][key])), rel_tol=1e-12),
                    f"deposited BH analytical {key} differs")
    return dict(bh_precision=precision, bh_timing=timing, overview=overview,
                code_switching_resources=read("data/dimension-comparison.json"),
                tqd_terminal_table=read("data/tqd-terminal-timing.json"))
