use rstim::sample_archive::corruption_corpus::{
    run_corruption_corpus, write_summary_json, CorruptionCorpusOptions, PASS_LINE,
};
use std::path::PathBuf;

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut catalog = None;
    let mut fixture_manifest = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--catalog" => catalog = Some(next_path(&mut args, "--catalog")?),
            "--fixture-manifest" => {
                fixture_manifest = Some(next_path(&mut args, "--fixture-manifest")?)
            }
            "--out" => out = Some(next_path(&mut args, "--out")?),
            "--help" | "-h" => {
                eprintln!(
                    "usage: rsmp_corruption_corpus --catalog PATH --fixture-manifest PATH --out PATH"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    let catalog_path = catalog.ok_or_else(|| "--catalog is required".to_string())?;
    let fixture_manifest_path =
        fixture_manifest.ok_or_else(|| "--fixture-manifest is required".to_string())?;
    let out_path = out.ok_or_else(|| "--out is required".to_string())?;

    let summary = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path,
        fixture_manifest_path,
    })?;
    write_summary_json(&summary, &out_path)?;
    if summary.status == "pass" {
        println!("{PASS_LINE}");
        Ok(())
    } else {
        let first_failure = summary
            .results
            .iter()
            .find(|result| result.status != "matched_error_code")
            .map(|result| {
                format!(
                    " first_failure={} status={} expected={} actual={}",
                    result.id,
                    result.status,
                    result.expected_error,
                    result.actual_error.as_deref().unwrap_or("success")
                )
            })
            .unwrap_or_default();
        Err(format!(
            "rsmp corruption corpus failed: unexpected_successes={} wrong_error_codes={} panics={} timeouts={}{}",
            summary.unexpected_successes,
            summary.wrong_error_codes,
            summary.panics,
            summary.timeouts,
            first_failure
        ))
    }
}

fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a path"))
}
