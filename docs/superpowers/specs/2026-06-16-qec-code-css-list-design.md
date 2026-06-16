# QEC Code CSS List Command Design

Date: 2026-06-16
Status: Design approved in-session, written for review
Scope: GitHub issue #60, `qec-code code css list`

## Summary

Issue #60 asks `qec-code` to expose a discoverability command for built-in CSS
code specs:

```text
qec-code code css list
```

The command should print the supported fixed ids and parameterized family
shapes in a human-readable form. A cold reader should be able to copy one of
the listed specs into a valid matrix export command such as:

```text
qec-code code css steane hx
qec-code code css repetition_x:d=5 hx
```

This issue depends on the already-merged parser, repetition-family, and `bb72`
work from issues #57, #58, and #59. It should not add new code families,
machine-readable JSON catalog output, README updates, or benchmark changes.

## Goals

- Add `qec-code code css list`.
- List the currently supported built-in CSS fixed ids and family shapes:
  `steane`, `bb72`, `repetition_x:d=<distance>`, and
  `repetition_z:d=<distance>`.
- Include one short description or constraint line for each entry.
- Keep list output human-readable and stable enough for tests.
- Source the list from registry-side metadata in `built_in_css.rs`, not from
  ad hoc CLI-only text.
- Preserve the existing export command:
  `qec-code code css <code-id> <hx|hz>`.
- Add focused CLI tests for successful listing and rejection of unexpected
  trailing arguments.

## Non-Goals

- Do not add JSON catalog output.
- Do not add new built-in CSS families.
- Do not change sparse-row JSON export.
- Do not change README, benchmark docs, or `rsinter` configuration.
- Do not change code-spec parsing rules except where required to keep the list
  metadata close to the registry.

## Current State

`qec-code/src/codes/built_in_css.rs` owns the built-in CSS parser and
registry:

- `parse_built_in_css_code_spec(...)` recognizes fixed ids and repetition
  family specs.
- `built_in_css_checks(...)` dispatches parsed specs to fixed-code or
  repetition-family matrix constructors.
- Supported fixed ids are `steane` and `bb72`.
- Supported families are `repetition_x:d=<distance>` and
  `repetition_z:d=<distance>`, with `distance >= 2` enforced by the generator.

`qec-code/src/cli.rs` currently models CSS export as:

```rust
CodeCommands::Css {
    code_id: String,
    matrix: CssMatrixKind,
}
```

That is enough for:

```text
qec-code code css steane hx
```

but it leaves no clean place for:

```text
qec-code code css list
```

because `list` appears in the same position as `<code-id>`.

## Alternatives Considered

### 1. Registry metadata plus a CSS command wrapper

Add a small built-in CSS catalog API beside the registry and refactor the CLI
CSS branch so it can parse both `list` and export forms.

Benefits:

- matches issue #60's recommendation to use the same registry metadata that
  drives dispatch
- keeps list text close to the supported fixed ids and family specs
- preserves existing export behavior
- gives future built-ins one obvious place to update discoverability metadata

Costs:

- requires a small `CodeCommands::Css` shape change
- needs care to keep the old positional export command working

This is the recommended approach.

### 2. Hard-code list output in `cli.rs`

Add a `list` special case in the CLI and render a string literal there.

Benefits:

- smallest immediate implementation

Costs:

- can drift from `built_in_css_checks(...)`
- duplicates supported ids and family shapes outside the registry
- conflicts with issue #60's technical recommendation

This is not recommended.

### 3. Add a JSON catalog API now

Expose a richer structured catalog and make the CLI print it as JSON or text.

Benefits:

- may be useful for downstream tooling later

Costs:

- explicitly out of scope for issue #60
- adds public API and serialization decisions before there is a consumer

This is not recommended for this issue.

## Decision

Add a small catalog type in `qec-code/src/codes/built_in_css.rs`:

```rust
pub struct BuiltInCssCatalogEntry {
    pub spec: &'static str,
    pub description: &'static str,
}
```

Expose the catalog through:

```rust
pub fn built_in_css_catalog() -> &'static [BuiltInCssCatalogEntry];
```

