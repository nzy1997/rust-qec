# Issue 97 rbposd BP Teeth Harness Design

## Context

Issues #95 and #96 are merged into `master`: `rbposd` now executes
`product_sum` and `serial` BP paths, and `rsinter` accepts and records those
settings for benchmark runs. Issue #97 is the verification layer that prevents
those settings from becoming cosmetic knobs. It needs deterministic Rust teeth
tests that show non-default BP settings change decoder behavior and Python
parity-harness tests that keep the same option names aligned with upstream
`ldpc` kwargs.

The existing `rbposd` parity fixture
`bp_product_sum_serial_sensitive.json` already documents a small chain case
whose public decode result changes under `product_sum + serial`. The existing
Python harness still maps only `minimum_sum + parallel` into upstream `ldpc`
kwargs and rejects other BP settings.

## Approach

Use the existing parity fixture and harness layers rather than adding a new
fixture format or decoder family.

1. Add a Rust integration test named `product_sum_serial_teeth_cases` in
   `rbposd/tests/bp.rs`. It will load the sensitive parity fixture, verify its
   expected `product_sum + serial` result, and then mutate only one BP option at
   a time to prove that `product_sum` and `serial` are behavior-affecting
   decoder selectors on the documented case.
2. Extend `rbposd/scripts/parity_harness.py` so the BP config-to-`ldpc` mapping
   accepts exactly the BP options implemented by this milestone:
   `minimum_sum`, `product_sum`, `parallel`, and `serial`.
3. Keep unsupported options explicit. Unknown BP methods or schedules still
   raise `ValueError` instead of falling back to the default path.
4. Reuse the same BP option mapper for OSD and LSD parity-harness kwargs so the
   shared `rbposd` BP config stays represented consistently in the differential
   comparison layer.

## Rejected Options

- Adding a second non-default fixture would broaden maintenance without adding
  much evidence, because the existing fixture is already the borrowed sensitive
  case from #95 and exercises the public parity runner.
- Testing only `product_sum + serial` against `minimum_sum + parallel` would
  prove the combined mode changes behavior but would not give separate teeth to
  the method and schedule selectors.
- Silently mapping unsupported BP values to upstream defaults would make the
  harness easier to run but would hide exactly the configuration drift this
  milestone is meant to catch.

## Error Handling

The Python harness remains strict:

- unsupported `bp_variant` raises `Unsupported bp_variant...`
- unsupported `schedule` raises `Unsupported schedule...`
- unsupported `early_stop` values remain rejected
- unsupported OSD and LSD options keep their current explicit errors

The Rust teeth test should fail with a direct assertion message if either
selector stops changing behavior on the sensitive fixture.

## Testing

Run the issue commands:

- `cargo test -p rbposd product_sum_serial_teeth_cases`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py -k bp_method`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported_schedule`

Also run the broader gates required by Agent Desk and the Superpowers finish
workflow:

- `cargo test -p rbposd`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py`
- `cargo test`
- `git diff --check`

## Scope

In scope:

- `rbposd/tests/bp.rs`
- `rbposd/scripts/parity_harness.py`
- `rbposd/scripts/test_parity_harness.py`
- Superpowers design and implementation plan notes for issue #97

Out of scope:

- new decoder families
- benchmark plot redesign
- full benchmark suite expansion
- additional upstream `ldpc` options beyond `product_sum` and `serial`

## Review Notes

This design keeps the harness narrow and grounded in the behavior already
implemented by #95. The Rust side proves the public decoder result changes; the
Python side proves those same setting names are handed to upstream `ldpc`
without changing the rejection policy for unsupported modes.
