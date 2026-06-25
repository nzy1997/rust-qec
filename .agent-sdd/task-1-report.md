Status: Completed

Commits created: `0315d83` (`test: cover built-in QP101 gallery rendering`)

One-line test summary: `cargo test -p rstim --test site_gallery -q` was initially red due missing `tools/build_qp101_gallery.py` and `qp101-viz/examples/{basic,repeat-detector}.stim` (2 failures), and after adding them it became green (2 passed).

Concerns if any: None.

Report path: `/Users/nzy/pycode/agent-desk/config/.agent-desk/worktrees/nzy1997-rstim/issue-175-run-1-agent-issue-175-switch-the-qp101-gallery-build-to-built-in-svg-o-run-1/.agent-sdd/task-1-report.md`

Fix details (review findings):
1. `tools/build_qp101_gallery.py` now returns the renderer’s actual nonzero exit code on `subprocess.CalledProcessError`:
   - catches `CalledProcessError` and returns `exc.returncode` after printing the same error message.
2. `rstim/tests/site_gallery.rs` now compiles on non-Unix by providing a non-Unix `create_failing_typst` stub and adds a focused regression test:
   - `qp101_gallery_propagates_renderer_exit_code` creates a temporary fake `rstim` that exits with code `37` and asserts the gallery script exits with status `37`.

Fix verification:
`cargo test -p rstim --test site_gallery -q`
Result: 3 tests, all passed.
