# Issue 390 Sample Speed Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add sample median shots/s, report-only `rstim`-compiled-vs-Stim ratios, and explicit unavailable comparison statuses to perf summary JSON and Markdown reports.

**Architecture:** Extend the existing summary model in `rstim/src/perf/summary.rs` so rates and report-only Stim context are derived once from raw JSONL. Keep `rstim` compiled-vs-interpreted comparisons in the existing `comparisons` array for gate evaluation, and add a separate report-only Stim context field for Markdown rendering. The CLI continues to summarize/report through the same library functions, with a checked fixture for the issue verification commands.

**Tech Stack:** Rust 2024, `serde` JSON/JSONL records, existing `rstim::perf` modules, integration tests under `rstim/tests/`, CLI tests via `CARGO_BIN_EXE_rstim`.

## Global Constraints

- Summary JSON must contain `median_shots_per_second` for completed sample variants.
- Summary JSON must contain `rstim_compiled_vs_stim_cli_ratio` when both `rstim-compiled` and `stim-cli` completed for compiled sample cases.
- When the Stim comparison cannot be computed, summary JSON must report explicit `missing_variant`, `tool_failed`, or `timed_out` status and the recorded reason when present.
- Do not synthesize a numeric ratio when either variant did not complete.
- Markdown report must contain `shots/s` for completed sample variants.
- Markdown report must contain the exact phrase `report-only Stim comparison`.
- Keep compiled-vs-interpreted `rstim` comparisons as the gating candidate in the existing `comparisons` array.
- Stim comparisons are report-only context and must not affect `perf gate`.
- The public selected case label is exactly `stim-style-surface-sample-d11-r100-b1024`.
- Summarizing a completed sample record with zero shots must exit nonzero with text containing `shots must be positive for sample rate`.
- Do not tune `rstim` speed.
- Do not hide or rewrite poor benchmark results.

---

## File Structure

- Modify `rstim/src/perf/summary.rs`: add summary fields, compute rates, and compute report-only Stim comparison status.
- Modify `rstim/src/perf/report.rs`: render sample rates and report-only Stim comparison lines.
- Modify `rstim/src/perf.rs`: re-export the new comparison summary type.
- Modify `rstim/tests/perf_summary.rs`: add focused unit coverage for rates, completed Stim ratio, unavailable statuses, and zero-shot rejection.
- Modify `rstim/tests/perf_gate.rs`: update the one `PerfVariantSummary` literal for the new field.
- Modify `rstim/tests/cli_perf.rs`: add CLI coverage for the checked fixture summarize/report workflow.
- Create `rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`: completed public sample fixture.
- Create `rstim/tests/fixtures/perf/stim_style_sample_zero_shots_raw.jsonl`: negative-control fixture.

### Task 1: Summary Model, Report Rendering, and Fixtures

**Files:**
- Modify: `rstim/src/perf/summary.rs`
- Modify: `rstim/src/perf/report.rs`
- Modify: `rstim/src/perf.rs`
- Modify: `rstim/tests/perf_summary.rs`
- Modify: `rstim/tests/perf_gate.rs`
- Modify: `rstim/tests/cli_perf.rs`
- Create: `rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl`
- Create: `rstim/tests/fixtures/perf/stim_style_sample_zero_shots_raw.jsonl`

**Interfaces:**
- Produces: `PerfVariantSummary.median_shots_per_second: Option<f64>`.
- Produces: `PerfReportOnlyComparisonSummary { kind: String, lhs_variant: String, rhs_variant: String, ratio: Option<f64>, status: String, failure_reason: Option<String> }`.
- Produces: `PerfCaseSummary.rstim_compiled_vs_stim_cli_ratio: Option<PerfReportOnlyComparisonSummary>`.
- Preserves: `PerfCaseSummary.comparisons` remains the same-run `rstim` gate comparison list.
- Preserves: `evaluate_summary(&PerfSummary, PerfGateConfig) -> PerfGateVerdict` behavior for Stim context.

- [ ] **Step 1: Add checked raw JSONL fixtures**

