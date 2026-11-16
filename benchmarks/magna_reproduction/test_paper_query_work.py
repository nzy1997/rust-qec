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


class ActualQueryWorkTests(unittest.TestCase):
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
