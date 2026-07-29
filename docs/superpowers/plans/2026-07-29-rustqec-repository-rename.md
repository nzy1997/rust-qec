# RustQEC Repository Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the public repository and umbrella workspace brand from `rstim` to `RustQEC` (`nzy1997/rust-qec`) while preserving `rstim` as the simulator crate, package, binary, CLI, protocol producer, and benchmark implementation name.

**Architecture:** Treat the rename as a two-layer identity migration: `RustQEC` names the repository, documentation site, and complete QEC workspace; `rstim` continues to name only the simulator component. Prepare and validate a draft pull request while the repository still has its old name, rename the GitHub repository only after that PR is green, then merge the PR to deploy the new GitHub Pages path. Active public links migrate to the new slug; historical plans and immutable benchmark provenance remain untouched and rely on GitHub's repository redirect.

**Tech Stack:** Cargo/Rust 2024 workspace, Zola static site, Python contract checkers, GitHub Actions, GitHub Pages, Codecov, `git`, and GitHub CLI (`gh`).

## Global Constraints

- Human-facing umbrella name is exactly `RustQEC`; the GitHub/repository slug is exactly `rust-qec`.
- The GitHub repository remains owned by `nzy1997`, public, and based on the `master` default branch.
- Keep the `rstim/` directory, Cargo package `rstim`, Rust crate path `rstim::`, binary `rstim`, CLI commands, `CARGO_BIN_EXE_rstim`, and path dependencies unchanged.
- Keep simulator-specific names unchanged, including `benchmarks/rstim_vs_stim_simulator/`, `rstim-compiled`, `rstim-interpreted`, `rstim-perf-artifacts`, and `rstim`-versus-Stim report titles.
- Keep versioned or machine-readable compatibility identifiers unchanged, including `RSTMSMP`, `CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1`, `rstim-rsmp-v1-zstd-frame`, QP101 provenance value `"framework": "rstim"`, and serialized method `rstim-ilp-exact`. Any migration of those identifiers requires a separate compatibility design and release.
- Do not rename crates, bump versions, create tags, or publish crates as part of this repository-brand migration.
- Do not perform a global `rstim` replacement. Every edit must classify the occurrence as umbrella brand, repository URL, or simulator identity.
- Do not rewrite archival material under `docs/plans/**`, `docs/superpowers/specs/**`, existing `docs/superpowers/plans/**`, `rmatching/docs/plans/**`, `rstim/docs/plans/**`, or `docs/test-reports/**`.
- Do not rewrite recorded host paths, environment metadata, hashes, or raw artifacts under `benchmarks/**/results/**`, except the currently derived sampler-readiness JSON explicitly listed in Task 4.
- Do not edit `_site/`; regenerate it with `make build-site`. `_site/` remains ignored.
- Accept a one-time QP101 schema identity migration from `https://nzy1997.github.io/rstim/qp101.schema.json` to `https://nzy1997.github.io/rust-qec/qp101.schema.json`. This plan assumes no external consumer requires the old schema URL. If that assumption is false, stop before Task 6 and configure a stable custom domain first.
- GitHub does not redirect project-site URLs. The old Pages path must not be presented as a supported redirect.
- Never create or reuse `nzy1997/rstim` after the rename; doing so disables GitHub's old repository/issue/PR redirects.
- Leave the current local checkout directory `/Users/nzy/rcode/rstim` in place during this task. New clones use a `rust-qec/` directory. Renaming the active Codex workspace directory is a separate, final local operation.

## Verified Starting State

- Repository: public `nzy1997/rstim`, numeric repository ID `1151335389`.
- Target `nzy1997/rust-qec` is currently unoccupied.
- Local origin: `git@github.com:nzy1997/rstim.git`.
- Default branch: `master`; no classic branch protection and no repository rulesets.
- The current feature branch PR #602 is merged, so implementation should start from a freshly updated `master`.
- GitHub Pages uses the Actions workflow at `.github/workflows/deploy-pages.yml`, has no custom domain, and currently serves `https://nzy1997.github.io/rstim/`.
- The `github-pages` environment is restricted to `master`.
- Repository secret `CODECOV_TOKEN` exists; no Actions variables, webhooks, deploy keys, or in-repository GitHub Action package were found.
- Four workflows exist: CI, Pages deployment, release creation, and scheduled `rbposd` parity.
- Release `v0.1.1` exists. No workspace crate is currently published on crates.io, so no crate registry rename is required.

