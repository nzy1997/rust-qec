use serde::Deserialize;

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
