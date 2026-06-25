use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::css::{CssCode, SparseRowsMatrix};
use crate::error::{QecError, Result};
use crate::gf2;

pub const LR_CAYLEY_NO_COVER_V1: &str = "lr_cayley_no_cover_v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerSpec {
    pub construction_mode: QuantumTannerConstructionMode,
    pub base_group: ExplicitFiniteGroup,
    pub a_generator_indices: Vec<usize>,
    pub b_generator_indices: Vec<usize>,
    pub local_codes: QuantumTannerLocalCodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantumTannerConstructionMode {
    LeftRightCayleyNoCoverV1,
}

impl QuantumTannerConstructionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeftRightCayleyNoCoverV1 => LR_CAYLEY_NO_COVER_V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitFiniteGroup {
    pub name: Option<String>,
    pub element_order: Option<String>,
    pub order: usize,
    pub identity: usize,
    pub multiplication_table: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalCodes {
    pub matrix_role: String,
    pub field: String,
    pub h_a: Vec<Vec<u8>>,
    pub h_b: Vec<Vec<u8>>,
    pub g_a: Option<Vec<Vec<u8>>>,
    pub g_b: Option<Vec<Vec<u8>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalBinaryCode {
    pub width: usize,
    pub generator_rows: Vec<Vec<u8>>,
    pub dual_rows: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerLocalCodeTensorDual {
    pub code_a: QuantumTannerLocalBinaryCode,
    pub code_b: QuantumTannerLocalBinaryCode,
    pub x_sector_rows: Vec<Vec<u8>>,
    pub z_sector_rows: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerCayleyComplex {
    pub faces: Vec<QuantumTannerCayleyFace>,
    pub oriented_faces: Vec<QuantumTannerOrientedFace>,
    pub x_incidence: Vec<QuantumTannerLocalIncidence>,
    pub z_incidence: Vec<QuantumTannerLocalIncidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantumTannerCayleyFace {
    pub id: usize,
    pub vertices: [usize; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantumTannerOrientedFace {
    pub root_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub vertices: [usize; 4],
    pub face_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuantumTannerLocalIncidence {
    pub source_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub face_id: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFiniteGroup {
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
    inverse_table: Vec<usize>,
    a_generators: Vec<usize>,
    b_generators: Vec<usize>,
}

impl ValidatedFiniteGroup {
    pub fn order(&self) -> usize {
        self.multiplication_table.len()
    }

    pub fn identity(&self) -> usize {
        self.identity
    }

    pub fn multiply(&self, left: usize, right: usize) -> Result<usize> {
        self.validate_element(left)?;
        self.validate_element(right)?;
        Ok(self.multiplication_table[left][right])
    }

    pub fn inv(&self, element: usize) -> Result<usize> {
        self.validate_element(element)?;
        Ok(self.inverse_table[element])
    }

    pub fn a_generators(&self) -> &[usize] {
        &self.a_generators
    }

    pub fn b_generators(&self) -> &[usize] {
        &self.b_generators
    }

    pub fn a_generator(&self, index: usize) -> Option<usize> {
        self.a_generators.get(index).copied()
    }

    pub fn b_generator(&self, index: usize) -> Option<usize> {
        self.b_generators.get(index).copied()
    }

    fn validate_element(&self, element: usize) -> Result<()> {
        let order = self.order();
        if element < order {
            Ok(())
        } else {
            Err(QecError::InvalidQuantumTannerGroupElement { element, order })
        }
    }
}

/// Validate the explicit finite group data used by quantum Tanner construction.
///
/// The group-side expectations mirror qLDPC's `CayleyComplex` vocabulary in
/// `drafts/qLDPC/src/qldpc/objects.py`; later `QTCode` consumption follows
/// `drafts/qLDPC/src/qldpc/codes/quantum.py`.
pub fn validate_quantum_tanner_group_table(
    spec: &QuantumTannerSpec,
) -> Result<ValidatedFiniteGroup> {
    let group = &spec.base_group;
    validate_group_table_shape(group.order, group.identity, &group.multiplication_table)?;
    let identity = find_unique_table_identity(group.order, &group.multiplication_table)?;
    if identity != group.identity {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!(
                "declared identity {} does not match table identity {identity}",
                group.identity
            ),
        });
    }
    let inverse_table = build_inverse_table(&group.multiplication_table, identity)?;
    validate_associativity(&group.multiplication_table)?;
    validate_generator_indices("A", &spec.a_generator_indices, group.order)?;
    validate_generator_indices("B", &spec.b_generator_indices, group.order)?;

    Ok(ValidatedFiniteGroup {
        identity,
        multiplication_table: group.multiplication_table.clone(),
        inverse_table,
        a_generators: spec.a_generator_indices.clone(),
        b_generators: spec.b_generator_indices.clone(),
    })
}

#[derive(Debug, Deserialize)]
struct QuantumTannerSpecJson {
    construction_mode: String,
    base_group: ExplicitFiniteGroupJson,
    a_generator_indices: Vec<usize>,
    b_generator_indices: Vec<usize>,
    local_codes: QuantumTannerLocalCodesJson,
}

#[derive(Debug, Deserialize)]
struct ExplicitFiniteGroupJson {
    name: Option<String>,
    element_order: Option<String>,
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
struct QuantumTannerLocalCodesJson {
    matrix_role: String,
    field: String,
    h_a: Vec<Vec<u8>>,
    h_b: Vec<Vec<u8>>,
    #[serde(default)]
    g_a: Option<Vec<Vec<u8>>>,
    #[serde(default)]
    g_b: Option<Vec<Vec<u8>>>,
}

/// Parse explicit quantum Tanner input JSON into typed Rust data.
///
/// Input concepts follow the qLDPC `QTCode` and `CayleyComplex` vocabulary in
/// `drafts/qLDPC/src/qldpc/codes/quantum.py` and
/// `drafts/qLDPC/src/qldpc/objects.py`. This parser intentionally stops before
/// semantic group validation, generator symmetry checks, face enumeration, or
/// CSS matrix generation.
pub fn quantum_tanner_spec_from_json_str(input: &str) -> Result<QuantumTannerSpec> {
    let parsed: QuantumTannerSpecJson = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidQuantumTannerSpecJson(error.to_string()))?;

    let construction_mode = parse_construction_mode(&parsed.construction_mode)?;
    validate_group_table(
        parsed.base_group.order,
        parsed.base_group.identity,
        &parsed.base_group.multiplication_table,
    )?;
    let local_codes = parse_local_codes(
        parsed.local_codes,
        parsed.a_generator_indices.len(),
        parsed.b_generator_indices.len(),
    )?;

    Ok(QuantumTannerSpec {
        construction_mode,
        base_group: ExplicitFiniteGroup {
            name: parsed.base_group.name,
            element_order: parsed.base_group.element_order,
            order: parsed.base_group.order,
            identity: parsed.base_group.identity,
            multiplication_table: parsed.base_group.multiplication_table,
        },
        a_generator_indices: parsed.a_generator_indices,
        b_generator_indices: parsed.b_generator_indices,
        local_codes,
    })
}

pub fn quantum_tanner_local_code_tensor_dual(
    spec: &QuantumTannerSpec,
) -> Result<QuantumTannerLocalCodeTensorDual> {
    let code_a = build_local_binary_code(
        "code_a",
        &spec.local_codes.h_a,
        spec.local_codes.g_a.as_deref(),
        spec.a_generator_indices.len(),
    )?;
    let code_b = build_local_binary_code(
        "code_b",
        &spec.local_codes.h_b,
        spec.local_codes.g_b.as_deref(),
        spec.b_generator_indices.len(),
    )?;
    let x_sector_rows = tensor_product_rows(&code_a.generator_rows, &code_b.generator_rows);
    let z_sector_rows = tensor_product_rows(&code_a.dual_rows, &code_b.dual_rows);

    Ok(QuantumTannerLocalCodeTensorDual {
        code_a,
        code_b,
        x_sector_rows,
        z_sector_rows,
    })
}

pub fn enumerate_quantum_tanner_cayley_faces(
    construction_mode: QuantumTannerConstructionMode,
    group: &ValidatedFiniteGroup,
) -> Result<QuantumTannerCayleyComplex> {
    match construction_mode {
        QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1 => {}
    }

    validate_construction_generators("A", group.a_generators(), group)?;
    validate_construction_generators("B", group.b_generators(), group)?;

    let mut face_keys = BTreeSet::new();
    let mut pending_oriented = Vec::new();
    for root_vertex in 0..group.order() {
        for (a_index, &a_generator) in group.a_generators().iter().enumerate() {
            for (b_index, &b_generator) in group.b_generators().iter().enumerate() {
                let vertices =
                    oriented_face_vertices(group, root_vertex, a_generator, b_generator)?;
                face_keys.insert(vertices);
                pending_oriented.push((
                    root_vertex,
                    a_index,
                    b_index,
                    a_generator,
                    b_generator,
                    vertices,
                ));
            }
        }
    }

    let faces = face_keys
        .iter()
        .enumerate()
        .map(|(id, &vertices)| QuantumTannerCayleyFace { id, vertices })
        .collect::<Vec<_>>();
    let face_ids = faces
        .iter()
        .map(|face| (face.vertices, face.id))
        .collect::<BTreeMap<_, _>>();

    let inverse_a_indices = inverse_generator_indices(group.a_generators(), group)?;
    let mut oriented_faces = Vec::with_capacity(pending_oriented.len());
    let mut x_incidence = Vec::with_capacity(pending_oriented.len());
    let mut z_incidence = Vec::with_capacity(pending_oriented.len());

    for (root_vertex, a_index, b_index, a_generator, b_generator, vertices) in pending_oriented {
        let face_id = face_ids[&vertices];
        oriented_faces.push(QuantumTannerOrientedFace {
            root_vertex,
            a_index,
            b_index,
            a_generator,
            b_generator,
            vertices,
            face_id,
        });
        x_incidence.push(QuantumTannerLocalIncidence {
            source_vertex: root_vertex,
            a_index,
            b_index,
            a_generator,
            b_generator,
            face_id,
        });

        let z_source_vertex = group.multiply(a_generator, root_vertex)?;
        let z_a_generator = group.inv(a_generator)?;
        let z_a_index = inverse_a_indices[&a_generator];
        z_incidence.push(QuantumTannerLocalIncidence {
            source_vertex: z_source_vertex,
            a_index: z_a_index,
            b_index,
            a_generator: z_a_generator,
            b_generator,
            face_id,
        });
    }

    x_incidence.sort();
    z_incidence.sort();

    Ok(QuantumTannerCayleyComplex {
        faces,
        oriented_faces,
        x_incidence,
        z_incidence,
    })
}

pub fn quantum_tanner_css_checks(spec: &QuantumTannerSpec) -> Result<QuantumTannerCssChecks> {
    let group = validate_quantum_tanner_group_table(spec)?;
    let complex = enumerate_quantum_tanner_cayley_faces(spec.construction_mode, &group)?;
    let local = quantum_tanner_local_code_tensor_dual(spec)?;
    quantum_tanner_css_checks_from_validated_parts(spec, &group, &complex, &local)
}

pub fn quantum_tanner_css_checks_from_validated_parts(
    spec: &QuantumTannerSpec,
    group: &ValidatedFiniteGroup,
    complex: &QuantumTannerCayleyComplex,
    local: &QuantumTannerLocalCodeTensorDual,
) -> Result<QuantumTannerCssChecks> {
    match spec.construction_mode {
        QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1 => {}
    }

    if spec.a_generator_indices.as_slice() != group.a_generators() {
        return Err(css_construction_error(
            "spec A generator indices do not match validated group",
        ));
    }
    if spec.b_generator_indices.as_slice() != group.b_generators() {
        return Err(css_construction_error(
            "spec B generator indices do not match validated group",
        ));
    }

    let a_width = group.a_generators().len();
    let b_width = group.b_generators().len();
    if local.code_a.width != a_width {
        return Err(css_construction_error(format!(
            "local code A width {} does not match |A| {a_width}",
            local.code_a.width
        )));
    }
    if local.code_b.width != b_width {
        return Err(css_construction_error(format!(
            "local code B width {} does not match |B| {b_width}",
            local.code_b.width
        )));
    }

    let local_width = a_width.checked_mul(b_width).ok_or_else(|| {
        css_construction_error(format!(
            "local coordinate width overflow for |A|={a_width}, |B|={b_width}"
        ))
    })?;
    validate_local_tensor_rows("X", &local.x_sector_rows, local_width)?;
    validate_local_tensor_rows("Z", &local.z_sector_rows, local_width)?;

    let num_cols = complex.faces.len();
    let (x_source_vertices, z_source_vertices) = quantum_tanner_source_vertex_partitions(group)?;
    let hx = sparse_rows_from_local_incidence(
        "X",
        group,
        &local.x_sector_rows,
        &complex.x_incidence,
        &x_source_vertices,
        num_cols,
    )?;
    let hz = sparse_rows_from_local_incidence(
        "Z",
        group,
        &local.z_sector_rows,
        &complex.z_incidence,
        &z_source_vertices,
        num_cols,
    )?;

    let hx_matrix = SparseRowsMatrix::new(num_cols, hx.clone())?;
    let hz_matrix = SparseRowsMatrix::new(num_cols, hz.clone())?;
    CssCode::from_hx_hz(hx_matrix.to_dense_rows(), hz_matrix.to_dense_rows())?;

    Ok(QuantumTannerCssChecks { num_cols, hx, hz })
}

fn sparse_rows_from_local_incidence(
    sector: &'static str,
    group: &ValidatedFiniteGroup,
    local_rows: &[Vec<u8>],
    incidence: &[QuantumTannerLocalIncidence],
    source_vertices: &[usize],
    num_cols: usize,
) -> Result<Vec<Vec<usize>>> {
    let mut rows = Vec::with_capacity(source_vertices.len() * local_rows.len());
    for &source_vertex in source_vertices {
        let local_faces =
            local_incidence_grid_for_source(sector, group, incidence, source_vertex, num_cols)?;
        for local_row in local_rows {
            let mut support = BTreeSet::new();
            for (coordinate, &bit) in local_row.iter().enumerate() {
                if bit == 0 {
                    continue;
                }
                let face_id = local_faces[coordinate].ok_or_else(|| {
                    css_construction_error(format!(
                        "{sector} incidence source {source_vertex} is missing local coordinate {coordinate}"
                    ))
                })?;
                if !support.insert(face_id) {
                    support.remove(&face_id);
                }
            }
            rows.push(support.into_iter().collect());
        }
    }
    Ok(rows)
}

fn quantum_tanner_source_vertex_partitions(
    group: &ValidatedFiniteGroup,
) -> Result<(Vec<usize>, Vec<usize>)> {
    let mut colors: Vec<Option<u8>> = vec![None; group.order()];
    for start in 0..group.order() {
        if colors[start].is_some() {
            continue;
        }
        colors[start] = Some(0);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(vertex) = queue.pop_front() {
            let color = colors[vertex].expect("queued vertices are colored");
            for neighbor in quantum_tanner_cayley_neighbors(group, vertex)? {
                match colors[neighbor] {
                    Some(neighbor_color) if neighbor_color == color => {
                        return Err(css_construction_error(format!(
                            "Cayley graph is not bipartite: adjacent vertices {vertex} and {neighbor} have the same source color"
                        )));
                    }
                    Some(_) => {}
                    None => {
                        colors[neighbor] = Some(color ^ 1);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    let mut x_source_vertices = Vec::new();
    let mut z_source_vertices = Vec::new();
    for (vertex, color) in colors.into_iter().enumerate() {
        match color {
            Some(0) => x_source_vertices.push(vertex),
            Some(1) => z_source_vertices.push(vertex),
            _ => unreachable!("all vertices are colored"),
        }
    }
    Ok((x_source_vertices, z_source_vertices))
}

fn quantum_tanner_cayley_neighbors(
    group: &ValidatedFiniteGroup,
    vertex: usize,
) -> Result<Vec<usize>> {
    let mut neighbors = Vec::with_capacity(group.a_generators().len() + group.b_generators().len());
    for &a_generator in group.a_generators() {
        neighbors.push(group.multiply(a_generator, vertex)?);
    }
    for &b_generator in group.b_generators() {
        neighbors.push(group.multiply(vertex, b_generator)?);
    }
    Ok(neighbors)
}

fn local_incidence_grid_for_source(
    sector: &'static str,
    group: &ValidatedFiniteGroup,
    incidence: &[QuantumTannerLocalIncidence],
    source_vertex: usize,
    num_cols: usize,
) -> Result<Vec<Option<usize>>> {
    let b_width = group.b_generators().len();
    let local_width = group.a_generators().len() * b_width;
    let mut local_faces = vec![None; local_width];
    for record in incidence
        .iter()
        .filter(|record| record.source_vertex == source_vertex)
    {
        if record.face_id >= num_cols {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} references face {} outside 0..{num_cols}",
                record.face_id
            )));
        }
        let Some(expected_a) = group.a_generator(record.a_index) else {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} has out-of-range A coordinate {}",
                record.a_index
            )));
        };
        if record.a_generator != expected_a {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} A coordinate {} uses generator {}, expected {expected_a}",
                record.a_index, record.a_generator
            )));
        }
        let Some(expected_b) = group.b_generator(record.b_index) else {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} has out-of-range B coordinate {}",
                record.b_index
            )));
        };
        if record.b_generator != expected_b {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} B coordinate {} uses generator {}, expected {expected_b}",
                record.b_index, record.b_generator
            )));
        }

        let coordinate = record.a_index * b_width + record.b_index;
        if local_faces[coordinate].replace(record.face_id).is_some() {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} has duplicate local coordinate {coordinate}"
            )));
        }
    }

    for (coordinate, face_id) in local_faces.iter().enumerate() {
        if face_id.is_none() {
            return Err(css_construction_error(format!(
                "{sector} incidence source {source_vertex} is missing local coordinate {coordinate}"
            )));
        }
    }

    Ok(local_faces)
}