External behavior references:

- GitHub repository rename and redirect behavior: <https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository>
- Git remote URL migration: <https://docs.github.com/en/get-started/git-basics/managing-remote-repositories>
- GitHub Pages custom-domain guidance: <https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/about-custom-domains-and-github-pages>

## File Responsibility Map

- Umbrella brand: `README.md`, `docs/showcases/README.md`, `site/config.toml`, `site/content/_index.md`, and `site/templates/*.html`.
- Canonical repository/Pages URLs: README badges, Zola configuration, site templates/JavaScript, Cargo `repository` metadata, active dependency examples, active issue links, and QP101 schema `$id`.
- Active generated evidence links: `tools/check_sampler_performance_readiness.py`, its tests, `sampler-performance-readiness.md`, the committed sampler-readiness JSON, and issue-225 evidence/tests.
- Rename contracts: new `rstim/tests/workspace_brand.rs`, existing `rstim/tests/site_contract.rs`, and `tools/test_check_site_build.py`.
- External cutover: GitHub repository name, local `origin`, repository description/homepage/topics, Pages deployment, and Codecov repository mapping.
- Deliberately unchanged interfaces: root `Cargo.toml` member names, `rstim/Cargo.toml`, path dependencies, `Cargo.lock`, simulator documentation, benchmark IDs, archive/protocol constants, historical plans, and immutable evidence.

---

### Task 1: Establish a Clean Baseline and Rename Branch

**Files:**
- Add: `docs/superpowers/plans/2026-07-29-rustqec-repository-rename.md`
- Test: existing workspace and site checks only

**Interfaces:**
- Consumes: merged `master` and existing `nzy1997/rstim` repository state
- Produces: clean branch `codex/rename-workspace-rustqec` based on current `master`

- [ ] **Step 1: Confirm the current worktree has no uncommitted changes**

Run:

```bash
git status --short --branch
```

Expected: either a clean worktree or only `docs/superpowers/plans/2026-07-29-rustqec-repository-rename.md` as untracked. If any other user changes exist, stop and preserve them before continuing.

- [ ] **Step 2: Start from the latest default branch**

Run:

```bash
git switch master
git pull --ff-only origin master
git switch -c codex/rename-workspace-rustqec
```

Expected: the new branch points at the current `origin/master` commit.

- [ ] **Step 3: Commit the approved implementation plan on the rename branch**

Run:

```bash
git add docs/superpowers/plans/2026-07-29-rustqec-repository-rename.md
git commit -m "docs: plan RustQEC repository rename"
```

Expected: the worktree is clean and the implementation plan travels with the rename branch.

- [ ] **Step 4: Reconfirm target availability and baseline GitHub identity**

Run:

```bash
gh repo view nzy1997/rust-qec --json nameWithOwner,url
gh api repos/nzy1997/rstim --jq '{id,full_name,default_branch,visibility,homepage}'
gh api repos/nzy1997/rstim/pages --jq '{html_url,build_type,cname,https_enforced}'
```

Expected: the first command reports that `nzy1997/rust-qec` does not exist; repository ID is `1151335389`, default branch is `master`, Pages is workflow-built, and `cname` is null.

- [ ] **Step 5: Run focused baseline checks**

Run:

```bash
cargo test --locked -p rstim --test site_contract
python3 -m unittest tools.test_check_site_build
make build-site
python3 tools/check_site_build.py _site
```

Expected: all commands pass before any rename edits.

---

### Task 2: Introduce the RustQEC Umbrella Brand

