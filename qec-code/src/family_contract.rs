use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::binary::try_binary_rank;
use crate::codes::built_in_css::{
    built_in_css_checks, parse_built_in_css_code_spec, BuiltInCssCodeSpec, BuiltInCssFamily,
    BuiltInCssParams,
};
pub use crate::codes::color_666::{Color666FamilySpec, Color666Layout};
use crate::codes::color_666::{COLOR_666_CONSTRUCTION_ID, color_666_sparse_checks};
use crate::codes::quantum_tanner::{
    quantum_tanner_css_checks, quantum_tanner_spec_from_json_str, QuantumTannerSpec,
};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceFamilySpec {
    pub distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssFamilySpec {
    Surface(SurfaceFamilySpec),
    QuantumTanner(QuantumTannerSpec),
    Color666(Color666FamilySpec),
}

impl CssFamilySpec {
    pub const fn callable_requested_family_ids() -> &'static [RequestedFamilyId] {
        &[
            RequestedFamilyId::Surface,
            RequestedFamilyId::QuantumTanner,
            RequestedFamilyId::Color666,
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
    HypergraphProduct(HypergraphProductSpec),
    LegacyBuiltIn(LegacyBuiltInCssSpec),
}

impl From<CssFamilySpec> for CssConstructionSpec {
    fn from(value: CssFamilySpec) -> Self {
        Self::Family(value)
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
            family: BuiltInCssFamily::Color666,
            params: BuiltInCssParams::Distance { distance },
        } = parsed
        {
            return Ok(CssFamilySpec::Color666(Color666FamilySpec {
                distance,
                layout: Color666Layout::Triangular,
            })
            .into());
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssConstructionProvenance {
    pub adapter: String,
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
        CssConstructionSpec::Family(CssFamilySpec::Surface(spec)) => {
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
            )
        }
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
            )
        }
        CssConstructionSpec::Family(CssFamilySpec::Color666(spec)) => {
            let checks = color_666_sparse_checks(&spec)?;
            let mut parameters = BTreeMap::new();
            parameters.insert("distance".to_owned(), Value::from(spec.distance));
            parameters.insert("layout".to_owned(), Value::from(spec.layout.as_str()));
            construction_result(
                COLOR_666_CONSTRUCTION_ID,
                Some(RequestedFamilyId::Color666),
                parameters,
                checks.num_cols,
                checks.rows.clone(),
                checks.rows,
                COLOR_666_CONSTRUCTION_ID,
            )
        }
        CssConstructionSpec::HypergraphProduct(spec) => construct_hypergraph_product(spec),
        CssConstructionSpec::LegacyBuiltIn(spec) => {
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
            )
        }
    }
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
        "surface" => Ok(CssFamilySpec::Surface(SurfaceFamilySpec {
            distance: required_usize(object, "distance", construction)?,
        })
        .into()),
        "color_666" => {
            let layout = optional_string(object, "layout")?
                .map(Color666Layout::parse)
                .transpose()?
                .unwrap_or(Color666Layout::Triangular);
            Ok(CssFamilySpec::Color666(Color666FamilySpec {
                distance: required_usize(object, "distance", construction)?,
                layout,
            })
            .into())
        }
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
) -> Result<CssConstructionResult> {
    let h_x = canonical_sparse_rows(n, h_x)?;
    let h_z = canonical_sparse_rows(n, h_z)?;
    verify_css_orthogonality(n, &h_x, &h_z)?;
    let rank_x = try_binary_rank(&dense_rows(n, &h_x))?;
    let rank_z = try_binary_rank(&dense_rows(n, &h_z))?;
    let stats = CssCodeStats {
        n,
        m_x: h_x.len(),
        m_z: h_z.len(),
        rank_x,
        rank_z,
        k: n.saturating_sub(rank_x + rank_z),
    };
    Ok(CssConstructionResult {
        schema_version: CSS_CONSTRUCTION_SCHEMA_VERSION,
        construction_id: construction_id.into(),
        requested_family_id,
        normalized_parameters,
        checks: CssChecks { h_x, h_z },
        stats,
        provenance: CssConstructionProvenance {
            adapter: adapter.into(),
        },
    })
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

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| QecError::InvalidCssConstructionJson(format!("missing or invalid {field}")))
}

fn optional_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<Option<&'a str>> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(QecError::InvalidCssConstructionJson(format!(
            "missing or invalid {field}"
        ))),
    }
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
