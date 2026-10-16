# Repository Guidelines

## Project Structure & Module Organization
This repository is a Cargo workspace with two Rust crates and one Typst package. `rstim/` contains the simulator, CLI, code generators, QP101 export, and most circuit logic; key submodules live under `rstim/src/codegen/` and `rstim/src/sim/`. `rsinter/` builds analysis and reporting tools on top of `rstim`. Integration tests are organized by feature in `rstim/tests/` and `rsinter/tests/` with file names such as `cli_export_json.rs` or `dem_ir.rs`. `qp101-viz/` is a local Typst package: `lib.typ` is the entrypoint, `examples/` holds committed demos, and `checks/` holds renderer fixtures. Scratch material belongs in ignored paths like root `drafts/`, `qp101-viz/drafts/`, and `qp101-viz/draft_figs/`.

## Agent Rules
Use `.AGENTS/rules/visualization-update-flow.md` for the end-to-end sync process when QP101 export changes affect docs, fixtures, and Typst rendering. Use `.AGENTS/rules/QP101-ZY.md` for the narrower rule that treats `rstim/doc/QP101-ZY.md` as the format contract and lists the exact files that must be kept in sync with it.

## Build, Test, and Development Commands
- `cargo build --workspace`: build both Rust crates.
- `cargo test --workspace`: run the full Rust test suite.
- `cargo test -p rstim --test qp101_highlights`: run one focused integration test file.
- `cargo run -p rstim -- <subcommand>`: use the CLI locally, for example `cargo run -p rstim -- export_json --help`.
- `cargo run -p rstim --example stim_parity_showcase`: reproduce the parity showcase from the README.
- `make -C qp101-viz`: compile all committed Typst examples to PDFs.
- `typst compile --root qp101-viz qp101-viz/examples/<file>.typ /tmp/out.pdf`: smoke-test one visualization example.

## Coding Style & Naming Conventions
Use Rust 2024 defaults and keep code `rustfmt`-clean with 4-space indentation. Prefer `snake_case` for functions, modules, files, and tests; use `CamelCase` for types. Name tests after behavior, not implementation, for example `qp101_export_marks_unambiguous_observable_symptom_highlights`. Keep CLI- and feature-specific tests in matching files (`cli_detect.rs`, `tracked_dem.rs`). In `qp101-viz`, keep `.typ` example names kebab-case and paired `.qp101.json` fixtures stable.

## Testing Guidelines
Every behavior change should update or add an integration test in the owning crate. Start with the narrowest useful command, then widen to crate- or workspace-level checks. For visualization work, compile at least one touched Typst example or check before claiming success. Do not commit scratch fixtures from ignored directories.

## Commit & Pull Request Guidelines
Recent commits use short imperative subjects, sometimes with prefixes such as `feat:`, `fix:`, or `docs:`. Follow that pattern and keep commits scoped to one subsystem when possible; avoid mixing `rstim` logic changes with `qp101-viz` asset churn unless they are directly linked. Pull requests should summarize behavior changes, list the exact verification commands run, and include a screenshot or generated PNG/PDF when renderer output changes.
