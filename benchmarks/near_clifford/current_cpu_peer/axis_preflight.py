"""Peer-only native guard contract for the immutable c095/c49 axis pair."""
import re

PROFILE = 'axis-cache-c095-c49-v1'
BASELINE = 'c095bcac7aa78dc3a17b516c4c2781df36b530f2'
CANDIDATE = 'c49a71de7cf1e59b9ec0d4bb0efd1d499c286852'
PUBLIC = 'wide_cached_amplitudes_preserve_noise_feedback_counts_records_and_rng'
LAYOUT = 'near_clifford::compiled::row_random_log_cache_tests::scalar_cache_adds_at_most_one_inline_word_and_no_dynamic_storage'
SELECTIONS = (
    ('--lib', 'phase_specialized_cdf_tests'),
    ('--test', 'near_clifford_postselected_counts', PUBLIC, '--', '--exact'),
    ('--lib', 'near_clifford::compiled::cached_coefficients::tests::'),
    ('--lib', LAYOUT, '--', '--exact'),
)
NAMES = (
    (
        'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::i_pow_modulo_literals_keep_exact_components_for_all_u8_values',
        'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::scalar_cdf_preserves_all_unsigned_modulo_phase_aliases',
        'near_clifford::compiled::coherent_packet::phase_specialized_cdf_tests::phase_specialized_scalar_and_packet_cdfs_match_independent_frozen_bits',
    ),
    (PUBLIC,),
    (
        'near_clifford::compiled::cached_coefficients::tests::axis_storage_preserves_every_component_bit_and_tail',
        'near_clifford::compiled::cached_coefficients::tests::mixed_components_and_small_vectors_keep_dense_storage',
        'near_clifford::compiled::cached_coefficients::tests::original_default_cultivation_uses_axis_storage_with_exact_rows_and_rng',
    ),
    (LAYOUT,),
)


def require_heads(heads):
    expected = dict(baseline=BASELINE, candidate=CANDIDATE, control=BASELINE)
    if heads != expected:
        raise ValueError('axis preflight requires the exact reviewed c095/c49 source roles')


def check(index, log):
    if type(index) is not int or not 0 <= index < len(NAMES):
        raise ValueError('axis preflight native-check index differs')
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
                raise ValueError('axis preflight test record is malformed')
            records.append(match.groups())
    count = len(expected)
    summary = (len(summaries) == 1 and re.fullmatch(
        rf'test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out(?:; finished in [0-9]+(?:\.[0-9]+)?s)?',
        summaries[0],
    ))
    if (len(records) != count or {name for name, _ in records} != expected
            or any(status != 'ok' for _, status in records) or not summary):
        raise ValueError('axis preflight requires every named test to execute successfully')
