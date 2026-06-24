# Issue 145 APM Searcher Integration Tracking Design

Issue: #145 Track APM searcher integration after fixed Table A1 instances land

## Context

The fixed APM Table A1 path is now the production path for the near-term
reproduction. Issue #140 registered `apm_kasai:p=96`, and issue #143
registered `apm_kasai:p=192` with structural acceptance coverage. Those issues
closed before this tracking pass, and their merged PRs (#158 and #160) expose
both instances through `qec-code code css` and `built_in_css_checks`.

The current native implementation builds sparse CSS matrices from pinned
`ApmCssManifestEntry` constants in `qec-code/src/codes/apm.rs` and
`qec-code/src/codes/built_in_css.rs`. The implementation contract in
`qec-code/doc/apm_css.md` already defines the affine-map invariants that future
search/import output must satisfy: unit affine slopes, active-row dimensions,
commutation residual checks, noncommuting controls, sorted sparse rows, regular
weights, and CSS orthogonality.

The issue references the Kasai APM-CSS construction and search approach in
arXiv:2601.08824, which describes using controlled commutativity and active
orthogonality to construct regular quantum LDPC codes. The issue also points to
a local reference clone under `drafts/construct_apm_css_code`, but that ignored
draft path is not present in this worktree. The future implementation should
treat that clone as reference material, not as a production dependency.

## Approaches Considered

1. Port the C++ searcher directly into native Rust.

   This could eventually give the best integrated developer experience, but it
   is the broadest first step. It risks mixing search heuristics with the
   already-stable fixed-instance generator and would make it harder to preserve
   the P=96/P=192 path as the production baseline.

2. Add a development-only wrapper around the reference C++ program.

   This would preserve the reference implementation's behavior and may be
   useful while comparing search results. It also depends on an ignored local
   draft checkout, compiler availability, and command-line behavior outside the
   Rust workspace. That makes it a poor production interface.

3. Start with a manifest import and validation path.

   This is the selected future path. It lets any searcher, wrapper, or manual
   candidate emit a small manifest entry, then reuses the native affine
   validator and generator before matrix construction. It keeps the fixed Table
   A1 built-ins untouched and gives the first split issue a focused acceptance
   test.

## Chosen Design

Track APM searcher integration as a staged roadmap, not as a searcher in this
issue. The first implementation split should add a tiny manifest import path
that accepts search output equivalent to an `ApmCssManifestEntry`, validates it,
and feeds it into the existing native generator. Search execution can remain
outside production until the project has evidence that a wrapper or native
searcher is worth carrying.

The future input contract should include:

- `P`, `J`, `L`
- `f` and `g` affine maps with explicit slope and offset values
- required noncommuting pairs
- optional required commuting pairs or cycle/Psi checks
- search provenance such as try limits, random seed, reference command, and
  reference-code revision when available

The future output contract should be a validated manifest entry plus provenance
metadata. A successful import must be able to call the native APM builder and
produce sparse rows with the same invariants used by the fixed Table A1
instances.

## Split-Issue Roadmap

The first child issue should implement a manifest import round trip and nothing
else. It should add a focused test named
`apm_search_tiny_case_round_trips_to_manifest` and run:

```sh
cargo test -p qec-code apm_search_tiny_case_round_trips_to_manifest -q
```

That test should generate or import a tiny known-valid APM case, validate its
affine constraints, and feed it into the native generator. The negative control
must reject a non-unit slope before matrix generation and reject a required
noncommuting pair that accidentally commutes.

The second child issue can add a development-only wrapper around
`drafts/construct_apm_css_code/apm_g8_mod.cpp` if the local reference clone is
available. The wrapper should emit the same importable manifest format instead
of introducing a separate matrix path.

Only after the import path and wrapper contract are stable should a later issue
consider a native Rust searcher. That issue should justify which heuristic
state, reuse tables, cycle/Psi checks, and stopping criteria are part of a
maintained Rust API.

## Production Boundary

The fixed `apm_kasai:p=96` and `apm_kasai:p=192` built-ins remain the production
source of truth for the current reproduction. Search/import features must not
replace or silently mutate those built-ins. New APM candidates should use
distinct code ids, explicit provenance, and validation before matrix generation.

## Acceptance Criteria For This Tracking Issue

- Add a durable project document describing the searcher integration roadmap.
- The document must recommend manifest import before wrapper or native searcher
  work.
- The document must preserve the fixed P=96/P=192 generator as the production
  path.
- The document must include the first child issue's concrete verification
  command and negative controls.
- Link the new tracking document from the existing APM construction contract.
- Do not add a searcher, wrapper, stochastic benchmark, or new public runtime
  API in this issue.

## Verification

This issue is documentation-only. The repository-level verification remains:

```sh
cargo test
```

## References

- Issue #140 and PR #158: P=96 registry and CLI path.
- Issue #143 and PR #160: P=192 registry and acceptance path.
- `qec-code/doc/apm_css.md`: current APM construction contract.
- arXiv:2601.08824: <https://arxiv.org/abs/2601.08824>.
- arXiv:2604.16209: <https://arxiv.org/abs/2604.16209>.
