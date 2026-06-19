# Issue 94 rbposd BP Config Design

## Context

`rbposd::DecoderConfig` already exposes `bp_variant: BpVariant` and
`schedule: Schedule`, but both enums currently have one variant:
`BpVariant::MinimumSum` and `Schedule::Parallel`. That makes the default
contract explicit but does not let callers select the next upstream `ldpc`
method and schedule names. Issue #94 asks for at least one additional BP method
and one additional BP schedule while keeping `DecoderConfig::default()` stable.

## Decision

Extend the existing public enum types instead of adding a second set of config
fields:

- Add `BpVariant::ProductSum`.
- Add `Schedule::Serial`.
- Keep `DecoderConfig` field names unchanged.
- Keep `DecoderConfig::default()` as `MinimumSum` and `Parallel`.

This preserves existing source compatibility and makes the new selections part
of the same public Rust config surface that downstream callers already use.

## BP Runner Behavior

Add a private BP dispatch entrypoint that matches on `(bp_variant, schedule)`
before invoking the current compiled BP kernel. Because issue #94 explicitly
leaves actual BP algorithm changes out of scope, the new variants are accepted
and routed through the existing MVP kernel for now. The dispatch point prevents
the decoder from continuing to hard-code the public surface at call sites and
creates the narrow place where later ProductSum or Serial implementations can
replace the compatibility path.

`BpOsdDecoder` and `BpLsdDecoder` should call the selector-aware entrypoint
through `BpCore`, not a method named only for minimum-sum.

## Tests

Update `rbposd/tests/smoke.rs` with two contract tests:

- `decoder_config_exposes_bp_method_and_schedule_variants` constructs
  configs using `BpVariant::ProductSum` and `Schedule::Serial`, proving the
  variants are public and usable through `DecoderConfig`.
- `decoder_config_defaults_do_not_silently_change` pins the default BP method
  and schedule to `MinimumSum` and `Parallel`.

Keep the existing broader `decoder_config_default_contract` assertions.

## Scope

In scope:

- `rbposd/src/config.rs`
- `rbposd/src/lib.rs` if public exports need adjustment
- `rbposd/src/bp.rs`
- `rbposd/src/decoder_core.rs`
- `rbposd/src/decoder.rs`
- `rbposd/src/lsd_decoder.rs`
- `rbposd/tests/smoke.rs`

Out of scope:

- mathematically distinct ProductSum updates
- a true serial message-update schedule
- `rsinter` runner parameter parsing
- Python differential harness updates

## Review Notes

The spec is complete, keeps one compatibility-preserving public API choice, and
limits behavior changes to selector-aware routing plus public contract tests.
