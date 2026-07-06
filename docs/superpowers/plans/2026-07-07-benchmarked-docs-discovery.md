# Benchmarked Docs Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Link the benchmarked documentation site from repository discovery docs and lock the GitHub Pages workflow to `make build-site`.

**Architecture:** Keep the existing static `site/` and `make build-site` pipeline as the only Pages build path. Add contract tests in `rstim/tests/site_contract.rs` that fail before the docs/build-copy changes and pass after README, showcase index, Makefile, and workflow copy are aligned.

**Tech Stack:** Rust integration tests, Markdown documentation, Makefile, GitHub Actions YAML, existing Python site checkers.

## Global Constraints

- Keep `.github/workflows/deploy-pages.yml` focused on `make build-site`.
- Avoid adding a frontend framework or package-manager install.
- README and `docs/showcases/README.md` must link to `https://nzy1997.github.io/rstim/`.
- README and `docs/showcases/README.md` must mention the benchmarked documentation site, benchmark evidence, QP101 integration, `make build-site`, and `python3 tools/check_site_build.py _site`.
- `build-site` must continue to produce `_site/` with `site/benchmark-site.json`, copied checked benchmark artifacts, QP101 assets, and generated gallery assets.
- Do not wire issues to the project board or apply `auto-resolve` labels.

---

### Task 1: README And Showcase Discovery Contract

**Files:**
- Modify: `rstim/tests/site_contract.rs`
- Modify: `README.md`
- Modify: `docs/showcases/README.md`

**Interfaces:**
- Consumes: `read_repo_file`, `assert_contains_all_case_insensitive` from `rstim/tests/site_contract.rs`.
- Produces: test `readme_links_benchmarked_site` for issue verification.

- [ ] **Step 1: Write the failing discovery test**

Append this test to `rstim/tests/site_contract.rs`:

```rust
#[test]
fn readme_links_benchmarked_site() {
    let readme = read_repo_file("README.md");
    let showcase_index = read_repo_file("docs/showcases/README.md");

    for (context, text) in [
        ("README.md", readme.as_str()),
        ("docs/showcases/README.md", showcase_index.as_str()),
    ] {
        assert_contains_all_case_insensitive(
            text,
            &[
                "benchmarked documentation site",
                "benchmark evidence",
                "qp101",
                "make build-site",
                "python3 tools/check_site_build.py _site",
            ],
            context,
        );
        assert!(
            text.contains("https://nzy1997.github.io/rstim/"),
            "{context} must link to the GitHub Pages documentation site"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test site_contract readme_links_benchmarked_site -q`

Expected: FAIL because the current README and showcase index do not mention the broader benchmarked documentation site or the Pages URL.

- [ ] **Step 3: Add README discovery copy**

Add this section to `README.md` after the opening project description and before `## What You Can Do`:

````markdown
## Benchmarked Documentation Site

