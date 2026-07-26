use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::binary::try_binary_rank;
use crate::codes::built_in_css::{
    BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams, built_in_css_checks,
    parse_built_in_css_code_spec,
};
use crate::codes::quantum_tanner::{
    QuantumTannerSpec, quantum_tanner_css_checks, quantum_tanner_spec_from_json_str,
};
use crate::codes::toric_3d::{Toric3dSpec, toric_3d_css_checks};
use crate::css::SparseRowsMatrix;
use crate::error::{QecError, Result};

pub const CSS_CONSTRUCTION_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedFamilyId {
    Directional,
    QuantumTanner,
    GeneralizedBicycle,
    LaCross,
    RandomHgp,
    LiftedProduct,
    #[serde(rename = "hyperbolic_5_5")]
    Hyperbolic55,
    CoprimeBb,
    #[serde(rename = "toric_3d")]
    Toric3d,
    #[serde(rename = "color_666")]
    Color666,
    Surface,
    ShorLike,
    RandomTwoBlock,
    PerturbedHgp,
}

impl RequestedFamilyId {
    pub const ALL: [Self; 14] = [
        Self::Directional,
        Self::QuantumTanner,
        Self::GeneralizedBicycle,
        Self::LaCross,
        Self::RandomHgp,
        Self::LiftedProduct,
        Self::Hyperbolic55,
        Self::CoprimeBb,
        Self::Toric3d,
        Self::Color666,
        Self::Surface,
        Self::ShorLike,
        Self::RandomTwoBlock,
        Self::PerturbedHgp,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::QuantumTanner => "quantum_tanner",
            Self::GeneralizedBicycle => "generalized_bicycle",
            Self::LaCross => "la_cross",
            Self::RandomHgp => "random_hgp",
            Self::LiftedProduct => "lifted_product",
            Self::Hyperbolic55 => "hyperbolic_5_5",
            Self::CoprimeBb => "coprime_bb",
            Self::Toric3d => "toric_3d",
            Self::Color666 => "color_666",
            Self::Surface => "surface",
            Self::ShorLike => "shor_like",
            Self::RandomTwoBlock => "random_two_block",
            Self::PerturbedHgp => "perturbed_hgp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceLayout {
    Rotated,
    Unrotated,
}

impl SurfaceLayout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rotated => "rotated",
            Self::Unrotated => "unrotated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSpec {
    pub layout: SurfaceLayout,
    pub row_distance: usize,
    pub column_distance: usize,
}

