"""Project verified observations into data tables and measured claim limits."""

from collections import Counter
import csv
from fractions import Fraction
import io
import math
from statistics import median

from remaining_contracts import require


REPRESENTATION_UNITS = {
    "magna": "reference_network_terms",
    "symft": "peak_amplitudes",
    "clifft": "peak_amplitudes",
    "ppvm": "peak_coefficients",
    "tsim": "deposited_tsim_resource",
}


def point(figure, family, p, metric, origin, estimate, standard_error=None, *, pull=None, resolved=True):
    return dict(figure=figure, family=family, p=p, metric=metric, origin=origin, estimate=estimate,
                standard_error=standard_error if resolved else None, pull=pull, precision_resolved=resolved)


def precision_points(bh, cultivation, decoder, deposit):
    for row, original in zip(bh["precision"], deposit["bh_precision"], strict=True):
        case = row["case"]
        family = f"bh{case['k']}-{case['variant']}"
        for key in ("acceptance", "conditional_error"):
            yield point("figure3", family, case["p"], key, "rust", row["estimates"][key],
                        row["standard_errors"][key], pull=row["pulls"][key])
            yield point("figure3", family, case["p"], key, "analytic", float(Fraction(row["exact"][key])))
            trial, = original["trials"]
            yield point("figure3", family, case["p"], key, "deposit", trial["estimates"][key],
                        trial["standard_errors"][key])

    original = cultivation["historical_curve"]
    for family, rows in cultivation["observed"]["precision"].items():
        historical = {Fraction(str(row["p"])): row for row in original[family]["rows"]}
        comparisons = cultivation["observed"]["historical_comparison"]["precision"][family]
        for row, comparison in zip(rows, comparisons, strict=True):
            archived = historical[Fraction(row["p"])]
            for key, error in [("acceptance", math.sqrt(row["acceptance_variance"])),
                               ("conditional_error", row["conditional_error_se"])]:
                yield point("figure4", family, row["p"], key, "rust", row[key], error,
                            pull=comparison[key+"_pull"], resolved=key == "acceptance" or row["precision_resolved"])
                yield point("figure4", family, row["p"], key, "deposit", archived[key], archived[key+"_se"])

    previous = cultivation["observed"]["historical_comparison"]["published_reference"]
    for family, key in (("color3", "google_d3"), ("color5", "google_d5_exact")):
        for row in previous[key]["rows"]:
            for metric in ("acceptance", "conditional_error"):
                yield point("figure4", family, str(Fraction(str(row["p"]))), metric, "literature",
                            row[metric], row.get(metric+"_se"))

    logical = decoder["logical-distillation"]
    original = logical["historical"]["encoded-input"]["points"]
    for row, old, comparison in zip(logical["measured"]["rows"], original, logical["comparison"]["points"], strict=True):
        for key in ("acceptance", "conditional_error"):
            error_key = key.replace("_error", "")+"_standard_error"
            yield point("figure5", "logical-distillation", row["p"], key, "rust", row[key+"_float"],
                        comparison[error_key], pull=comparison[key+"_pull"],
                        resolved=key == "acceptance" or logical["measured"]["precision_resolved"])
            yield point("figure5", "logical-distillation", row["p"], key, "deposit", old[key], old[key+"_se"])
            yield point("figure5", "logical-distillation", row["p"], key, "analytic", float(Fraction(old["exact"][key])))
        yield point("figure5", "logical-distillation", row["p"], "q_eff", "exact_enumeration", float(Fraction(row["q_eff"])))

    for family, rows in decoder["code-switching"]["curves"].items():
        for row in rows:
            observed, old = row["observed"], row["historical"]
            for key in ("acceptance", "conditional_error"):
                yield point("code-switching", family, row["p"], key, "rust", observed["estimates"][key],
                            math.sqrt(observed["estimates"]["variance_"+key]), pull=row["comparison"][key+"_pull"],
                            resolved=key == "acceptance" or observed["precision_resolved"])
                yield point("code-switching", family, row["p"], key, "deposit", old[key],
                            math.sqrt(old["variance_"+key]), resolved=key == "acceptance" or old["positive"] > 0)

    for family, rows in decoder["tqd"]["curves"].items():
        for row in rows:
            for key, old in row["historical"]["estimates"].items():
                measured = row["measured"][key]
                yield point("tqd", family, row["p"], key, "rust", measured["mean"], measured["standard_error"],
                            pull=row["pulls"][key], resolved=not measured["zero_observed"])
                yield point("tqd", family, row["p"], key, "deposit", old["mean"], old["se"],
                            resolved=not old.get("zero_observed", False))


def paired_timing(trials):
    """Subtract each trial's own decoder time before aggregating trials."""
    inclusive, adjusted = [], []
    for wall, decoder, attempts in trials:
        require(type(attempts) is int and attempts > 0 and 0 <= decoder < wall < math.inf,
                "invalid per-attempt timing observation")
        inclusive.append(wall/attempts)
        adjusted.append((wall-decoder)/attempts)
    require(bool(inclusive), "missing timing observations")
    return median(inclusive), median(adjusted)


