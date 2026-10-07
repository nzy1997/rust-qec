# Rust-only affine counts ablation

These original exploratory receipts compare packed counts with affine counts on
the same M4 host. They use four originals, separate Strict/Fused, 1/64/1024 shots,
five paired process rounds and seven observations of at least 50 ms per process.
Each of the 24 cells has both routes checked against complete structured and flat
records plus 16 subsequent RNG words at four seeds before timing. All 288 events,
the original before/after closure, identities and derived medians are retained.

The recorded candidate checkout was dirty. `bindings.json` documents independently
checked byte equivalence of every tracked measured input to immutable Git commit
3ef49cf9c3f156a492682d24e392915510f2a3a8; the packed source is equivalent to
1ccb5e92a330ed42efab7323aef4f2daeeaa1802. Both remain reachable through the retained
benchmark-source producer tags. All seven untracked probe inputs and the actual
original driver are retained byte-for-byte. The original driver contains the
collector's absolute workspace paths; they are provenance, not portable paths.

To reproduce, create separate checkouts at the two retained commits, copy `probe/`
to each checkout's ignored `drafts/counts-path-ablation/`, build its locked release
Cargo manifest, and run the original driver after adapting its roots and output
directory to those checkouts. The driver hashes all source/probe/binary inputs
before and after each closed campaign. Do not run concurrent local workloads.
No binary artifact or claim of an independent reproducible native build is included.

Warmups exclude affine preparation cost in this exploratory comparison. It cannot
establish a cold-call advantage or a peer/SOTA lead. Source-bound formal v5 peer
comparisons and cold first-call medians are retained separately in
[the affine campaign](../apple-m4-affine-counts-2026-10-08/README.md).

`summary.json` reports roughly 3× surface bulk gains and 19–22× warm one-shot gains;
it also retains MSC5 Fused1024 about 2.4% regression (all paired ratios below one)
and Strict64 about 4.6% regression. Paired ranges are process-median ranges, not
confidence intervals. The M4 host is shared and unpinned.