fn validate_local_tensor_rows(
    sector: &'static str,
    rows: &[Vec<u8>],
    expected_width: usize,
) -> Result<()> {
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != expected_width {
            return Err(css_construction_error(format!(
                "{sector} local tensor row {row_index} has width {}, expected {expected_width}",
                row.len()
            )));
        }
        for (col_index, &bit) in row.iter().enumerate() {
            if bit > 1 {
                return Err(css_construction_error(format!(
                    "{sector} local tensor row {row_index}, column {col_index} is {bit}, expected 0 or 1"
                )));
            }
        }
    }
    Ok(())
}

fn parse_construction_mode(input: &str) -> Result<QuantumTannerConstructionMode> {
    match input {
        LR_CAYLEY_NO_COVER_V1 => Ok(QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1),
        mode => Err(QecError::UnsupportedQuantumTannerConstructionMode {
            mode: mode.to_owned(),
        }),
    }
}

fn validate_group_table(order: usize, identity: usize, table: &[Vec<usize>]) -> Result<()> {
    validate_group_table_shape(order, identity, table)?;
    if identity != 0 {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("identity must be 0 in v1, got {identity}"),
        });
    }

    Ok(())
}

fn validate_construction_generators(
    set: &'static str,
    generators: &[usize],
    group: &ValidatedFiniteGroup,
) -> Result<()> {
    if generators.is_empty() {
        return Err(QecError::InvalidQuantumTannerGeneratorSet {
            set,
            reason: "generator set must be nonempty".to_owned(),
        });
    }

    let mut seen = BTreeSet::new();
    for (index, &generator) in generators.iter().enumerate() {
        if !seen.insert(generator) {
            return Err(QecError::InvalidQuantumTannerGeneratorSet {
                set,
                reason: format!("duplicate generator {generator} at coordinate {index}"),
            });
        }
    }

    for &generator in generators {
        let inverse = group.inv(generator)?;
        if !seen.contains(&inverse) {
            return Err(QecError::InvalidQuantumTannerGeneratorSet {
                set,
                reason: format!("generator {generator} is missing inverse {inverse}"),
            });
        }
    }

    Ok(())
}

