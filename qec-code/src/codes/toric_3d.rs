use crate::binary_chain_complex::{BinaryBoundaryMap, BinaryChainComplex};
use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Toric3dSpec {
    pub lx: usize,
    pub ly: usize,
    pub lz: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toric3dCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub distances: Toric3dDistances,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toric3dDistances {
    pub d_x: usize,
    pub d_z: usize,
    pub distance: usize,
}

#[derive(Debug, Clone, Copy)]
struct Toric3dDimensions {
    spec: Toric3dSpec,
    volume: usize,
    num_edges: usize,
    num_plaquettes: usize,
}

impl Toric3dDimensions {
    fn new(spec: Toric3dSpec) -> Result<Self> {
        validate_period("lx", spec.lx)?;
        validate_period("ly", spec.ly)?;
        validate_period("lz", spec.lz)?;
        let xy = checked_mul(spec.lx, spec.ly)?;
        let volume = checked_mul(xy, spec.lz)?;
        let num_edges = checked_mul(3, volume)?;
        let num_plaquettes = checked_mul(3, volume)?;
        Ok(Self {
            spec,
            volume,
            num_edges,
            num_plaquettes,
        })
    }

    fn cell(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        let xy = checked_add(checked_mul(x, self.spec.ly)?, y)?;
        checked_add(checked_mul(xy, self.spec.lz)?, z)
    }

    fn x_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        self.cell(x, y, z)
    }

    fn y_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        checked_add(self.volume, self.cell(x, y, z)?)
    }

    fn z_edge(&self, x: usize, y: usize, z: usize) -> Result<usize> {
        checked_add(checked_mul(2, self.volume)?, self.cell(x, y, z)?)
    }
}

pub fn toric_3d_chain_complex(spec: Toric3dSpec) -> Result<BinaryChainComplex> {
    let dims = Toric3dDimensions::new(spec)?;
    let boundary_1 = BinaryBoundaryMap::new(
        1,
        0,
        SparseGf2Matrix::new(dims.volume, dims.num_edges, vertex_edge_rows(&dims)?)?,
    )?;
    let boundary_2 = BinaryBoundaryMap::new(
        2,
        1,
        SparseGf2Matrix::new(
            dims.num_edges,
            dims.num_plaquettes,
            edge_plaquette_rows(&dims)?,
        )?,
    )?;
    BinaryChainComplex::new(vec![boundary_1, boundary_2])
}

pub fn toric_3d_css_checks(spec: Toric3dSpec) -> Result<Toric3dCssChecks> {
    let dims = Toric3dDimensions::new(spec)?;
    let complex = toric_3d_chain_complex(spec)?;
    let css = complex.css_view(1)?;
    Ok(Toric3dCssChecks {
        num_cols: css.num_qubits(),
        hx: css.hx().rows().to_vec(),
        hz: css.hz().rows().to_vec(),
        distances: analytic_distances(&dims)?,
    })
}

fn validate_period(parameter: &str, value: usize) -> Result<()> {
    if value < 3 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "toric_3d".to_owned(),
            parameter: parameter.to_owned(),
            value,
        });
    }
    Ok(())
}

fn checked_mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or(QecError::SparseGf2DimensionOverflow {
            operation: "toric_3d",
        })
}

fn checked_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or(QecError::SparseGf2DimensionOverflow {
            operation: "toric_3d",
        })
}

fn vertex_edge_rows(dims: &Toric3dDimensions) -> Result<Vec<Vec<usize>>> {
    let mut rows = Vec::with_capacity(dims.volume);
    for x in 0..dims.spec.lx {
        let previous_x = previous_coordinate(x, dims.spec.lx);
        for y in 0..dims.spec.ly {
            let previous_y = previous_coordinate(y, dims.spec.ly);
            for z in 0..dims.spec.lz {
                let previous_z = previous_coordinate(z, dims.spec.lz);
                rows.push(vec![
                    dims.x_edge(x, y, z)?,
                    dims.x_edge(previous_x, y, z)?,
                    dims.y_edge(x, y, z)?,
                    dims.y_edge(x, previous_y, z)?,
                    dims.z_edge(x, y, z)?,
                    dims.z_edge(x, y, previous_z)?,
                ]);
            }
        }
    }
    Ok(rows)
}