**Files:**
- Create: `rstim/tests/workspace_brand.rs`
- Modify: `README.md:1-35`
- Modify: `docs/showcases/README.md:1-15`
- Modify: `site/config.toml:2`
- Modify: `site/content/_index.md:2`
- Modify: `site/templates/base.html:7-20,39`
- Modify: `site/templates/index.html:3-13`
- Modify: `site/templates/simulator.html:3`
- Modify: `site/templates/detector-models.html:3`
- Modify: `site/templates/decoding.html:3-4`
- Modify: `site/templates/css-codes.html:3`
- Modify: `site/templates/qp101.html:3`
- Modify: `site/templates/rsmp-v1-showcase.html:3`
- Modify: `qec-code/README.md:14`
- Modify: `rmatching/README.md:143`
- Test: `rstim/tests/workspace_brand.rs`

**Interfaces:**
- Consumes: two-layer naming rule from Global Constraints
- Produces: `RustQEC` as the only active umbrella brand while preserving `rstim` as the simulator identity

- [ ] **Step 1: Add a failing workspace-brand contract**

Create `rstim/tests/workspace_brand.rs` with:

```rust
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rstim must live directly below the workspace root")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn rustqec_is_the_workspace_brand_while_rstim_remains_the_simulator() {
    let readme = read_repo_file("README.md");
    let site_config = read_repo_file("site/config.toml");
    let base_template = read_repo_file("site/templates/base.html");
    let workspace_manifest = read_repo_file("Cargo.toml");
    let simulator_manifest = read_repo_file("rstim/Cargo.toml");

    assert!(readme.starts_with("# RustQEC\n"));
    assert!(readme.contains("RustQEC is a Rust workspace for quantum error correction."));
    assert!(readme.contains("the `rstim` Stim-like circuit simulator and CLI"));
    assert!(site_config.contains("title = \"RustQEC\""));
    assert!(base_template.contains("RustQEC — a quantum error correction workspace in Rust"));

    assert!(workspace_manifest.contains("\"rstim\""));
    assert!(simulator_manifest.contains("name = \"rstim\""));
    assert!(readme.contains("cargo run -p rstim --bin rstim"));
}
```

- [ ] **Step 2: Run the new contract and verify it fails for the old umbrella name**

Run:

```bash
cargo test --locked -p rstim --test workspace_brand
```

Expected: FAIL because `README.md` and the site still use `rstim` as the umbrella brand.

- [ ] **Step 3: Rewrite the root entry-point copy without changing simulator commands**

Change the heading to `# RustQEC`, keep the existing badge block in place, and replace the workspace-description paragraph with this exact copy:

```markdown
RustQEC is a Rust workspace for quantum error correction. It brings together
the `rstim` Stim-like circuit simulator and CLI, code-construction tools,
decoder experiments, and reproducible benchmark evidence.
```

Change “With `rstim` you can” to “With RustQEC you can”. Keep every `rstim/` path, `rstim` command, Cargo package reference, compatibility statement, and simulator showcase name unchanged.

- [ ] **Step 4: Update umbrella wording in the showcase index and component READMEs**

Change active descriptions of “the rstim workspace” to “the RustQEC workspace” in:

```text
docs/showcases/README.md
qec-code/README.md
rmatching/README.md
```

Do not alter `rstim` CLI examples or simulator-specific descriptions.

- [ ] **Step 5: Update the Zola site brand**

Use these exact site strings:

```text
Title: RustQEC
Tagline: RustQEC — a quantum error correction workspace in Rust
Home description: RustQEC brings together the rstim simulator and CLI, MWPM/BP-OSD/ILP decoders, and a parallel benchmark harness in one Rust workspace.
```

Change page-title suffixes to `— RustQEC`. Keep “with rstim” in simulator/DEM descriptions where it refers to the executable. Keep the brand mark as the single letter `R` so this task does not require a CSS layout change.

- [ ] **Step 6: Run brand and site checks**

Run:

```bash
cargo test --locked -p rstim --test workspace_brand --test site_contract
make build-site
python3 tools/check_site_build.py _site
```

Expected: all checks pass; built page titles and visible header/footer use `RustQEC`, while command examples still use `rstim`.

