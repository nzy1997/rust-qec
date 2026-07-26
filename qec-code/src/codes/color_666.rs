use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};

pub const COLOR_666_CONSTRUCTION_ID: &str = "color_666";
pub const COLOR_666_TRIANGULAR_LAYOUT: &str = "triangular";
pub const COLOR_666_STEANE_PERMUTATION: [usize; 7] = [0, 3, 6, 5, 1, 4, 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color666Layout {
    Triangular,
}

impl Color666Layout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Triangular => COLOR_666_TRIANGULAR_LAYOUT,
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            COLOR_666_TRIANGULAR_LAYOUT => Ok(Self::Triangular),
            _ => Err(QecError::InvalidCssConstruction {
                construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
                reason: format!(
                    "unsupported layout {value:?}; supported: {COLOR_666_TRIANGULAR_LAYOUT}"
                ),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color666FamilySpec {
    pub distance: usize,
    pub layout: Color666Layout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color666SparseChecks {
    pub num_cols: usize,
    pub rows: Vec<Vec<usize>>,
}

pub fn color_666_sparse_checks(spec: &Color666FamilySpec) -> Result<Color666SparseChecks> {
    validate_distance(spec.distance)?;
    let num_cols = color_666_num_qubits(spec.distance)?;
    let rows = match spec.layout {
        Color666Layout::Triangular => triangular_face_supports(spec.distance, num_cols)?,
    };
    Ok(Color666SparseChecks { num_cols, rows })
}

fn validate_distance(distance: usize) -> Result<()> {
    if distance < 3 {
        return Err(QecError::InvalidCssConstruction {
            construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
            reason: format!("distance must be at least 3, got {distance}"),
        });
    }
    if distance % 2 == 0 {
        return Err(QecError::InvalidCssConstruction {
            construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
            reason: format!("distance must be odd, got {distance}"),
        });
    }
    Ok(())
}

fn color_666_num_qubits(distance: usize) -> Result<usize> {
    distance
        .checked_mul(distance)
        .and_then(|square| square.checked_mul(3))
        .and_then(|triple| triple.checked_add(1))
        .map(|value| value / 4)
        .ok_or_else(|| QecError::InvalidCssConstruction {
            construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
            reason: "size arithmetic overflow while computing n=(3d^2+1)/4".to_owned(),
        })
}

fn triangular_bound(distance: usize) -> Result<usize> {
    distance
        .checked_sub(1)
        .and_then(|value| value.checked_mul(3))
        .map(|value| value / 2)
        .ok_or_else(|| QecError::InvalidCssConstruction {
            construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
            reason: "size arithmetic overflow while computing triangular lattice bound".to_owned(),
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LatticeIndex {
    row: usize,
    column: usize,
}

fn is_plaquette(index: LatticeIndex) -> bool {
    index.column % 3 == 2 - (index.row % 3)
}

fn is_site(index: LatticeIndex) -> bool {
    !is_plaquette(index)
}

fn site_index_map(bound: usize, num_cols: usize) -> Result<Vec<Vec<Option<usize>>>> {
    let mut next = 0usize;
    let mut map = vec![Vec::new(); bound + 1];
    for row in 0..=bound {
        map[row] = vec![None; row + 1];
        for column in 0..=row {
            let index = LatticeIndex { row, column };
            if is_site(index) {
                if next >= num_cols {
                    return Err(QecError::InvalidCssConstruction {
                        construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
                        reason: "site count exceeded n=(3d^2+1)/4".to_owned(),
                    });
                }
                map[row][column] = Some(next);
                next += 1;
            }
        }
    }
    if next != num_cols {
        return Err(QecError::InvalidCssConstruction {
            construction: COLOR_666_CONSTRUCTION_ID.to_owned(),
            reason: format!("site count {next} did not match n={num_cols}"),
        });
    }
    Ok(map)
}

fn triangular_face_supports(distance: usize, num_cols: usize) -> Result<Vec<Vec<usize>>> {
    let bound = triangular_bound(distance)?;
    let site_indices = site_index_map(bound, num_cols)?;
    let mut rows = Vec::new();

    for row in 0..=bound {
        for column in 0..=row {
            let index = LatticeIndex { row, column };
            if is_plaquette(index) {
                let mut support = face_support(bound, &site_indices, index);
                support.sort_unstable();
                rows.push(support);
            }
        }
    }

    Ok(rows)
}

fn face_support(
    bound: usize,
    site_indices: &[Vec<Option<usize>>],
    face: LatticeIndex,
) -> Vec<usize> {
    let row = face.row as isize;
    let column = face.column as isize;
    let mut support = Vec::with_capacity(6);
    for (neighbor_row, neighbor_column) in [
        (row - 1, column - 1),
        (row - 1, column),
        (row, column - 1),
        (row, column + 1),
        (row + 1, column),
        (row + 1, column + 1),
    ] {
        if neighbor_row < 0 || neighbor_column < 0 {
            continue;
        }
        let neighbor_row = neighbor_row as usize;
        let neighbor_column = neighbor_column as usize;
        if neighbor_row > bound || neighbor_column > neighbor_row {
            continue;
        }
        if let Some(site_index) = site_indices[neighbor_row][neighbor_column] {
            support.push(site_index);
        }
    }
    support
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangular_d3_rows_match_issue_fixture() {
        let checks = color_666_sparse_checks(&Color666FamilySpec {
            distance: 3,
            layout: Color666Layout::Triangular,
        })
        .unwrap();

        assert_eq!(checks.num_cols, 7);
        assert_eq!(
            checks.rows,
            vec![vec![0, 1, 2, 3], vec![1, 2, 4, 5], vec![2, 3, 5, 6]]
        );
    }

    #[test]
    fn triangular_d5_rows_match_reviewed_fixture() {
        let checks = color_666_sparse_checks(&Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .unwrap();

        assert_eq!(checks.num_cols, 19);
        assert_eq!(
            checks.rows,
            vec![
                vec![0, 1, 2, 3],
                vec![1, 2, 4, 5],
                vec![2, 3, 5, 6, 8, 9],
                vec![4, 5, 7, 8, 10, 11],
                vec![6, 9, 12, 13],
                vec![7, 10, 14, 15],
                vec![8, 9, 11, 12, 16, 17],
                vec![10, 11, 15, 16],
                vec![12, 13, 17, 18],
            ]
        );
    }

    #[test]
    fn rejects_invalid_distance_values() {
        assert!(
            color_666_sparse_checks(&Color666FamilySpec {
                distance: 2,
                layout: Color666Layout::Triangular,
            })
            .is_err()
        );
        assert!(
            color_666_sparse_checks(&Color666FamilySpec {
                distance: 4,
                layout: Color666Layout::Triangular,
            })
            .is_err()
        );
        assert!(
            color_666_sparse_checks(&Color666FamilySpec {
                distance: usize::MAX,
                layout: Color666Layout::Triangular,
            })
            .is_err()
        );
    }
}