def timing_row(table, family, backend, protocol, repeats, seconds, subtracted=None, *, status="complete",
               draws=None, preparation=None, preparation_range=None, sampling_range=None):
    return dict(table=table, family=family, backend=backend, timing_protocol=protocol, repeats=repeats, status=status,
                draws_per_repeat=draws, preparation_seconds=preparation, preparation_seconds_range=preparation_range,
                seconds_per_attempt=seconds, seconds_per_attempt_range=sampling_range,
                decoder_subtracted_seconds_per_attempt=subtracted)


def decoder_timings(decoder):
    for family_type in ("code-switching", "tqd"):
        for family, block in decoder[family_type]["timing"].items():
            trials = block["trials"]
            clocks = [(row["timings"], row["attempts"]) if family_type == "code-switching"
                      else (row["timing"], row["draws"]) for row in trials]
            rust = paired_timing((row["sampling_seconds"], row["decoder_seconds"], n) for row, n in clocks)
            require(rust == (block["median_sampling_seconds_per_attempt"], block["median_decoder_subtracted_seconds_per_attempt"]),
                    f"{family} decoder time must be subtracted within each trial")
            yield timing_row("table1", family, "rust", "paper-design-rust-sampling", len(trials), *rust,
                             draws=clocks[0][1], preparation=median(row["preparation_seconds"] for row, _ in clocks))
            historical = block["historical"]
            deposit = historical["trials"]
            old = paired_timing((row["wall_seconds"], row["decoder_seconds"], row["attempts"]) for row in deposit)
            require(old == (historical["median_seconds_per_attempt"], historical["median_decoder_subtracted_seconds_per_attempt"]),
                    f"{family} deposited decoder timing aggregation changed")
            yield timing_row("table1", family, "magna", "deposited-outer-trial", len(deposit), *old,
                             draws=deposit[0]["attempts"],
                             preparation=median(row["internal_timings"]["preparation_seconds"] for row in deposit))

    logical = decoder["logical-distillation"]
    serial = logical["serial_timing"]
    measured = serial["timing"]
    actual = paired_timing([(measured["sampling_seconds"], measured["decoder_seconds"], serial["attempted"])])
    require(actual == (serial["sampling_seconds_per_attempt"], serial["decoder_subtracted_seconds_per_attempt"]),
            "logical serial decoder timing changed")
    yield timing_row("table1", "logical-distillation", "rust", "paper-design-rust-sampling", 1, *actual,
                     draws=serial["attempted"], preparation=measured["preparation_seconds"])
    old = logical["historical"]["serial-matched"]
    archived = paired_timing([(old["sampling_seconds"], old["decoder_seconds"], old["draws"])])
    require(archived == (old["sampling_seconds_per_attempt"], old["decoder_subtracted_seconds_per_attempt"]),
            "deposited serial decoder timing changed")
    yield timing_row("table1", "logical-distillation", "magna", "deposited-serial", 1,
                     *archived, draws=old["draws"])