- [ ] **Step 7: Commit the umbrella-brand change**

Run:

```bash
git add README.md docs/showcases/README.md site qec-code/README.md rmatching/README.md rstim/tests/workspace_brand.rs
git commit -m "docs: rename workspace brand to RustQEC"
```

---

### Task 3: Migrate Canonical Repository and Pages URLs

**Files:**
- Modify: `README.md:3-15`
- Modify: `site/config.toml:1`
- Modify: `site/templates/base.html:40`
- Modify: `site/templates/rsmp-v1-showcase.html:71-73`
- Modify: `site/static/js/benchmarks.js:138`
- Modify: `qec-code/Cargo.toml:7`
- Modify: `qec-ilp-core/Cargo.toml:7`
- Modify: `qec-code/README.md:19,27-33`
- Modify: `rmatching/README.md:14`
- Modify: `qec-code/doc/apm_css.md:10-12`
- Modify: `docs/showcases/rstim-vs-stim-simulator.md:13,26,184-187`
- Modify: `rstim/doc/qp101.schema.json:3`
- Modify: `rstim/doc/QP101-ZY.md`
- Modify: `rstim/tests/site_contract.rs:270,525`
- Modify: `tools/test_check_site_build.py:177-178`
- Modify: `rstim/tests/workspace_brand.rs`
- Test: `rstim/tests/workspace_brand.rs`
- Test: `rstim/tests/site_contract.rs`
- Test: `tools/test_check_site_build.py`

**Interfaces:**
- Consumes: repository slug `nzy1997/rust-qec` and new Pages root `https://nzy1997.github.io/rust-qec/`
- Produces: canonical active links and schema identity that resolve after the external cutover

- [ ] **Step 1: Add a failing active-link contract**

Append this test to `rstim/tests/workspace_brand.rs`:

```rust
#[test]
fn active_public_links_use_the_rust_qec_slug() {
    const ACTIVE_FILES: &[&str] = &[
        "README.md",
        "docs/showcases/README.md",
        "site/config.toml",
        "site/templates/base.html",
        "site/templates/rsmp-v1-showcase.html",
        "site/static/js/benchmarks.js",
        "qec-code/Cargo.toml",
        "qec-ilp-core/Cargo.toml",
        "qec-code/README.md",
        "rmatching/README.md",
        "rstim/doc/qp101.schema.json",
        "rstim/tests/site_contract.rs",
        "tools/test_check_site_build.py",
    ];

    for path in ACTIVE_FILES {
        let text = read_repo_file(path);
        assert!(
            !text.contains("github.com/nzy1997/rstim"),
            "{path} still contains the old GitHub slug"
        );
        assert!(
            !text.contains("nzy1997.github.io/rstim"),
            "{path} still contains the old Pages path"
        );
        assert!(
            !text.contains("codecov.io/gh/nzy1997/rstim"),
            "{path} still contains the old Codecov slug"
        );
    }

    assert!(read_repo_file("README.md").contains("github.com/nzy1997/rust-qec"));
    assert!(read_repo_file("site/config.toml")
        .contains("base_url = \"https://nzy1997.github.io/rust-qec\""));
}
```

- [ ] **Step 2: Run the active-link contract and verify it fails**

Run:

```bash
cargo test --locked -p rstim --test workspace_brand active_public_links_use_the_rust_qec_slug
```

Expected: FAIL on the old GitHub, Pages, and Codecov URLs.

- [ ] **Step 3: Update active GitHub and Codecov links**

Apply these exact mappings only to the active files listed above plus the active issue-link docs listed in this task:

```text
https://github.com/nzy1997/rstim      -> https://github.com/nzy1997/rust-qec
https://github.com/nzy1997/rstim.git  -> https://github.com/nzy1997/rust-qec.git
https://codecov.io/gh/nzy1997/rstim   -> https://codecov.io/gh/nzy1997/rust-qec
```

Issue and pull-request numbers do not change. Do not update archived plans or immutable benchmark result bundles.

