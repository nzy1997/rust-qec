use super::{PerfCaseTier, PerfSummary};

pub fn render_markdown_report(summary: &PerfSummary, verdict_summary: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("# rstim Performance Evidence Report\n\n");

    if let Some(verdict_summary) = verdict_summary {
        out.push_str("## Gate Verdict\n\n");
        out.push_str(verdict_summary);
        if !verdict_summary.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }

    let gating_cases = summary
        .cases
        .iter()
        .filter(|case| case.tier == PerfCaseTier::Gating.as_str())
        .collect::<Vec<_>>();
    let report_only_cases = summary
        .cases
        .iter()
        .filter(|case| case.tier == PerfCaseTier::ReportOnly.as_str())
        .collect::<Vec<_>>();

    out.push_str("## Gating Cases\n\n");
    if gating_cases.is_empty() {
        out.push_str("_None._\n\n");
    } else {
        for case in gating_cases {
            render_case_section(&mut out, case);
        }
    }

    out.push_str("## Report-Only Cases\n\n");
    if report_only_cases.is_empty() {
        out.push_str("_None._\n\n");
    } else {
        for case in report_only_cases {
            render_case_section(&mut out, case);
        }
    }

    if !summary.issues.is_empty() {
        out.push_str("## Summary Issues\n\n");
        for issue in &summary.issues {
            out.push_str(&format!(
                "- `{:?}` in `{}`: {}\n",
                issue.kind, issue.case_label, issue.message
            ));
        }
        out.push('\n');
    }

    out
}

fn render_case_section(out: &mut String, case: &super::PerfCaseSummary) {
    out.push_str(&format!("### {}\n\n", case.case_label));
    out.push_str(&format!("- workload: `{}`\n", case.workload));
    out.push_str(&format!(
        "- expected variants: `{}`\n",
        case.expected_variants.join("`, `")
    ));
    out.push_str(&format!(
        "- present variants: `{}`\n",
        case.present_variants.join("`, `")
    ));
    if case
        .expected_variants
        .iter()
        .any(|variant| variant == "rstim-interpreted-atom-loss")
    {
        out.push_str(
            "- atom-loss probability: each two-qubit gate has one depolarization event and two independent per-atom loss events; using `p = 1 - 0.999^(1/3) ~= 0.0003334445062` keeps the probability of at least one error equal to `0.001`.\n",
        );
        out.push_str(
            "- atom-loss execution: loss masks and Pauli frames are propagated in 64-shot bitsets; the reported wall time includes the complete loss-aware batch.\n",
        );
    }
    for variant in &case.variants {
        if variant.status != "completed" {
            out.push_str(&format!(
                "- {} status: `{}`",
                variant.tool_variant, variant.status
            ));
            if let Some(reason) = &variant.failure_reason {
                out.push_str(&format!("; reason: `{}`", reason));
            }
            out.push('\n');
        }
    }
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
    for comparison in &case.comparisons {
        out.push_str(&format!(
            "- {}: `{}` / `{}` = `{:.6}`\n",
            comparison.kind, comparison.lhs_variant, comparison.rhs_variant, comparison.ratio
        ));
    }
    out.push('\n');
}
