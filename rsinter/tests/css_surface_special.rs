#![cfg(feature = "rmatching-runner")]

use rand::rngs::StdRng;
use rand::SeedableRng;
use rsinter::decode::{Decoder, RmatchingDemDecoder};
use rstim::codegen::css::{
    css_memory, CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use rstim::codegen::surface_code::rotated_memory_x;
use rstim::codegen::NoiseParams;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::output::write_shots_b8;
use rstim::sampler::sample_batch;
use rstim::stats;

#[test]
fn css_surface_style_counts_match_rotated_surface_memory_x() {
    for distance in [3, 5] {
        let css = rotated_surface_css_memory_x(distance, distance, 0.001);
        let rotated = rotated_memory_x(distance, distance, 0.001);

        assert_eq!(
            stats::num_observables(&css),
            stats::num_observables(&rotated)
        );
        assert_eq!(stats::num_detectors(&css), stats::num_detectors(&rotated));
        ErrorAnalyzer::circuit_to_dem_decomposed(&css).unwrap();
    }
}

#[test]
fn css_surface_style_rmatching_smoke_tracks_rotated_baseline() {
    let css = rotated_surface_css_memory_x(3, 3, 0.002);
    let rotated = rotated_memory_x(3, 3, 0.002);

    let css_rate = logical_error_rate(&css, 256, 12_345);
    let rotated_rate = logical_error_rate(&rotated, 256, 12_345);

    assert!(
        (css_rate - rotated_rate).abs() <= 0.15,
        "css_rate={css_rate}, rotated_rate={rotated_rate}"
    );
}

#[test]
fn bb72_css_smoke_builds_dem_with_twelve_observables() {
    let (hx, hz) = bb72_checks();
    let observables = logical_x_observables(&hx, &hz, 72, 12);
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx,
            hz,
            num_data_qubits: 72,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(observables),
    })
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();

    assert_eq!(stats::num_observables(&circuit), 12);
    assert_eq!(dem.num_observables(), 12);
    assert_eq!(stats::num_detectors(&circuit), 72);
}

fn logical_error_rate(circuit: &[rstim::ir::StimInstr], shots: usize, seed: u64) -> f64 {
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(circuit).unwrap();
    let decoder = RmatchingDemDecoder;
    let compiled = decoder.compile_for_dem(&dem).unwrap();
    let num_dets = dem.effective_num_detectors();
    let num_obs = dem.num_observables();
    let obs_bytes = num_obs.div_ceil(8);
    let mut rng = StdRng::seed_from_u64(seed);
    let batch = sample_batch(circuit, shots, &mut rng).unwrap();
    let mut dets = Vec::new();
    write_shots_b8(&batch.detections, &mut dets).unwrap();
    let mut obs = Vec::new();
    write_shots_b8(&batch.observable_flips, &mut obs).unwrap();
    let predictions = compiled
        .decode_shots_bit_packed(&dets, shots, num_dets, num_obs)
        .unwrap();
    let mut errors = 0usize;
    for shot in 0..shots {
        let start = shot * obs_bytes;
        let end = start + obs_bytes;
        if predictions[start..end] != obs[start..end] {
            errors += 1;
        }
    }
    errors as f64 / shots as f64
}

fn rotated_surface_css_memory_x(
    distance: usize,
    rounds: usize,
    noise: f64,
) -> Vec<rstim::ir::StimInstr> {
    let (hx, hz, logical_x) = rotated_surface_css_checks(distance);
    css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx,
            hz,
            num_data_qubits: distance * distance,
        },
        rounds,
        noise: NoiseParams::uniform(noise),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(vec![logical_x]),
    })
    .unwrap()
}

fn rotated_surface_css_checks(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>, Vec<usize>) {
    let data_index = |x: usize, y: usize| -> usize { x * distance + y };
    let mut hx = Vec::new();
    let mut hz = Vec::new();
    for ax in 0..=distance {
        for ay in 0..=distance {
            let on_boundary_1 = ax == 0 || ax == distance;
            let on_boundary_2 = ay == 0 || ay == distance;
            let parity = (ax % 2) != (ay % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }
            let mut support = Vec::new();
            let mx = (2 * ax) as isize;
            let my = (2 * ay) as isize;
            for (dx, dy) in [(1isize, 1isize), (1, -1), (-1, 1), (-1, -1)] {
                let x = mx + dx;
                let y = my + dy;
                if x >= 1
                    && x <= (2 * distance - 1) as isize
                    && y >= 1
                    && y <= (2 * distance - 1) as isize
                    && x % 2 == 1
                    && y % 2 == 1
                {
                    let qx = ((x - 1) / 2) as usize;
                    let qy = ((y - 1) / 2) as usize;
                    if qx < distance && qy < distance {
                        support.push(data_index(qx, qy));
                    }
                }
            }
            support.sort_unstable();
            support.dedup();
            if support.is_empty() {
                continue;
            }
            if parity {
                hx.push(support);
            } else {
                hz.push(support);
            }
        }
    }
    let logical_x = (0..distance).map(|y| data_index(0, y)).collect();
    (hx, hz, logical_x)
}