- [ ] **Step 4: Update checkout and dependency examples**

Replace the existing Quick Start workspace-build block with this exact clone-and-build sequence in `README.md`:

```bash
git clone https://github.com/nzy1997/rust-qec.git
cd rust-qec
cargo build --workspace
```

Change the checkout-relative example in `qec-code/README.md` to:

```toml
qec-code = { path = "../rust-qec/qec-code" }
```

Do not change Cargo path dependencies that point to the simulator member `../rstim`.

- [ ] **Step 5: Migrate the Pages root and QP101 schema identity**

Set:

```toml
base_url = "https://nzy1997.github.io/rust-qec"
```

Set the schema `$id` to:

```json
"$id": "https://nzy1997.github.io/rust-qec/qp101.schema.json"
```

Add a short “Schema identity” note near the start of `rstim/doc/QP101-ZY.md` stating that the canonical URI moved from `/rstim/qp101.schema.json` to `/rust-qec/qp101.schema.json` during the RustQEC repository rename, with no schema-shape or QP101 version change.

- [ ] **Step 6: Update exact URL assertions**

Update `rstim/tests/site_contract.rs` and `tools/test_check_site_build.py` to expect:

```text
https://nzy1997.github.io/rust-qec/
https://github.com/nzy1997/rust-qec/blob/master/
```

- [ ] **Step 7: Run focused URL and site tests**

Run:

```bash
cargo test --locked -p rstim --test workspace_brand --test site_contract
python3 -m unittest tools.test_check_site_build
make build-site
python3 tools/check_site_build.py _site
```

Expected: all checks pass locally. External new URLs need not resolve until Task 6.

- [ ] **Step 8: Commit canonical URL migration**

Run:

```bash
git add README.md docs/showcases qec-code qec-ilp-core rmatching/README.md site rstim/doc/QP101-ZY.md rstim/doc/qp101.schema.json rstim/tests/site_contract.rs rstim/tests/workspace_brand.rs tools/test_check_site_build.py
git commit -m "docs: migrate canonical repository URLs"
```

---

### Task 4: Migrate Active GitHub-Linked Evidence and Tooling

**Files:**
- Modify: `tools/check_sampler_performance_readiness.py:39`
- Modify: `tools/test_check_sampler_performance_readiness.py:167-270`
- Modify: `sampler-performance-readiness.md:24-26`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json:120-122`
- Modify: `benchmarks/qec_code_random_window/issue225_evidence.json:4-104`
- Modify: `benchmarks/qec_code_random_window/tests/test_issue225_readiness.py:101-201,377-379`
- Test: `tools/test_check_sampler_performance_readiness.py`
- Test: `benchmarks/qec_code_random_window/tests/test_issue225_readiness.py`

**Interfaces:**
- Consumes: new canonical repository slug
- Produces: newly generated/current evidence links using `nzy1997/rust-qec` without rewriting immutable historical benchmark provenance

- [ ] **Step 1: Change tests to require the new repository slug**

Replace test inputs and expected GitHub URLs with:

```text
nzy1997/rust-qec
https://github.com/nzy1997/rust-qec/issues/
https://github.com/nzy1997/rust-qec/pull/
```

- [ ] **Step 2: Run focused tests and verify they fail against old generators/data**

Run:

```bash
python3 -m unittest tools.test_check_sampler_performance_readiness
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness
```

Expected: FAIL on old issue/PR URLs or old repository slug expectations.

- [ ] **Step 3: Update the active URL generator and current evidence input**

Set:

```python
ISSUE_BASE_URL = "https://github.com/nzy1997/rust-qec/issues"
```

Mechanically update only GitHub URL fields in `benchmarks/qec_code_random_window/issue225_evidence.json`; keep issue numbers, PR numbers, timestamps, titles, and evidence prose byte-for-byte otherwise unchanged.

- [ ] **Step 4: Synchronize the currently derived readiness artifacts**

In the committed sampler-readiness JSON, change the three issue URLs and `issues.milestone.repo` to `nzy1997/rust-qec`. Change only the corresponding three rendered Markdown issue links in `sampler-performance-readiness.md`. Do not rerun performance benchmarks and do not alter measured values, hashes, case IDs, or claims.

- [ ] **Step 5: Run the focused evidence tests**

Run:

```bash
python3 -m unittest tools.test_check_sampler_performance_readiness
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness
```

Expected: PASS, including the committed-Markdown-is-derived-from-JSON check.

- [ ] **Step 6: Commit the active evidence-link migration**

Run:

```bash
git add tools/check_sampler_performance_readiness.py tools/test_check_sampler_performance_readiness.py sampler-performance-readiness.md benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json benchmarks/qec_code_random_window/issue225_evidence.json benchmarks/qec_code_random_window/tests/test_issue225_readiness.py
git commit -m "chore: migrate active GitHub evidence links"
```

---

### Task 5: Verify the Prepared Rename and Open a Draft PR

**Files:**
- Modify: none unless verification exposes drift
- Test: complete workspace, Python tools, generated site, and naming-boundary audits

**Interfaces:**
- Consumes: Tasks 2-4 commits
- Produces: green draft PR on the old repository, ready for the atomic GitHub cutover

- [ ] **Step 1: Verify formatting, build, and Rust tests**

Run:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo run --locked -p rstim --bin rstim -- --version
```

