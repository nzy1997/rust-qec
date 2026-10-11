"""Native packed-scheduler guards; three Rust roles use the identical reviewed source.

Baseline/control are null controls, so no Rust optimization speedup is inferred.
"""
import re

PROFILE = 'packed-structural-97fc-peer-null-controls-v1'
BASELINE = '97fc7e5a79f34741785df9d85a4f70575a8844d6'
CANDIDATE = '97fc7e5a79f34741785df9d85a4f70575a8844d6'
SELECTIONS = (('--lib', 'phase_specialized_cdf_tests'), ('--lib', 'near_clifford::compiled::compile_frame::packed_compile_frame_tests::'), ('--lib', 'near_clifford::compiled::noise_schedule::tests::packed_effect_matches_boolean_coordinates_across_words_and_basis_changes', '--', '--exact'), ('--test', 'near_clifford_structural_limits'))
NAMES = (('near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::i_pow_modulo_literals_keep_exact_components_for_all_u8_values', 'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::scalar_cdf_preserves_all_unsigned_modulo_phase_aliases', 'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::phase_specialized_scalar_and_packet_cdfs_match_independent_frozen_bits'), ('near_clifford::compiled::compile_frame::packed_compile_frame_tests::packed_compile_inverse_matches_signed_incumbent_after_left_and_right_gates', 'near_clifford::compiled::compile_frame::packed_compile_frame_tests::packed_compile_s_inverse_and_padding_have_explicit_sign_contracts'), ('near_clifford::compiled::noise_schedule::tests::packed_effect_matches_boolean_coordinates_across_words_and_basis_changes',), ('original_coherent_d5_compiles_and_samples_with_the_default_resource_limits', 'feasible_rank_five_order_preserves_records_counts_and_rng_at_the_caller_limit'))

def require_heads(heads):
    expected = dict(baseline=BASELINE, candidate=CANDIDATE, control=BASELINE)
    if heads != expected:
        raise ValueError('packed preflight requires the exact reviewed identical Rust source roles')

def check(index, log):
    if type(index) is not int or not 0 <= index < len(NAMES):
        raise ValueError('packed preflight native-check index differs')
    expected = set(NAMES[index])
    records, summaries = [], []
    for line in log.splitlines():
        if not line.startswith('test '):
            continue
        if line.startswith('test result:'):
            summaries.append(line)
        else:
            match = re.fullmatch(r'test (\S+) \.\.\. (.+)', line)
            if match is None:
                raise ValueError('packed preflight test record is malformed')
            records.append(match.groups())
    count = len(expected)
    summary = (len(summaries) == 1 and re.fullmatch(
        rf'test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?',
        summaries[0],
    ))
    if (len(records) != count or {name for name, _ in records} != expected
            or any(status != 'ok' for _, status in records) or not summary):
        raise ValueError('packed preflight requires every named test to execute successfully')
