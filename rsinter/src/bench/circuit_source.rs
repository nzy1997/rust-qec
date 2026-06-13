use std::path::{Path, PathBuf};

use rstim::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::surface_code::rotated_memory_x;
use rstim::codegen::NoiseParams;
use rstim::ir::StimInstr;

use crate::bench::registry::BenchCasePoint;
use crate::bench::result::{CaseSummary, PairMapExt, ParamMap};

pub struct BuiltCircuit {
    pub circuit: Vec<StimInstr>,
    pub params: ParamMap,
    pub case_summary: CaseSummary,
}

pub fn build_circuit_for_point(
    point: &BenchCasePoint,
    spec_dir: &Path,
) -> Result<BuiltCircuit, String> {
    match point.input_type.as_str() {
        "surface_rotated_memory_x" => build_surface(point),
        "css" => build_css(point, spec_dir),
        other => Err(format!("unknown input_type: {other}")),
    }
}

fn build_surface(point: &BenchCasePoint) -> Result<BuiltCircuit, String> {
    let distance = point
        .distance
        .ok_or_else(|| "surface point is missing distance".to_string())?;
    let circuit = rotated_memory_x(distance, point.rounds, point.p);
    Ok(BuiltCircuit {
        circuit,
        params: ParamMap::from_pairs([
            ("input_type", serde_json::json!("surface_rotated_memory_x")),
            ("distance", serde_json::json!(distance)),
            ("rounds", serde_json::json!(point.rounds)),
            ("p", serde_json::json!(point.p)),
            ("max_shots", serde_json::json!(point.max_shots)),
            ("max_errors", serde_json::json!(point.max_errors)),
            ("batch_size", serde_json::json!(point.batch_size)),
        ]),
        case_summary: CaseSummary::new(),
    })
}

fn build_css(point: &BenchCasePoint, spec_dir: &Path) -> Result<BuiltCircuit, String> {
    let hx_path = point
        .hx_path
        .as_deref()
        .ok_or_else(|| "css point is missing hx".to_string())?;
    let hz_path = point
        .hz_path
        .as_deref()
        .ok_or_else(|| "css point is missing hz".to_string())?;
    let hx_text = std::fs::read_to_string(resolve_spec_path(spec_dir, hx_path))
        .map_err(|error| error.to_string())?;
    let hz_text = std::fs::read_to_string(resolve_spec_path(spec_dir, hz_path))
        .map_err(|error| error.to_string())?;
    let hx = parse_css_matrix_json(&hx_text).map_err(|error| error.to_string())?;
    let hz = parse_css_matrix_json(&hz_text).map_err(|error| error.to_string())?;
    if hx.num_cols != hz.num_cols {
        return Err(format!(
            "hx and hz widths differ: {} != {}",
            hx.num_cols, hz.num_cols
        ));
    }
    let basis_text = point.basis.as_deref().unwrap_or("x");
    let schedule_text = point.schedule.as_deref().unwrap_or("greedy");
    let basis = parse_memory_basis(basis_text)?;
    let schedule = parse_css_schedule(schedule_text)?;
    let num_data_qubits = hx.num_cols;
    let num_x_checks = hx.rows.len();
    let num_z_checks = hz.rows.len();
    let mut case_summary = CaseSummary::new();
    let observables = if let Some(path) = point.observables_path.as_deref() {
        let text = std::fs::read_to_string(resolve_spec_path(spec_dir, path))
            .map_err(|error| error.to_string())?;
        let parsed = parse_css_observable_json(&text).map_err(|error| error.to_string())?;
        if parsed.num_cols != num_data_qubits {
            return Err(format!(
                "observable width differs from CSS width: {} != {}",
                parsed.num_cols, num_data_qubits
            ));
        }
        let checks = match basis {
            MemoryBasis::X => &hz.rows,
            MemoryBasis::Z => &hx.rows,
        };
        if observables_commute_with_checks(&parsed.rows, checks) {
            case_summary.insert("observables_source".into(), serde_json::json!("explicit"));
            CssObservableSource::Explicit(parsed.rows)
        } else {
            case_summary.insert(
                "observables_source".into(),
                serde_json::json!("canonical_fallback"),
            );
            CssObservableSource::CanonicalFallback
        }
    } else {
        case_summary.insert(
            "observables_source".into(),
            serde_json::json!("canonical_fallback"),
        );
        CssObservableSource::CanonicalFallback
    };
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: hx.rows,
            hz: hz.rows,
            num_data_qubits,
        },
        rounds: point.rounds,
        noise: NoiseParams::uniform(point.p),
        basis,
        schedule,
        observables,
    })
    .map_err(|error| error.to_string())?;
    Ok(BuiltCircuit {
        circuit,
        params: ParamMap::from_pairs([
            ("input_type", serde_json::json!("css")),
            (
                "code_id",
                serde_json::json!(point.code_id.as_deref().unwrap_or("css")),
            ),
            ("basis", serde_json::json!(basis_text)),
            ("schedule", serde_json::json!(schedule_text)),
            ("rounds", serde_json::json!(point.rounds)),
            ("p", serde_json::json!(point.p)),
            ("hx", serde_json::json!(hx_path)),
            ("hz", serde_json::json!(hz_path)),
            ("max_shots", serde_json::json!(point.max_shots)),
            ("max_errors", serde_json::json!(point.max_errors)),
            ("batch_size", serde_json::json!(point.batch_size)),
        ]),
        case_summary: {
            case_summary.insert("num_data_qubits".into(), serde_json::json!(num_data_qubits));
            case_summary.insert("num_x_checks".into(), serde_json::json!(num_x_checks));
            case_summary.insert("num_z_checks".into(), serde_json::json!(num_z_checks));
            case_summary
        },
    })
}

fn observables_commute_with_checks(observables: &[Vec<usize>], checks: &[Vec<usize>]) -> bool {
    observables.iter().all(|observable| {
        checks.iter().all(|check| {
            observable
                .iter()
                .filter(|&&qubit| check.binary_search(&qubit).is_ok())
                .count()
                % 2
                == 0
        })
    })
}

fn resolve_spec_path(spec_dir: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        spec_dir.join(path)
    }
}

fn parse_memory_basis(value: &str) -> Result<MemoryBasis, String> {
    match value {
        "x" | "X" => Ok(MemoryBasis::X),
        "z" | "Z" => Ok(MemoryBasis::Z),
        other => Err(format!("unknown CSS memory basis: {other}")),
    }
}

fn parse_css_schedule(value: &str) -> Result<CssSchedule, String> {
    match value {
        "sequential" => Ok(CssSchedule::Sequential),
        "greedy" => Ok(CssSchedule::Greedy),
        other => Err(format!("unknown CSS schedule: {other}")),
    }
}
