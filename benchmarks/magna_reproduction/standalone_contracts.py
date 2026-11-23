"""Decompose pinned aggregate acceptance; candidate owners never certify parity."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import coverage_rust as rust

import standalone_discovery as discovery
# Explicit semantic ranges in the pinned foundation main; helper context is retained whole.
MAIN = [(771, 'wide-clifford-elimination'),
 (833, 'word-shifts'),
 (853, 'history-efficiency'),
 (863, 'prepared-clifford'),
 (927, 'certificate-cache'),
 (939, 'categorical-law-interning'),
 (949, 'virtual-noise-actions'),
 (979, 'independent-proposal'),
 (988, 'paired-ratio'),
 (995, 'boundary-coordinate-rhs'),
 (999, 'literal-assignment-and-controls'),
 (1057, 'operation-sampling'),
 (1087, 'true-reported-records'),
 (1093, 'target-inverse'),
 (1099, 'recovery-ancilla'),
 (1108, 'file-target'),
 (1119, 'malformed-history-task'),
 (1137, 'file-causal-runtime'),
 (1183, 'online-classical-knowledge'),
 (1189, 'file-parser'),
 (1199, 'scaled-noise-model'),
 (1219, 'cached-noise-law-and-weight'),
 (1308, 'exact-categorical'),
 (1322, 'fixed-virtual-template'),
 (1333, 'selection-offset'),
 (1336, 'coherent-file-pipeline'),
 (1345, 'trace-purpose'),
 (1384, 'prepared-trace-runtime'),
 (1414, 'character-geometry'),
 (1471, 'reference-resource-transport'),
 (1498, 'strict-parity-representation'),
 (1532, 'dimension-changing-transport'),
 (1571, 'consuming-contraction'),
 (1599, 'finite-sum-contraction'),
 (1614, 'completed-pair-sampling'),
 (1636, 'sparse-noise-stream'),
 (1645, 'weight-sectors'),
 (1655, 'exact-statistics'),
 (1661, 'wide-exact-scalars'),
 (1672, 'caller-rng'),
 (1674, 'signed-affine-grouping'),
 (1678, 'coherent-expansion'),
 (1689, 'literal-callback-certificates'),
 (1717, 'affine-proposal-hull'),
 (1774, 'quadratic-radical-support')]

SCHEMA = 'rmagna.standalone-contracts.v2'
CLAIM = 'source_bound_rust_obligations'
SOURCES = {'causal': 'tests/native/causal.cpp',
           'foundation': 'tests/native/foundation.cpp'}
AGGREGATES = {
    'causal': 'test_native_causal.NativeCausalTest.test_online_basis_without_cpython',
    'foundation': 'test_native_standalone.NativeStandaloneTest.test_foundation_without_cpython',
}
DISCOVERY_DIGESTS = {
    'causal': '613789a30c994716919d2c367b9925fab60392922b3042b24ac47ee3c266ebac',
    'foundation': '0aa6f936b04114564f1106dabc4618c3ef1a2f9af85368a2b7504a90df35d584',
}

ASSESSMENTS = {'validation_gap', 'implementation_gap', 'representation_difference', 'helper_context'}
ASSESSMENTS.add('source_bound_witness')

CAUSAL_TARGET = 'standalone_causal_knowledge'
CAUSAL_TESTS = ['check_pending_semantics', 'check_finite_assignments',
                'check_large_request_order', 'check_dense_spans']
QUALIFICATIONS = {
    **{f'causal/{name}': [(CAUSAL_TARGET, name)] for name in CAUSAL_TESTS},
    **{f'causal/{name}': [(CAUSAL_TARGET, test) for test in CAUSAL_TESTS]
       for name in ('require', 'scalar_parity', 'same_request', 'main')},
    'foundation/main.online-classical-knowledge': [(CAUSAL_TARGET, 'foundation_online_classical_knowledge')],
    'foundation/main.certificate-cache': [('standalone_certificate_snapshots', 'foundation_certificate_cache')],
}
ADAPTATIONS = {group: ('924af70cbbedc790fa17dc5f09de8d46f05b02d35e37902db8514443d4f5d0cf'
                       if group == 'foundation/main.certificate-cache' else
                       '3ba859ceb1d8ea8653df78feca9a2f44a5d15d0589530398bb3153dcf1635b98')
               for group in QUALIFICATIONS}
SUPPORT_SOURCES = {group: ({'rmagna/tests/support/original_gf2.rs':
                           '64814d7a310cee97f39e74a7f6027eb7d488e165b0e51a71d131ed076099077c'}
                          if any(target == CAUSAL_TARGET for target, _ in witnesses) else {})
                   for group, witnesses in QUALIFICATIONS.items()}

# Source-specific foundation fixtures. Trace storage remains an explicit gap.
FOUNDATION_TARGET = "standalone_foundation_contracts"
FOUNDATION_TESTS = {'require': ['affine_proposal_hull_and_projection',
             'boundary_coordinate_rhs',
             'cached_noise_law_and_weight',
             'character_geometry_and_resource_transport',
             'check_axis_prefix',
             'check_composed_feedback_frames',
             'check_incremental_history_session',
             'check_packed_clifford_updates',
             'check_quadratic_batches',
             'check_reference_boundary',
             'check_reference_coordinates',
             'checked_primitives_preserve_padding_shapes_and_refusal',
             'coherent_expansion_and_target_amplitude',
             'coherent_file_pipeline_and_prepared_trace_runtime',
             'completed_pair_sampling_and_branch_limit',
             'consuming_and_finite_sum_contraction',
             'dimension_changing_clifford_transport',
             'entry_dependent_geometry_matches_finite_sums',
             'exact_categorical_zero_channel_uses_one_caller_word',
             'exact_file_parser',
             'file_causal_runtime_and_enumeration',
             'fixed_virtual_template_and_selection_offset',
             'history_efficiency_and_paired_statistics',
             'independent_proposal_and_exact_range_order',
             'literal_branch_order_and_declared_controls',
             'literal_callback_certificate_owns_signed_wire_result',
             'literal_layout_avoids_inline_record_collisions',
             'malformed_history_task_is_rejected_before_execution',
             'operation_sampling_reported_input_and_terminal_mass',
             'prepared_clifford',
             'quadratic_radical_support',
             'scaled_noise_model_and_categorical_law_interning',
             'signed_affine_grouping',
             'sparse_noise_stream_and_weight_sectors',
             'strict_parity_representation',
             'target_and_recovery_mapping',
             'target_projection_keeps_seven_protocol_records_and_independent_owner',
             'trace_purpose_preserves_dilation_axes_and_every_fault_location',
             'true_reported_records_and_coincident_fault_order',
             'virtual_noise_actions',
             'wide_clifford_elimination',
             'wide_exact_scalars_and_caller_rng',
             'word_shifts'],
 'scalar_value': ['character_geometry_and_resource_transport',
                  'check_reference_boundary',
                  'check_incremental_history_session',
                  'wide_exact_scalars_and_caller_rng'],
 'dense': ['consuming_and_finite_sum_contraction',
           'wide_clifford_elimination',
           'prepared_clifford',
           'character_geometry_and_resource_transport',
           'strict_parity_representation',
           'dimension_changing_clifford_transport',
           'entry_dependent_geometry_matches_finite_sums',
           'check_reference_boundary',
           'check_incremental_history_session',
           'coherent_expansion_and_target_amplitude'],
 'finite_phase_sum': ['prepared_clifford',
                      'character_geometry_and_resource_transport',
                      'strict_parity_representation',
                      'dimension_changing_clifford_transport',
                      'entry_dependent_geometry_matches_finite_sums'],
 'check_transported_phase': ['prepared_clifford',
                             'character_geometry_and_resource_transport',
                             'strict_parity_representation',
                             'dimension_changing_clifford_transport',
                             'entry_dependent_geometry_matches_finite_sums'],
 'full_axis_sector': ['check_axis_prefix'],
 'check_axis_prefix': ['check_axis_prefix'],
 'check_reference_coordinates': ['check_reference_coordinates'],
 'check_packed_clifford_updates': ['check_packed_clifford_updates'],
 'check_quadratic_batches': ['check_quadratic_batches'],
 'check_composed_feedback_frames': ['check_composed_feedback_frames'],
 'check_reference_boundary': ['check_reference_boundary'],
 'check_incremental_history_session': ['check_incremental_history_session'],
 'main.wide-clifford-elimination': ['wide_clifford_elimination'],
 'main.word-shifts': ['word_shifts', 'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.history-efficiency': ['history_efficiency_and_paired_statistics'],
 'main.prepared-clifford': ['prepared_clifford'],
 'main.categorical-law-interning': ['scaled_noise_model_and_categorical_law_interning'],
 'main.virtual-noise-actions': ['virtual_noise_actions',
                                'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.independent-proposal': ['independent_proposal_and_exact_range_order'],
 'main.paired-ratio': ['history_efficiency_and_paired_statistics'],
 'main.boundary-coordinate-rhs': ['boundary_coordinate_rhs'],
 'main.literal-assignment-and-controls': ['literal_branch_order_and_declared_controls',
                                          'literal_layout_avoids_inline_record_collisions',
                                          'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.operation-sampling': ['operation_sampling_reported_input_and_terminal_mass'],
 'main.true-reported-records': ['true_reported_records_and_coincident_fault_order'],
 'main.target-inverse': ['target_and_recovery_mapping',
                         'target_projection_keeps_seven_protocol_records_and_independent_owner'],
 'main.recovery-ancilla': ['target_and_recovery_mapping'],
 'main.file-target': ['target_and_recovery_mapping'],
 'main.malformed-history-task': ['malformed_history_task_is_rejected_before_execution'],
 'main.file-causal-runtime': ['file_causal_runtime_and_enumeration'],
 'main.file-parser': ['exact_file_parser',
                      'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.scaled-noise-model': ['scaled_noise_model_and_categorical_law_interning'],
 'main.cached-noise-law-and-weight': ['cached_noise_law_and_weight'],
 'main.exact-categorical': ['exact_categorical_zero_channel_uses_one_caller_word'],
 'main.fixed-virtual-template': ['fixed_virtual_template_and_selection_offset'],
 'main.selection-offset': ['fixed_virtual_template_and_selection_offset'],
 'main.coherent-file-pipeline': ['coherent_file_pipeline_and_prepared_trace_runtime'],
 'main.character-geometry': ['character_geometry_and_resource_transport',
                             'entry_dependent_geometry_matches_finite_sums'],
 'main.reference-resource-transport': ['character_geometry_and_resource_transport'],
 'main.strict-parity-representation': ['strict_parity_representation'],
 'main.dimension-changing-transport': ['dimension_changing_clifford_transport',
                                       'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.consuming-contraction': ['consuming_and_finite_sum_contraction'],
 'main.finite-sum-contraction': ['consuming_and_finite_sum_contraction'],
 'main.completed-pair-sampling': ['completed_pair_sampling_and_branch_limit'],
 'main.sparse-noise-stream': ['sparse_noise_stream_and_weight_sectors'],
 'main.weight-sectors': ['sparse_noise_stream_and_weight_sectors'],
 'main.exact-statistics': ['history_efficiency_and_paired_statistics'],
 'main.wide-exact-scalars': ['wide_exact_scalars_and_caller_rng'],
 'main.caller-rng': ['wide_exact_scalars_and_caller_rng'],
 'main.signed-affine-grouping': ['signed_affine_grouping',
                                 'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.coherent-expansion': ['coherent_expansion_and_target_amplitude'],
 'main.literal-callback-certificates': ['literal_callback_certificate_owns_signed_wire_result'],
 'main.affine-proposal-hull': ['affine_proposal_hull_and_projection',
                               'checked_primitives_preserve_padding_shapes_and_refusal'],
 'main.quadratic-radical-support': ['quadratic_radical_support']}
QUALIFICATIONS.update({"foundation/"+group: [(FOUNDATION_TARGET, test) for test in tests]
                       for group, tests in FOUNDATION_TESTS.items()})
ADAPTATIONS.update({'foundation/require': 'a6638ced2241c8ba8d28e89359b35c675ce3c99ac3b50a8ccac79a115d81b139',
 'foundation/scalar_value': 'a6638ced2241c8ba8d28e89359b35c675ce3c99ac3b50a8ccac79a115d81b139',
 'foundation/dense': 'a6638ced2241c8ba8d28e89359b35c675ce3c99ac3b50a8ccac79a115d81b139',
 'foundation/finite_phase_sum': 'a6638ced2241c8ba8d28e89359b35c675ce3c99ac3b50a8ccac79a115d81b139',
 'foundation/check_transported_phase': 'a6638ced2241c8ba8d28e89359b35c675ce3c99ac3b50a8ccac79a115d81b139',
 'foundation/full_axis_sector': '64df42f944dcd9ed4c45051f8c795036287a46da7365095c7b094d8256747e21',
 'foundation/check_axis_prefix': '64df42f944dcd9ed4c45051f8c795036287a46da7365095c7b094d8256747e21',
 'foundation/check_reference_coordinates': '1bb6feb5af26eb55d2d4364957b8e88b59b8652c3cbf294d5bfffc02efb140a6',
 'foundation/check_packed_clifford_updates': '20bde27394764a23c5e94cbeaf45ffd80bb7cdc7ff33446b7bbe0832b949c64c',
 'foundation/check_quadratic_batches': 'e25949575ac3e1175a78aca0faccb0ae2ee209616178171e759237243a1c3edd',
 'foundation/check_composed_feedback_frames': '8336c992551008a1578cb08ddaa7257e537c2e10fd9f74b90fd103d52d42b2d2',
 'foundation/check_reference_boundary': 'b2553af1bcfa1609a7e0aed364beb86aeb3761fc5bc7a8c6fc07d150da33463f',
 'foundation/check_incremental_history_session': '5b97dd13c0b5168278ad2f99dc20ad929ab167ab901753f7b3747e51977dcb84',
 'foundation/main.wide-clifford-elimination': '434e4c6bda01f3c34b170f5ad291762a14a100b0d1a58d074391d83c590b9fd4',
 'foundation/main.word-shifts': '93e705db39cb98f6937106182593c40da94aa04869326d06c25a50a78e7b8ece',
 'foundation/main.history-efficiency': 'd425548a9d52c13ca6b327d92087dc7700a0720b24216ca1e0073d3e91f719b3',
 'foundation/main.prepared-clifford': '078ecb4b83a7f9504f5277d5fb9ade409e30d539825df38c744620e05e3e476b',
 'foundation/main.categorical-law-interning': '3b6c5f84e2dedf8782aaee88acb819bcebdaeaeb3925bbb729aa6de97483fde6',
 'foundation/main.virtual-noise-actions': 'bdd7f9f6ea4f2750d321cfbfb41eb46ad63e108e913fde6ded154a70b5f2769c',
 'foundation/main.independent-proposal': 'd6e29d02a65ce4e521e6152c36eb8906c4b654fe67756b4674d3d48b4ff10b52',
 'foundation/main.paired-ratio': '376906b16f350a33d0f8fab7eac7782d627d3b22c27f361a6dc1a4d97edb4940',
 'foundation/main.boundary-coordinate-rhs': 'ac39b5db5ca7b92c2faa677e36049dd214bb6555f2a9e59d1674868ed4ffe261',
 'foundation/main.literal-assignment-and-controls': '242c05d661e2f95acf09a4e495181e90fe7db9c7cd5499ac2771865db05d8e10',
 'foundation/main.operation-sampling': '43e4b52466c43f1d59528a4b6a0a4f242126d4b21c05cbb30ef84220e2e9aab9',
 'foundation/main.true-reported-records': '2db63dc3aebf83b6ec7cb51ccecea72bedb6ae49c149eaf2fb8ad3ee34e13c44',
 'foundation/main.target-inverse': '6b2abc6309b879c907f1cf78a09151b82d2b98ecee65866a3ad839ecfbc56066',
 'foundation/main.recovery-ancilla': '8685b21e3efdbda6a1d41fa812b4df8549acf9e1af4fd5878dcadb55d6ad8e28',
 'foundation/main.file-target': 'd8bd41c743f4aee7d4aaddb814f111ef3d5742f46e9edc344b6b3ee1514cbd21',
 'foundation/main.malformed-history-task': '8cc58059ba86ccebdaec989423fa192c13fd28143128c40e6ada606331941dae',
 'foundation/main.file-causal-runtime': 'e4d1f1456439e3da7f0a54a22bcba9ffae4a50792d42378dc964e74e4c449bcf',
 'foundation/main.file-parser': '431ac93f557b2e32001f8097e3b75da3dfebea667aafd9c2de1012329ce02b13',
 'foundation/main.scaled-noise-model': 'c6be4238ad57a2e717381c828f67c2727b94793a7af727b1c047d46e723d32cf',
 'foundation/main.cached-noise-law-and-weight': '77a4dc3182e57e7da5fffa5a49794a0f044a94116be153d7d8d9318e78be9c25',
 'foundation/main.exact-categorical': 'd96e7cafff51bf3cf5d5a01dfcc6d5b0fac917004cc07ba9f0f3a50ce40545b9',
 'foundation/main.fixed-virtual-template': '60956011e56b03a1bae55383f952dc31cc178365513a7e18fa5fa60992d9361d',
 'foundation/main.selection-offset': 'd47e2f7c37bf72749b8be7d79057b3f8af39d69ea4e09725043fe84c6b2b9aa8',
 'foundation/main.coherent-file-pipeline': '0606071d0086191954708218d5ae51b2e1f402f3d2ede56b585dbe62c2879b0d',
 'foundation/main.character-geometry': '236a408494f8a5c6ff54e86bfacd258d3e3182b1f6a28c57279e1158b92e7b65',
 'foundation/main.reference-resource-transport': '608ab03c4fcc4216dab42060c205231370065a68064dcf63365e98215b89b9ce',
 'foundation/main.strict-parity-representation': '2b1050487c5c9cccefc6dc746250310c9ac7e9699446a97926f2557adfc6308d',
 'foundation/main.dimension-changing-transport': '83a120862d35ff40ef91d767d78386f5ccee7d1db1cbc844d2c32e0c2f145fb8',
 'foundation/main.consuming-contraction': 'c9c711e49bc1f9c7a0db6f9059cbb673ced8f56b4b28a0bec975a5597d443bdd',
 'foundation/main.finite-sum-contraction': '88ad554aa4653308f73454b467a0fec322d3d111a577fbadcece5c7eaa000a01',
 'foundation/main.completed-pair-sampling': '8c84f8e8c27d6832544de822dfb079fb3d2836e3e2ceedd8bda982d8a621f496',
 'foundation/main.sparse-noise-stream': 'affe424ab6c01ea4f026766e51b3088ef947f18536fae507df54fd4f80ecd549',
 'foundation/main.weight-sectors': 'd24b335d267383f86894d6bbfda24631b29bc56987dee5387b46be11aa4ff94e',
 'foundation/main.exact-statistics': '1aebcccede8c9a43cd64b065ce444b02dd80298da3b9dbce00e8549fa8ef7a97',
 'foundation/main.wide-exact-scalars': '2cce823654eb30c8298cdfceeeb132ff55424ef5bd044da9a4619b991789c944',
 'foundation/main.caller-rng': '3ad3e2f476210cc5fab5b29446832b3cd6a3700bc809e253828508cfb6525e75',
 'foundation/main.signed-affine-grouping': '9f432d6ce0af8abda2c61737e65c58540760ee7e8a00d7fdbce6e31201511a9c',
 'foundation/main.coherent-expansion': '7f3d3942ae1c45b2aaf649e8c31ac70efac78100d5f5bc0b3dbc112d62cbf6b8',
 'foundation/main.literal-callback-certificates': '782329397eb56a31d58e465c588a19a48bc657b48654d18e060ee56375a1857b',
 'foundation/main.affine-proposal-hull': '1b455d4df90f4cf91a019b9a43452c861cd19cf4d77ab56432b0fadfac1d9a3d',
 'foundation/main.quadratic-radical-support': 'c6b0f71c164377584b466285cf93d2f095a6f9ac677273334546da5bbf3f8e6f'})
SUPPORT_SOURCES.update({"foundation/"+group: {'rmagna/tests/support/original_gf2.rs': '64814d7a310cee97f39e74a7f6027eb7d488e165b0e51a71d131ed076099077c', 'rmagna/tests/common/mod.rs': 'ecacfdbf65863a1690414edeb9ab09364c2ae01eaf0726f50b61e6e20a4f03d9'}
                        for group in FOUNDATION_TESTS})


def qualification(root, group):
    require(group['id'] in QUALIFICATIONS, 'standalone group has no reviewed Rust qualification')
    expected = QUALIFICATIONS[group['id']]
    value = group.get('qualification')
    require(isinstance(value, dict) and set(value) == {'witnesses', 'adaptation', 'support_sources'},
            'missing source-specific standalone qualification')
    require(value['support_sources'] == SUPPORT_SOURCES[group['id']],
            'standalone fixture support inventory changed')
    for path, digest in value['support_sources'].items():
        require(rust.sha(rust.safe_file(root, path)) == digest,
                f'standalone fixture support source changed: {path}')
    require(isinstance(value['adaptation'], str)
            and hashlib.sha256(value['adaptation'].encode()).hexdigest() == ADAPTATIONS[group['id']],
            'standalone representation adaptation changed')
    witnesses = value['witnesses']
    require(isinstance(witnesses, list) and len(witnesses) == len(expected),
            'standalone qualification witness inventory changed')
    require([rust.witness(root, witness) for witness in witnesses] == expected,
            'standalone fixture requires its source-specific witnesses')
    return witnesses


def require(condition, message):
    if not condition:
        raise ValueError(message)


def compact_discovery(source):
    """Retain source identity, complete helper closure and every literal site."""
    found = discovery.discover(source)
    return dict(
        source_sha256=found['source_sha256'],
        helpers=found['functions'],
        assertions=[{key: value for key, value in a.items() if key != 'expression'}
                    for a in found['assertions']],
        counts=found['counts'],
    )


def bindings_from_facts(facts):
    """Explicit semantic subdivisions of the pinned main, full other helpers."""
    groups = []
    for name, found in facts.items():
        for helper in found['helpers']:
            spans = [(helper['line'], helper['end_line'], helper['name'])]
            if name == 'foundation' and helper['name'] == 'main':
                spans = [(line, MAIN[i+1][0]-1 if i+1 < len(MAIN) else helper['end_line'],
                          f'main.{label}') for i, (line, label) in enumerate(MAIN)]
            for lo, hi, label in spans:
                assertions = [a for a in found['assertions']
                              if a['function'] == helper['name'] and lo <= a['line'] <= hi]
                # A group's setup can reference other source groups, but its
                # assertion itself must never be cut across semantic ranges.
                require(all(a['end_line'] <= hi for a in assertions),
                        f'assertion crosses standalone group: {name}/{label}')
                groups.append(dict(
                    id=f'{name}/{label}', function=helper['name'], line=lo, end_line=hi,
                    assertions=[a['id'] for a in assertions],
                    loops=[i for i, loop in enumerate(helper['loops'])
                           if lo <= loop['line'] <= hi],
                ))
    return groups


def group_bindings(sources):
    facts = {name: compact_discovery(source) for name, source in sources.items()}
    return facts, bindings_from_facts(facts)


def check_catalogue(sources, catalogue, ledger, reference_manifest, reference_sha256, root, *, rust_run=None):
    require(set(sources) == set(SOURCES), 'missing standalone source suite')
    facts, _ = group_bindings(sources)
    for name, path in SOURCES.items():
        require(facts[name]['source_sha256'] == reference_manifest['upstream']['files'][path],
                f'standalone pinned source changed: {path}')
    require(catalogue.get('sources') == facts, 'standalone helper/assertion/loop inventory changed')
    result = check_inventory(catalogue, ledger, reference_manifest, reference_sha256, root, rust_run=rust_run)
    result['upstream_verified'] = True
    return result


def check_inventory(catalogue, ledger, reference_manifest, reference_sha256, root, *, rust_run=None):
    """Offline structural check of the exact retained discovery snapshot.

    Snapshot digests bind the complete original discovery, including empty
    helpers and loop-only oracles. Bound groups require their reviewed fixtures.
    Executable coverage additionally requires a source/compiler/process receipt;
    structural-only callers report those groups as unexecuted.
    """
    require(catalogue.get('schema') == SCHEMA, 'unsupported standalone schema')
    require(catalogue.get('claim') == CLAIM, 'standalone qualification scope changed')
    require(catalogue.get('reference_sha256') == reference_sha256,
            'standalone reference identity changed')
    require(catalogue.get('upstream_commit') == reference_manifest['upstream']['commit'],
            'standalone upstream commit changed')
    facts = catalogue.get('sources')
    require(isinstance(facts, dict) and set(facts) == set(SOURCES), 'missing standalone source inventory')
    for name, path in SOURCES.items():
        encoded = json.dumps(facts[name], sort_keys=True, separators=(',', ':')).encode()
        require(hashlib.sha256(encoded).hexdigest() == DISCOVERY_DIGESTS[name],
                f'standalone helper/assertion/loop inventory changed: {name}')
        require(facts[name]['source_sha256'] == reference_manifest['upstream']['files'][path],
                f'standalone pinned source changed: {path}')
    bindings = bindings_from_facts(facts)
    groups = catalogue.get('groups')
    require(isinstance(groups, list) and len(groups) == len(bindings),
            'missing or duplicate standalone assertion group')
    require(all(isinstance(g, dict) for g in groups), 'invalid standalone group')
    require(len({g.get('id') for g in groups}) == len(groups), 'duplicate standalone group')
    require({g.get('id') for g in groups} == {g['id'] for g in bindings},
            'missing or unknown standalone helper group')
    expected = {g['id']: g for g in bindings}
    for group in groups:
        require(all(group.get(key) == value for key, value in expected[group['id']].items()),
                f'standalone assertion/loop partition changed: {group["id"]}')
        require(group.get('status') in {'unresolved', 'bound'},
                'standalone groups require source-specific executable qualification before promotion')
        if group['status'] == 'bound':
            qualification(root, group)
        else:
            require('qualification' not in group, 'unresolved standalone group has a qualification')
        require(group.get('assessment') in ASSESSMENTS, 'unknown standalone assessment')
        require(type(group.get('follow_up_issue')) is int and group['follow_up_issue'] > 0,
                'missing standalone unresolved follow-up issue')
        require(isinstance(group.get('reason'), str) and group['reason'].strip(),
                'missing explicit unresolved standalone obligation')
        owners = group.get('candidate_owners')
        require(isinstance(owners, list), 'missing standalone owner assessment')
        if group['assessment'] == 'validation_gap':
            require(owners, 'validation gap must name existing Rust ownership candidate')
        for owner in owners:
            path = owner.get('path', '')
            parts = Path(path).parts
            require(path and not Path(path).is_absolute() and '..' not in parts
                    and '\\' not in path and path.endswith('.rs'),
                    'invalid relative standalone Rust owner')
            resolved = root / path
            require(resolved.is_file() and not resolved.is_symlink()
                    and resolved.resolve().is_relative_to(root.resolve())
                    and isinstance(owner.get('anchor'), str) and owner['anchor']
                    and owner['anchor'] in resolved.read_text(),
                    f'standalone Rust owner missing: {path}')
        witnesses = group.get('candidate_witnesses')
        require(isinstance(witnesses, list), 'missing standalone witness assessment')
        for witness in witnesses:
            # Declaration/source binding only. Reusing these already existing
            # tests never changes this group's unresolved executable status.
            rust.witness(root, witness)
    statuses = {row['id']: row['status'] for row in ledger['cases']}
    require(len(statuses) == len(ledger['cases']), 'duplicate ledger method')
    declared = {}
    for name, method in AGGREGATES.items():
        own = [group for group in groups if group['id'].startswith(name+'/')]
        declared[name] = 'verified' if all(group['status'] == 'bound' for group in own) else 'unmapped'
        require(statuses.get(method) == declared[name], 'unresolved standalone aggregate cannot be promoted')
        if declared[name] == 'verified':
            expected_witnesses = set(witness for group in own for witness in QUALIFICATIONS[group['id']])
            row = next(row for row in ledger['cases'] if row['id'] == method)
            require({rust.witness(root, witness) for witness in row['rust_tests']} == expected_witnesses,
                    'standalone aggregate witness inventory changed')
    # Explicit partitions also protect assertion-free helpers and loop-only
    # oracles, which a require()-only scanner would otherwise silently lose.
    for name, found in facts.items():
        own = [g for g in groups if g['id'].startswith(name + '/')]
        actual = [a for g in own for a in g['assertions']]
        require(sorted(actual) == sorted(a['id'] for a in found['assertions']),
                'standalone assertion partition incomplete')
        helpers = {h['name']: h for h in found['helpers']}
        for helper in helpers.values():
            assigned = [i for g in own if g['function'] == helper['name'] for i in g['loops']]
            require(sorted(assigned) == list(range(len(helper['loops']))),
                    'standalone loop partition incomplete')
            require(set(helper['helper_calls']) <= set(helpers), 'missing standalone source helper')
    bound = [group for group in groups if group['status'] == 'bound']
    observed = dict(status='not_run', source_bound=False, compiler_bound=False)
    if rust_run is not None:
        from standalone_execution import validate
        witnesses = [witness for group in bound for witness in qualification(root, group)]
        observed = validate(root, rust_run, witnesses)
    covered = bound if observed['status'] == 'PASS' else []
    unexecuted = [] if covered else bound
    unresolved = [group for group in groups if group['status'] == 'unresolved']
    suites = {}
    for name, found in facts.items():
        own = [g for g in groups if g['id'].startswith(name+'/')]
        executed = [g for g in covered if g['id'].startswith(name+'/')]
        remaining = [g for g in unresolved if g['id'].startswith(name+'/')]
        suites[name] = dict(
            helpers=len(found['helpers']),
            covered_helpers=sum(all(g in executed for g in own if g['function'] == h['name'])
                                for h in found['helpers']),
            groups=len(own), covered_groups=len(executed),
            assertions=found['counts']['assertions'],
            covered_assertions=sum(len(g['assertions']) for g in executed),
            unresolved_assertions=sum(len(g['assertions']) for g in remaining),
            unresolved_groups=[g['id'] for g in remaining],
            loop_headers=found['counts']['loop_headers'],
            covered_loop_headers=sum(len(g['loops']) for g in executed))
    return dict(status='PASS', scope=catalogue['claim'], upstream_verified=False,
                suites=suites,
                helpers=sum(v['counts']['functions'] for v in facts.values()),
                groups=len(groups), covered_groups=len(covered), unresolved_groups=len(unresolved),
                bound_groups=len(bound), unexecuted_groups=len(unexecuted),
                assertions=sum(v['counts']['assertions'] for v in facts.values()),
                covered_assertions=sum(len(group['assertions']) for group in covered),
                unresolved_assertions=sum(len(group['assertions']) for group in unresolved),
                unexecuted_assertions=sum(len(group['assertions']) for group in unexecuted),
                loop_headers=sum(v['counts']['loop_headers'] for v in facts.values()),
                assessments=dict(sorted(Counter(g['assessment'] for g in groups).items())),
                follow_up_issues=sorted({g['follow_up_issue'] for g in unresolved}),
                execution=observed, aggregate_declarations=declared,
                aggregate_statuses={name: ('requires_execution' if state == 'verified' and not covered else state)
                                    for name, state in declared.items()})


def require_complete(result):
    """Foundation completion is distinct from causal and external consumer status.

    Structural declarations alone never complete the aggregate. This function
    consumes only check_catalogue's validated source/compiler/process result.
    """
    foundation = result['suites']['foundation']
    require(result['upstream_verified'], 'foundation completion requires the actual pinned source')
    require(not foundation['unresolved_groups'],
            'foundation incomplete: '+str(foundation['unresolved_assertions'])+
            ' assertions remain in '+', '.join(foundation['unresolved_groups']))
    require(result['execution']['status'] == 'PASS'
            and result['execution']['source_bound'] and result['execution']['compiler_bound'],
            'foundation completion requires source/compiler/process receipts')
    require(result['aggregate_statuses']['foundation'] == 'verified'
            and foundation['covered_assertions'] == 192
            and foundation['covered_groups'] == 58
            and foundation['covered_helpers'] == 14
            and foundation['covered_loop_headers'] == 197,
            'foundation completion requires all192 assertions,58 groups,14 helpers and197 loops')