Expected: all commands pass and the last command still identifies the executable as `rstim`.

- [ ] **Step 2: Verify Python tooling and the documentation site**

Run:

```bash
python3 -m unittest discover -s tools -p 'test_*.py'
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness
make build-site
python3 tools/check_site_build.py _site
```

Expected: all tests pass and `_site/` contains the RustQEC brand with relative assets suitable for `/rust-qec/` deployment.

- [ ] **Step 3: Audit active old-URL leakage**

Run:

```bash
rg -n 'github\.com/nzy1997/rstim|codecov\.io/gh/nzy1997/rstim|nzy1997\.github\.io/rstim' README.md docs/showcases site qec-code qec-ilp-core rmatching/README.md rstim/doc/QP101-ZY.md rstim/doc/qp101.schema.json rstim/tests/site_contract.rs rstim/tests/workspace_brand.rs tools/check_sampler_performance_readiness.py tools/test_check_sampler_performance_readiness.py tools/test_check_site_build.py sampler-performance-readiness.md benchmarks/qec_code_random_window/issue225_evidence.json benchmarks/qec_code_random_window/tests/test_issue225_readiness.py
```

Expected: no output. A repository-wide search is allowed to find archived plans and immutable result provenance.

- [ ] **Step 4: Audit preserved simulator and protocol identities**

Run:

```bash
rg -n 'name = "rstim"|RSTMSMP|CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1|rstim-rsmp-v1-zstd-frame|rstim-ilp-exact' rstim qec-code
```

Expected: preserved package/protocol/method matches remain. Do not “clean up” these occurrences.

- [ ] **Step 5: Visually inspect the generated homepage**

In a separate terminal, serve `_site/` locally:

```bash
python3 -m http.server 8765 --directory _site
```

Open `http://127.0.0.1:8765/` in the in-app browser, capture a desktop screenshot of the homepage, and verify:

```text
Header brand: RustQEC
Hero: complete quantum error correction workspace in Rust
Simulator commands: rstim
Repository link: https://github.com/nzy1997/rust-qec
No broken CSS, navigation, or benchmark cards
```

Attach the screenshot to the draft PR because the site header/footer visibly change.

- [ ] **Step 6: Push the prepared branch to the still-old repository**

Run:

```bash
git push -u origin codex/rename-workspace-rustqec
```

- [ ] **Step 7: Open a draft PR before the external rename**

Run:

```bash
gh pr create --repo nzy1997/rstim --base master --head codex/rename-workspace-rustqec --draft --title "Rename workspace brand to RustQEC" --body "Renames the repository-level brand and canonical URLs to RustQEC/rust-qec while preserving rstim as the simulator crate, package, binary, CLI, protocol identity, and benchmark implementation. The GitHub repository rename will occur only after this draft is green; Pages will deploy from master after the cutover."
```

