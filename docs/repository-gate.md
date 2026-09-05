# Default branch repository gate

`master` should accept ordinary changes only through a pull request whose current
head passes the five always-applicable jobs in `.github/workflows/ci.yml`:

- `test`
- `rsmp-v1-readiness`
- `checked-evidence-portability`
- `publication-evidence-check`
- `coverage`

All five checks are bound to the GitHub Actions app (`integration_id` 15368).
`perf-gate` is deliberately excluded because unlabeled pull requests skip it.
`Interactive shot quality` is path-filtered and is also unsuitable as a required
default-branch check.

The proposed repository ruleset is
[`tools/repository_gate_ruleset.json`](../tools/repository_gate_ruleset.json).
It applies only to `refs/heads/master`, requires a pull request with zero mandatory
approvals, requires the branch to be current before its checks count, blocks branch
deletion and force pushes, and gives repository administrators one emergency path.
That bypass is `pull_request`-only: an administrator must still use a pull request,
and GitHub records the bypass. It does not exempt routine administrator pushes or
grant a bypass to the broader maintain/write roles.

## Apply after review

Do not run this command until the implementation pull request is merged and a
repository administrator has reviewed the JSON payload:

```sh
gh api \
  --method POST \
  repos/nzy1997/rust-qec/rulesets \
  --input tools/repository_gate_ruleset.json
```

Save the returned ruleset ID. If an equivalent ruleset was created through the UI,
update that ruleset with `PATCH repos/nzy1997/rust-qec/rulesets/RULESET_ID` rather
than creating a duplicate.

## Verify without changing settings

Run the reviewer command from a clean checkout:

```sh
python3 tools/check_repository_gate.py \
  --repo nzy1997/rust-qec \
  --branch master
```

The checker reads both legacy branch protection and active rulesets. It confirms
that the rules effectively match `master`, requires the exact five check names and
GitHub Actions identity, resolves them on the exact current `master` commit, and
requires three recent ordinary pull-request runs from each supplying workflow.
Skipped jobs do not count as evidence. API or permission failures print
`UNAVAILABLE default branch gate` and never print PASS.

The check selection is grounded in exact GitHub runs rather than workflow job
names alone. On 2026-09-05, the exact `master` commit
`cfc935fc13e73469f413a08e08d0c19ecad0e42a` passed all five checks in
[CI run 33947294824](https://github.com/nzy1997/rust-qec/actions/runs/33947294824).
An ordinary, unlabeled pull request at
`6d2ad2223db65713028d8d7bbdd9a63d445e2efa` passed the same five checks in
[CI run 33859520856](https://github.com/nzy1997/rust-qec/actions/runs/33859520856),
while `perf-gate` was present with conclusion `skipped`. The live checker repeats
this resolution from current API data instead of trusting these historical links.

After applying the rule, use a disposable pull request for the final enforcement
evidence. Make one check fail on the PR branch only, record that GitHub reports the
PR as not mergeable, then repair the same branch and record that all five required
checks pass and the PR becomes mergeable. Close the PR without merging it and
delete its branch. Never push the deliberately failing commit to `master`.

Offline negative controls require no GitHub access:

```sh
python3 -m unittest tools.test_check_repository_gate
```
