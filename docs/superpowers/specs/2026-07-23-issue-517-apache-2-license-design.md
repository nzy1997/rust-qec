# Issue 517: Apache-2.0 License Policy Design

Issue: #517

## Decision

The maintainer approved `Apache-2.0` as the license for the entire tracked
repository, including every Cargo workspace package and the non-workspace
`qp101-viz` package. The maintainer also confirmed authorization to apply this
policy to the repository's tracked contributions.

Untracked and ignored content under `drafts/` is not part of this policy.

## Repository License

Add the official Apache License 2.0 text as the root `LICENSE`. The root README
will name `Apache-2.0`, state that the policy covers all tracked repository
content, and identify the code and tests derived from Stim and PyMatching.

Stim and PyMatching are both distributed under Apache-2.0 and neither upstream
repository currently contains a `NOTICE` file. There is therefore no upstream
`NOTICE` text to reproduce. Existing source-level comments identifying ported
or adapted tests and implementation details will remain intact.

## Cargo Metadata

Add this inherited package property to the root workspace manifest:

```toml
[workspace.package]
license = "Apache-2.0"
```

Every workspace member will declare:

```toml
license.workspace = true
```

This applies to all eight current workspace packages, including the benchmark
bridge. No package's publishability will change.

## Existing License Statements

Replace the MIT statement in `rmatching/README.md` with Apache-2.0 and update
`qp101-viz/typst.toml` to declare `Apache-2.0`. The root README is the
repository-level source of truth; there are no local license exceptions.

## Verification

Run the issue's positive verification from a clean checkout:

- the root `LICENSE` is non-empty;
- Cargo metadata reports `Apache-2.0` for every publishable workspace package;
- exactly one resolved workspace policy exists;
- the root README names that policy; and
- the verification leaves the checkout clean.

Run the issue's negative control in a temporary archive after removing
`license.workspace = true` from `rbposd/Cargo.toml`. The metadata assertion
must fail and name `rbposd`, proving that the root license file alone does not
satisfy the package metadata contract.

Finally, run formatting checks and the full Cargo workspace test suite. This
change does not alter runtime behavior or public APIs.

## Out of Scope

- Relicensing untracked or ignored content.
- Changing crate publishability.
- Modifying decoder APIs, behavior, or benchmark results.
- Publishing any crate.
- Providing legal advice or resolving disputed ownership.
