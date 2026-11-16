"""Validate actual retained-owner work snapshots, separately from cache sizes."""
import re

SCHEMA = "rmagna.query-work.v1"
SCOPE = "retained_source_target_prepared_scalar_and_labelled_factories_excluding_direct_unprepared_expansion_bare_phase_and_transient_cold_prefix_owners_not_rss_or_native_private_parent_counts"
COUNTERS = (
    "affine_closures_prepared", "affine_geometry_preparations", "affine_geometry_reuses",
    "support_projections_prepared", "support_boundary_maps_prepared", "support_contractions_prepared",
    "support_phase_maps_prepared", "support_phase_resources_prepared", "support_parents_prepared",
    "support_evaluations", "support_changed_boundary_fallbacks", "phase_maps_prepared",
    "phase_resources_prepared", "closure_evaluations", "cold_evaluations", "fixed_evaluations",
)
STAGES = ("after_compile", "after_first_draw", "after_warm_replays", "after_pilot")


def require(ok, message):
    if not ok:
        raise ValueError("query work: " + message)


def count(value):
    return type(value) is int and 0 <= value <= 2**64-1


def validate(payload, *, required=False, expected_source_sha256=None):
    work = payload.get("query_work")
    if work is None and not required:
        return False  # Historical captures have no factory observations.
    require(isinstance(work, dict) and set(work) == {"schema", "scope", *STAGES}, "missing work stages")
    require(work["schema"] == SCHEMA and work["scope"] == SCOPE, "unsupported schema or scope")
    identity = payload.get("rust_source_sha256")
    require(isinstance(identity, str) and re.fullmatch(r"[0-9a-f]{64}", identity), "missing build source identity")
    if expected_source_sha256 is not None:
        require(identity == expected_source_sha256, "stale Rust diagnostic executable")
    route = payload.get("execution_route")
    require(route in ("linear_affine", "causal_fixed_template", "causal_cold_prefix"), "unknown compiled-owner route")
    previous = None
    for stage in STAGES:
        snapshot = work[stage]
        if stage == "after_pilot" and payload.get("pilot") is None:
            require(snapshot is None, "work reported for an absent pilot")
            continue
        require(isinstance(snapshot, dict) and set(snapshot) == {"source", "target"}, "invalid owner snapshot")
        require((snapshot["source"] is None) == (route == "causal_cold_prefix"), "source-owner scope changed")
        require((snapshot["target"] is None) == (payload.get("target_normalization") is None), "target-owner scope changed")
        for owner in ("source", "target"):
            current = snapshot[owner]
            if current is None:
                continue
            require(isinstance(current, dict) and set(current) == {*COUNTERS, "live_affine_geometries", "saturated"}, "incomplete factory counters")
            require(current["saturated"] is False, "saturated counters cannot certify actual work")
            require(all(count(current[k]) for k in COUNTERS), "invalid factory/evaluation count")
            live = current["live_affine_geometries"]
            require(live is None or count(live), "invalid live geometry observation")
            if previous is not None:
                prior = previous[owner]
                require(prior is not None and all(current[k] >= prior[k] for k in COUNTERS), "retained-owner counters decreased")
        previous = snapshot
    # No warm-zero or speedup threshold is implied here. Cache cardinality,
    # resource rank and live retention may differ from monotonic factory work.
    return True
