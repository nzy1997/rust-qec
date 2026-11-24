import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compatibility_boundaries as compatibility
import remaining_contracts


# Selected pinned pure functions. The real CLI extracts these functions from
# hash-verified upstream files; these small fixtures let CI test probe behavior
# without installing or importing the Native package.
HOOK_SOURCE = """
def lower(circuit, outputs, values, noise, *, feedback_plan=None):
    native_testing.load().lower_circuit_file(
        _circuit_handle(circuit), tuple(outputs), (),
        tuple((oid, tuple((g['kind'], g['qubits'][0]) for g in gates)) for oid, gates in noise.gates.items()),
        None)
"""
IR_SOURCE = """
def fingerprint(value):
    return sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), default=str).encode()).hexdigest()
"""


class CompatibilityBoundaryTests(unittest.TestCase):
    def setUp(self):
        read = remaining_contracts.read_json
        here = compatibility.HERE
        self.catalogue = read(here / "compatibility-boundaries.json")
        self.ledger = read(here / "coverage.json")
        self.remaining = read(here / "remaining-contracts.json")
        self.manifest = compatibility.reference.load_manifest()

    def check(self, catalogue=None, ledger=None, remaining=None):
        return compatibility.check_catalogue(
            self.catalogue if catalogue is None else catalogue,
            self.ledger if ledger is None else ledger, self.manifest,
            self.remaining if remaining is None else remaining)

    def test_six_decisions_do_not_qualify_original_methods_or_execute_rust(self):
        result = self.check()
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(result["decisions"], 6)
        self.assertEqual(result["original_denominator"], 352)
        self.assertEqual(result["original_statuses"], {"partial": 4, "unmapped": 2})
        self.assertEqual(result["unresolved_original_obligations"], 12)
        self.assertEqual(result["promoted_original_methods"], 0)
        self.assertEqual(result["rust_execution"], "not_run_source_bound_declarations_only")
        self.assertFalse(result["upstream_verified"])
        self.assertEqual(result["ledger_counts"], {"verified": 346, "partial": 4, "unmapped": 2})
        before = copy.deepcopy((self.catalogue, self.ledger, self.remaining))
        self.check()
        self.assertEqual(before, (self.catalogue, self.ledger, self.remaining))

    def test_reports_actual_counts_without_assuming_f1_or_f2_were_promoted(self):
        ledger = copy.deepcopy(self.ledger)
        row = next(r for r in ledger["cases"] if r["id"].endswith("test_installed_conditional_noise_fixed_reference_networks"))
        row["status"] = "partial"
        result = self.check(ledger=ledger)
        self.assertEqual(result["ledger_counts"], {"verified": 345, "partial": 5, "unmapped": 2})
        self.assertEqual(result["original_statuses"], {"partial": 4, "unmapped": 2})

    def test_each_record_obligation_and_helper_is_required(self):
        for i, row in enumerate(self.catalogue["methods"]):
            with self.subTest(record=row["id"]):
                changed = copy.deepcopy(self.catalogue)
                changed["methods"].pop(i)
                with self.assertRaisesRegex(ValueError, "compatibility record"):
                    self.check(changed)
            for j in range(len(row["original_obligations"])):
                with self.subTest(record=row["id"], obligation=j):
                    changed = copy.deepcopy(self.catalogue)
                    changed["methods"][i]["original_obligations"].pop(j)
                    with self.assertRaisesRegex(ValueError, "original obligation"):
                        self.check(changed)
            for helper in row["helper_sources"]:
                with self.subTest(record=row["id"], helper=helper):
                    changed = copy.deepcopy(self.catalogue)
                    del changed["methods"][i]["helper_sources"][helper]
                    with self.assertRaisesRegex(ValueError, "helper source closure"):
                        self.check(changed)

    def test_coordinated_context_deletion_cannot_hide_a_source_assertion_or_helper(self):
        for key in ["required_checks", "module_imports", "module_helpers"]:
            changed = copy.deepcopy(self.catalogue)
            remaining = copy.deepcopy(self.remaining)
            row = next(r for r in changed["methods"] if r["method"][key])
            row["method"][key].pop()
            next(r for r in remaining["methods"] if r["id"] == row["id"])["method"] = copy.deepcopy(row["method"])
            with self.subTest(field=key), self.assertRaisesRegex(ValueError, "complete method/helper context"):
                self.check(changed, remaining=remaining)

    def test_source_identity_adjacent_witness_and_owner_substitution_are_rejected(self):
        mutations = [
            (lambda d: d["methods"][0]["source"].update(source_sha256="0" * 64), "method/helper context"),
            (lambda d: d["methods"][0].update(helper_sources={}), "helper source closure"),
            (lambda d: d["methods"][1]["rust_witnesses"][0].update(test="original_explicit_literal_policy_compiles_once_without_structural_preparation"), "adjacent/missing"),
            (lambda d: d["methods"][-1]["installed_boundary_witnesses"][0].update(test="original_installed_scenarios_run_from_one_binary_without_checkout_imports"), "installed boundary witness"),
            (lambda d: d["methods"][0]["support_sources"][0].update(anchor="def generated("), "source owner substitution"),
            (lambda d: d.update(upstream_commit="0" * 40), "reference changed"),
        ]
        for mutate, message in mutations:
            with self.subTest(message=message):
                changed = copy.deepcopy(self.catalogue)
                mutate(changed)
                with self.assertRaisesRegex(ValueError, message):
                    self.check(changed)

    def test_no_promotion_even_with_jointly_modified_ledger_and_decision(self):
        for i, row in enumerate(self.catalogue["methods"]):
            changed = copy.deepcopy(self.catalogue)
            ledger = copy.deepcopy(self.ledger)
            remaining = copy.deepcopy(self.remaining)
            changed["methods"][i]["original_status"] = "verified"
            next(r for r in ledger["cases"] if r["id"] == row["id"])["status"] = "verified"
            next(r for r in remaining["methods"] if r["id"] == row["id"])["ledger_status"] = "verified"
            with self.subTest(record=row["id"]), self.assertRaisesRegex(ValueError, "unsupported original verified"):
                self.check(changed, ledger, remaining)
            changed = copy.deepcopy(self.catalogue)
            changed["methods"][i]["original_obligations"][0]["status"] = "verified"
            with self.assertRaisesRegex(ValueError, "original obligation or implementation claim"):
                self.check(changed)

    def test_policy_cannot_relabel_a_graph_cache_or_loader_as_implemented(self):
        mutations = [
            lambda d: d["policies"]["metadata_domain"].update(annotation_graph_api="implemented"),
            lambda d: d["policies"]["metadata_domain"].update(json_tree_annotation_api="implemented"),
            lambda d: d["policies"]["graph_model_if_selected"].update(status="implemented"),
            lambda d: d["policies"]["certificate_identity"].update(metadata_certificate_lru="implemented"),
            lambda d: d["policies"]["certificate_identity"].update(pinned_prepared_domain="cyclic_cached_certificates_supported"),
            lambda d: d["policies"]["supplied_noise_lowering"].update(pinned_hook="retains_arbitrary_extra"),
            lambda d: d["policies"]["installation"].update(cpython_extension_loader="implemented"),
            lambda d: d["policies"]["installation"].update(diagnostic_inside_missing_program=True),
            lambda d: d.update(parity_claim="all352_verified"),
        ]
        for mutate in mutations:
            changed = copy.deepcopy(self.catalogue)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "scope|policy/implementation"):
                self.check(changed)

    def test_graph_cache_order_and_independent_tools_are_explicit(self):
        graph = self.catalogue["policies"]["graph_model_if_selected"]
        self.assertEqual(graph["status"], "prospective_not_implemented")
        self.assertEqual(graph["rust_feasibility"], "cycles_and_aliases_are_possible_with_an_explicit_model")
        for node in ["G2", "G3", "N1", "S1", "I1"]:
            changed = copy.deepcopy(self.catalogue)
            changed["implementation_plan"][node]["depends_on"] = ["I1"]
            with self.subTest(node=node), self.assertRaisesRegex(ValueError, "dependency/order"):
                self.check(changed)

    def test_pinned_projection_and_fingerprint_probes_reject_overstated_metadata_domains(self):
        result = compatibility.probe_source_semantics(HOOK_SOURCE, IR_SOURCE)
        self.assertEqual(result["noise_projection"], "kind_first_qubit_only_nested_and_cyclic_extras_discarded")
        self.assertEqual(result["captured_payload_isolation"], "unchanged_after_caller_qubit_and_extra_mutation")
        self.assertEqual(result["fingerprint"], "acyclic_metadata_sensitive_alias_identity_insensitive_dict_list_cycles_rejected")
        self.assertEqual(result["native_anchor_lowering"], "source_bound_only_not_executed")
        self.assertEqual(result["cached_certificate_execution"], "not_run_no_cyclic_certificate_qualification")
        retaining_hook = HOOK_SOURCE.replace("g['kind'], g['qubits'][0]", "g['kind'], g['qubits'][0], g['extra']")
        with self.assertRaisesRegex(ValueError, "discard nested/cyclic extras"):
            compatibility.probe_source_semantics(retaining_hook, IR_SOURCE)
        accepting_cycles = IR_SOURCE.replace("def fingerprint(value):", "def fingerprint(value):\n    if isinstance(value, dict) and 'self' in value.get('metadata', {}): return '0' * 64")
        with self.assertRaisesRegex(ValueError, "unexpectedly accepts cyclic metadata"):
            compatibility.probe_source_semantics(HOOK_SOURCE, accepting_cycles)

    def test_original_noise_hook_and_acyclic_cache_obligations_do_not_invent_graph_requirements(self):
        noise = next(r for r in self.catalogue["methods"] if r["id"] == compatibility.NOISE)
        cache = next(r for r in self.catalogue["methods"] if r["id"].startswith(compatibility.OPTIMIZATION))
        self.assertEqual(noise["implementation_dependencies"], ["N1"])
        self.assertEqual(self.catalogue["implementation_plan"]["N1"]["depends_on"], [])
        self.assertEqual([o["id"] for o in noise["original_obligations"]], ["noise_hook_projection"])
        self.assertIn("discarded, not preserved", noise["original_obligations"][0]["requirement"])
        self.assertIn("cyclic prepared fingerprints are rejected", cache["original_obligations"][2]["requirement"])
        for name, false_requirement in [(noise["id"], "Lowering snapshots arbitrary nested gate extra metadata."),
                                        (cache["id"], "Cached branch certificates preserve aliases/cycles.")]:
            changed = copy.deepcopy(self.catalogue)
            row = next(r for r in changed["methods"] if r["id"] == name)
            row["original_obligations"][0]["requirement"] = false_requirement
            with self.assertRaisesRegex(ValueError, "original obligation"):
                self.check(changed)

    def test_denominator_unknown_ids_exclusions_duplicates_and_unknown_claim_fields_fail(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["cases"] = [r for r in ledger["cases"] if r["id"] != compatibility.LOADER]
        with self.assertRaisesRegex(ValueError, "denominator/inventory"):
            self.check(ledger=ledger)
        ledger = copy.deepcopy(self.ledger)
        ledger["cases"][0]["id"] = "unknown.Suite.test_fake"
        with self.assertRaisesRegex(ValueError, "denominator/inventory"):
            self.check(ledger=ledger)
        ledger = copy.deepcopy(self.ledger)
        ledger["cases"][0]["status"] = "architecture_specific"
        with self.assertRaisesRegex(ValueError, "architecture exclusions"):
            self.check(ledger=ledger)
        changed = copy.deepcopy(self.catalogue)
        changed["methods"][0] = copy.deepcopy(changed["methods"][1])
        with self.assertRaisesRegex(ValueError, "compatibility record"):
            self.check(changed)
        changed = copy.deepcopy(self.catalogue)
        changed["methods"][0]["graph_supported"] = True
        with self.assertRaisesRegex(ValueError, "invalid fields"):
            self.check(changed)

    def test_real_exact_path_and_missing_program_boundaries_do_not_claim_loader_parity(self):
        result = compatibility.probe_boundaries()
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(result["exact_paths"], "missing_rejected_unique_admitted_duplicate_basenames_distinct")
        self.assertEqual(result["missing_executable"], "caller_observed_FileNotFoundError_no_program_executed")
        self.assertEqual(result["cpython_extension_import"], "not_implemented_not_qualified")
        self.assertEqual(result["upstream_recursive_lookup"], "not_run")

    def test_standard_check_includes_decisions_and_cli_requires_the_pinned_source(self):
        report = remaining_contracts.check_contracts(self.remaining, self.ledger, self.manifest)
        self.assertEqual(report["compatibility_boundaries"]["decisions"], 6)
        self.assertEqual(report["compatibility_boundaries"]["promoted_original_methods"], 0)
        with contextlib.redirect_stderr(io.StringIO()) as error:
            status = remaining_contracts.main(["compatibility"])
        self.assertEqual(status, 2)
        self.assertIn("requires --upstream", error.getvalue())
