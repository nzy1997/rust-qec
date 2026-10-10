"""Offline negative tests for clock binding and exact sample-window accounting."""
import copy
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import warm_profile_contract as contract


def raw_attr():
    data = bytearray(248)
    data[:8] = b'PERFILE2'
    struct.pack_into('<4Q', data, 8, 104, 144, 104, 144)
    struct.pack_into('<IIQ', data, 104, 1, 128, 0)
    struct.pack_into('<Q', data, 128, (1 << 0) | (1 << 1) | (1 << 2) | (1 << 7))
    struct.pack_into('<Q', data, 144, (1 << 25) | (1 << 5))
    struct.pack_into('<i', data, 196, 1)
    return data


def phase():
    return dict(schema='rstim.perf-warm-loop.v1', completed=True, pid=77,
                acknowledgements=dict(enable=[97, 99, 107, 10, 0], disable=[97, 99, 107, 10, 0]),
                clock='CLOCK_MONOTONIC', start_ns=10_000_000_005, end_ns=10_000_000_009)


def sample(nanos, comm='probe.bin', pid=77, cpu=2, symbol='actual_symbol'):
    return (f'{comm} {pid}/{pid} [00{cpu}] 10.{nanos:09d}: cpu-clock:u:\n'
            f'    1234 {symbol} (/native/probe.bin)\n\n')


def inline_sample(nanos, symbol='actual_symbol', dso='/native/probe.bin'):
    return f'probe.bin 77/77 [002] 10.{nanos:09d}: cpu-clock:u: 1234 {symbol} ({dso})\n\n'