Create `rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl` with exactly these lines:

```jsonl
{"case_label":"stim-style-surface-sample-d11-r100-b1024","tool_variant":"stim-cli","workload":"sample","tier":"report_only","measurement_index":0,"warmup":false,"qubits":1,"measurements":1,"detectors":0,"observables":0,"repeat_depth":1,"repeat_count":100,"shots":1024,"wall_time_ns":2000000,"peak_memory_bytes":1000,"status":"completed","failure_reason":null,"stderr":null}
{"case_label":"stim-style-surface-sample-d11-r100-b1024","tool_variant":"rstim-interpreted","workload":"sample","tier":"report_only","measurement_index":0,"warmup":false,"qubits":1,"measurements":1,"detectors":0,"observables":0,"repeat_depth":1,"repeat_count":100,"shots":1024,"wall_time_ns":5000000,"peak_memory_bytes":2000,"status":"completed","failure_reason":null,"stderr":null}
{"case_label":"stim-style-surface-sample-d11-r100-b1024","tool_variant":"rstim-compiled","workload":"sample","tier":"report_only","measurement_index":0,"warmup":false,"qubits":1,"measurements":1,"detectors":0,"observables":0,"repeat_depth":1,"repeat_count":100,"shots":1024,"wall_time_ns":4000000,"peak_memory_bytes":1500,"status":"completed","failure_reason":null,"stderr":null}
```

Create `rstim/tests/fixtures/perf/stim_style_sample_zero_shots_raw.jsonl` with exactly this line:

```jsonl
{"case_label":"stim-style-surface-sample-d11-r100-b1024","tool_variant":"stim-cli","workload":"sample","tier":"report_only","measurement_index":0,"warmup":false,"qubits":1,"measurements":1,"detectors":0,"observables":0,"repeat_depth":1,"repeat_count":100,"shots":0,"wall_time_ns":2000000,"peak_memory_bytes":1000,"status":"completed","failure_reason":null,"stderr":null}
```

- [ ] **Step 2: Write failing summary and report tests**

In `rstim/tests/perf_summary.rs`, add these tests after the existing selected summary tests:

```rust
#[test]
fn summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio() {
    let raw = include_str!("fixtures/perf/stim_style_sample_raw.jsonl");
    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");

    let stim = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "stim-cli")
        .expect("stim variant");
    let compiled = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-compiled")
        .expect("compiled variant");

    assert_eq!(stim.median_shots_per_second, Some(512_000.0));
    assert_eq!(compiled.median_shots_per_second, Some(256_000.0));

    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");
    assert_eq!(comparison.kind, "rstim_compiled_vs_stim_cli");
    assert_eq!(comparison.lhs_variant, "rstim-compiled");
    assert_eq!(comparison.rhs_variant, "stim-cli");
    assert_eq!(comparison.status, "completed");
    assert_eq!(comparison.failure_reason, None);
    assert_eq!(comparison.ratio, Some(2.0));

    let summary_json = serde_json::to_value(&summary).unwrap();
    let public_case = summary_json["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_label"] == "stim-style-surface-sample-d11-r100-b1024")
        .unwrap();
    assert!(public_case.to_string().contains("median_shots_per_second"));
    assert!(public_case
        .to_string()
        .contains("rstim_compiled_vs_stim_cli_ratio"));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("stim-style-surface-sample-d11-r100-b1024"));
    assert!(report.contains("shots/s"));
    assert!(report.contains("report-only Stim comparison"));
    assert!(report.contains("2.000000"));
}

#[test]
fn summarize_report_only_stim_comparison_surfaces_failed_variant_status() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"stim failed: boom\",\"stderr\":\"boom\\n\"}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":4000000,\"peak_memory_bytes\":1500,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(comparison.ratio, None);
    assert_eq!(comparison.status, "tool_failed");
    assert_eq!(comparison.failure_reason.as_deref(), Some("stim failed: boom"));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("report-only Stim comparison unavailable"));
    assert!(report.contains("tool_failed"));
    assert!(report.contains("stim failed: boom"));
}

#[test]
fn summarize_report_only_stim_comparison_surfaces_missing_variant_status() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":2000000,\"peak_memory_bytes\":1000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":5000000,\"peak_memory_bytes\":2000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(comparison.ratio, None);
    assert_eq!(comparison.status, "missing_variant");
    assert_eq!(
        comparison.failure_reason.as_deref(),
        Some("missing variant rstim-compiled")
    );
}

#[test]
fn summarize_rejects_zero_shot_sample_rate() {
    let raw = include_str!("fixtures/perf/stim_style_sample_zero_shots_raw.jsonl");
    let err = summarize_jsonl_str(raw).unwrap_err();
    assert!(err.contains("shots must be positive for sample rate"));
}
```

