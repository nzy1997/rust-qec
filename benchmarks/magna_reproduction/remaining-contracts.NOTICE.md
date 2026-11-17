# Outstanding Magna method requirements

`remaining-contracts.json` contains source-derived assertion, loop, helper-name
and import summaries from `tests/core/` in
[itpyi/magna](https://github.com/itpyi/magna), revision
`56847478c840395735146ef1e1ccdc3d8aedc10f`.
The original methods and their per-module SHA-256 identities are recorded in
the catalog. The summaries have been extracted through Python's AST; the
classification, Rust ownership notes and issue dependencies are additions.

This Source Code Form is subject to the terms of the Mozilla Public License,
v. 2.0. The full license is distributed in
[`original_many_record_input.LICENSE`](../../rmagna/tests/fixtures/original_many_record_input.LICENSE).
The catalog is requirement data, not executable upstream simulator code or
a claim of completed Rust parity.

`standalone-contracts.json` additionally retains the helper identities, assertion
locations and hashes, loop headers and local helper calls from
`tests/native/causal.cpp` and `tests/native/foundation.cpp` at the same revision.
The original source-file SHA-256 identities are in `reference.json` and the
catalogue. The semantic group boundaries, candidate Rust owners, unresolved
obligations and conservative lexical discovery/checking tools are additions.
This catalogue has the same MPL-2.0 source attribution and license as above.
It is a requirements decomposition, not an executable parity qualification.