Expected: draft PR exists, CI passes, and Pages is not deployed because the branch is not `master`.

---

### Task 6: Perform the GitHub Repository Cutover

**Files:**
- Modify: GitHub repository name and local `.git/config` remote URL
- Test: GitHub repository identity, redirects, PR continuity, workflows, environments, and secret names

**Interfaces:**
- Consumes: green draft PR and available `nzy1997/rust-qec` target
- Produces: renamed repository with the same repository ID and an updated local origin

- [ ] **Step 1: Reconfirm the target is free immediately before mutation**

Run:

```bash
gh repo view nzy1997/rust-qec --json nameWithOwner,url
```

Expected: repository-not-found. If it exists, stop; do not select another name automatically.

- [ ] **Step 2: Rename the GitHub repository**

Run:

```bash
gh repo rename -R nzy1997/rstim rust-qec --yes
```

Expected: `https://github.com/nzy1997/rust-qec` exists and retains repository ID `1151335389`.

- [ ] **Step 3: Update and verify the local origin**

Run:

```bash
git remote set-url origin git@github.com:nzy1997/rust-qec.git
git remote -v
git fetch origin
```

Expected: fetch and push URLs both use `nzy1997/rust-qec.git`; fetch succeeds.

- [ ] **Step 4: Verify redirect and repository continuity**

Run:

```bash
gh api repos/nzy1997/rust-qec --jq '{id,full_name,default_branch,visibility}'
gh pr view --repo nzy1997/rust-qec --json state,isDraft,headRefName,baseRefName,url
curl --head https://github.com/nzy1997/rstim
```

Expected: ID `1151335389`, `master`, public visibility, the draft PR remains attached, and the old GitHub URL redirects to the new repository. Do not expect the old Pages URL to redirect.

- [ ] **Step 5: Verify settings survived the rename**

Run:

```bash
gh workflow list --repo nzy1997/rust-qec
gh secret list --repo nzy1997/rust-qec
gh api repos/nzy1997/rust-qec/environments --jq '.environments[] | {name,deployment_branch_policy}'
gh release view v0.1.1 --repo nzy1997/rust-qec
```

Expected: four workflows remain active, `CODECOV_TOKEN` remains listed, `github-pages` still targets `master`, and release `v0.1.1` is accessible.

---

### Task 7: Merge, Deploy, and Verify External Integrations

**Files:**
- Modify: GitHub repository description, homepage, and topics
- Modify: prepared branch only if live verification finds an actual mismatch
- Test: post-merge CI, Pages, schema, Codecov, readiness generation, clone/fetch, and release continuity

**Interfaces:**
- Consumes: renamed GitHub repository and green draft PR
- Produces: live RustQEC repository/site with working external integrations

- [ ] **Step 1: Mark the PR ready and merge using the repository's merge-commit convention**

Run from `codex/rename-workspace-rustqec`:

```bash
gh pr ready --repo nzy1997/rust-qec
gh pr checks --repo nzy1997/rust-qec --watch
gh pr merge --repo nzy1997/rust-qec --merge --delete-branch
```

Expected: PR merges into `master`; CI and Pages workflows start on the new repository.

- [ ] **Step 2: Update public GitHub metadata**

Run:

```bash
gh repo edit nzy1997/rust-qec --description "Rust workspace for quantum error-correction research: code construction, Stim-like simulation, decoding, and reproducible benchmarks." --homepage "https://nzy1997.github.io/rust-qec/" --add-topic rust --add-topic quantum-error-correction --add-topic qec --add-topic stim --add-topic decoding
```

- [ ] **Step 3: Verify post-merge workflows**

Run:

```bash
gh run list --repo nzy1997/rust-qec --branch master --limit 10
```

Expected: the latest CI and Deploy GitHub Pages runs for `master` complete successfully. The release workflow need not run because this rename creates no tag.

- [ ] **Step 4: Verify the new Pages site and schema**

Run:

