# APM Searcher Integration Roadmap

Issue #145 tracks future integration of an APM parameter searcher after the
fixed Table A1 instances have landed. The current production path is still the
native fixed-instance generator exposed as `apm_kasai:p=96` and
`apm_kasai:p=192`.

## Current Boundary

`qec-code` builds the fixed Table A1 instances from pinned affine constants and
validates their sparse-row output through the existing APM construction
contract. Search work must not replace or silently mutate those built-ins.

The reference C++ searcher named in the issue lives outside the tracked
workspace under `drafts/construct_apm_css_code/apm_g8_mod.cpp` when a local
draft checkout is available. Treat that clone as reference material. A
production command should not depend on the ignored draft directory.

## Recommended Split

Start with a manifest import path before adding a wrapper or native searcher.
That keeps the search algorithm outside the production generator while proving
that discovered candidates can enter the same validation and sparse-row build
path as the fixed Table A1 instances.

1. Manifest import tool.

   Accept a tiny APM candidate manifest, validate affine and commutation
   constraints, then feed it into the native generator. This should be the first
   concrete child issue.

2. Development-only reference wrapper.

   If the local Kasai reference clone is available, add a wrapper that runs the
   C++ tool and emits the same manifest format. Keep the wrapper out of the
   production fixed-instance path.

3. Native Rust searcher.

   Consider a native searcher only after the import format and wrapper
   provenance prove which search settings are stable enough to maintain:
   `P`, `J`, `L`, required noncommuting pairs, try limits, seeds, optional
   cycle/Psi checks, and any reusable learned state.

## Future Input And Output Contract

Input should include:

- `P`, `J`, and `L`
- affine `f` and `g` map families with explicit slope and offset values
- required noncommuting pairs
- optional required commuting pairs, cycle checks, or Psi checks
- search provenance such as seed, try limit, command line, and reference-code
  revision

Output should be a validated manifest entry compatible with the native APM
generator. The importer must reject invalid affine data before matrix
generation and should preserve enough provenance to reproduce the search or
wrapper command.

## First Child Issue Acceptance

The first implementation issue should provide this focused command:

```sh
cargo test -p qec-code apm_search_tiny_case_round_trips_to_manifest -q
```

The test should generate or import a tiny known-valid APM case, validate its
affine constraints, and feed it into the native generator.

Required negative controls:

- a search/import result with a non-unit affine slope is rejected before matrix
  generation
- a required noncommuting pair that accidentally commutes is rejected before matrix
  generation

## Non-Goals

- Do not port the full searcher as the first child issue.
- Do not make the ignored `drafts/` clone a production dependency.
- Do not add stochastic decoding or benchmark coverage as part of the import
  contract.
- Do not replace the fixed `apm_kasai:p=96` or `apm_kasai:p=192` built-ins.