fn bb72_checks() -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    bivariate_bicycle_checks(6, 6, &[(3, 0), (0, 1), (0, 2)], &[(0, 3), (1, 0), (2, 0)])
}

fn bivariate_bicycle_checks(
    lx: usize,
    ly: usize,
    a_terms: &[(usize, usize)],
    b_terms: &[(usize, usize)],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block = lx * ly;
    let index = |x: usize, y: usize| -> usize { (x % lx) * ly + (y % ly) };
    let mut hx = Vec::with_capacity(block);
    let mut hz = Vec::with_capacity(block);
    for x in 0..lx {
        for y in 0..ly {
            let mut x_row = Vec::new();
            for &(dx, dy) in a_terms {
                x_row.push(index(x + dx, y + dy));
            }
            for &(dx, dy) in b_terms {
                x_row.push(block + index(x + dx, y + dy));
            }
            x_row.sort_unstable();
            hx.push(x_row);

            let mut z_row = Vec::new();
            for &(dx, dy) in b_terms {
                z_row.push(index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            for &(dx, dy) in a_terms {
                z_row.push(block + index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            z_row.sort_unstable();
            hz.push(z_row);
        }
    }
    (hx, hz)
}

fn logical_x_observables(
    hx: &[Vec<usize>],
    hz: &[Vec<usize>],
    width: usize,
    count: usize,
) -> Vec<Vec<usize>> {
    let nullspace = nullspace_basis(hz, width);
    let mut span = dense_rows(hx, width);
    let hx_rank = gf2_rank(&span, width);
    assert_eq!(nullspace.len() - hx_rank, count);

    let mut rank = hx_rank;
    let mut observables = Vec::new();
    for support in nullspace {
        let dense = dense_row(&support, width);
        let mut trial = span.clone();
        trial.push(dense.clone());
        let trial_rank = gf2_rank(&trial, width);
        if trial_rank > rank {
            span.push(dense);
            rank = trial_rank;
            observables.push(support);
            if observables.len() == count {
                break;
            }
        }
    }
    assert_eq!(observables.len(), count);
    observables
}

fn nullspace_basis(rows: &[Vec<usize>], width: usize) -> Vec<Vec<usize>> {
    let mut matrix = dense_rows(rows, width);
    let mut pivot_cols = Vec::new();
    let mut rank = 0usize;
    for col in 0..width {
        if let Some(pivot_row) = (rank..matrix.len()).find(|&row| matrix[row][col] == 1) {
            matrix.swap(rank, pivot_row);
            for row in 0..matrix.len() {
                if row != rank && matrix[row][col] == 1 {
                    for entry_col in col..width {
                        matrix[row][entry_col] ^= matrix[rank][entry_col];
                    }
                }
            }
            pivot_cols.push(col);
            rank += 1;
        }
    }

    let mut is_pivot = vec![false; width];
    for &col in &pivot_cols {
        is_pivot[col] = true;
    }

    let mut basis = Vec::new();
    for free_col in 0..width {
        if is_pivot[free_col] {
            continue;
        }
        let mut vector = vec![0u8; width];
        vector[free_col] = 1;
        for (pivot_row, &pivot_col) in pivot_cols.iter().enumerate() {
            vector[pivot_col] = matrix[pivot_row][free_col];
        }
        basis.push(
            vector
                .iter()
                .enumerate()
                .filter_map(|(col, &bit)| (bit == 1).then_some(col))
                .collect(),
        );
    }
    basis
}

fn dense_rows(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter().map(|row| dense_row(row, width)).collect()
}

fn dense_row(row: &[usize], width: usize) -> Vec<u8> {
    let mut dense = vec![0; width];
    for &col in row {
        dense[col] = 1;
    }
    dense
}

fn gf2_rank(rows: &[Vec<u8>], width: usize) -> usize {
    let mut matrix = rows.to_vec();
    let mut rank = 0usize;
    for col in 0..width {
        if let Some(pivot_row) = (rank..matrix.len()).find(|&row| matrix[row][col] == 1) {
            matrix.swap(rank, pivot_row);
            for row in 0..matrix.len() {
                if row != rank && matrix[row][col] == 1 {
                    for entry_col in col..width {
                        matrix[row][entry_col] ^= matrix[rank][entry_col];
                    }
                }
            }
            rank += 1;
        }
    }
    rank
}