```bash
curl --fail --silent --show-error --output /dev/null https://nzy1997.github.io/rust-qec/
curl --fail --silent --show-error --output /dev/null https://nzy1997.github.io/rust-qec/qp101.schema.json
curl --fail --silent --show-error --output /dev/null https://nzy1997.github.io/rust-qec/simulator/
```

Expected: HTTP success for all three new URLs. Record the old `/rstim/` Pages URL as retired, not redirected.

- [ ] **Step 5: Regenerate current readiness artifacts against the renamed repository**

Run:

```bash
python3 tools/check_sampler_performance_readiness.py --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml --out benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json --markdown-out sampler-performance-readiness.md --verify-github nzy1997/rust-qec
git diff --exit-code -- sampler-performance-readiness.md benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json
```

Expected: readiness passes and live regeneration produces no diff from Task 4. If it does produce a semantic diff, review it, rerun focused tests, and commit only the justified generated change.

- [ ] **Step 6: Verify Codecov mapping and badge**

Open the Codecov repository settings through the GitHub integration, synchronize repositories, and confirm the project slug is `nzy1997/rust-qec`. Rerun the coverage workflow if required, then verify:

```bash
curl --fail --silent --show-error --output /dev/null https://codecov.io/gh/nzy1997/rust-qec
```

Expected: the project page and README badge resolve under the new slug, and a new coverage upload is associated with `nzy1997/rust-qec`.

- [ ] **Step 7: Verify clone and component identity from the new URL**

Use a dedicated smoke-test path that must not already exist:

```bash
test ! -e /tmp/rust-qec-rename-smoke
git clone --depth 1 https://github.com/nzy1997/rust-qec.git /tmp/rust-qec-rename-smoke
```

Run against that fresh clone:

```bash
cargo metadata --manifest-path /tmp/rust-qec-rename-smoke/Cargo.toml --locked --no-deps --format-version 1
cargo run --manifest-path /tmp/rust-qec-rename-smoke/Cargo.toml --locked -p rstim --bin rstim -- --version
```

Expected: the checkout directory is `rust-qec`, while Cargo and the executable still identify the simulator as `rstim`.

- [ ] **Step 8: Manually confirm no separately linked GitHub Package needs migration**

Inspect the `nzy1997` GitHub Packages page. No package reference was found in the repository, but the current token cannot enumerate account packages. If no package is linked to `nzy1997/rstim`, record “not applicable” in the PR verification notes. Package renaming is out of scope for this plan.

---

## Rollback Procedure

Use rollback only while `nzy1997/rstim` remains unclaimed.

1. Rename the repository back:

   ```bash
   gh repo rename -R nzy1997/rust-qec rstim --yes
   ```

2. Restore the local remote:

   ```bash
   git remote set-url origin git@github.com:nzy1997/rstim.git
   git fetch origin
   ```

3. Revert the merged rename commits on `master` using new revert commits; do not reset published history.
4. Restore the GitHub homepage to `https://nzy1997.github.io/rstim/` and redeploy Pages.
5. Re-sync Codecov to `nzy1997/rstim` and verify the old badge.
6. Recheck repository ID `1151335389`, four workflows, `github-pages` environment policy, `CODECOV_TOKEN`, release `v0.1.1`, old Pages homepage, and old schema URL.

## Completion Criteria

- GitHub repository is `nzy1997/rust-qec` with repository ID `1151335389` and old repository URLs redirect.
- README, active documentation, Cargo metadata, dependency examples, site source links, active issue links, and Codecov badge use the new slug.
- GitHub Pages serves the RustQEC-branded site at `/rust-qec/`, including the migrated QP101 schema `$id`.
- `rstim` remains unchanged as crate, package, directory, binary, CLI, protocol identity, and benchmark implementation.
- Workspace Rust tests, Python tool tests, Zola build checks, post-rename Actions, Pages, Codecov, and fresh-clone smoke checks pass.
- Historical plans and immutable benchmark provenance remain unchanged.
- No crate version, tag, release, or crates.io publication is created solely for the repository rename.
