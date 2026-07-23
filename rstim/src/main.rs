use clap::Parser;

fn main() {
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
