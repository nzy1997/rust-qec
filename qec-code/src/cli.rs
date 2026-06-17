use std::fs;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::codes::built_in_css::{built_in_css_catalog, built_in_css_checks};
use crate::codes::steane::Steane;
use crate::css::{sparse_rows_matrix_from_json_str, CssCode, SparseRowsMatrix};
use crate::distance::compute_distance;
use crate::distance_bound::{randomized_css_upper_bound, RandomizedUpperBoundOptions};
use crate::distance_exact::{
    ExactCssDistanceInput, ExactCssDistanceOptions, ExactCssDistanceResult,
};
use crate::error::CssMatrixReadSource;
use crate::QecError;

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
    Css(CssArgs),
    CssDistance {
        #[command(subcommand)]
        command: CssDistanceCommands,
    },
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_negates_reqs = true)]
#[command(arg_required_else_help = true)]
pub struct CssArgs {
    #[command(subcommand)]
    command: Option<CssCommands>,
    #[arg(value_name = "CODE_ID", required = true)]
    code_id: Option<String>,
    #[arg(value_name = "MATRIX", required = true)]
    matrix: Option<CssMatrixKind>,
}

impl CssArgs {
    pub fn list() -> Self {
        Self {
            command: Some(CssCommands::List),
            code_id: None,
            matrix: None,
        }
    }

    pub fn export(code_id: String, matrix: CssMatrixKind) -> Self {
        Self {
            command: None,
            code_id: Some(code_id),
            matrix: Some(matrix),
        }
    }

