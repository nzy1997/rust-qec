use clap::Parser;

fn main() {
    let cli = rstim::cli::Cli::parse();
    if let Err(e) = rstim::cli::run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