fn oriented_face_vertices(
    group: &ValidatedFiniteGroup,
    root: usize,
    a: usize,
    b: usize,
) -> Result<[usize; 4]> {
    let ag = group.multiply(a, root)?;
    let gb = group.multiply(root, b)?;
    let agb = group.multiply(ag, b)?;
    let mut vertices = [root, ag, gb, agb];
    vertices.sort_unstable();
    if vertices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(QecError::DegenerateQuantumTannerFace {
            root,
            a,
            b,
            vertices: vertices.to_vec(),
        });
    }
    Ok(vertices)
}

fn inverse_generator_indices(
    generators: &[usize],
    group: &ValidatedFiniteGroup,
) -> Result<BTreeMap<usize, usize>> {
    let generator_indices = generators
        .iter()
        .enumerate()
        .map(|(index, &generator)| (generator, index))
        .collect::<BTreeMap<_, _>>();
    generators
        .iter()
        .map(|&generator| {
            let inverse = group.inv(generator)?;
            Ok((generator, generator_indices[&inverse]))
        })
        .collect()
}

fn parse_local_codes(
    local_codes: QuantumTannerLocalCodesJson,
    a_width: usize,
    b_width: usize,
) -> Result<QuantumTannerLocalCodes> {
    if local_codes.matrix_role != "parity_check" {
        return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
            matrix: "local_codes",
            reason: format!(
                "matrix_role must be parity_check, got {}",
                local_codes.matrix_role
            ),
        });
    }
    if local_codes.field != "GF(2)" {
        return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
            matrix: "local_codes",
            reason: format!("field must be GF(2), got {}", local_codes.field),
        });
    }

    validate_binary_matrix_width("h_a", &local_codes.h_a, a_width)?;
    validate_binary_matrix_width("h_b", &local_codes.h_b, b_width)?;
    validate_optional_generator_rows(
        "code_a",
        "g_a",
        &local_codes.h_a,
        local_codes.g_a.as_deref(),
        a_width,
    )?;
    validate_optional_generator_rows(
        "code_b",
        "g_b",
        &local_codes.h_b,
        local_codes.g_b.as_deref(),
        b_width,
    )?;

    Ok(QuantumTannerLocalCodes {
        matrix_role: local_codes.matrix_role,
        field: local_codes.field,
        h_a: local_codes.h_a,
        h_b: local_codes.h_b,
        g_a: local_codes.g_a,
        g_b: local_codes.g_b,
    })
}