Expected before implementation: these tests fail to compile because `median_shots_per_second` and `rstim_compiled_vs_stim_cli_ratio` do not exist.

- [ ] **Step 3: Write failing CLI fixture test**

In `rstim/tests/cli_perf.rs`, add this test at the end of the file:

```rust
#[test]
fn perf_summarize_and_report_public_fixture_show_rates_and_report_only_stim_context() {
    let dir = tempfile::tempdir().unwrap();
    let summary_path = dir.path().join("summary.json");
    let report_path = dir.path().join("report.md");
    let raw_path = std::path::Path::new("rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl");

    let summarize = rstim_cmd()
        .args([
            "perf",
            "summarize",
            "--in",
            raw_path.to_str().unwrap(),
            "--out",
            summary_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        summarize.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&summarize.stderr)
    );

    let summary = std::fs::read_to_string(&summary_path).unwrap();
    assert!(summary.contains("\"median_shots_per_second\""));
    assert!(summary.contains("\"rstim_compiled_vs_stim_cli_ratio\""));

    let report = rstim_cmd()
        .args([
            "perf",
            "report",
            "--in",
            summary_path.to_str().unwrap(),
            "--out",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&report.stderr)
    );

    let report_text = std::fs::read_to_string(report_path).unwrap();
    assert!(report_text.contains(PUBLIC_SELECTED_CASE_LABEL));
    assert!(report_text.contains("shots/s"));
    assert!(report_text.contains("report-only Stim comparison"));
}
```

Expected before implementation: this test fails after summarization because the summary/report do not contain the new fields or report text.

- [ ] **Step 4: Verify RED**

Run:

```sh
cargo test -p rstim --test perf_summary summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio
cargo test -p rstim --test perf_summary summarize_report_only_stim_comparison_surfaces_failed_variant_status
cargo test -p rstim --test perf_summary summarize_report_only_stim_comparison_surfaces_missing_variant_status
cargo test -p rstim --test perf_summary summarize_rejects_zero_shot_sample_rate
cargo test -p rstim --test cli_perf perf_summarize_and_report_public_fixture_show_rates_and_report_only_stim_context
```

Expected: each command fails for the expected missing field/report behavior.

- [ ] **Step 5: Implement summary fields**

In `rstim/src/perf/summary.rs`, add `PerfWorkload` to the existing `use super::{ ... }` import list.

Add this struct immediately after `PerfComparisonSummary`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfReportOnlyComparisonSummary {
    pub kind: String,
    pub lhs_variant: String,
    pub rhs_variant: String,
    pub ratio: Option<f64>,
    pub status: String,
    pub failure_reason: Option<String>,
}
```

Add this field to `PerfVariantSummary` after `median_wall_time_ns`:

```rust
    #[serde(default)]
    pub median_shots_per_second: Option<f64>,
```

Add this field to `PerfCaseSummary` after `comparisons`:

```rust
    #[serde(default)]
    pub rstim_compiled_vs_stim_cli_ratio: Option<PerfReportOnlyComparisonSummary>,
