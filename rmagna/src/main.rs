mod cli;

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(cli::main())
}
