use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rsinter",
    version,
    about = "Rust benchmark and sampling harness",
    after_help = "bench subcommands: run, merge, plot"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Bench {
        #[command(subcommand)]
        command: BenchCommands,
    },
}

#[derive(Subcommand)]
enum BenchCommands {
    Run {
        #[arg(long)]
        spec: String,
        #[arg(long)]
        language: String,
    },
    Merge {
        #[arg(long)]
        spec: String,
        #[arg(long = "input")]
        input: Vec<String>,
        #[arg(long)]
        out: String,
    },
    Plot {
        #[arg(long)]
        spec: String,
        #[arg(long = "input")]
        input: Vec<String>,
        #[arg(long)]
        out: String,
    },
}

fn main() {
    let _cli = Cli::parse();
}