    pub fn export_subcommand(code_id: String, matrix: CssMatrixKind) -> Self {
        Self {
            command: Some(CssCommands::Export { code_id, matrix }),
            code_id: None,
            matrix: None,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CssCommands {
    List,
    Export {
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
pub enum CssDistanceCommands {
    Exact(ExactCssDistanceCli),
    RandomizedUpperBound(RandomizedUpperBoundCli),
}

#[derive(Debug, Args)]
pub struct ExactCssDistanceCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
pub struct RandomizedUpperBoundCli {
    #[arg(long)]
    code_id: Option<String>,
    #[arg(long)]
    hx: Option<PathBuf>,
    #[arg(long)]
    hz: Option<PathBuf>,
    #[arg(long)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    restarts: usize,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    target_weight: Option<usize>,
    #[arg(long)]
    json: bool,
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
        CodeCommands::Css(args) => run_css_args(args),
        CodeCommands::CssDistance { command } => run_css_distance(command),
    }
}

fn run_css_args(args: CssArgs) -> Result<String, QecError> {
    match args.command {
        Some(CssCommands::List) => Ok(run_css_list()),
        Some(CssCommands::Export { code_id, matrix }) => run_css(&code_id, matrix),
        None => {
            let code_id = args
                .code_id
                .expect("clap requires CODE_ID when no css subcommand is used");
            let matrix = args
                .matrix
                .expect("clap requires MATRIX when no css subcommand is used");

            run_css(&code_id, matrix)
        }
    }
}

fn run_css_list() -> String {
    let catalog = built_in_css_catalog();
    let width = catalog
        .iter()
        .map(|entry| entry.spec.len())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::with_capacity(catalog.len() + 1);

    lines.push("Built-in CSS codes:".to_owned());
    lines.extend(catalog.iter().map(|entry| {
        format!(
            "  {:width$}  {}",
            entry.spec,
            entry.description,
            width = width
        )
    }));

    lines.join("\n")
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

fn run_css_distance(command: CssDistanceCommands) -> Result<String, QecError> {
    match command {
        CssDistanceCommands::Exact(options) => run_css_exact_distance(options),
        CssDistanceCommands::RandomizedUpperBound(options) => {
            run_css_randomized_upper_bound(options)
        }
    }
}

fn run_css_exact_distance(cli: ExactCssDistanceCli) -> Result<String, QecError> {
    const COMMAND: &str = "code css-distance exact";

    if !cli.json {
        return Err(QecError::JsonOutputRequired { command: COMMAND });
    }

    let (css, options) = css_code_and_exact_options_from_cli(&cli)?;
    let distance = compute_distance(css.code())?;
    let result = ExactCssDistanceResult::completed(distance, options);

    serde_json::to_string(&result).map_err(|err| QecError::InvalidCssDistanceInput(err.to_string()))
}

fn css_code_and_exact_options_from_cli(
    cli: &ExactCssDistanceCli,
) -> Result<(CssCode, ExactCssDistanceOptions), QecError> {
    match (&cli.code_id, &cli.hx, &cli.hz) {
        (Some(code_id), None, None) => Ok((
            css_code_from_built_in(code_id)?,
            ExactCssDistanceOptions {
                input: ExactCssDistanceInput::CodeId {
                    code_id: code_id.clone(),
                },
            },
        )),
        (None, Some(hx), Some(hz)) => Ok((
            css_code_from_files(hx, hz)?,
            ExactCssDistanceOptions {
                input: ExactCssDistanceInput::Files {
                    hx: hx.display().to_string(),
                    hz: hz.display().to_string(),
                },
            },
        )),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "use either --code-id or --hx/--hz, not both".to_owned(),
        )),
        (None, Some(_), None) | (None, None, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "--hx and --hz must be provided together".to_owned(),
        )),
        (None, None, None) => Err(QecError::InvalidCssDistanceInput(
            "provide --code-id or both --hx and --hz".to_owned(),
        )),
    }
}

fn run_css_randomized_upper_bound(cli: RandomizedUpperBoundCli) -> Result<String, QecError> {
    const COMMAND: &str = "code css-distance randomized-upper-bound";

    if !cli.json {
        return Err(QecError::JsonOutputRequired { command: COMMAND });
    }

    let css = css_code_from_randomized_upper_bound_cli(&cli)?;
    let options = RandomizedUpperBoundOptions {
        iterations: cli.iterations,
        restarts: cli.restarts,
        seed: cli.seed,
        target_weight: cli.target_weight,
    };
    let result = randomized_css_upper_bound(&css, options)?;

    serde_json::to_string(&result).map_err(|err| QecError::InvalidCssDistanceInput(err.to_string()))
}

fn css_code_from_randomized_upper_bound_cli(
    cli: &RandomizedUpperBoundCli,
) -> Result<CssCode, QecError> {
    match (&cli.code_id, &cli.hx, &cli.hz) {
        (Some(code_id), None, None) => css_code_from_built_in(code_id),
        (None, Some(hx), Some(hz)) => css_code_from_files(hx, hz),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "use either --code-id or --hx/--hz, not both".to_owned(),
        )),
        (None, Some(_), None) | (None, None, Some(_)) => Err(QecError::InvalidCssDistanceInput(
            "--hx and --hz must be provided together".to_owned(),
        )),
        (None, None, None) => Err(QecError::InvalidCssDistanceInput(
            "provide --code-id or both --hx and --hz".to_owned(),
        )),
    }
}

fn css_code_from_built_in(code_id: &str) -> Result<CssCode, QecError> {
    let checks = built_in_css_checks(code_id)?;
    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx)?.to_dense_rows();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz)?.to_dense_rows();
    CssCode::from_hx_hz(hx, hz)
}

fn css_code_from_files(hx_path: &PathBuf, hz_path: &PathBuf) -> Result<CssCode, QecError> {
    let hx = read_css_sparse_rows_matrix(hx_path)?;
    let hz = read_css_sparse_rows_matrix(hz_path)?;

    if hx.num_cols() != hz.num_cols() {
        return Err(QecError::InvalidCssDistanceInput(format!(
            "hx width {} does not match hz width {}",
            hx.num_cols(),
            hz.num_cols()
        )));
    }

    CssCode::from_hx_hz(hx.to_dense_rows(), hz.to_dense_rows())
}

fn read_css_sparse_rows_matrix(path: &PathBuf) -> Result<SparseRowsMatrix, QecError> {
    let input = fs::read_to_string(path).map_err(|err| QecError::CssMatrixReadFailed {
        path: path.display().to_string(),
        source: CssMatrixReadSource(err.to_string()),
    })?;

    sparse_rows_matrix_from_json_str(&input)
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
