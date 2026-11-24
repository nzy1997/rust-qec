"""F3 source-bound decisions, deliberately separate from original parity."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import tempfile

import coverage_ledger as ledger_tools

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
require = ledger_tools.require
reference = ledger_tools.reference
rust = ledger_tools.rust

# These are semantic requirements, not a free-form decision that can be changed
# into an implementation claim by editing the manifest and coverage together.
OBLIGATIONS = {
    "recursive_lookup": "Recursive basename lookup rejects zero/multiple matches and returns the sole match.",
    "public_graph_copy": "Public deferral copies the entire annotation graph, preserving internal aliases/cycles while isolating caller mutation.",
    "owned_graph_transfer": "Owned deferral transfers mutable annotation leaves under an explicit caller-relinquishment contract.",
    "noise_extra_copy": "Lowering snapshots arbitrary nested gate extra metadata as well as physical guarded-noise actions.",
    "constructor_graph_copy": "Public IR construction preserves cross-operation aliases/cycles inside an isolated graph snapshot.",
    "mutable_prepared_matches": "Prepared snapshots detect nested mutable operation/initial/control changes in the public source.",
    "nested_metadata": "Public operations/branches retain arbitrary nested metadata, with a declared alias/cycle domain.",
    "metadata_identity": "Metadata-only edits participate in preparation/certificate identity and invalidate old metadata-bearing hits.",
    "certificate_graph_copy": "Returned cached branch certificates preserve metadata and isolate caller mutation, including aliases/cycles.",
    "certificate_lru": "A bounded metadata-keyed branch-certificate LRU returns cache_hit and fresh metadata; a probability-value LRU is insufficient.",
    "extension_import": "Import of the CPython package without its compiled extension fails through the package loader.",
    "extension_build_hint": "The loader gives the original missing-extension/build-script diagnostic without misleading circular-import text.",
}
QUERY = "test_query_reuse.OwnershipAndDependencies."
OPTIMIZATION = "test_simulator_optimization.OptimizationTests."
SOURCE = "test_native_source_tools.NativeSourceToolsTests.test_source_resolution_rejects_absent_and_ambiguous_hooks"
LOADER = "test_user_interface.MissingExtensionTests.test_import_without_native_extension_explains_how_to_build"
BORROWED = "original_borrowed_and_owned_traces_keep_all_sixty_four_control_fault_event_queries"
IR = "core/magna/ir.py"
DEFER = "core/magna/deferred_feedback.py"
LOWER = "core/magna/native/bindings/file_lowering_bindings.hpp"
DEFERRED = "core/magna/native/compile/deferred.cpp"
# status, original obligations, required helper-file closure, exact typed-subset
# witnesses. Approved adjacent *subsets* never certify an unresolved obligation.
SPECS = {
    SOURCE: ("unmapped", ["recursive_lookup"], ["scripts/native_sources.py"], []),
    QUERY + "test_deferred_public_graph_isolation_and_owned_transfer": (
        "partial", ["public_graph_copy", "owned_graph_transfer"], [IR, DEFER, DEFERRED], [BORROWED]),
    QUERY + "test_lowered_operations_are_owned_even_for_guarded_noise": (
        "partial", ["noise_extra_copy"], [IR, "tests/support/file_hooks.py", LOWER,
                                     "core/magna/native/compile/file_lowering.cpp"],
        ["original_guarded_supplied_noise_retains_owned_actions_after_inputs_are_dropped"]),
    QUERY + "test_public_ir_still_copies_arbitrary_graph_and_prepared_snapshot": (
        "partial", ["constructor_graph_copy", "mutable_prepared_matches"], [IR], [BORROWED]),
    OPTIMIZATION + "test_prepared_ir_nested_mutation_and_returned_snapshot": (
        "partial", ["nested_metadata", "metadata_identity", "certificate_graph_copy", "certificate_lru"],
        [IR, "core/magna/simulator.py", "core/magna/native/runtime/certificate_cache.hpp",
         "core/magna/native/bindings/certificate_cache_bindings.hpp"],
        ["original_probability_queries_generate_no_input_key_but_explicit_compile_does",
         "original_lazy_batches_capture_one_source_while_next_single_query_uses_replacement", BORROWED]),
    LOADER: ("unmapped", ["extension_import", "extension_build_hint"],
             ["core/magna/__init__.py", "scripts/build_native_core.py"], []),
}
POLICIES = {
    "product": "independent_rust_plus_paper_experiments",
    "selection": "retain_current_typed_boundaries",
    "original_parity": "unresolved_obligations_remain",
    "metadata_domain": {
        "implemented": "typed_operations_owned_labels_and_owned_result_values",
        "annotation_graph_api": "not_implemented",
        "json_tree_annotation_api": "not_implemented",
        "arbitrary_python_object_compatibility": "not_implemented",
    },
    "graph_model_if_selected": {
        "status": "prospective_not_implemented",
        "representation": "budgeted_node_arena_with_reference_edges_for_aliases_and_cycles",
        "public_copy": "memoized_old_to_new_node_map_preserves_internal_aliases_cycles_and_source_isolation",
        "owned_transfer": "consume_graph_owner_without_copy_and_require_caller_relinquishment",
        "custom_objects": "define_supported_scalar_key_and_custom_object_domain_or_use_a_python_compatibility_layer",
        "rust_feasibility": "cycles_and_aliases_are_possible_with_an_explicit_model",
    },
    "certificate_identity": {
        "arbitrary_annotations_participate": False,
        "typed_identity": "explicit_source_sessions_and_captured_input_labels_not_nested_annotation_invalidation",
        "current_probability_cache": "exact_probability_values_keyed_by_source_session_and_compiled_physical_state",
        "current_certificate_json": "owned_export_not_a_metadata_keyed_branch_certificate_cache",
        "metadata_certificate_lru": "not_implemented",
        "if_selected": "define_graph_identity_and_nested_mutation_invalidation_before_caching_owned_branch_certificates",
    },
    "source_navigation": {
        "current": "manifest_relative_exact_paths_with_existence_hash_and_symlink_checks",
        "duplicate_basename": "distinct_explicit_paths_remain_valid",
        "upstream": "recursive_basename_zero_unique_ambiguous_resolution",
        "rust_recursive_lookup": "not_implemented",
    },
    "installation": {
        "current": "independent_rust_executable_and_public_api",
        "missing_executable": "operating_system_error_observed_and_diagnosed_by_caller_or_launcher",
        "diagnostic_inside_missing_program": False,
        "cpython_extension_loader": "not_implemented",
        "python_wrapper_added": False,
    },
}
IMPLEMENTATION_PLAN = {
    "G1": {"capability": "explicit_metadata_graph_domain_alias_cycle_identity_and_budgets", "depends_on": []},
    "G2": {"capability": "public_copy_owned_transfer_nested_mutation_and_prepared_matching", "depends_on": ["G1"]},
    "G3": {"capability": "metadata_keyed_certificate_identity_lru_and_returned_graph_isolation", "depends_on": ["G1", "G2"]},
    "S1": {"capability": "zero_unique_ambiguous_recursive_source_lookup_if_exact_compatibility_is_selected", "depends_on": []},
    "I1": {"capability": "explicit_cpython_distribution_loader_and_build_diagnostics_if_selected", "depends_on": []},
}
# Frozen from the pinned complete upstream test inventory, not from a mutable
# decision record. It retains all original methods even when they are unsupported.
ORIGINAL_IDS_SHA256 = "e8658757f4931ba266ef1bdc336778f6e47b704097718ae34fb41cec631432e9"


METHOD_CONTEXT_SHA256 = {
    'test_native_source_tools.NativeSourceToolsTests.test_source_resolution_rejects_absent_and_ambiguous_hooks':
        'b0dab182e5d0a39c34d29718c99a6698cbfe5fd9c73a34a38594586d61cf3bd6',
    'test_query_reuse.OwnershipAndDependencies.test_deferred_public_graph_isolation_and_owned_transfer':
        '2dc6ea4a062c6c1ed3f6357f88c5c89be5baf75dbe6247f9793caecb2615a0e1',
    'test_query_reuse.OwnershipAndDependencies.test_lowered_operations_are_owned_even_for_guarded_noise':
        'a4612042cb89ae86b7115b8937e0981db2f5d85a390adfe6d5041995d7507c3f',
    'test_query_reuse.OwnershipAndDependencies.test_public_ir_still_copies_arbitrary_graph_and_prepared_snapshot':
        '11559f52055de104193cfaa20b46061789f9e13e418e04f7f230ef15c3b8a441',
    'test_simulator_optimization.OptimizationTests.test_prepared_ir_nested_mutation_and_returned_snapshot':
        '543f64e00333884b806e9c0fa846add7af0f74c6b8c070921f1b2bb6067cf8d9',
    'test_user_interface.MissingExtensionTests.test_import_without_native_extension_explains_how_to_build':
        '11a16d26f0218e60348629a2e41b24b928a32f24966fb33e3e72d351724bc4d5',
}

SUPPORT_SOURCES = {
    SOURCE: [("benchmarks/magna_reproduction/reference.py", "def relative_file("),
             ("benchmarks/magna_reproduction/reference.py", "def verify_files(")],
    QUERY + "test_deferred_public_graph_isolation_and_owned_transfer": [
        ("rmagna/src/instrument/original_assignment_plan.rs", "pub fn from_owned("),
        ("rmagna/src/instrument/original.rs", "pub fn with_operations(")],
    QUERY + "test_lowered_operations_are_owned_even_for_guarded_noise": [
        ("rmagna/tests/rust_snapshot_contracts.rs", "fn original_guarded_supplied_noise_retains_owned_actions_after_inputs_are_dropped(")],
    QUERY + "test_public_ir_still_copies_arbitrary_graph_and_prepared_snapshot": [
        ("rmagna/src/instrument/original_assignment_plan.rs", "pub fn new("),
        ("rmagna/src/instrument/original.rs", "pub enum OriginalGate")],
    OPTIMIZATION + "test_prepared_ir_nested_mutation_and_returned_snapshot": [
        ("rmagna/src/instrument/original_probability_cache.rs", "pub struct OriginalProbabilityCache"),
        ("rmagna/src/instrument/original_assignment_certificate.rs", "pub fn to_json("),
        ("rmagna/src/instrument/original_assignment_plan.rs", "pub fn from_owned(")],
    LOADER: [("rmagna/tests/installed_cli_contracts.rs", "fn downstream_rust_consumer_executes_both_tutorial_apis_and_embedded_catalogue(")],
}


def support_sources(root, case):
    return [dict(path=path, anchor=anchor, sha256=reference.digest(root / path))
            for path, anchor in SUPPORT_SOURCES[case]]


def ids_digest(ids):
    return hashlib.sha256(json.dumps(sorted(ids), separators=(",", ":")).encode()).hexdigest()


def method_signature(source, method):
    return hashlib.sha256(json.dumps([source, method], sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def witnesses(root, names, target="rust_snapshot_contracts"):
    digest = reference.digest(root / f"rmagna/tests/{target}.rs")
    return [dict(package="rmagna", target=target, test=name, source_sha256=digest) for name in names]


def check_catalogue(catalogue, ledger, manifest, remaining, *, root=ROOT, upstream=None):
    rust.fields(catalogue, {"schema", "issue", "parity_claim", "reference_sha256", "upstream_commit",
                            "original_inventory_sha256", "policies", "implementation_plan", "methods"},
                "compatibility catalogue")
    require(catalogue.get("schema") == "rmagna.compatibility-boundaries.v1", "unsupported compatibility schema")
    require(catalogue.get("issue") == 1341 and catalogue.get("parity_claim") == "decisions_only_no_promotions",
            "compatibility scope cannot claim implementation/parity")
    require(catalogue.get("reference_sha256") == reference.digest(HERE / "reference.json")
            and catalogue.get("upstream_commit") == manifest["upstream"]["commit"], "compatibility reference changed")
    require(catalogue.get("policies") == POLICIES, "unsupported compatibility policy/implementation claim")
    require(catalogue.get("implementation_plan") == IMPLEMENTATION_PLAN,
            "compatibility implementation dependency/order changed")
    cases = {c["id"]: c for c in ledger["cases"]}
    require(len(cases) == len(ledger["cases"]) == 352 and ids_digest(cases) == ORIGINAL_IDS_SHA256,
            "original352-method denominator/inventory changed")
    require(all(c["status"] in {"verified", "partial", "unmapped"} for c in cases.values()),
            "compatibility cannot introduce architecture exclusions")
    require(catalogue.get("original_inventory_sha256") == ORIGINAL_IDS_SHA256, "compatibility original inventory changed")
    rows = catalogue["methods"]
    require(len(rows) == 6 and len({r["id"] for r in rows}) == 6
            and {r["id"] for r in rows} == set(SPECS), "missing/duplicate/unknown compatibility record")
    remaining_rows = {r["id"]: r for r in remaining["methods"]}
    declarations = None
    if upstream is not None:
        _, declarations, _ = ledger_tools.inventory(upstream, manifest)
        require(ids_digest(declarations) == ORIGINAL_IDS_SHA256, "pinned original inventory changed")
    for row in rows:
        rust.fields(row, {"id", "original_status", "source", "method", "helper_sources", "original_obligations",
                          "boundary", "support_status", "implementation_dependencies", "rust_witnesses",
                          "installed_boundary_witnesses", "supported_contract", "unsupported_behavior",
                          "rationale", "support_sources"}, "compatibility method")
        name = row["id"]
        status, obligations, helpers, required_tests = SPECS[name]
        require(name in remaining_rows, f"original obligation removed from remaining catalog: {name}")
        baseline = remaining_rows[name]
        require(cases[name]["status"] == row["original_status"] == baseline["ledger_status"] == status,
                f"unsupported original verified promotion/exclusion: {name}")
        require(method_signature(row["source"], row["method"]) == METHOD_CONTEXT_SHA256[name],
                f"required complete method/helper context changed: {name}")
        require(row["source"] == baseline["source"] and row["method"] == baseline["method"],
                f"whole method/helper requirement changed: {name}")
        require(manifest["upstream"]["files"].get(row["source"]["path"]) == row["source"]["source_sha256"],
                f"compatibility source identity changed: {name}")
        expected_helpers = {path: manifest["upstream"]["files"][path] for path in helpers}
        require(row["helper_sources"] == expected_helpers, f"required helper source closure changed: {name}")
        expected_obligations = [dict(id=key, requirement=OBLIGATIONS[key], status="not_implemented") for key in obligations]
        require(row["original_obligations"] == expected_obligations,
                f"missing/unsupported original obligation or implementation claim: {name}")
        require(row["boundary"] == "additional_compatibility_scope_not_selected"
                and row["support_status"] == "source_bound_subset_not_original_parity",
                f"unsupported compatibility support/parity status: {name}")
        require(row["implementation_dependencies"] == (
            ["S1"] if name == SOURCE else ["I1"] if name == LOADER else
            ["G1", "G2", "G3"] if name.startswith(OPTIMIZATION) else ["G1", "G2"]),
            f"compatibility method implementation order changed: {name}")
        require(row["rust_witnesses"] == witnesses(root, required_tests),
                f"adjacent/missing Rust subset witness: {name}")
        for witness in row["rust_witnesses"]:
            rust.witness(root, witness)
        require(row["installed_boundary_witnesses"] == (witnesses(root,
            ["downstream_rust_consumer_executes_both_tutorial_apis_and_embedded_catalogue"],
            "installed_cli_contracts") if name == LOADER else []), f"installed boundary witness changed: {name}")
        for witness in row["installed_boundary_witnesses"]:
            rust.witness(root, witness)
        for field in ["supported_contract", "unsupported_behavior", "rationale"]:
            rust.text(row[field], f"empty compatibility {field}: {name}")
        require(row["support_sources"] == support_sources(root, name),
                f"supported source owner substitution: {name}")
        for owner in row["support_sources"]:
            path = rust.safe_file(root, owner["path"])
            require(reference.digest(path) == owner["sha256"] and owner["anchor"] in path.read_text(),
                    f"compatibility support source changed: {name}")
        require(row["support_sources"], f"missing supported owner: {name}")
        if declarations is not None:
            require(declarations.get(name) == row["source"], f"compatibility declaration changed: {name}")
            from remaining_contracts import method_details
            details = method_details((upstream / row["source"]["path"]).read_text(), name)
            require(details == row["method"], f"complete method/helper context changed: {name}")
    return dict(status="PASS", scope=catalogue["parity_claim"], issue=1341, decisions=6,
                original_denominator=352, ledger_counts=dict(Counter(c["status"] for c in cases.values())),
                original_statuses=dict(Counter(r["original_status"] for r in rows)),
                supported_subset_records=6, unresolved_original_obligations=sum(len(r["original_obligations"]) for r in rows),
                rust_execution="not_run_source_bound_declarations_only", promoted_original_methods=0,
                upstream_verified=upstream is not None)


def probe_boundaries(upstream=None):
    """Observe actual tool/OS boundaries, not a new source lookup or launcher."""
    upstream_lookup = None
    if upstream is not None:
        path = reference.relative_file(upstream, "scripts/native_sources.py")
        reference.verify_files(upstream, {"scripts/native_sources.py": reference.load_manifest()["upstream"]["files"]["scripts/native_sources.py"]})
        upstream_lookup = runpy.run_path(str(path))["native_source"]
    with tempfile.TemporaryDirectory(prefix="magna-compatibility-") as directory:
        root = Path(directory).resolve()
        body = b"root exact hook\n"
        digest = hashlib.sha256(body).hexdigest()
        try:
            reference.verify_files(root, {"probe.hpp": digest})
        except reference.ReferenceError:
            pass
        else:
            raise ValueError("missing exact source was admitted")
        if upstream_lookup:
            try:
                upstream_lookup(root, "probe.hpp")
            except ValueError:
                pass
            else:
                raise ValueError("upstream missing source was admitted")
        (root / "probe.hpp").write_bytes(body)
        require(reference.verify_files(root, {"probe.hpp": digest}) == 1, "unique exact source failed")
        if upstream_lookup:
            require(upstream_lookup(root, "probe.hpp") == root / "probe.hpp", "upstream unique source differs")
        (root / "nested").mkdir()
        (root / "nested/probe.hpp").write_bytes(b"distinct nested hook\n")
        both = {"probe.hpp": digest, "nested/probe.hpp": reference.digest(root / "nested/probe.hpp")}
        require(reference.verify_files(root, both) == 2, "duplicate basenames broke explicit source paths")
        if upstream_lookup:
            try:
                upstream_lookup(root, "probe.hpp")
            except ValueError:
                pass
            else:
                raise ValueError("upstream ambiguous source was admitted")
        try:
            subprocess.run([str(root / "missing-rmagna")], cwd=root, capture_output=True, timeout=5, check=False)
        except FileNotFoundError:
            pass
        else:
            raise ValueError("missing executable did not fail at the caller boundary")
    return dict(status="PASS", exact_paths="missing_rejected_unique_admitted_duplicate_basenames_distinct",
                upstream_recursive_lookup="zero_unique_ambiguous_pass" if upstream_lookup else "not_run",
                missing_executable="caller_observed_FileNotFoundError_no_program_executed",
                cpython_extension_import="not_implemented_not_qualified")
