# Original noise-stream witness

`original_noise_stream.json` embeds the unmodified
`tests/core/test_weighted_noise.py` source from
[Magna](https://github.com/itpyi/magna), commit
`56847478c840395735146ef1e1ccdc3d8aedc10f`.
That embedded source is covered by the Mozilla Public License 2.0, reproduced
in `original_noise_stream.LICENSE`. Its exact source digest is retained in the
fixture and checked against the pinned reference manifest. The source is data;
the generator executes only the authenticated original noise-stream class AST,
without importing the upstream test module or its build-capable helper.

The portable numerical witness records the exact sequential range requests,
results, caller channel choices, faults, readout flips, probabilities and
likelihoods for the original four configurations and 480 draws. Python random
state equality is checked during generation; Rust replays the explicit range
source protocol and per-draw tape positions. Host binary, producer and runtime
identities are recorded in a separate capture receipt.