class WarmContractTests(unittest.TestCase):
    def test_record_tool_path_accepts_retained_argv0_without_assuming_record_arguments(self):
        for suffix in [' ', ' record -e cpu-clock:u']:
            self.assertEqual(contract.record_perf_path('# cmdline : /usr/lib/linux/perf' + suffix + '\n'),
                             Path('/usr/lib/linux/perf'))
        for text in ['# cmdline : perf record\n', '# cmdline : /usr/lib/linux/not-perf\n',
                     '# cmdline : /usr/lib/linux/perf\n# cmdline : /usr/lib/linux/perf\n']:
            with self.assertRaises(ValueError):
                contract.record_perf_path(text)

    def test_raw_attribute_requires_explicit_monotonic_clock(self):
        self.assertEqual(contract.perf_clock(raw_attr())['clock_id'], 1)
        for offset, format, value in [(196, '<i', 4), (144, '<Q', 1 << 5),
                                       (128, '<Q', 1 << 0), (104, '<I', 0),
                                       (16, '<Q', 95), (32, '<Q', 288)]:
            data = raw_attr()
            struct.pack_into(format, data, offset, value)
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                contract.perf_clock(data)

    def test_every_sample_is_retained_or_explicitly_accounted_at_exact_boundaries(self):
        text = '# header preserved in raw export\n' + ''.join(sample(n) for n in [4, 5, 8, 9])
        selected, result = contract.select_samples(text, phase(), 2, 77, '/native/probe.bin', minimum=2)
        self.assertEqual(selected, sample(5) + sample(8))
        self.assertEqual([row['region'] for row in result['samples']], ['before', 'warm', 'warm', 'after'])
        self.assertEqual(result['counts']['total'], 4)
        self.assertEqual(result['counts']['mapped'], 2)
        self.assertFalse(result['performance_valid'])

    def test_delayed_record_allows_one_non_sampling_dummy_tracker(self):
        data = raw_attr()
        data += data[104:248]
        struct.pack_into('<Q', data, 32, 288)
        struct.pack_into('<Q', data, 248 + 8, 9)
        struct.pack_into('<Q', data, 248 + 16, 1)
        result = contract.perf_clock(data)
        self.assertEqual([row['config'] for row in result['attributes']], [0, 9])
        struct.pack_into('<Q', data, 248 + 40, (1 << 25) | (1 << 10))
        with self.assertRaises(ValueError):
            contract.perf_clock(data)

    def test_missing_leaf_remains_in_warm_population(self):
        selected, result = contract.select_samples(sample(6, symbol='[unknown]'), phase(), 2, 77,
                                                   '/native/probe.bin', minimum=1)
        self.assertIn('[unknown]', selected)
        self.assertEqual(result['counts']['missing_leaf'], 1)

    def test_delayed_dummy_accepts_inherited_frequency_but_rejects_unrelated_parameters(self):
        data = raw_attr()
        struct.pack_into('<Q', data, 104 + 16, 499)
        struct.pack_into('<Q', data, 104 + 40, (1 << 25) | (1 << 5) | (1 << 10))
        data += data[104:248]
        struct.pack_into('<Q', data, 32, 288)
        struct.pack_into('<Q', data, 248 + 8, 9)
        result = contract.perf_clock(data)
        self.assertEqual([row['period'] for row in result['attributes']], [499, 499])
        for offset, value in [(248 + 16, 498), (248 + 16, 0),
                              (248 + 40, (1 << 25) | (1 << 5)),
                              (104 + 40, (1 << 25) | (1 << 5))]:
            bad = bytearray(data)
            struct.pack_into('<Q', bad, offset, value)
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                contract.perf_clock(bad)
        # Recognizing tracker attributes never admits dummy samples into the
        # selected population, including samples outside the warm interval.
        for nanos in [4, 6, 9]:
            with self.assertRaises(ValueError):
                contract.select_samples(sample(nanos).replace('cpu-clock:u', 'dummy:u'),
                                        phase(), 2, 77, '/native/probe.bin', minimum=1)

    def test_mixed_unrecognized_blocks_cannot_enter_the_selection_or_evade_the_ledger(self):
        good = sample(6) * 100
        bad_blocks = [sample(9, comm='foreign task', pid=88), sample(9, comm='# foreign', pid=88),
                      sample(9, comm='#foreign', pid=88), sample(9, pid=-1),
                      sample(9).replace('10.000000009', 'malformed-time')]
        for bad in bad_blocks:
            for text in [bad + good, sample(6) * 50 + bad + sample(6) * 50, good + bad]:
                with self.subTest(bad=bad, placement=text.index(bad)), self.assertRaises(ValueError):
                    contract.select_samples(text, phase(), 2, 77, '/native/probe.bin')

    def test_undelimited_sample_cannot_be_accepted_as_a_callchain_frame(self):
        text = sample(6).rstrip('\n') + '\n' + sample(9, comm='foreign task', pid=88)
        with self.assertRaises(ValueError):
            contract.select_samples(text, phase(), 2, 77, '/native/probe.bin', minimum=1)

    def test_inline_leaf_fallback_preserves_exact_blocks_and_all_three_time_regions(self):
        before = inline_sample(4) + inline_sample(4, symbol='[unknown]', dso='[unknown]')
        warm = sample(6) * 100 + inline_sample(6) + inline_sample(6, symbol='[unknown]', dso='[unknown]')
        after = inline_sample(9) + inline_sample(9, symbol='[unknown]', dso='[unknown]')
        selected, result = contract.select_samples(before + warm + after, phase(), 2, 77, '/native/probe.bin')
        self.assertEqual(selected, warm)
        self.assertEqual(result['counts'], dict(total=106, warm=102, before=2, after=2,
                                               native=102, mapped=101, missing_leaf=1))
        self.assertEqual(len(result['samples']), 106)

    def test_inline_leaf_support_does_not_admit_arbitrary_tails_or_a_second_header(self):
        good = sample(6) * 100
        for tail in ['garbage', ' 1234 symbol',
                     ' 1234 88/88 [002] 10.000000009: cpu-clock:u: evil (/native/probe.bin)']:
            bad = 'probe.bin 77/77 [002] 10.000000006: cpu-clock:u:' + tail + '\n\n'
            for text in [bad + good, good + bad]:
                with self.subTest(tail=tail), self.assertRaises(ValueError):
                    contract.select_samples(text, phase(), 2, 77, '/native/probe.bin')
        # Even an all-hex comm must not look like a stack-frame address when
        # a broken export omits the sample separator.
        bad = '    deadbeef 88/88 [002] 10.000000009: cpu-clock:u: 1234 evil (/native/probe.bin)\n\n'
        with self.assertRaises(ValueError):
            contract.select_samples(sample(6).rstrip('\n') + '\n' + bad, phase(), 2, 77,
                                    '/native/probe.bin', minimum=1)

    def test_phase_and_sample_identity_mismatches_fail_closed(self):
        for key, value in [('completed', False), ('pid', 78), ('clock', 'CLOCK_MONOTONIC_RAW'),
                           ('start_ns', True), ('end_ns', 10_000_000_005)]:
            receipt = phase()
            receipt[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                contract.select_samples(sample(6), receipt, 2, 77, '/native/probe.bin', minimum=1)
        for text in [sample(6, pid=78), sample(6, cpu=3), sample(6, comm='python3'),
                     sample(6).replace('.000000006', '.000006'), sample(6).replace('cpu-clock:u', 'cycles:u')]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                contract.select_samples(text, phase(), 2, 77, '/native/probe.bin', minimum=1)

    def test_partial_or_malformed_perf_acknowledgements_cannot_certify_a_window(self):
        for value in [None, {}, {'enable': [97, 99, 107, 10, 0]},
                      {'enable': [97, 99, 107, 10, 0], 'disable': [97, 99, 107, 10]},
                      {'enable': [97, 99, 107, 10], 'disable': [97, 99, 107, 10, 0]},
                      {'enable': [97, 99, 107, 10, 0], 'disable': [97, 99, 107, 10, 120]},
                      {'enable': [97, 99, 107, 10, False], 'disable': [97, 99, 107, 10, 0]}]:
            receipt = phase()
            receipt['acknowledgements'] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                contract.select_samples(sample(6), receipt, 2, 77, '/native/probe.bin', minimum=1)

    def test_finite_counts_equality_ignores_only_diagnostic_timing_fields(self):
        original = dict(backend='rstim', status='ok', input_sha256='abc', arithmetic='strict', shots=8192,
                        call_shots=64, width=1, measurements=[0], exact_native_counts_rng=True,
                        compile_ns=100, peak_rss_bytes=200, observations=[dict(calls=128, attempted=8192,
                        accepted=6000, discarded=2192, logical_errors=10, elapsed_ns=100, ns_per_call=1)])
        other = copy.deepcopy(original)
        other['compile_ns'] += 10
        other['peak_rss_bytes'] += 10
        other['observations'][0]['elapsed_ns'] += 10
        self.assertEqual(contract.finite_payload(original, 'counts'), contract.finite_payload(other, 'counts'))
        other['measurements'][0] = 1
        self.assertNotEqual(contract.finite_payload(original, 'counts'), contract.finite_payload(other, 'counts'))


if __name__ == '__main__':
    unittest.main()