The [benchmarked documentation site](https://nzy1997.github.io/rstim/)
is the broad repository reference: workspace walkthroughs, benchmark evidence,
checked results, methodology and claims limits, plus the QP101 schema browser
and gallery that used to be the whole Pages surface.

Build and check the same Pages tree locally:

```sh
make build-site
python3 tools/check_site_build.py _site
```
````

- [ ] **Step 4: Add showcase-index discovery copy**

Add this section to `docs/showcases/README.md` after the opening directory description and before `## Visual Highlights`:

````markdown
## Benchmarked Documentation Site

The [benchmarked documentation site](https://nzy1997.github.io/rstim/)
turns these runnable showcase pages into a broader Pages reference with
workspace walkthroughs, benchmark evidence, checked results, methodology and
claims limits, and the QP101 schema browser.

Build and check the same Pages tree locally:

```sh
make build-site
python3 tools/check_site_build.py _site
```
````

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p rstim --test site_contract readme_links_benchmarked_site -q`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add README.md docs/showcases/README.md rstim/tests/site_contract.rs
git commit -m "docs: link benchmarked documentation site"
```

### Task 2: Pages Workflow And Build-Site Contract

**Files:**
- Modify: `rstim/tests/site_contract.rs`
- Modify: `.github/workflows/deploy-pages.yml`
- Modify: `Makefile`

**Interfaces:**
- Consumes: `read_repo_file`, `assert_contains_all`, and `assert_contains_all_case_insensitive` from `rstim/tests/site_contract.rs`.
- Produces: test `pages_workflow_builds_benchmarked_site` for issue verification.

- [ ] **Step 1: Write the failing Pages/build contract test**

Append this test to `rstim/tests/site_contract.rs`:

```rust
#[test]
fn pages_workflow_builds_benchmarked_site() {
    let workflow = read_repo_file(".github/workflows/deploy-pages.yml");
    let makefile = read_repo_file("Makefile");

    assert_contains_all(
        &workflow,
        &[
            "actions/configure-pages@v5",
            "run: make build-site",
            "actions/upload-pages-artifact@v3",
            "path: _site",
            "actions/deploy-pages@v4",
        ],
        "Pages deployment workflow",
    );

    for forbidden in [
        "npm install",
        "npm ci",
        "pnpm install",
        "yarn install",
        "vite build",
        "next build",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "Pages workflow must stay focused on make build-site, found {forbidden}"
        );
    }

    assert_contains_all_case_insensitive(
        &makefile,
        &["build-site", "benchmarked documentation site"],
        "Makefile build-site help",
    );
    assert_contains_all(
        &makefile,
        &[
            "cp site/index.html site/styles.css site/app.js _site/",
            "cp site/benchmark-site.json _site/data/benchmark-site.json",
            "python3 tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery",
            "python3 tools/copy_site_benchmark_data.py --repo-root . --site-root _site site/benchmark-site.json",
        ],
        "Makefile build-site target",
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test site_contract pages_workflow_builds_benchmarked_site -q`

Expected: FAIL because the current Makefile help text still describes `build-site` as the QP101 GitHub Pages site instead of the benchmarked documentation site.

- [ ] **Step 3: Update Makefile help copy**

Change both Makefile help strings from:

```make
build-site           - Build the QP101 GitHub Pages site into _site
```

to:

```make
build-site           - Build the benchmarked documentation site into _site
```

- [ ] **Step 4: Clarify the Pages workflow step name**

In `.github/workflows/deploy-pages.yml`, change:

```yaml
      - name: Build site
        run: make build-site
```

to:

```yaml
      - name: Build benchmarked documentation site
        run: make build-site
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p rstim --test site_contract pages_workflow_builds_benchmarked_site -q`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Makefile .github/workflows/deploy-pages.yml rstim/tests/site_contract.rs
git commit -m "test: cover Pages build-site contract"
```

### Task 3: Final Site Verification

**Files:**
- Generated: `_site/`

**Interfaces:**
- Consumes: `make build-site`, `tools/check_showcase_docs.py`, `tools/check_site_build.py`, and the two new `site_contract` tests.
- Produces: verified built site and command evidence for PR description.

- [ ] **Step 1: Build the site**

Run: `make build-site`

Expected: PASS and `_site/index.html`, `_site/data/benchmark-site.json`, QP101 assets, gallery SVGs, and checked benchmark artifacts exist.

- [ ] **Step 2: Validate README links**

Run: `python3 tools/check_showcase_docs.py --readme README.md`

Expected: PASS with `ok: README.md`.

- [ ] **Step 3: Validate built site**

Run: `python3 tools/check_site_build.py _site`

Expected: PASS summary naming QP101 assets, workspace overview, benchmark methodology, checked benchmark artifacts, and local-only/future benchmark classifications.

- [ ] **Step 4: Run required focused Rust tests**

Run:

```sh
cargo test -p rstim --test site_contract readme_links_benchmarked_site -q
cargo test -p rstim --test site_contract pages_workflow_builds_benchmarked_site -q
```

Expected: both commands PASS.

- [ ] **Step 5: Run broad Rust verification**

Run: `cargo test`

Expected: PASS. If this exact command fails only because Cargo attempts blocked network access, run `cargo test --offline` to verify local test behavior and record the network failure in the final report.
