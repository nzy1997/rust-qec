# BH reference inputs and exact values

`bh-code-reference.json` records code rows derived from the literal encoding
prefixes in `examples/bh{k}-T/physical.magna` of
[itpyi/magna](https://github.com/itpyi/magna), revision
`56847478c840395735146ef1e1ccdc3d8aedc10f`. It includes the complete BH2 encoding
prefix and preserves original source paths and SHA-256 identities.

Published exact acceptance/infidelity values are from `data/bh-curves.json` and
the available `exact_reference` entries of `data/bh-k100.json` in the
[paper v1 deposit](https://doi.org/10.5281/zenodo.23386778). Their complete member
identities are pinned in `reference.json` and this fixture. Unavailable larger-k
exact-reference entries remain null; they are not invented published values.
The independent dual-code sums, binomial symmetry check, full small-instance
fault enumeration and Rust runner are additions. No original simulator algorithm
or historical Monte Carlo output is substituted for new Rust production draws.

The source-derived circuit material is subject to MPL-2.0. A copy is distributed
in `rmagna/tests/fixtures/original_many_record_input.LICENSE`. This fixture is
small, intentional verification data; production observations remain external.

`bh-checker-reference.json` retains selected fields from one actual 2,048-draw
Rust execution and its three small input texts. It is a verifier regression
fixture, not the complete BH campaign or a historical Native result. Its
executable/compiled-source identities and exact sufficient statistics are
preserved; source-derived input texts have the same attribution above.
