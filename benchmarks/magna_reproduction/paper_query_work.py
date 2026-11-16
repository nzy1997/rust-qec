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
INVENTORY_SCHEMA = "rmagna.reference-inventory.v1"
INVENTORY_SCOPE = "retained_source_target_programs_and_geometry_weak_slots_not_rss_or_query_cache_resources_excludes_cold_transient_owners_external_plans_and_draw_local_numeric_views"
STORAGE_FIELDS = (
    "physical_width", "raw_boundary_width", "query_boundary_width", "unique_reference_boundaries",
    "raw_phase_variables", "original_positions", "expanded_clifford_gates",
    "geometry_weak_slots", "geometry_slot_capacity", "live_geometries",
)
VARIABLE_STORAGE = {"geometry_weak_slots", "geometry_slot_capacity", "live_geometries"}


def require(ok, message):
    if not ok:
        raise ValueError("query work: " + message)


def count(value):
    return type(value) is int and 0 <= value <= 2**64-1


def validate(payload, *, required=False, expected_source_sha256=None, require_inventory=False):
    work = payload.get("query_work")
    if work is None and not required and not require_inventory:
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
    validate_inventory(payload, required=require_inventory)
    return True


def validate_inventory(payload, *, required=False):
    inventory = payload.get("reference_inventory")
    if inventory is None and not required:
        return False
    require(isinstance(inventory, dict) and set(inventory) == {"schema", "scope", *STAGES},
            "missing reference inventory stages")
    require(inventory["schema"] == INVENTORY_SCHEMA and inventory["scope"] == INVENTORY_SCOPE,
            "unsupported reference inventory scope")
    previous = None
    for stage in STAGES:
        current = inventory[stage]
        if stage == "after_pilot" and payload.get("pilot") is None:
            require(current is None, "reference inventory for absent pilot")
            continue
        require(isinstance(current, dict) and set(current) == {"source_assignments", "storage"},
                "invalid reference inventory snapshot")
        source_absent = payload["execution_route"] == "causal_cold_prefix"
        assignments = current["source_assignments"]
        require((assignments is None) == source_absent and (assignments is None or count(assignments)),
                "invalid retained source assignment count")
        storage = current["storage"]
        require(isinstance(storage, dict) and set(storage) == {"source", "target"}, "invalid storage owners")
        require((storage["source"] is None) == source_absent
                and (storage["target"] is None) == (payload.get("target_normalization") is None),
                "reference storage owner scope changed")
        if previous is not None and assignments is not None:
            require(assignments >= previous["source_assignments"], "source assignments decreased")
        for owner in ("source", "target"):
            value = storage[owner]
            if value is None:
                continue
            require(isinstance(value, dict) and set(value) == set(STORAGE_FIELDS)
                    and all(count(value[key]) for key in STORAGE_FIELDS), "invalid storage counts")
            require(value["unique_reference_boundaries"] in (1, 2), "invalid Gaussian owner count")
            require(value["physical_width"] <= value["query_boundary_width"] <= value["raw_boundary_width"]
                    and value["raw_boundary_width"] - value["physical_width"] == value["raw_phase_variables"],
                    "inconsistent reference dimensions")
            require(value["live_geometries"] <= value["geometry_weak_slots"] <= value["geometry_slot_capacity"],
                    "inconsistent weak geometry retention")
            observed = payload["query_work"][stage][owner]["live_affine_geometries"]
            require(observed is None or observed == value["live_geometries"], "work/inventory live counts differ")
            if previous is not None:
                prior = previous["storage"][owner]
                require(all(value[key] == prior[key] for key in STORAGE_FIELDS if key not in VARIABLE_STORAGE),
                        "retained program structure changed")
        previous = current
    # Inventory admits real growth/eviction and reports capacity. Fixed warm
    # retention and family quota claims need the actual task's separate proof.
    return True
