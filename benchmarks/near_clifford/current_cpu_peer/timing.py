"""Seeded public counts invocation with explicit owned-result release."""
import time

TIMING_CONTRACT = 'seeded-counts-invocation-v1'
COUNT_FIELDS = ('attempted', 'accepted', 'discarded', 'logical_errors')
RUST_ROLES = ('baseline', 'rstim', 'control')


def timed_counts(sample, seed):
    # The seed value is selected by the caller before this clock. Backend RNG
    # construction and native-result conversion/release occur in sample(seed).
    start = time.perf_counter_ns()
    result = sample(seed)
    attempted = result['attempted']
    accepted = result['accepted']
    discarded = result['discarded']
    logical_errors = result['logical_errors']
    del result
    elapsed = time.perf_counter_ns() - start
    return elapsed, attempted, accepted, discarded, logical_errors