fn validate_group_table_shape(order: usize, identity: usize, table: &[Vec<usize>]) -> Result<()> {
    if order == 0 {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: "order must be positive".to_owned(),
        });
    }
    if identity >= order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("identity {identity} is out of range for order {order}"),
        });
    }
    if table.len() != order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("expected {order} rows, got {}", table.len()),
        });
    }

    for (row_index, row) in table.iter().enumerate() {
        if row.len() != order {
            return Err(QecError::InvalidQuantumTannerGroupTable {
                reason: format!("row {row_index} has width {}, expected {order}", row.len()),
            });
        }
        for (col_index, &entry) in row.iter().enumerate() {
            if entry >= order {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "entry at row {row_index}, column {col_index} is {entry}, expected < {order}"
                    ),
                });
            }
        }
    }

    Ok(())
}

fn find_unique_table_identity(order: usize, table: &[Vec<usize>]) -> Result<usize> {
    let candidates = (0..order)
        .filter(|&candidate| {
            (0..order).all(|element| {
                table[candidate][element] == element && table[element][candidate] == element
            })
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [identity] => Ok(*identity),
        [] => Err(QecError::InvalidQuantumTannerGroupTable {
            reason: "expected exactly one two-sided identity, found none".to_owned(),
        }),
        many => Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("expected exactly one two-sided identity, found {many:?}"),
        }),
    }
}