def timings(bh, cultivation, decoder, deposit):
    for row in bh["timing"]:
        yield timing_row("table1", f"bh{row['k']}-{row['variant']}", "rust", "paper-design-rust-sampling",
                         row["repetitions"], row["sampling_seconds_median"], draws=2048,
                         preparation=row["preparation_seconds_median"], preparation_range=row["preparation_seconds_range"],
                         sampling_range=row["sampling_seconds_range"])
    for row in deposit["bh_timing"]:
        yield timing_row("table1", row["family"], "magna", "deposited-inner-sampling", row["repetitions"],
                         row["sampling_seconds_median"], draws=2048, preparation=row["preparation_seconds_median"],
                         preparation_range=row["preparation_seconds_range"], sampling_range=row["sampling_seconds_range"])
    observed = cultivation["observed"]
    for name, data in observed["timing"].items():
        yield timing_row("table2", name, "rust", "paper-design-rust-sampling",
                         len(data["batches"]), data["median_seconds_per_draw"], draws=1024,
                         preparation=median(row["preparation_seconds"] for row in data["batches"]))
    for family, peers in cultivation["peers"]["summary"].items():
        for backend, data in peers.items():
            yield timing_row("table2", family, backend, "same-host-matched-peers", 3, data["median_seconds_per_draw"],
                             draws=data["shots"]//3)
    for row in observed["historical_comparison"]["timings"]:
        raw = row["raw"]
        seconds = raw["sampling_ms_per_trial"]
        yield timing_row("table2", row["family"], row["backend"], "deposited-peer-timing",
                         raw["timed_batches"], None if seconds is None else seconds/1000,
                         status=raw["status"], draws=raw["shots_per_batch"])
    yield from decoder_timings(decoder)


def cultivation_efficiency(cultivation):
    original = cultivation["historical_curve"]
    for family, measured in cultivation["observed"]["precision"].items():
        historical = {Fraction(str(row["p"])): row for row in original[family]["rows"]}
        for row in measured:
            old = historical[Fraction(row["p"])]
            yield dict(family=family, p=row["p"],
                       rust=dict(acceptance_draws=row["acceptance_draws"], failure_draws=row["failure_draws"],
                                 positive_failure_draws=row["positive_failure_draws"],
                                 failure_weight_ess=row["failure_ess"], **row["time_to_observed_precision"]),
                       deposit=dict(acceptance_draws=old["acceptance_shots"], failure_draws=old["failure_shots"],
                                    positive_failure_draws=old["failure_positive"],
                                    acceptance_wall_seconds=old["acceptance_seconds"],
                                    failure_wall_seconds=old["failure_seconds"],
                                    relative_conditional_error_se=(old["conditional_error_se"]/old["conditional_error"]
                                                                   if old["conditional_error"] else None)))


def cultivation_representation(cultivation):
    for row in cultivation["observed"]["historical_comparison"]["timings"]:
        raw = row["raw"]
        require(raw["resource"] is None or (type(raw["resource"]) is int and raw["resource"] > 0),
                "deposited cultivation representation count differs")
        yield dict(family=row["family"], backend=row["backend"], origin="deposit", status=raw["status"],
                   quantity=REPRESENTATION_UNITS[row["backend"]], count=raw["resource"])

    for family, batches in cultivation["rust_representation"].items():
        require(len(batches) == 3 and len({batch["case"] for batch in batches}) == len(batches)
                and all(type(batch[key]) is int and batch[key] > 0 for batch in batches for key in ("acceptance", "target")),
                "missing or invalid Rust representation observations")
        maximum = max(batch[key] for batch in batches for key in ("acceptance", "target"))
        yield dict(family=family, backend="rust", origin="rust", status="complete",
                   quantity="reference_network_terms", count=maximum)


def report(plan, checked, artifact_count, deposit, ablations):
    bh = checked["bh"]
    cultivation = checked["cultivation"]
    decoder = checked["decoder-workloads"]
    observed = decoder["observed"]
    points = list(precision_points(bh, cultivation, observed, deposit))
    timing = list(timings(bh, cultivation, observed, deposit))
    # Failed peer attempts remain independent observations even when a corrected
    # attempt at the same workload later completes.
    failures = cultivation["failed_peer_attempts"]
    completed = (bh["timing_batches"]+bh["precision_points"]+cultivation["summary"]["executed_batches"]
                 +cultivation["peers"]["executed_batches"]+decoder["summary"]["completed_batches"])
    cells = dict(selected=completed+len(failures), completed=completed, failed=len(failures))
    unresolved = [row for row in points if row["origin"] == "rust" and not row["precision_resolved"]]
    coverage = decoder["coverage"]
    return dict(
        schema="rmagna.paper-evidence-regenerated.v1", status="PASS", paper=plan["paper"],
        artifacts=artifact_count, cells=cells, checked_captures=plan["captures"],
        capture_builds=dict(bh=bh["build"], cultivation=cultivation["build"], decoder=decoder["builds"],
                            matched_peers=dict(host=cultivation["peer_host"], builds=cultivation["peers"]["builds"],
                                               rust_binary_sha256=cultivation["peer_rust_binary_sha256"])),
        precision_draws=dict(bh=bh["independent_precision_draws"],
                             cultivation=cultivation["summary"]["independent_precision_draws"],
                             **{name: row["precision_draws"] for name, row in decoder["summary"]["campaigns"].items()}),
        points=points, timings=timing, failed_peer_attempts=failures, matched_ablations=ablations,
        historical_overview=deposit["overview"], historical_table2=cultivation["observed"]["historical_comparison"]["timings"],
        cultivation_representation=list(cultivation_representation(cultivation)),
        historical_code_switching_resources=deposit["code_switching_resources"],
        historical_tqd_terminal_table=deposit["tqd_terminal_table"],
        cultivation_time_to_observed_precision=list(cultivation_efficiency(cultivation)),
        readiness=dict(
            measured_scope="archived paper workloads on the pinned public inputs",
            coverage_counts=dict(Counter(coverage.values())),
            parity_complete=all(value == "verified" for value in coverage.values()),
            remaining_methods={name: value for name, value in coverage.items() if value != "verified"},
            unresolved_precision=unresolved,
            ablation_scope="one changed factor at a time on the declared exact fixtures; work counts only",
            deposited_timing_outcomes=[row for row in timing if row["status"] == "failed"],
            related_issues=[794, 601],
            publication_protocol="paper and same-host matched repeats kept in their respective protocols",
            later_paper_version="2610.22315v2 remains outside this analysis"),
        decoder_joint_estimates=dict(
            logical_covariance=observed["logical-distillation"]["measured"]["covariance"],
            logical_endpoint=observed["logical-distillation"]["comparison"]["endpoint"],
            code_switching_curves=observed["code-switching"]["curves"],
            tqd_deposited_reference_networks=observed["tqd"]["declared_reference_networks"],
            tqd_rust_terminal_resources=observed["tqd"]["rust_terminal_resources"],
            tqd_paired_curves=observed["tqd"]["curves"]))


def csv_table(rows):
    output = io.StringIO()
    writer = csv.DictWriter(output, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return output.getvalue().encode()
