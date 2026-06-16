use clap::{Parser, Subcommand, ValueEnum};

use crate::QecError;
use crate::codes::built_in_css::built_in_css_checks;
use crate::codes::steane::Steane;
use crate::css::SparseRowsMatrix;
use crate::distance::compute_distance;

#[derive(Debug, Parser)]
#[command(name = "qec-code")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Code {
        #[command(subcommand)]
        command: CodeCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodeCommands {
    Steane {
        #[command(subcommand)]
        command: SteaneCommands,
    },
    Css {
        code_id: String,
        matrix: CssMatrixKind,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CssMatrixKind {
    Hx,
    Hz,
}

#[derive(Debug, Subcommand)]
pub enum SteaneCommands {
    Summary,
    Stabilizers,
    Logicals,
    Distance,
}

pub fn run(cli: Cli) -> Result<String, QecError> {
    match cli.command {
        Commands::Code { command } => run_code(command),
    }
}

fn run_code(command: CodeCommands) -> Result<String, QecError> {
    match command {
        CodeCommands::Steane { command } => run_steane(command),
        CodeCommands::Css { code_id, matrix } => run_css(&code_id, matrix),
    }
}

fn run_css(code_id: &str, matrix: CssMatrixKind) -> Result<String, QecError> {
    let checks = built_in_css_checks(code_id)?;
    let num_cols = checks.num_cols;
    let rows = match matrix {
        CssMatrixKind::Hx => checks.hx,
        CssMatrixKind::Hz => checks.hz,
    };

    let matrix = SparseRowsMatrix::new(num_cols, rows)?;
    Ok(matrix.to_json_string())
}

fn run_steane(command: SteaneCommands) -> Result<String, QecError> {
    let steane = Steane::new()?;
    let code = steane.code();

    match command {
        SteaneCommands::Summary => Ok(format!(
            "name: steane\nn: {}\nstabilizer_rank: {}\nk: {}",
            code.n(),
            code.stabilizer_rank(),
            code.num_logical_qubits()
        )),
        SteaneCommands::Stabilizers => {
            let lines = code
                .stabilizers()
                .iter()
                .enumerate()
                .map(|(index, stabilizer)| format!("g{}: {}", index + 1, format_pauli(stabilizer)))
                .collect::<Vec<_>>();
            Ok(lines.join("\n"))
        }
        SteaneCommands::Logicals => {
            let basis = code.logical_basis()?;
            Ok(format!(
                "k: {}\nlogical_x:\n{}\nlogical_z:\n{}",
                basis.k,
                format_pauli_list(&basis.logical_x),
                format_pauli_list(&basis.logical_z)
            ))
        }
        SteaneCommands::Distance => {
            let distance = compute_distance(code)?;
            Ok(format!(
                "distance: {}\nlogical_class: {:?}\nwitness: {}",
                distance.distance,
                distance.logical_class,
                format_pauli(&distance.witness)
            ))
        }
    }
}

fn format_pauli_list(paulis: &[crate::Pauli]) -> String {
    paulis
        .iter()
        .enumerate()
        .map(|(index, pauli)| format!("  {}: {}", index + 1, format_pauli(pauli)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_pauli(pauli: &crate::Pauli) -> String {
    format!(
        "x={:?} z={:?} weight={}",
        pauli.x_bits(),
        pauli.z_bits(),
        pauli.weight()
    )
}
