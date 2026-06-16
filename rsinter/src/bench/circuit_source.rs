use std::path::{Path, PathBuf};

use rstim::codegen::NoiseParams;
use rstim::codegen::css::{
    CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis, css_memory,
    parse_css_matrix_json, parse_css_observable_json,
};
use rstim::codegen::surface_code::{rotated_memory_x, rotated_memory_z};
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
        "surface_rotated_memory_x" | "surface_rotated_memory_z" => build_surface(point),
        "css" => build_css(point, spec_dir),
        other => Err(format!("unknown input_type: {other}")),
    }
}

fn build_surface(point: &BenchCasePoint) -> Result<BuiltCircuit, String> {
    let distance = point
        .distance
        .ok_or_else(|| "surface point is missing distance".to_string())?;
    let circuit = match point.input_type.as_str() {
        "surface_rotated_memory_x" => rotated_memory_x(distance, point.rounds, point.p),
        "surface_rotated_memory_z" => rotated_memory_z(distance, point.rounds, point.p),
        other => return Err(format!("unknown input_type: {other}")),
    };
    let mut params = ParamMap::from_pairs([
        ("input_type", serde_json::json!(point.input_type.as_str())),
        ("distance", serde_json::json!(distance)),
        ("rounds", serde_json::json!(point.rounds)),
        ("p", serde_json::json!(point.p)),
        ("max_shots", serde_json::json!(point.max_shots)),
        ("max_errors", serde_json::json!(point.max_errors)),
        ("batch_size", serde_json::json!(point.batch_size)),
    ]);
    insert_max_wall_seconds(&mut params, point.max_wall_seconds);
    Ok(BuiltCircuit {
        circuit,
        params,
        case_summary: CaseSummary::new(),
    })
}

fn insert_max_wall_seconds(params: &mut ParamMap, max_wall_seconds: Option<f64>) {
    if let Some(seconds) = max_wall_seconds {
        params.insert("max_wall_seconds".into(), serde_json::json!(seconds));
    }
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
    let hx_text = read_spec_text(spec_dir, "hx", hx_path)?;
    let hz_text = read_spec_text(spec_dir, "hz", hz_path)?;
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
    let observables = if let Some(path) = point.observables_path.as_deref() {
        let text = read_spec_text(spec_dir, "observables", path)?;
        let parsed = parse_css_observable_json(&text).map_err(|error| error.to_string())?;
        if parsed.num_cols != num_data_qubits {
            return Err(format!(
                "observable width differs from CSS width: {} != {}",
                parsed.num_cols, num_data_qubits
            ));
        }
        CssObservableSource::Explicit(parsed.rows)
    } else {
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
    let mut params = ParamMap::from_pairs([
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
    ]);
    params.insert(
        "observables".into(),
        serde_json::json!(
            point
                .observables_path
                .as_deref()
                .unwrap_or("canonical_fallback")
        ),
    );
    insert_max_wall_seconds(&mut params, point.max_wall_seconds);
    Ok(BuiltCircuit {
        circuit,
        params,
        case_summary: CaseSummary::new(),
    })
}

fn read_spec_text(spec_dir: &Path, field: &str, value: &str) -> Result<String, String> {
    let path = resolve_spec_path(spec_dir, value);
    std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read CSS {field} file {}: {error}",
            path.display()
        )
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
