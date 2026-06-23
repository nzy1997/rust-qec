use std::collections::{HashSet, VecDeque};

use qec_code::binary::try_binary_rank;

#[derive(Debug, Clone, Copy)]
pub struct ApmSparseMatrixView<'a> {
    pub name: &'static str,
    pub num_cols: usize,
    pub rows: &'a [Vec<usize>],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightStats {
    pub min: usize,
    pub average: f64,
    pub max: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GirthStatus {
    Exact(usize),
    AtLeast(usize),
    Acyclic,
}

impl GirthStatus {
    pub fn meets_lower_bound(self, expected: usize) -> bool {
        match self {
            Self::Exact(value) | Self::AtLeast(value) => value >= expected,
            Self::Acyclic => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApmSparseMatrixReport {
    pub num_cols: usize,
    pub num_rows: usize,
    pub row_weight: WeightStats,
    pub column_weight: WeightStats,
    pub rank: usize,
    pub girth: GirthStatus,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApmCssVerifierExpectations {
    pub num_cols: Option<usize>,
    pub mx: Option<usize>,
    pub mz: Option<usize>,
    pub row_weight_x: Option<usize>,
    pub row_weight_z: Option<usize>,
    pub column_weight_x: Option<usize>,
    pub column_weight_z: Option<usize>,
    pub k: Option<usize>,
    pub orthogonal: Option<bool>,
    pub girth_lower_bound: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApmCssVerifierReport {
    pub num_cols: usize,
    pub mx: usize,
    pub mz: usize,
    pub x: ApmSparseMatrixReport,
    pub z: ApmSparseMatrixReport,
    pub rank_x: usize,
    pub rank_z: usize,
    pub k: usize,
    pub orthogonal: bool,
}

pub fn verify_apm_css_matrices(
    hx: ApmSparseMatrixView<'_>,
    hz: ApmSparseMatrixView<'_>,
    expectations: &ApmCssVerifierExpectations,
) -> Result<ApmCssVerifierReport, String> {
    validate_sparse_matrix(hx)?;
    validate_sparse_matrix(hz)?;

    if hx.num_cols != hz.num_cols {
        return Err(format!(
            "expected shared width, got Hx={} columns and Hz={} columns",
            hx.num_cols, hz.num_cols
        ));
    }

    let x = matrix_report(hx)?;
    let z = matrix_report(hz)?;
    let orthogonal = sparse_rows_are_orthogonal(hx, hz);
    let k = hx.num_cols.checked_sub(x.rank + z.rank).ok_or_else(|| {
        format!(
            "invalid CSS dimensions: n={} is smaller than rank_x + rank_z = {} + {}",
            hx.num_cols, x.rank, z.rank
        )
    })?;

    let report = ApmCssVerifierReport {
        num_cols: hx.num_cols,
        mx: x.num_rows,
        mz: z.num_rows,
        rank_x: x.rank,
        rank_z: z.rank,
        x,
        z,
        k,
        orthogonal,
    };

    check_expectations(&report, expectations)?;
    Ok(report)
}

fn validate_sparse_matrix(matrix: ApmSparseMatrixView<'_>) -> Result<(), String> {
    if matrix.num_cols == 0 {
        return Err(format!("{} has invalid sparse-rows width 0", matrix.name));
    }
    for (row_index, row) in matrix.rows.iter().enumerate() {
        let mut sorted = row.clone();
        sorted.sort_unstable();
        for pair in sorted.windows(2) {
            if pair[0] == pair[1] {
                return Err(format!(
                    "{} row {row_index} contains duplicate support {}",
                    matrix.name, pair[0]
                ));
            }
        }
        for &support in row {
            if support >= matrix.num_cols {
                return Err(format!(
                    "{} row {row_index} contains out-of-range support {support} for width {}",
                    matrix.name, matrix.num_cols
                ));
            }
        }
    }
    Ok(())
}

fn matrix_report(matrix: ApmSparseMatrixView<'_>) -> Result<ApmSparseMatrixReport, String> {
    let row_weights = matrix.rows.iter().map(Vec::len).collect::<Vec<_>>();
    let dense = dense_rows(matrix);
    let rank = try_binary_rank(&dense)
        .map_err(|err| format!("failed to compute {} rank: {err}", matrix.name))?;

    Ok(ApmSparseMatrixReport {
        num_cols: matrix.num_cols,
        num_rows: matrix.rows.len(),
        row_weight: weight_stats(&row_weights),
        column_weight: weight_stats(&column_weights(matrix)),
        rank,
        girth: tanner_girth(matrix),
    })
}

fn check_expectations(
    report: &ApmCssVerifierReport,
    expectations: &ApmCssVerifierExpectations,
) -> Result<(), String> {
    if let Some(expected) = expectations.num_cols {
        if report.num_cols != expected {
            return Err(format!(
                "expected num_cols={expected}, got {}",
                report.num_cols
            ));
        }
    }
    if let Some(expected) = expectations.mx {
        if report.mx != expected {
            return Err(format!("expected mx={expected}, got {}", report.mx));
        }
    }
    if let Some(expected) = expectations.mz {
        if report.mz != expected {
            return Err(format!("expected mz={expected}, got {}", report.mz));
        }
    }
    if let Some(expected) = expectations.row_weight_x {
        if !row_weight_report_matches(report.x.row_weight, expected) {
            return Err(format!(
                "expected Hx row weight {expected}, got min/avg/max {}",
                format_weight_stats(report.x.row_weight)
            ));
        }
    }
    if let Some(expected) = expectations.row_weight_z {
        if !row_weight_report_matches(report.z.row_weight, expected) {
            return Err(format!(
                "expected Hz row weight {expected}, got min/avg/max {}",
                format_weight_stats(report.z.row_weight)
            ));
        }
    }
    if let Some(expected) = expectations.column_weight_x {
        if !row_weight_report_matches(report.x.column_weight, expected) {
            return Err(format!(
                "expected Hx column weight {expected}, got min/avg/max {}",
                format_weight_stats(report.x.column_weight)
            ));
        }
    }
    if let Some(expected) = expectations.column_weight_z {
        if !row_weight_report_matches(report.z.column_weight, expected) {
            return Err(format!(
                "expected Hz column weight {expected}, got min/avg/max {}",
                format_weight_stats(report.z.column_weight)
            ));
        }
    }
    if let Some(expected) = expectations.k {
        if report.k != expected {
            return Err(format!("expected k={expected}, got {}", report.k));
        }
    }
    if let Some(expected) = expectations.orthogonal {
        if report.orthogonal != expected {
            return Err(format!(
                "expected orthogonal={expected}, got {}",
                report.orthogonal
            ));
        }
    }
    if let Some(expected) = expectations.girth_lower_bound {
        if !report.x.girth.meets_lower_bound(expected) {
            return Err(format!(
                "expected Hx Tanner girth >= {expected}, got {:?}",
                report.x.girth
            ));
        }
        if !report.z.girth.meets_lower_bound(expected) {
            return Err(format!(
                "expected Hz Tanner girth >= {expected}, got {:?}",
                report.z.girth
            ));
        }
    }
    Ok(())
}

fn dense_rows(matrix: ApmSparseMatrixView<'_>) -> Vec<Vec<u8>> {
    matrix
        .rows
        .iter()
        .map(|row| {
            let mut dense = vec![0; matrix.num_cols];
            for &col in row {
                dense[col] = 1;
            }
            dense
        })
        .collect()
}

fn weight_stats(weights: &[usize]) -> WeightStats {
    if weights.is_empty() {
        return WeightStats {
            min: 0,
            average: 0.0,
            max: 0,
        };
    }

    let sum = weights.iter().sum::<usize>();
    WeightStats {
        min: *weights.iter().min().unwrap(),
        average: sum as f64 / weights.len() as f64,
        max: *weights.iter().max().unwrap(),
    }
}

fn column_weights(matrix: ApmSparseMatrixView<'_>) -> Vec<usize> {
    let mut weights = vec![0; matrix.num_cols];
    for row in matrix.rows {
        for &col in row {
            weights[col] += 1;
        }
    }
    weights
}

fn row_weight_report_matches(stats: WeightStats, expected: usize) -> bool {
    stats.min == expected && stats.max == expected && stats.average == expected as f64
}

fn sparse_rows_are_orthogonal(hx: ApmSparseMatrixView<'_>, hz: ApmSparseMatrixView<'_>) -> bool {
    for x_row in hx.rows {
        let x_support = x_row.iter().copied().collect::<HashSet<_>>();
        for z_row in hz.rows {
            let overlap = z_row
                .iter()
                .filter(|&&col| x_support.contains(&col))
                .count();
            if overlap % 2 != 0 {
                return false;
            }
        }
    }
    true
}

fn tanner_girth(matrix: ApmSparseMatrixView<'_>) -> GirthStatus {
    let row_count = matrix.rows.len();
    let total_nodes = row_count + matrix.num_cols;
    let mut graph = vec![Vec::new(); total_nodes];

    for (row_index, row) in matrix.rows.iter().enumerate() {
        for &col in row {
            let col_node = row_count + col;
            graph[row_index].push(col_node);
            graph[col_node].push(row_index);
        }
    }

    let mut best: Option<usize> = None;
    for start in 0..total_nodes {
        if let Some(length) = shortest_cycle_from(start, &graph) {
            best = Some(match best {
                Some(current) => current.min(length),
                None => length,
            });
            if best == Some(4) {
                break;
            }
        }
    }

    match best {
        Some(length) => GirthStatus::Exact(length),
        None => GirthStatus::Acyclic,
    }
}

fn shortest_cycle_from(start: usize, graph: &[Vec<usize>]) -> Option<usize> {
    let mut distance = vec![usize::MAX; graph.len()];
    let mut parent = vec![usize::MAX; graph.len()];
    let mut queue = VecDeque::new();
    let mut best: Option<usize> = None;

    distance[start] = 0;
    queue.push_back(start);

    while let Some(node) = queue.pop_front() {
        let next_distance = distance[node] + 1;
        for &neighbor in &graph[node] {
            if distance[neighbor] == usize::MAX {
                distance[neighbor] = next_distance;
                parent[neighbor] = node;
                queue.push_back(neighbor);
                continue;
            }

            if parent[node] != neighbor && parent[neighbor] != node {
                let cycle_length = distance[node] + distance[neighbor] + 1;
                best = Some(match best {
                    Some(current) => current.min(cycle_length),
                    None => cycle_length,
                });
            }
        }
    }

    best
}

fn format_weight_stats(stats: WeightStats) -> String {
    format!("{}/{:.2}/{}", stats.min, stats.average, stats.max)
}
