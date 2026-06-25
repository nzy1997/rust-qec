# Issue 220 Showcase Claim Policy Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #220, a documentation follow-up policy for uncertain showcase claims

## Summary

Issue #220 tightens the showcase documentation contract added by issue #211.
Showcase authors should write only high-confidence existing behavior in
user-facing pages. When a detail needs algorithm review, benchmark
interpretation, or scientific review, authors should open a follow-up issue
and either link it from `Limits` or omit the claim.

The change should be small and focused:

- add a short policy section to `docs/showcases/README.md`
- update `docs/showcases/_template.md` so future `Limits` sections apply the
  policy from the start
- extend `tools/check_showcase_docs.py` so individual showcase pages reject
  empty, placeholder, or copied template boilerplate `Limits` content
- keep the index and template validated by their own explicit rules

## Current State

Issue #211 is closed and merged. The repository now has:

- `docs/showcases/README.md`, with categories and a page contract
- `docs/showcases/_template.md`, with author guidance for the six showcase
  page sections
- `tools/check_showcase_docs.py`, with explicit validators for the showcase
  index, template, individual pages, README mode, link-only mode, and self-test
  fixtures

The checker already rejects missing `Limits`, empty `Limits`, `TBD`, `TODO`,
and several common placeholder prefixes for individual showcase pages. It does
not yet reject the template's own instructional sentence if an author copies it
unchanged into a real showcase page, and it does not require the index to carry
the new uncertainty policy.

There are no comments on issue #220 and no existing pull request for this
branch. The dependency on #211 is satisfied by the current base commit.

## Goals

- Document that showcase pages must state only high-confidence existing
  behavior.
- Document that claims needing algorithm review, benchmark interpretation, or
  scientific review should become follow-up issues instead of user-facing
  claims.
- Tell authors to link a concrete follow-up issue from `Limits` when that link
  helps explain a real known gap, or omit the uncertain claim entirely.
- Keep the policy short and close to the existing page contract.
- Update the template `Limits` guidance so new pages are written consistently.
- Extend checker self-tests with negative controls for:
  - an individual page whose `Limits` section copies the template boilerplate
    instead of naming real limits
  - an index fixture missing the new policy section
- Validate the real index by its own explicit rule for the policy section.
- Do not write individual showcase pages.
- Do not file follow-up issues unless a concrete uncertain claim is discovered.

## Non-Goals

- Do not add a broader Markdown linter.
- Do not validate the contents of linked GitHub issues.
- Do not require every `Limits` section to contain a follow-up issue link.
- Do not block concise real limits such as cost, runtime, platform assumptions,
  known gaps, or scope boundaries.
- Do not rewrite showcase categories or add individual showcase pages.

## Approaches Considered

### 1. Add a concise policy section plus focused checker fixtures

Add a `Documentation Follow-Up Policy` section to the showcase index, adjust
the template guidance, require that section during index validation, and extend
`limits_is_placeholder` with exact boilerplate phrases that should fail on
individual pages.

Benefits:

- matches the issue text directly
- keeps the rule visible to authors
- avoids over-validating subjective scientific content
- makes the copied-template failure mode testable
- preserves the explicit index/template handling added in issue #211

Costs:

- the checker can only catch known boilerplate and obvious placeholders; human
  review still decides whether a claim is high-confidence

This is the chosen approach.

### 2. Require a follow-up issue link in every `Limits` section

Force every individual showcase page to link at least one follow-up issue from
`Limits`.

Benefits:

- mechanically enforces traceability for unknowns

Costs:

- conflicts with real pages that have no uncertain claim
- would encourage unnecessary issues
- exceeds the issue text, which allows authors to omit uncertain claims

This is rejected.

### 3. Documentation-only policy with no checker change

Only update the index and template prose.

Benefits:

- smallest diff

Costs:

- misses the requested checker support
- allows copied boilerplate `Limits` text to pass as if it were real content
- gives follow-up showcase issues weaker validation than requested

This is rejected.

## Documentation Design

`docs/showcases/README.md` should gain a short second-level section titled
`Documentation Follow-Up Policy` before the page contract. The section should
say:

- write only high-confidence behavior that exists in the repository today
- if a claim needs algorithm review, benchmark interpretation, or scientific
  review, do not present it as a showcase claim
- open a follow-up issue for the review question when it matters
- link that issue from `Limits` when the uncertainty is a known gap readers
  should see, or omit the claim entirely

`docs/showcases/_template.md` should update its `Limits` guidance to ask for
real constraints and to route uncertain claims through follow-up issues. The
template remains authoring guidance, so its own boilerplate is valid for the
template file but invalid when copied into an individual showcase page.

## Checker Design

`tools/check_showcase_docs.py` should remain a single standard-library script.

Changes:

- add `REQUIRED_INDEX_SECTIONS = ("Categories", "Documentation Follow-Up Policy", "Page Contract")`
- update `validate_index` to require those second-level sections
- add exact boilerplate phrases to `BOILERPLATE_LIMITS`
- update `limits_is_placeholder` to reject normalized bodies that match one of
  those boilerplate phrases
- extend `run_self_test` with:
  - `boilerplate-limits.md`, which copies the template's `Limits` guidance and
    must fail as an individual showcase page
  - an index fixture missing the policy section and expected to fail index
    validation
- update the passing index fixture so it includes the policy section

The checker should continue to validate `docs/showcases/README.md` and
`docs/showcases/_template.md` explicitly instead of treating either as an
individual page.

## Testing Design

Use TDD for checker behavior:

1. Add the self-test fixtures and expected failures first.
2. Run `python3 tools/check_showcase_docs.py --self-test` and observe the
   expected failure because the checker does not yet reject copied boilerplate
   or missing index policy.
3. Implement the validator changes.
4. Re-run the focused checker commands and the requested repository command.

Required verification commands:

```sh
python3 tools/check_showcase_docs.py --self-test
python3 tools/check_showcase_docs.py docs/showcases
cargo test
```

Additional useful checks:

```sh
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py --links docs/showcases/README.md
git diff --check
```