impl SurfaceSpec {
    pub const fn rotated_square(distance: usize) -> Self {
        Self {
            layout: SurfaceLayout::Rotated,
            row_distance: distance,
            column_distance: distance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceFamilySpec {
    pub distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssFamilySpec {
    Surface(SurfaceFamilySpec),
    QuantumTanner(QuantumTannerSpec),
    Toric3d(Toric3dSpec),
}

impl CssFamilySpec {
    pub const fn callable_requested_family_ids() -> &'static [RequestedFamilyId] {
        &[
            RequestedFamilyId::Surface,
            RequestedFamilyId::QuantumTanner,
            RequestedFamilyId::Toric3d,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssClassicalCheckSpec {
    pub num_cols: usize,
    pub rows: Vec<Vec<usize>>,
}

pub static CLASSICAL_IDENTITY_2: LazyLock<CssClassicalCheckSpec> =
    LazyLock::new(|| CssClassicalCheckSpec {
        num_cols: 2,
        rows: vec![vec![0], vec![1]],
    });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypergraphProductSpec {
    pub left: CssClassicalCheckSpec,
    pub right: CssClassicalCheckSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyBuiltInCssSpec {
    pub code_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssConstructionSpec {
    Family(CssFamilySpec),
    Surface(SurfaceSpec),
    HypergraphProduct(HypergraphProductSpec),
    LegacyBuiltIn(LegacyBuiltInCssSpec),
}

impl From<CssFamilySpec> for CssConstructionSpec {
    fn from(value: CssFamilySpec) -> Self {
        Self::Family(value)
    }
}

impl From<SurfaceSpec> for CssConstructionSpec {
    fn from(value: SurfaceSpec) -> Self {
        Self::Surface(value)
    }
}

impl CssConstructionSpec {
    pub fn from_inline(input: &str) -> Result<Self> {
        let parsed = parse_built_in_css_code_spec(input)?;
        if let BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::SurfaceRotated,
            params: BuiltInCssParams::Distance { distance },
        } = parsed
        {
            return Ok(CssFamilySpec::Surface(SurfaceFamilySpec { distance }).into());
        }

        if let BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::Toric3d,
            params: BuiltInCssParams::Toric3d(spec),
        } = parsed
        {
            return Ok(CssFamilySpec::Toric3d(spec).into());
        }

        Ok(Self::LegacyBuiltIn(LegacyBuiltInCssSpec {
            code_id: input.to_owned(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssChecks {
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssCodeStats {
    pub n: usize,
    pub m_x: usize,
    pub m_z: usize,
    pub rank_x: usize,
    pub rank_z: usize,
    pub k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d_x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d_z: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssConstructionProvenance {
    pub adapter: String,
    pub source: String,
    pub normalized_input_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CssConstructionResult {
    pub schema_version: u64,
    pub construction_id: String,
    pub requested_family_id: Option<RequestedFamilyId>,
    pub normalized_parameters: BTreeMap<String, Value>,
    pub checks: CssChecks,
    pub stats: CssCodeStats,
    pub provenance: CssConstructionProvenance,
}

pub fn construct_css(spec: CssConstructionSpec) -> Result<CssConstructionResult> {
    match spec {
        CssConstructionSpec::Family(CssFamilySpec::Surface(spec)) => construct_legacy_surface(spec),
        CssConstructionSpec::Family(CssFamilySpec::QuantumTanner(spec)) => {
            let checks = quantum_tanner_css_checks(&spec)?;
            let parameters = quantum_tanner_normalized_parameters(&spec);
            construction_result(
                "quantum_tanner",
                Some(RequestedFamilyId::QuantumTanner),
                parameters,
                checks.num_cols,
                checks.hx,
                checks.hz,
                "quantum_tanner",
                "CssFamilySpec::QuantumTanner",
                None,
            )
        }
        CssConstructionSpec::Family(CssFamilySpec::Toric3d(spec)) => {
            let checks = toric_3d_css_checks(spec)?;
            let mut parameters = BTreeMap::new();
            parameters.insert("lx".to_owned(), Value::from(spec.lx));
            parameters.insert("ly".to_owned(), Value::from(spec.ly));
            parameters.insert("lz".to_owned(), Value::from(spec.lz));
            construction_result(
                "toric_3d",
                Some(RequestedFamilyId::Toric3d),
                parameters,
                checks.num_cols,
                checks.hx,
                checks.hz,
                "toric_3d_chain_complex",
                "CssFamilySpec::Toric3d",
                Some((checks.distances.d_x, checks.distances.d_z)),
            )
        }
        CssConstructionSpec::Surface(spec) => construct_surface(spec),
        CssConstructionSpec::HypergraphProduct(spec) => construct_hypergraph_product(spec),
        CssConstructionSpec::LegacyBuiltIn(spec) => {
            if let Some(distance) = legacy_surface_distance_from_code_id(&spec.code_id) {
                preflight_legacy_surface_overflow(distance)?;
            }

            let checks = built_in_css_checks(&spec.code_id)?;
            let mut parameters = BTreeMap::new();
            parameters.insert("code_id".to_owned(), Value::from(spec.code_id));
            construction_result(
                checks.code_id,
                None,
                parameters,
                checks.num_cols,
                checks.hx,
                checks.hz,
                "built_in_css",
                "CssConstructionSpec::LegacyBuiltIn",
                None,
            )
        }
    }
}

fn legacy_surface_distance_from_code_id(code_id: &str) -> Option<usize> {
    match parse_built_in_css_code_spec(code_id).ok()? {
        BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::SurfaceRotated,
            params: BuiltInCssParams::Distance { distance },
        } => Some(distance),
        _ => None,
    }
}

fn construct_legacy_surface(spec: SurfaceFamilySpec) -> Result<CssConstructionResult> {
    preflight_legacy_surface_overflow(spec.distance)?;

    let checks = built_in_css_checks(&format!("surface_rotated:d={}", spec.distance))?;
    let mut parameters = BTreeMap::new();
    parameters.insert("distance".to_owned(), Value::from(spec.distance));
    construction_result(
        checks.code_id,
        Some(RequestedFamilyId::Surface),
        parameters,
        checks.num_cols,
        checks.hx,
        checks.hz,
        "built_in_css",
        "CssFamilySpec::Surface",
        Some((spec.distance, spec.distance)),
    )
}

fn construct_surface(spec: SurfaceSpec) -> Result<CssConstructionResult> {
    validate_surface_spec(&spec)?;
    let (n, h_x, h_z, construction_id) = match spec.layout {
        SurfaceLayout::Rotated => {
            let n = spec
                .row_distance
                .checked_mul(spec.column_distance)
                .ok_or_else(|| surface_overflow("data qubit count"))?;
            let (h_x, h_z) = rotated_surface_supports(spec.row_distance, spec.column_distance);
            (n, h_x, h_z, "surface_rotated")
        }
        SurfaceLayout::Unrotated => {
            let n = unrotated_surface_num_data_qubits(spec.row_distance, spec.column_distance)?;
            let (h_x, h_z) = unrotated_surface_supports(spec.row_distance, spec.column_distance)?;
            (n, h_x, h_z, "surface_unrotated")
        }
    };
    let mut parameters = BTreeMap::new();
    parameters.insert("layout".to_owned(), Value::from(spec.layout.as_str()));
    parameters.insert("row_distance".to_owned(), Value::from(spec.row_distance));
    parameters.insert(
        "column_distance".to_owned(),
        Value::from(spec.column_distance),
    );
    construction_result(
        construction_id,
        Some(RequestedFamilyId::Surface),
        parameters,
        n,
        h_x,
        h_z,
        "surface",
        "CssConstructionSpec::Surface",
        Some((spec.column_distance, spec.row_distance)),
    )
}

fn validate_surface_spec(spec: &SurfaceSpec) -> Result<()> {
    validate_surface_distance("row_distance", spec.row_distance)?;
    validate_surface_distance("column_distance", spec.column_distance)?;

    if matches!(spec.layout, SurfaceLayout::Rotated)
        && (spec.row_distance > isize::MAX as usize / 2
            || spec.column_distance > isize::MAX as usize / 2)
    {
        return Err(surface_overflow("rotated coordinate arithmetic"));
    }

    Ok(())
}

fn validate_surface_distance(parameter: &'static str, value: usize) -> Result<()> {
    if value < 2 {
        return Err(QecError::InvalidCssConstruction {
            construction: "surface".to_owned(),
            reason: format!("{parameter} must be at least 2, got {value}"),
        });
    }
    Ok(())
}

fn preflight_legacy_surface_overflow(distance: usize) -> Result<()> {
    distance
        .checked_mul(distance)
        .ok_or_else(|| surface_overflow("data qubit count"))?;
    if distance > isize::MAX as usize / 2 {
        return Err(surface_overflow("rotated coordinate arithmetic"));
    }
    Ok(())
}

fn surface_overflow(operation: &'static str) -> QecError {
    QecError::InvalidCssConstruction {
        construction: "surface".to_owned(),
        reason: format!("surface dimension overflow during {operation}"),
    }
}

fn rotated_surface_supports(
    row_distance: usize,
    column_distance: usize,
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut h_x = Vec::new();
    let mut h_z = Vec::new();

    for ax in 0..=row_distance {
        for ay in 0..=column_distance {
            let on_row_boundary = ax == 0 || ax == row_distance;
            let on_column_boundary = ay == 0 || ay == column_distance;
            let parity = (ax % 2) != (ay % 2);
            if on_row_boundary && parity {
                continue;
            }
            if on_column_boundary && !parity {
                continue;
            }

            let support = rotated_surface_measure_support(row_distance, column_distance, ax, ay);
            if support.is_empty() {
                continue;
            }

            if parity {
                h_x.push(support);
            } else {
                h_z.push(support);
            }
        }
    }

    (h_x, h_z)
}

fn rotated_surface_measure_support(
    row_distance: usize,
    column_distance: usize,
    ax: usize,
    ay: usize,
) -> Vec<usize> {
    let mut support = Vec::new();
    let mx = (2 * ax) as isize;
    let my = (2 * ay) as isize;

    for (dx, dy) in [(1isize, 1isize), (1, -1), (-1, 1), (-1, -1)] {
        let x = mx + dx;
        let y = my + dy;
        if x >= 1
            && x <= (2 * row_distance - 1) as isize
            && y >= 1
            && y <= (2 * column_distance - 1) as isize
            && x % 2 == 1
            && y % 2 == 1
        {
            let qx = ((x - 1) / 2) as usize;
            let qy = ((y - 1) / 2) as usize;
            if qx < row_distance && qy < column_distance {
                support.push(qx * column_distance + qy);
            }
        }
    }

    support.sort_unstable();
    support.dedup();
    support
}

fn unrotated_surface_num_data_qubits(row_distance: usize, column_distance: usize) -> Result<usize> {
    let grid_rows = checked_surface_grid_extent(row_distance, "row grid extent")?;
    let grid_columns = checked_surface_grid_extent(column_distance, "column grid extent")?;
    grid_rows
        .checked_mul(grid_columns)
        .and_then(|count| count.checked_add(1))
        .map(|count| count / 2)
        .ok_or_else(|| surface_overflow("data qubit count"))
}

fn checked_surface_grid_extent(distance: usize, operation: &'static str) -> Result<usize> {
    distance
        .checked_mul(2)
        .and_then(|extent| extent.checked_sub(1))
        .ok_or_else(|| surface_overflow(operation))
}

fn unrotated_surface_data_indices(
    grid_rows: usize,
    grid_columns: usize,
) -> Result<Vec<Vec<Option<usize>>>> {
    let mut data_indices = Vec::with_capacity(grid_rows);
    let mut next_index = 0usize;
    for row in 0..grid_rows {
        let mut indices = Vec::with_capacity(grid_columns);
        for column in 0..grid_columns {
            if (row % 2) == (column % 2) {
                indices.push(Some(next_index));
                next_index = next_index
                    .checked_add(1)
                    .ok_or_else(|| surface_overflow("data qubit count"))?;
            } else {
                indices.push(None);
            }
        }
        data_indices.push(indices);
    }
    Ok(data_indices)
}

fn unrotated_surface_supports(
    row_distance: usize,
    column_distance: usize,
) -> Result<(Vec<Vec<usize>>, Vec<Vec<usize>>)> {
    let grid_rows = checked_surface_grid_extent(row_distance, "row grid extent")?;
    let grid_columns = checked_surface_grid_extent(column_distance, "column grid extent")?;
    let data_indices = unrotated_surface_data_indices(grid_rows, grid_columns)?;
    let mut h_x = Vec::with_capacity(
        row_distance
            .checked_sub(1)
            .and_then(|rows| rows.checked_mul(column_distance))
            .ok_or_else(|| surface_overflow("X-check count"))?,
    );
    let mut h_z = Vec::with_capacity(
        column_distance
            .checked_sub(1)
            .and_then(|columns| row_distance.checked_mul(columns))
            .ok_or_else(|| surface_overflow("Z-check count"))?,
    );

    for row in (1..grid_rows).step_by(2) {
        for column in (0..grid_columns).step_by(2) {
            h_x.push(unrotated_surface_check_support(
                grid_rows,
                grid_columns,
                &data_indices,
                row,
                column,
            ));
        }
    }
    for row in (0..grid_rows).step_by(2) {
        for column in (1..grid_columns).step_by(2) {
            h_z.push(unrotated_surface_check_support(
                grid_rows,
                grid_columns,
                &data_indices,
                row,
                column,
            ));
        }
    }

    Ok((h_x, h_z))
}

fn unrotated_surface_check_support(
    grid_rows: usize,
    grid_columns: usize,
    data_indices: &[Vec<Option<usize>>],
    row: usize,
    column: usize,
) -> Vec<usize> {
    let mut support = Vec::with_capacity(4);
    for (neighbor_row, neighbor_column) in [
        (row.checked_sub(1), Some(column)),
        (Some(row), column.checked_sub(1)),
        (Some(row), column.checked_add(1)),
        (row.checked_add(1), Some(column)),
    ] {
        if let (Some(neighbor_row), Some(neighbor_column)) = (neighbor_row, neighbor_column)
            && neighbor_row < grid_rows
            && neighbor_column < grid_columns
            && let Some(index) = data_indices[neighbor_row][neighbor_column]
        {
            support.push(index);
        }
    }
    support
}

fn quantum_tanner_normalized_parameters(spec: &QuantumTannerSpec) -> BTreeMap<String, Value> {
    let mut base_group = BTreeMap::new();
    base_group.insert(
        "name".to_owned(),
        spec.base_group
            .name
            .as_ref()
            .map_or(Value::Null, |value| Value::from(value.clone())),
    );
    base_group.insert(
        "element_order".to_owned(),
        spec.base_group
            .element_order
            .as_ref()
            .map_or(Value::Null, |value| Value::from(value.clone())),
    );
    base_group.insert("order".to_owned(), Value::from(spec.base_group.order));
    base_group.insert("identity".to_owned(), Value::from(spec.base_group.identity));
    base_group.insert(
        "multiplication_table".to_owned(),
        serde_json::to_value(&spec.base_group.multiplication_table).expect("serializable table"),
    );

    let mut local_codes = BTreeMap::new();
    local_codes.insert(
        "matrix_role".to_owned(),
        Value::from(spec.local_codes.matrix_role.clone()),
    );
    local_codes.insert(
        "field".to_owned(),
        Value::from(spec.local_codes.field.clone()),
    );
    local_codes.insert(
        "h_a".to_owned(),
        serde_json::to_value(&spec.local_codes.h_a).expect("serializable h_a"),
    );
    local_codes.insert(
        "h_b".to_owned(),
        serde_json::to_value(&spec.local_codes.h_b).expect("serializable h_b"),
    );
    local_codes.insert(
        "g_a".to_owned(),
        serde_json::to_value(&spec.local_codes.g_a).expect("serializable g_a"),
    );
    local_codes.insert(
        "g_b".to_owned(),
        serde_json::to_value(&spec.local_codes.g_b).expect("serializable g_b"),
    );

    let mut parameters = BTreeMap::new();
    parameters.insert(
        "construction_mode".to_owned(),
        Value::from(spec.construction_mode.as_str()),
    );
    parameters.insert(
        "base_group".to_owned(),
        serde_json::to_value(base_group).expect("serializable base_group"),
    );
    parameters.insert(
        "a_generator_indices".to_owned(),
        serde_json::to_value(&spec.a_generator_indices).expect("serializable a generators"),
    );
    parameters.insert(
        "b_generator_indices".to_owned(),
        serde_json::to_value(&spec.b_generator_indices).expect("serializable b generators"),
    );
    parameters.insert(
        "local_codes".to_owned(),
        serde_json::to_value(local_codes).expect("serializable local_codes"),
    );
    parameters
}

pub fn parse_css_construction_json(input: &str) -> Result<CssConstructionSpec> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidCssConstructionJson(error.to_string()))?;
    let object = value.as_object().ok_or_else(|| {
        QecError::InvalidCssConstructionJson(
            "construction request must be a JSON object".to_owned(),
        )
    })?;
    let version = required_u64(object, "schema_version")?;
    if version != CSS_CONSTRUCTION_SCHEMA_VERSION {
        return Err(QecError::UnsupportedCssConstructionSchemaVersion { version });
    }
    let construction = required_string(object, "construction")?;
    match construction {
        "surface" => surface_construction_from_json(object, construction),
        "quantum_tanner" => {
            let spec_value = object.get("spec").unwrap_or(&value);
            let mut spec_object = spec_value.as_object().cloned().ok_or_else(|| {
                QecError::InvalidCssConstruction {
                    construction: construction.to_owned(),
                    reason: "spec must be a JSON object".to_owned(),
                }
            })?;
            spec_object.remove("schema_version");
            spec_object.remove("construction");
            let spec_json = serde_json::to_string(&spec_object)
                .expect("JSON object serialization should not fail");
            Ok(CssFamilySpec::QuantumTanner(quantum_tanner_spec_from_json_str(&spec_json)?).into())
        }
        "toric_3d" => {
            let spec = Toric3dSpec {
                lx: required_usize(object, "lx", construction)?,
                ly: required_usize(object, "ly", construction)?,
                lz: required_usize(object, "lz", construction)?,
            };
            toric_3d_css_checks(spec)?;
            Ok(CssFamilySpec::Toric3d(spec).into())
        }
        "hypergraph_product" => Ok(CssConstructionSpec::HypergraphProduct(
            serde_json::from_value(value.clone()).map_err(|error| {
                QecError::InvalidCssConstruction {
                    construction: construction.to_owned(),
                    reason: error.to_string(),
                }
            })?,
        )),
        "legacy_built_in" => Ok(CssConstructionSpec::LegacyBuiltIn(LegacyBuiltInCssSpec {
            code_id: required_string(object, "code_id")?.to_owned(),
        })),
        unknown => Err(QecError::UnknownCssConstruction {
            construction: unknown.to_owned(),
        }),
    }
}

pub fn verify_css_orthogonality(n: usize, h_x: &[Vec<usize>], h_z: &[Vec<usize>]) -> Result<()> {
    let h_x = canonical_sparse_rows(n, h_x.to_vec())?;
    let h_z = canonical_sparse_rows(n, h_z.to_vec())?;
    if h_x.iter().all(|x_row| {
        h_z.iter().all(|z_row| {
            let mut x_index = 0;
            let mut z_index = 0;
            let mut parity = false;
            while x_index < x_row.len() && z_index < z_row.len() {
                match x_row[x_index].cmp(&z_row[z_index]) {
                    std::cmp::Ordering::Less => x_index += 1,
                    std::cmp::Ordering::Greater => z_index += 1,
                    std::cmp::Ordering::Equal => {
                        parity = !parity;
                        x_index += 1;
                        z_index += 1;
                    }
                }
            }
            !parity
        })
    }) {
        Ok(())
    } else {
        Err(QecError::InvalidCssOrthogonality)
    }
}

fn construct_hypergraph_product(spec: HypergraphProductSpec) -> Result<CssConstructionResult> {
    let HypergraphProductSpec {
        left: left_spec,
        right: right_spec,
    } = spec;
    let left = canonical_sparse_rows(left_spec.num_cols, left_spec.rows)?;
    let right = canonical_sparse_rows(right_spec.num_cols, right_spec.rows)?;
    let m_1 = left.len();
    let n_1 = left_spec.num_cols;
    let m_2 = right.len();
    let n_2 = right_spec.num_cols;
    let right_columns = transpose_supports(n_2, &right);
    let left_columns = transpose_supports(n_1, &left);
    let right_offset = n_1 * n_2;

    let mut h_x = Vec::with_capacity(m_1 * n_2);
    for row_1 in 0..m_1 {
        for column_2 in 0..n_2 {
            let mut row = left[row_1]
                .iter()
                .map(|&column_1| column_1 * n_2 + column_2)
                .collect::<Vec<_>>();
            row.extend(
                right_columns[column_2]
                    .iter()
                    .map(|&row_2| right_offset + row_1 * m_2 + row_2),
            );
            h_x.push(row);
        }
    }

    let mut h_z = Vec::with_capacity(n_1 * m_2);
    for column_1 in 0..n_1 {
        for row_2 in 0..m_2 {
            let mut row = right[row_2]
                .iter()
                .map(|&column_2| column_1 * n_2 + column_2)
                .collect::<Vec<_>>();
            row.extend(
                left_columns[column_1]
                    .iter()
                    .map(|&row_1| right_offset + row_1 * m_2 + row_2),
            );
            h_z.push(row);
        }
    }

    let mut parameters = BTreeMap::new();
    parameters.insert(
        "left".to_owned(),
        serde_json::to_value(CssClassicalCheckSpec {
            num_cols: n_1,
            rows: left.clone(),
        })
        .expect("serializable spec"),
    );
    parameters.insert(
        "right".to_owned(),
        serde_json::to_value(CssClassicalCheckSpec {
            num_cols: n_2,
            rows: right.clone(),
        })
        .expect("serializable spec"),
    );
    construction_result(
        "hypergraph_product",
        None,
        parameters,
        n_1 * n_2 + m_1 * m_2,
        h_x,
        h_z,
        "hypergraph_product",
        "CssConstructionSpec::HypergraphProduct",
        None,
    )
}

fn construction_result(
    construction_id: impl Into<String>,
    requested_family_id: Option<RequestedFamilyId>,
    normalized_parameters: BTreeMap<String, Value>,
    n: usize,
    h_x: Vec<Vec<usize>>,
    h_z: Vec<Vec<usize>>,
    adapter: impl Into<String>,
    source: impl Into<String>,
    known_distances: Option<(usize, usize)>,
) -> Result<CssConstructionResult> {
    let construction_id = construction_id.into();
    let adapter = adapter.into();
    let source = source.into();
    let normalized_input_digest = normalized_input_digest(
        &construction_id,
        requested_family_id,
        &normalized_parameters,
    );
    let h_x = canonical_sparse_rows(n, h_x)?;
    let h_z = canonical_sparse_rows(n, h_z)?;
    verify_css_orthogonality(n, &h_x, &h_z)?;
    let rank_x = try_binary_rank(&dense_rows(n, &h_x))?;
    let rank_z = try_binary_rank(&dense_rows(n, &h_z))?;
    let (d_x, d_z) = known_distances
        .map(|(d_x, d_z)| (Some(d_x), Some(d_z)))
        .unwrap_or((None, None));
    let stats = CssCodeStats {
        n,
        m_x: h_x.len(),
        m_z: h_z.len(),
        rank_x,
        rank_z,
        k: n.saturating_sub(rank_x + rank_z),
        d_x,
        d_z,
    };
    Ok(CssConstructionResult {
        schema_version: CSS_CONSTRUCTION_SCHEMA_VERSION,
        construction_id,
        requested_family_id,
        normalized_parameters,
        checks: CssChecks { h_x, h_z },
        stats,
        provenance: CssConstructionProvenance {
            adapter,
            source,
            normalized_input_digest,
        },
    })
}

fn normalized_input_digest(
    construction_id: &str,
    requested_family_id: Option<RequestedFamilyId>,
    normalized_parameters: &BTreeMap<String, Value>,
) -> String {
    let payload = serde_json::json!({
        "schema_version": CSS_CONSTRUCTION_SCHEMA_VERSION,
        "construction_id": construction_id,
        "requested_family_id": requested_family_id,
        "normalized_parameters": normalized_parameters,
    });
    let json = serde_json::to_vec(&payload).expect("normalized construction input is serializable");
    format!("sha256:{}", lower_hex(&Sha256::digest(json)))
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn canonical_sparse_rows(n: usize, mut rows: Vec<Vec<usize>>) -> Result<Vec<Vec<usize>>> {
    SparseRowsMatrix::new(n, rows.clone())?;
    for row in &mut rows {
        row.sort_unstable();
    }
    Ok(rows)
}

fn dense_rows(n: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; n];
            for &column in row {
                dense[column] = 1;
            }
            dense
        })
        .collect()
}

fn transpose_supports(num_cols: usize, rows: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut columns = vec![Vec::new(); num_cols];
    for (row, support) in rows.iter().enumerate() {
        for &column in support {
            columns[column].push(row);
        }
    }
    columns
}

fn surface_construction_from_json(
    object: &Map<String, Value>,
    construction: &str,
) -> Result<CssConstructionSpec> {
    let has_legacy_distance = object.contains_key("distance");
    let has_layout_aware_fields = object.contains_key("layout")
        || object.contains_key("row_distance")
        || object.contains_key("column_distance");
    if has_legacy_distance && has_layout_aware_fields {
        return Err(QecError::InvalidCssConstruction {
            construction: construction.to_owned(),
            reason: "conflicting legacy distance and layout-aware surface parameters".to_owned(),
        });
    }
    if has_legacy_distance {
        return Ok(CssFamilySpec::Surface(SurfaceFamilySpec {
            distance: required_usize(object, "distance", construction)?,
        })
        .into());
    }
    if !has_layout_aware_fields {
        return Err(QecError::InvalidCssConstruction {
            construction: construction.to_owned(),
            reason: "missing or invalid distance".to_owned(),
        });
    }

    let layout = match required_string(object, "layout")? {
        "rotated" => SurfaceLayout::Rotated,
        "unrotated" => SurfaceLayout::Unrotated,
        value => {
            return Err(QecError::InvalidCssConstruction {
                construction: construction.to_owned(),
                reason: format!("unknown surface layout {value}"),
            });
        }
    };
    Ok(SurfaceSpec {
        layout,
        row_distance: required_usize(object, "row_distance", construction)?,
        column_distance: required_usize(object, "column_distance", construction)?,
    }
    .into())
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| QecError::InvalidCssConstructionJson(format!("missing or invalid {field}")))
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| QecError::InvalidCssConstructionJson(format!("missing or invalid {field}")))
}

fn required_usize(object: &Map<String, Value>, field: &str, construction: &str) -> Result<usize> {
    let value = object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        QecError::InvalidCssConstruction {
            construction: construction.to_owned(),
            reason: format!("missing or invalid {field}"),
        }
    })?;
    usize::try_from(value).map_err(|_| QecError::InvalidCssConstruction {
        construction: construction.to_owned(),
        reason: format!("{field} is outside usize range"),
    })
}
