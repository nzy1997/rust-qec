use std::path::Path;
use std::process::ExitCode;

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

    let case = parity_schema::load_case(Path::new(&case_path));
    let report = parity_runner::run_case(&case);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    ExitCode::SUCCESS
}