fn build_inverse_table(table: &[Vec<usize>], identity: usize) -> Result<Vec<usize>> {
    let order = table.len();
    let mut inverse_table = Vec::with_capacity(order);
    for element in 0..order {
        let candidates = (0..order)
            .filter(|&candidate| {
                table[element][candidate] == identity && table[candidate][element] == identity
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [inverse] => inverse_table.push(*inverse),
            [] => {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "element {element} has no two-sided inverse under identity {identity}"
                    ),
                });
            }
            many => {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "element {element} has multiple two-sided inverses under identity {identity}: {many:?}"
                    ),
                });
            }
        }
    }
    Ok(inverse_table)
}

fn validate_associativity(table: &[Vec<usize>]) -> Result<()> {
    let order = table.len();
    for a in 0..order {
        for b in 0..order {
            for c in 0..order {
                let left = table[table[a][b]][c];
                let right = table[a][table[b][c]];
                if left != right {
                    return Err(QecError::InvalidQuantumTannerGroupTable {
                        reason: format!(
                            "associativity failed for ({a}, {b}, {c}): ({a} * {b}) * {c} = {left}, but {a} * ({b} * {c}) = {right}"
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_generator_indices(set: &'static str, generators: &[usize], order: usize) -> Result<()> {
    for (index, &element) in generators.iter().enumerate() {
        if element >= order {
            return Err(QecError::InvalidQuantumTannerGeneratorIndex {
                set,
                index,
                element,
                order,
            });
        }
    }
    Ok(())
}

fn validate_binary_matrix_width(
    matrix: &'static str,
    rows: &[Vec<u8>],
    expected_width: usize,
) -> Result<()> {
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != expected_width {
            return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
                matrix,
                reason: format!(
                    "row {row_index} has width {}, expected {expected_width}",
                    row.len()
                ),
            });
        }
        for (col_index, &entry) in row.iter().enumerate() {
            if entry > 1 {
                return Err(QecError::InvalidQuantumTannerLocalCodeMatrix {
                    matrix,
                    reason: format!(
                        "entry at row {row_index}, column {col_index} is {entry}, expected 0 or 1"
                    ),
                });
            }
        }
    }

    Ok(())
}

fn build_local_binary_code(
    code: &'static str,
    check_rows: &[Vec<u8>],
    supplied_generator_rows: Option<&[Vec<u8>]>,
    width: usize,
) -> Result<QuantumTannerLocalBinaryCode> {
    validate_binary_matrix_width(code, check_rows, width)?;
    let dual_rows = gf2::try_select_independent_rows(check_rows)
        .map_err(|error| local_code_error(code, error.to_string()))?;
    let generator_rows = match supplied_generator_rows {
        Some(rows) => validate_generator_rows(code, "generator", check_rows, rows, width)?,
        None => gf2::try_nullspace_basis_with_width(check_rows, width)
            .map_err(|error| local_code_error(code, error.to_string()))?,
    };

    Ok(QuantumTannerLocalBinaryCode {
        width,
        generator_rows,
        dual_rows,
    })
}

fn validate_optional_generator_rows(
    code: &'static str,
    matrix: &'static str,
    check_rows: &[Vec<u8>],
    generator_rows: Option<&[Vec<u8>]>,
    width: usize,
) -> Result<()> {
    let Some(generator_rows) = generator_rows else {
        return Ok(());
    };
    validate_generator_rows(code, matrix, check_rows, generator_rows, width)?;
    Ok(())
}

fn validate_generator_rows(
    code: &'static str,
    matrix: &'static str,
    check_rows: &[Vec<u8>],
    generator_rows: &[Vec<u8>],
    width: usize,
) -> Result<Vec<Vec<u8>>> {
    validate_binary_matrix_width(matrix, generator_rows, width)?;
    for (check_index, check_row) in check_rows.iter().enumerate() {
        for (generator_index, generator_row) in generator_rows.iter().enumerate() {
            if dot_mod2(check_row, generator_row) != 0 {
                return Err(local_code_error(
                    code,
                    format!(
                        "{matrix} row {generator_index} is not orthogonal to check row {check_index}"
                    ),
                ));
            }
        }
    }

    let check_rank =
        gf2::try_rank(check_rows).map_err(|error| local_code_error(code, error.to_string()))?;
    let expected_generator_rank = width - check_rank;
    let generator_basis = gf2::try_select_independent_rows(generator_rows)
        .map_err(|error| local_code_error(code, error.to_string()))?;
    if generator_basis.len() != expected_generator_rank {
        return Err(local_code_error(
            code,
            format!(
                "{matrix} rank is {}, expected {expected_generator_rank}",
                generator_basis.len()
            ),
        ));
    }

    Ok(generator_basis)
}

fn dot_mod2(lhs: &[u8], rhs: &[u8]) -> u8 {
    lhs.iter()
        .zip(rhs)
        .fold(0, |parity, (&left, &right)| parity ^ (left & right))
}

fn tensor_product_rows(lhs: &[Vec<u8>], rhs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    lhs.iter()
        .flat_map(|left| {
            rhs.iter().map(move |right| {
                left.iter()
                    .flat_map(|&left_bit| right.iter().map(move |&right_bit| left_bit & right_bit))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

fn local_code_error(matrix: &'static str, reason: String) -> QecError {
    QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason }
}

fn css_construction_error(reason: impl Into<String>) -> QecError {
    QecError::InvalidQuantumTannerCssConstruction {
        reason: reason.into(),
    }
}
