import copy
import unittest

import paper_query_work as work


def payload():
    def owner(evaluations, live):
        value = dict.fromkeys(work.COUNTERS, 0)
        value.update(closure_evaluations=evaluations, fixed_evaluations=evaluations,
                     affine_closures_prepared=1, affine_geometry_preparations=1,
                     phase_maps_prepared=1, phase_resources_prepared=1,
                     live_affine_geometries=live, saturated=False)
        return value
    value = dict(schema=work.SCHEMA, scope=work.SCOPE)
    for stage, count, live in zip(work.STAGES, (0, 1, 3, 11), (0, 1, 1, 0)):
        value[stage] = dict(source=owner(count, live), target=None)
    return dict(query_work=value, rust_source_sha256="a"*64, execution_route="linear_affine",
                target_normalization=None, pilot=dict(completed=8))


def inventory_payload():
    value = payload()
    inventory = dict(schema=work.INVENTORY_SCHEMA, scope=work.INVENTORY_SCOPE)
    for stage, assignments in zip(work.STAGES, (0, 1, 3, 11)):
        live = value["query_work"][stage]["source"]["live_affine_geometries"]
        source = dict(physical_width=1, raw_boundary_width=4, query_boundary_width=2,
                      unique_reference_boundaries=2, raw_phase_variables=3,
                      original_positions=7, expanded_clifford_gates=12,
                      geometry_weak_slots=1, geometry_slot_capacity=4, live_geometries=live)
        inventory[stage] = dict(source_assignments=assignments, storage=dict(source=source, target=None))
    value["reference_inventory"] = inventory
    return value


class ActualQueryWorkTests(unittest.TestCase):
    def test_retention_inventory_requires_capacity_and_agrees_with_actual_factory_scope(self):
        value = inventory_payload()
        self.assertTrue(work.validate(value, required=True, require_inventory=True))
        self.assertFalse(work.validate_inventory(payload()))
        with self.assertRaisesRegex(ValueError, "missing reference inventory"):
            work.validate(payload(), required=True, require_inventory=True)
        # Dead weak slots and spare capacity are observable retention, not a
        # false live-resource count; their growth is allowed by this schema.
        final = value["reference_inventory"]["after_pilot"]["storage"]["source"]
        final.update(geometry_weak_slots=2, geometry_slot_capacity=8)
        self.assertTrue(work.validate(value, required=True, require_inventory=True))

    def test_corrupt_assignment_capacity_dimensions_and_duplicate_prefix_inventories_fail(self):
        def source(p, stage="after_pilot"):
            return p["reference_inventory"][stage]["storage"]["source"]
        changes = (
            lambda p: p["reference_inventory"]["after_pilot"].update(source_assignments=2),
            lambda p: p["reference_inventory"]["after_compile"].update(source_assignments=True),
            lambda p: source(p).update(geometry_slot_capacity=0),
            lambda p: source(p).update(geometry_weak_slots=0, live_geometries=1),
            lambda p: source(p).update(live_geometries=1),
            lambda p: source(p).update(raw_boundary_width=5),
            lambda p: source(p).update(query_boundary_width=0),
            lambda p: source(p).update(unique_reference_boundaries=3),
            lambda p: source(p).update(original_positions=14),
            lambda p: source(p).update(expanded_clifford_gates=24),
            lambda p: source(p).pop("geometry_slot_capacity"),
            lambda p: p["reference_inventory"].update(scope="all_process_memory"),
            lambda p: p["reference_inventory"]["after_pilot"]["storage"].update(target={}),
            lambda p: p["reference_inventory"].update(after_pilot=None),
        )
        for index, change in enumerate(changes):
            value = inventory_payload()
            change(value)
            with self.subTest(corruption=index), self.assertRaises(ValueError):
                work.validate(value, required=True, require_inventory=True)

    def test_inventory_keeps_cold_transients_and_missing_pilots_out_of_retained_scope(self):
        value = inventory_payload()
        value.update(execution_route="causal_cold_prefix", pilot=None)
        for stage in work.STAGES[:-1]:
            value["query_work"][stage]["source"] = None
            value["reference_inventory"][stage]["source_assignments"] = None
            value["reference_inventory"][stage]["storage"]["source"] = None
        value["query_work"]["after_pilot"] = None
        value["reference_inventory"]["after_pilot"] = None
        self.assertTrue(work.validate(value, required=True, require_inventory=True))

    def test_actual_evaluations_are_monotonic_while_live_retention_can_decrease(self):
        value = payload()
        self.assertTrue(work.validate(value, required=True, expected_source_sha256="a"*64))
        # Cold queries may construct additional resources. Reporting that work
        # is valid; the validator does not turn it into a fabricated warm zero.
        value["query_work"]["after_pilot"]["source"]["phase_resources_prepared"] = 10
        self.assertTrue(work.validate(value, required=True))

    def test_historical_payload_is_explicitly_distinct_from_new_required_evidence(self):
        self.assertFalse(work.validate({}))
        with self.assertRaisesRegex(ValueError, "missing work stages"):
            work.validate({}, required=True)

    def test_decreased_missing_boolean_saturated_and_stale_counts_are_rejected(self):
        changes = (
            lambda p: p["query_work"]["after_warm_replays"]["source"].update(closure_evaluations=0),
            lambda p: p["query_work"]["after_compile"]["source"].pop("support_parents_prepared"),
            lambda p: p["query_work"]["after_compile"]["source"].update(phase_resources_prepared=True),
            lambda p: p["query_work"]["after_compile"]["source"].update(live_affine_geometries=True),
            lambda p: p["query_work"]["after_compile"]["source"].update(saturated=True),
            lambda p: p["query_work"].update(scope="all_native_parents"),
            lambda p: p.update(rust_source_sha256="b"*64),
            lambda p: p["query_work"].update(after_pilot=None),
            lambda p: p["query_work"]["after_compile"].update(source=None),
            lambda p: p.update(target_normalization={}),
        )
        for i, change in enumerate(changes):
            value = payload()
            change(value)
            with self.subTest(corruption=i), self.assertRaises(ValueError):
                work.validate(value, required=True, expected_source_sha256="a"*64)

    def test_cold_prefix_owners_and_absent_pilot_are_not_reported_as_fake_zeros(self):
        value = payload()
        value.update(execution_route="causal_cold_prefix", pilot=None)
        for stage in work.STAGES[:-1]:
            value["query_work"][stage]["source"] = None
        value["query_work"]["after_pilot"] = None
        self.assertTrue(work.validate(value, required=True))
        changed = copy.deepcopy(value)
        changed["query_work"]["after_compile"]["source"] = payload()["query_work"]["after_compile"]["source"]
        with self.assertRaisesRegex(ValueError, "source-owner scope"):
            work.validate(changed, required=True)
