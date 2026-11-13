# Qude dependency source snapshot

This maintenance branch prepares the coordinated seven-package RustQEC 0.4.0
series from source baseline `d84005b577f3a9edd8572c1995f9d3115e5ddd16` for Qude's
registry distribution. Simulator, code and decoder runtime source files remain
byte-identical to that baseline. Package metadata, internal version requirements,
installation examples and lockfile identities are updated together.

## Why this is not a 0.3.x patch

Relative to published 0.3.0 (source commit
`c5b23bf02a10724b0b377214c76c26605f23b390`), public `CompiledOp::Measure` and
`MeasureReset` gain the required `readout_probability` field. Existing named-field
construction and exhaustive field patterns must be updated. Set this probability
to `0.0` to retain an ideal readout in code that previously omitted the field.

Archive types and public archive support modules now require `archive-zstd` when
building without default features. Consumers using `SampleArchiveReader`,
`SampleArchiveWriter` or `ArchiveSummary` must enable that feature explicitly.
The public plotting-enabled command enum also gains a variant. These changes
require the incompatible 0.4 release line; Cargo must not automatically select
this snapshot for an existing `rstim = "0.3.0"` dependency.

The existing CLI command variants remain available through legacy dispatch. New
unified entry points and expanded defaults are described by the current feature
catalog. Qude uses its explicitly selected library/runners; verification of that
use does not certify every optional CLI, research, solver or visualization path.

## Publication sequence and remaining gates

The repository's series policy moves `qec-ilp-core`, `qec-code`, `rmatching`,
`rbposd`, `rstim`, `rilpqec` and `rsinter` together to 0.4.0. Publish in that
runtime dependency order. The matching crate's compatible development-only rstim
range permits packaging it before the new simulator version is uploaded; its
runtime dependency graph does not depend on rstim.

This branch is a candidate, not evidence of a public upload or a completed
repository-wide release. Actual clean archives, source-inventory comparisons,
downstream package installations, required checks and explicit final approval
must precede permanent uploads or tags. The source baseline has pre-existing
strict Clippy style failures; preserving it is not a lint waiver. Qude's ideal
synthetic workflows establish neither experimental fidelity nor a threshold.
