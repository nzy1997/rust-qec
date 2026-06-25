# Issue 220 Showcase Claim Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a showcase documentation policy for uncertain claims and extend the checker so copied placeholder `Limits` text fails on individual showcase pages.

**Architecture:** Keep the policy in the showcase authoring surface (`docs/showcases/README.md` and `_template.md`) and keep validation in the existing standard-library checker. The checker continues to classify the index, template, and individual pages explicitly, with self-test fixtures proving the new negative controls.

**Tech Stack:** Markdown, Python 3 standard library, existing Cargo workspace verification.

## Global Constraints

- Document that showcase pages must write only high-confidence existing behavior.
- When a claim needs algorithm review, benchmark interpretation, or scientific review, authors must open a follow-up issue and link it from `Limits` when readers should see the known gap, or omit the uncertain claim.
- Individual showcase pages must have a non-placeholder `Limits` section.
- The checker must reject empty `Limits`, `TBD`, `TODO`, missing `Limits`, and boilerplate-only `Limits` copied from the template.
- The checker must explicitly validate the showcase index and template by their own rules.
- Do not write individual showcase pages.
- Do not file follow-up issues unless a concrete uncertain claim is discovered.
- Required verification commands:
  - `python3 tools/check_showcase_docs.py --self-test`
  - `python3 tools/check_showcase_docs.py docs/showcases`
  - `cargo test`

---

### Task 1: Add Showcase Claim Policy And Checker Enforcement

**Files:**
- Modify: `docs/showcases/README.md`
- Modify: `docs/showcases/_template.md`
- Modify: `tools/check_showcase_docs.py`

**Interfaces:**
- Consumes: existing `validate_index(path: Path, repo_root: Path) -> list[str]`.
- Consumes: existing `limits_is_placeholder(body: str) -> bool`.
- Produces: `REQUIRED_INDEX_SECTIONS: tuple[str, ...]`.
- Produces: `BOILERPLATE_LIMITS: set[str]`.
- Produces: self-test fixtures that fail when an individual page has boilerplate-only `Limits` content or when an index lacks the policy section.

- [ ] **Step 1: Add the failing self-test fixtures first**

In `tools/check_showcase_docs.py`, update `run_self_test()` before changing the validation logic.

Add this fixture immediately after `placeholder_limits`:

```python
        template_limits_text = (
            "State real constraints, assumptions, cost, runtime, platform expectations, or\n"
            "known gaps. Do not leave this section empty, and do not use placeholder text."
        )
        boilerplate_limits = write_fixture(
            root,
            "docs/showcases/boilerplate-limits.md",
            VALID_SHOWCASE.replace(
                "This fixture covers checker structure only, not full documentation prose.",
                template_limits_text,
            ),
        )
```

Replace the `index = write_fixture(...)` body with this version so the valid index fixture includes the new policy:

```python
        index = write_fixture(
            root,
            "docs/showcases/README.md",
            "# Showcase Index\n\n## Categories\n\n### Example\n\nSee [`README.md`](README.md).\n\n"
            "## Documentation Follow-Up Policy\n\n"
            "Write only high-confidence existing behavior. Open follow-up issues for claims "
            "that need algorithm review, benchmark interpretation, or scientific review.\n\n"
            "## Page Contract\n\n"
            + "\n".join(f"- `{section}`" for section in REQUIRED_SHOWCASE_SECTIONS)
            + "\n",
        )
```

Add this missing-policy fixture immediately after the valid `index` fixture:

```python
        index_missing_policy = write_fixture(
            root,
            "docs/showcases/index-missing-policy.md",
            "# Showcase Index\n\n## Categories\n\n### Example\n\nSee [`README.md`](README.md).\n\n## Page Contract\n\n"
            + "\n".join(f"- `{section}`" for section in REQUIRED_SHOWCASE_SECTIONS)
            + "\n",
        )
```

Add `boilerplate_limits` to `expected_failures`:

```python
            (boilerplate_limits, "non-placeholder"),
```

Add this assertion after the `expected_failures` loop:

```python
        missing_policy_errors = validate_index(index_missing_policy, root)
        if not any("Documentation Follow-Up Policy" in error for error in missing_policy_errors):
            errors.append(
                "index without policy did not fail with Documentation Follow-Up Policy: "
                f"{missing_policy_errors}"
            )
```

- [ ] **Step 2: Run the focused test and verify it fails for the expected reason**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
```

Expected: exit code `1`. The output must mention `boilerplate-limits.md` not failing with `non-placeholder` and `index without policy did not fail with Documentation Follow-Up Policy`.

- [ ] **Step 3: Add the documentation policy**

In `docs/showcases/README.md`, add this section between the category list and `## Page Contract`:

```markdown
## Documentation Follow-Up Policy

Write only high-confidence behavior that exists in the repository today. If a
claim needs algorithm review, benchmark interpretation, or scientific review,
do not present it as a showcase claim.

Open a follow-up issue for the review question when it matters. Link that
issue from `Limits` when the uncertainty is a known gap readers should see, or
omit the uncertain claim entirely.
```

In `docs/showcases/_template.md`, replace the `## Limits` body with:

```markdown
State real constraints, assumptions, cost, runtime, platform expectations,
known gaps, and follow-up issue links for uncertainties readers should know
about. Do not leave this section empty, and do not use placeholder text.
```

- [ ] **Step 4: Implement the checker logic**

In `tools/check_showcase_docs.py`, add this constant after `REQUIRED_SHOWCASE_SECTIONS`:

```python
REQUIRED_INDEX_SECTIONS = (
    "Categories",
    "Documentation Follow-Up Policy",
    "Page Contract",
)
```

Add these constants near the placeholder constants:

```python
LIMITS_NORMALIZATION_RE = re.compile(r"[\s`*_>.,:;!()\[\]-]+")
BOILERPLATE_LIMITS = {
    "state real constraints assumptions cost runtime platform expectations or known gaps do not leave this section empty and do not use placeholder text",
    "state real constraints assumptions cost runtime platform expectations known gaps and follow up issue links for uncertainties readers should know about do not leave this section empty and do not use placeholder text",
}
```

Add this helper before `limits_is_placeholder`:

```python
def normalize_limits_body(body: str) -> str:
    return LIMITS_NORMALIZATION_RE.sub(" ", body).strip().lower()
```

Replace `limits_is_placeholder` with:

```python
def limits_is_placeholder(body: str) -> bool:
    normalized = normalize_limits_body(body)
    if normalized in PLACEHOLDER_LIMITS or normalized in BOILERPLATE_LIMITS:
        return True
    return any(
        normalized == prefix or re.match(rf"^{re.escape(prefix)}(?:\W|$)", normalized) is not None
        for prefix in PLACEHOLDER_LIMITS_PREFIXES
    )
```

In `validate_index`, replace the two explicit section checks for `Categories` and `Page Contract` with:

```python
    for section in REQUIRED_INDEX_SECTIONS:
        if section not in headings:
            errors.append(f"showcase index missing {section} section")
```

- [ ] **Step 5: Run focused verification**

Run:

```sh
python3 tools/check_showcase_docs.py --self-test
python3 tools/check_showcase_docs.py docs/showcases/README.md
python3 tools/check_showcase_docs.py docs/showcases
python3 tools/check_showcase_docs.py --links docs/showcases/README.md
```

Expected: all commands exit `0`. The directory command prints `ok:` lines for `README.md` and `_template.md`.

- [ ] **Step 6: Run repository verification**

Run:

```sh
cargo test
git diff --check
```

Expected: both commands exit `0`.

- [ ] **Step 7: Commit the implementation**

Run:

```sh
git add docs/showcases/README.md docs/showcases/_template.md tools/check_showcase_docs.py docs/superpowers/plans/2026-06-25-issue-220-showcase-claim-policy.md
git commit -m "docs: add showcase claim follow-up policy"
```

Expected: commit succeeds with only the documentation policy, checker update, and implementation plan.