```

Add these helper functions near the median helpers:

```rust
fn sample_rate_for_variant(
    case_label: &str,
    tool_variant: &str,
    measured: &[&PerfMeasurementRecord],
    median_wall_time_ns: u128,
) -> Result<Option<f64>, String> {
    if measured
        .first()
        .map(|record| record.workload.as_str())
        != Some(PerfWorkload::Sample.as_str())
    {
        return Ok(None);
    }
    let shots = measured[0].shots.ok_or_else(|| {
        format!("shots must be present for sample rate for case {case_label} variant {tool_variant}")
    })?;
    if shots == 0 {
        return Err(format!(
            "shots must be positive for sample rate for case {case_label} variant {tool_variant}"
        ));
    }
    if median_wall_time_ns == 0 {
        return Ok(Some(f64::INFINITY));
    }
    Ok(Some(shots as f64 * 1_000_000_000.0 / median_wall_time_ns as f64))
}

fn unavailable_stim_comparison(
    lhs_variant: &str,
    rhs_variant: &str,
    status: &str,
    failure_reason: Option<String>,
) -> PerfReportOnlyComparisonSummary {
    PerfReportOnlyComparisonSummary {
        kind: "rstim_compiled_vs_stim_cli".to_string(),
        lhs_variant: lhs_variant.to_string(),
        rhs_variant: rhs_variant.to_string(),
        ratio: None,
        status: status.to_string(),
        failure_reason,
    }
}

fn report_only_stim_comparison(
    case: super::PerfBenchmarkCase,
    variant_lookup: &BTreeMap<String, PerfVariantSummary>,
) -> Option<PerfReportOnlyComparisonSummary> {
    if case.workload != PerfWorkload::Sample || !case.requires_compiled {
        return None;
    }

    let lhs_variant = "rstim-compiled";
    let rhs_variant = "stim-cli";
    let Some(lhs) = variant_lookup.get(lhs_variant) else {
        return Some(unavailable_stim_comparison(
            lhs_variant,
            rhs_variant,
            PerfRecordStatus::MissingVariant.as_str(),
            Some(format!("missing variant {lhs_variant}")),
        ));
    };
    let Some(rhs) = variant_lookup.get(rhs_variant) else {
        return Some(unavailable_stim_comparison(
            lhs_variant,
            rhs_variant,
            PerfRecordStatus::MissingVariant.as_str(),
            Some(format!("missing variant {rhs_variant}")),
        ));
    };

    for variant in [lhs, rhs] {
        if variant.status != PerfRecordStatus::Completed.as_str() || variant.sample_count == 0 {
            return Some(unavailable_stim_comparison(
                lhs_variant,
                rhs_variant,
                &variant.status,
                variant.failure_reason.clone(),
            ));
        }
    }

    let ratio = if rhs.median_wall_time_ns == 0 {
        f64::INFINITY
    } else {
        lhs.median_wall_time_ns as f64 / rhs.median_wall_time_ns as f64
    };
    Some(PerfReportOnlyComparisonSummary {
        kind: "rstim_compiled_vs_stim_cli".to_string(),
        lhs_variant: lhs_variant.to_string(),
        rhs_variant: rhs_variant.to_string(),
        ratio: Some(ratio),
        status: PerfRecordStatus::Completed.as_str().to_string(),
        failure_reason: None,
    })
}
```

When creating a failed `PerfVariantSummary`, set:

```rust
                        median_shots_per_second: None,
```

When creating a completed `PerfVariantSummary`, compute:

```rust
                let median_shots_per_second = sample_rate_for_variant(
                    case.label,
                    &tool_variant,
                    &measured,
                    median_wall_time_ns,
                )?;
```

and set:

```rust
                    median_shots_per_second,
```

Before pushing each `PerfCaseSummary`, compute:

```rust
        let rstim_compiled_vs_stim_cli_ratio =
            report_only_stim_comparison(*case, &variant_lookup);
```

and include:

```rust
            rstim_compiled_vs_stim_cli_ratio,
```

- [ ] **Step 6: Re-export and update literals**

In `rstim/src/perf.rs`, add `PerfReportOnlyComparisonSummary` to the `pub use summary::{ ... }` list.

In `rstim/tests/perf_gate.rs`, update the `PerfVariantSummary` literal in `gate_rejects_fallback_cases_that_report_compiled_analyzer_variant` with:

```rust
        median_shots_per_second: None,
