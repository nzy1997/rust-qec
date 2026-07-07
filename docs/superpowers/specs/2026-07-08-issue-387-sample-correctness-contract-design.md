# Issue 387 Sample Correctness Contract Design

## Objective

Add an internal `rstim` correctness contract that compares the compiled and
interpreted sampler paths on the shared simulator fixture catalog from issue
#385. The test must validate fixture metadata, make routing decisions visible,
and reject deterministic mismatch controls without involving Stim.

## Selected Approach

Use one focused Rust integration test module:

```text
rstim/tests/sample_correctness_contract.rs
```

The test loads `benchmarks/rstim_vs_stim_simulator/cases.smoke.toml` with a
TOML parser, reads each checked `.stim` input, parses it through `rstim`, and
checks the fixture metadata against `rstim::stats::summarize`. Executable
catalog entries are sampled through both `SamplingBackend::Interpreted` and
`SamplingBackend::Compiled` with a deterministic seed list. Documentation-only
entries are metadata-checked but not sampled, preserving the catalog's stated
contract that the large d=11 case is present for provenance without implying a
fast smoke run.

Alternatives considered:

- Extending `rstim::perf` to own catalog parsing. This would create production
  API surface for a test-only contract and broaden the change unnecessarily.
- Adding a Python verifier. That belongs to issue #386, which compares Stim and
  `rstim`; this issue is explicitly internal to `rstim`.
- Duplicating fixture circuits in Rust. That would drift from #385 and weaken
  the shared-catalog contract.

## Contract Details

The Rust test uses typed manifest structs for the fields needed by this issue:
`case_id`, `tier`, `canonical_input_path`, `shots`, `expected_qubits`,
`expected_measurements`, `expected_detectors`, and `expected_observables`.

For every catalog case, the test:

- parses the canonical `.stim` file with `rstim::parser::parse_lines`;
- compares `rstim::stats::summarize` against the manifest metadata;
- reports `metadata mismatch` if any expected count is wrong.

For executable cases, the test then:

- compiles the parsed circuit and calls `choose_sampler_path`;
- if the compiled sampler can run, samples both `SamplingBackend::Interpreted`
  and `SamplingBackend::Compiled`;
- compares detector and observable comparison streams across deterministic
  seeds and reports `statistical mismatch` on any disagreement;
- if the compiled sampler cannot run, records the explicit fallback reason and
  requires that reason to be non-empty.

The deterministic seed list is local to the test. It is not exposed as a public
API and is intentionally independent from Stim.

## Negative Controls

The same test module includes two direct negative controls:

- A valid catalog output stream is cloned, one detector or observable bit is
  flipped in one backend's comparison stream, and the agreement assertion must
  reject it with `statistical mismatch`.
- The smoke manifest is loaded and one fixture's expected detector count is
  intentionally changed in memory. The metadata assertion must reject it with
  `metadata mismatch`.

These controls prove the contract can fail for both sample-path disagreement
and bad fixture metadata.

## Dependencies

Add `toml` as an `rstim` dev-dependency. This keeps catalog loading structured
without introducing runtime dependencies or adding a custom TOML parser.

## Scope Limits

This change does not optimize compiled sampling, add Stim comparisons, add
benchmark result artifacts, or change production sampler behavior.

## Self-Review

- No unresolved marker text remains.
- The design uses the checked fixture catalog from #385 and does not duplicate
  circuit definitions.
- Fallback routing is explicit and non-silent.
- The negative controls cover both required failure substrings.