The catalog should include exactly the supported discoverable specs:

```text
steane
bb72
repetition_x:d=<distance>
repetition_z:d=<distance>
```

Then update `qec-code/src/cli.rs` so the CSS command can dispatch to either:

```text
qec-code code css list
qec-code code css <code-id> <hx|hz>
qec-code code css export <code-id> <hx|hz>
```

The explicit `export` form is optional from the user's perspective but useful
for a clean subcommand tree. The legacy positional export form remains part of
the public behavior so existing commands and tests keep working.

## Output Format

The list command should return a human-readable multi-line string without a
trailing newline from `qec_code::cli::run(...)`. The binary writer in
`qec-code/src/main.rs` remains responsible for adding the final stdout newline,
matching existing CLI behavior.

Recommended output:

```text
Built-in CSS codes:
  steane                         fixed [[7,1,3]] CSS code
  bb72                           fixed [[72,12,6]] bivariate-bicycle CSS code
  repetition_x:d=<distance>      X-check chain, distance >= 2
  repetition_z:d=<distance>      Z-check chain, distance >= 2
```

Tests should assert the important substrings and shape names rather than rely
on every column of whitespace. The headings and spec spellings should be
stable.

## Data Flow

The list path should be:

```text
qec-code code css list
  -> clap parses the CSS list command
  -> cli::run_css_list()
  -> built_in_css_catalog()
  -> render one human-readable line per catalog entry
  -> main.rs writes the result plus one trailing newline
```

The export path should stay:

```text
qec-code code css <code-id> hx|hz
  -> cli::run_css(...)
  -> built_in_css_checks(code_id)
  -> SparseRowsMatrix::new(...)
  -> sparse_rows JSON output
```

The list path should never construct matrices. In particular, listing `bb72`
should not call the bivariate-bicycle matrix constructor.

## Error Handling

No new `QecError` variants are needed.

Expected behavior:

- `qec-code code css list` succeeds with exit code `0`, catalog stdout, and
  empty stderr.
- `qec-code code css list extra` fails during CLI usage parsing and does not
  enter registry lookup.
- Existing unknown-code behavior stays unchanged for export:
  `qec-code code css unknown hx` returns `UnknownBuiltInCssCode`.
- Existing malformed family-spec behavior stays unchanged for export:
  parser and generator validation continue to own bad parameter errors.

## Testing

Add focused binary tests in `qec-code/tests/cli.rs`.

### `code_css_list_includes_supported_built_ins`

Run:

```text
qec-code code css list
```

Assert:

- exit status succeeds
- stderr is empty
- stdout contains `Built-in CSS codes:`
- stdout contains `steane`
- stdout contains `bb72`
- stdout contains `repetition_x:d=<distance>`
- stdout contains `repetition_z:d=<distance>`
- stdout contains `distance >= 2`

### `code_css_list_rejects_unexpected_extra_arguments`

Run:

```text
qec-code code css list extra
```

Assert:

- exit status fails
- stdout is empty

The exact clap stderr text does not need a brittle full-string assertion; it is
enough to assert that stderr is non-empty and references the unexpected extra
argument or usage.

### Optional library test

Add `built_in_css_catalog_lists_supported_specs` in `qec-code/tests/code.rs` if
the implementation benefits from directly locking the catalog API. It should
assert the catalog contains the four expected specs and no duplicates.

Run the issue verification tests with a shared filter:

```bash
cargo test -p qec-code --test cli code_css_list_
```

This runs exactly the two issue-requested list tests while avoiding Cargo's
single positional test-filter limit.

Also run the existing nearby CLI tests:

```bash
cargo test -p qec-code --test cli code_css_
```

If those pass, run the full package test:

```bash
cargo test -p qec-code
```

## Compatibility Notes

Existing commands must keep working:

```text
qec-code code css steane hx
qec-code code css steane hz
qec-code code css bb72 hx
qec-code code css repetition_x:d=5 hx
```

This is important because prior issues and tests already established
`code css <code-id> <hx|hz>` as the export surface. Issue #60 should add
discoverability without forcing downstream callers to migrate.
