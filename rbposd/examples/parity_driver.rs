use std::fs;
use std::process::ExitCode;

#[allow(dead_code)]
#[path = "../dev/parity_schema.rs"]
mod parity_schema;
#[path = "../dev/parity_runner.rs"]
mod parity_runner;

fn main() -> ExitCode {
    let mut args = std::env::args();
    let program = args.next().unwrap_or_else(|| "parity_driver".to_string());
    let Some(case_path) = args.next() else {
        eprintln!("usage: {program} <parity-case.json>");
        return ExitCode::FAILURE;
    };

    let contents = match fs::read_to_string(&case_path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {}: {err}", case_path);
            return ExitCode::FAILURE;
        }
    };
    let case: parity_schema::ParityCase = match serde_json::from_str(&contents) {
        Ok(case) => case,
        Err(err) => {
            eprintln!("failed to parse {}: {err}", case_path);
            return ExitCode::FAILURE;
        }
    };
    let report = parity_runner::run_case(&case);
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("failed to serialize report: {err}");
            ExitCode::FAILURE
        }
    }
}