fn edge_plaquette_rows(dims: &Toric3dDimensions) -> Result<Vec<Vec<usize>>> {
    let mut rows = vec![Vec::new(); dims.num_edges];
    let mut plaquette = 0;

    for x in 0..dims.spec.lx {
        let next_x = next_coordinate(x, dims.spec.lx);
        for y in 0..dims.spec.ly {
            let next_y = next_coordinate(y, dims.spec.ly);
            for z in 0..dims.spec.lz {
                for edge in [
                    dims.x_edge(x, y, z)?,
                    dims.x_edge(x, next_y, z)?,
                    dims.y_edge(x, y, z)?,
                    dims.y_edge(next_x, y, z)?,
                ] {
                    rows[edge].push(plaquette);
                }
                plaquette = checked_add(plaquette, 1)?;
            }
        }
    }

    for x in 0..dims.spec.lx {
        let next_x = next_coordinate(x, dims.spec.lx);
        for y in 0..dims.spec.ly {
            for z in 0..dims.spec.lz {
                let next_z = next_coordinate(z, dims.spec.lz);
                for edge in [
                    dims.x_edge(x, y, z)?,
                    dims.x_edge(x, y, next_z)?,
                    dims.z_edge(x, y, z)?,
                    dims.z_edge(next_x, y, z)?,
                ] {
                    rows[edge].push(plaquette);
                }
                plaquette = checked_add(plaquette, 1)?;
            }
        }
    }

    for x in 0..dims.spec.lx {
        for y in 0..dims.spec.ly {
            let next_y = next_coordinate(y, dims.spec.ly);
            for z in 0..dims.spec.lz {
                let next_z = next_coordinate(z, dims.spec.lz);
                for edge in [
                    dims.y_edge(x, y, z)?,
                    dims.y_edge(x, y, next_z)?,
                    dims.z_edge(x, y, z)?,
                    dims.z_edge(x, next_y, z)?,
                ] {
                    rows[edge].push(plaquette);
                }
                plaquette = checked_add(plaquette, 1)?;
            }
        }
    }

    Ok(rows)
}

fn previous_coordinate(coordinate: usize, period: usize) -> usize {
    if coordinate == 0 {
        period - 1
    } else {
        coordinate - 1
    }
}

fn next_coordinate(coordinate: usize, period: usize) -> usize {
    if coordinate == period - 1 {
        0
    } else {
        coordinate + 1
    }
}

fn analytic_distances(dims: &Toric3dDimensions) -> Result<Toric3dDistances> {
    let d_x = checked_mul(dims.spec.lx, dims.spec.ly)?
        .min(checked_mul(dims.spec.lx, dims.spec.lz)?)
        .min(checked_mul(dims.spec.ly, dims.spec.lz)?);
    let d_z = dims.spec.lx.min(dims.spec.ly).min(dims.spec.lz);
    Ok(Toric3dDistances {
        d_x,
        d_z,
        distance: d_x.min(d_z),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_boundary_composition_is_rejected() {
        let dims = Toric3dDimensions::new(Toric3dSpec {
            lx: 3,
            ly: 3,
            lz: 3,
        })
        .unwrap();
        let boundary_1 = BinaryBoundaryMap::new(
            1,
            0,
            SparseGf2Matrix::new(
                dims.volume,
                dims.num_edges,
                vertex_edge_rows(&dims).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut rows = edge_plaquette_rows(&dims).unwrap();
        rows[0].remove(0);
        let boundary_2 = BinaryBoundaryMap::new(
            2,
            1,
            SparseGf2Matrix::new(dims.num_edges, dims.num_plaquettes, rows).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            BinaryChainComplex::new(vec![boundary_1, boundary_2]),
            Err(QecError::NonzeroBoundaryComposition {
                lower_dimension: 1,
                upper_dimension: 2,
                ..
            })
        ));
    }
}