```

- [ ] **Step 7: Implement report rendering**

In `rstim/src/perf/report.rs`, replace the variant median line loop with logic equivalent to:

```rust
    for variant in &case.variants {
        out.push_str(&format!(
            "- {} median wall time: `{}` ns over `{}` measured rounds",
            variant.tool_variant, variant.median_wall_time_ns, variant.sample_count
        ));
        if let Some(rate) = variant.median_shots_per_second {
            out.push_str(&format!(" (`{:.3}` shots/s)", rate));
        }
        out.push('\n');
    }
    if let Some(comparison) = &case.rstim_compiled_vs_stim_cli_ratio {
        if let Some(ratio) = comparison.ratio {
            out.push_str(&format!(
                "- report-only Stim comparison: `{}` / `{}` = `{:.6}`\n",
                comparison.lhs_variant, comparison.rhs_variant, ratio
            ));
        } else {
            out.push_str(&format!(
                "- report-only Stim comparison unavailable: status `{}`",
                comparison.status
            ));
            if let Some(reason) = &comparison.failure_reason {
                out.push_str(&format!("; reason: `{}`", reason));
            }
            out.push('\n');
        }
    }
```

Keep the existing non-completed variant status rendering above this loop.

- [ ] **Step 8: Verify GREEN with focused tests**

Run:

```sh
cargo test -p rstim --test perf_summary summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio
cargo test -p rstim --test perf_summary summarize_report_only_stim_comparison_surfaces_failed_variant_status
cargo test -p rstim --test perf_summary summarize_report_only_stim_comparison_surfaces_missing_variant_status
cargo test -p rstim --test perf_summary summarize_rejects_zero_shot_sample_rate
cargo test -p rstim --test cli_perf perf_summarize_and_report_public_fixture_show_rates_and_report_only_stim_context
```

Expected: all commands pass.

- [ ] **Step 9: Verify issue commands**

Run:

```sh
cargo run -p rstim --bin rstim -- perf summarize \
  --in rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl \
  --out /tmp/summary.json
cargo run -p rstim --bin rstim -- perf report \
  --in /tmp/summary.json \
  --out /tmp/report.md
```

Expected: `/tmp/summary.json` contains `median_shots_per_second`, `/tmp/report.md` contains `stim-style-surface-sample-d11-r100-b1024`, `shots/s`, and `report-only Stim comparison`.

Run the negative control:

```sh
cargo run -p rstim --bin rstim -- perf summarize \
  --in rstim/tests/fixtures/perf/stim_style_sample_zero_shots_raw.jsonl \
  --out /tmp/zero-summary.json
```

Expected: command exits nonzero and stderr contains `shots must be positive for sample rate`.

- [ ] **Step 10: Run broader rstim verification and commit**

Run:

```sh
cargo test -p rstim --test perf_summary
cargo test -p rstim --test cli_perf
```

Expected: all commands pass.

Commit all implementation files:

```sh
git add rstim/src/perf/summary.rs rstim/src/perf/report.rs rstim/src/perf.rs rstim/tests/perf_summary.rs rstim/tests/perf_gate.rs rstim/tests/cli_perf.rs rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl rstim/tests/fixtures/perf/stim_style_sample_zero_shots_raw.jsonl
git commit -m "feat: report sample speed evidence"
```

Expected: commit succeeds with only issue #390 implementation files staged.

The controller runs the final required repository-level `cargo test` after task
review and records that result before opening the PR.

---

## Plan Self-Review

- Spec coverage: every issue requirement maps to Task 1 steps and verification commands.
- Placeholder scan: no TBD/TODO/placeholder instructions remain.
- Type consistency: the plan uses `median_shots_per_second`, `PerfReportOnlyComparisonSummary`, and `rstim_compiled_vs_stim_cli_ratio` consistently.
- Scope check: a single task is appropriate because the change is one summary/report surface with shared tests and fixtures.
