use clap::Parser;

#[cfg(feature = "unified-cli")]
fn runs_unified_command() -> bool {
    matches!(
        std::env::args().nth(1).as_deref(),
        Some("circuit" | "dataset" | "decode" | "capabilities" | "--error-format")
    )
}

fn main() {
    #[cfg(feature = "unified-cli")]
    if runs_unified_command() {
        use std::io;
        let mut stdin = io::stdin().lock();
        let mut stdout = io::stdout().lock();
        match rstim::unified_cli::run(std::env::args_os(), &mut stdin, &mut stdout) {
            Ok(()) => return,
            Err(error) => {
                rstim::unified_cli::write_error(&error, &mut stdout, &mut io::stderr().lock());
                std::process::exit(rstim::unified_cli::exit_code(&error).into());
            }
        }
    }
    let cli = rstim::cli::Cli::parse();
    if let Err(e) = rstim::cli::run(cli) {
        if e.starts_with("rsmp error [") {
            eprintln!("{e}");
        } else {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }
}
