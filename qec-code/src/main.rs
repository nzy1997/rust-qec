use clap::Parser;
use qec_code::cli::{Cli, run};

fn main() {
    let cli = Cli::parse();

    match run(cli) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
