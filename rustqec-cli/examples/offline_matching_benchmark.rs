//! Bounded benchmark adapter: the same exported public model as PyMatching.
//! No private answers; JSON transport excluded, all batch work and output included.
use rmatching::Matching;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

#[derive(Deserialize)]
struct Edge {
    u: usize,
    v: Option<usize>,
    observables: Vec<usize>,
    weight: f64,
    loss_factor: f64,
}
#[derive(Deserialize)]
struct Graph {
    edges: Vec<Edge>,
    loss_edges: Vec<Vec<usize>>,
    mean_weight: f64,
    num_observables: usize,
    syndromes: Vec<Vec<u8>>,
    losses: Vec<Vec<usize>>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: offline_matching_benchmark GRAPH PREDICTIONS STATS".into());
    }
    let graph: Graph = serde_json::from_reader(File::open(&args[1])?)?;
    let started = Instant::now();
    if graph.num_observables != 1 || graph.syndromes.len() != graph.losses.len() {
        return Err("invalid batch".into());
    }
    let scale = graph.edges.iter().map(|e| e.weight).fold(1.0, f64::max);
    let mut by_pattern = HashMap::<Vec<usize>, usize>::new();
    let mut groups = Vec::<(Vec<usize>, Vec<usize>)>::new();
    for (row, losses) in graph.losses.iter().enumerate() {
        let index = *by_pattern.entry(losses.clone()).or_insert_with(|| {
            groups.push((losses.clone(), Vec::new()));
            groups.len() - 1
        });
        groups[index].1.push(row);
    }
    let graph_builds = groups.len();
    let mut predictions = vec![0u8; graph.syndromes.len()];
    let mut graph_seconds = 0.;
    let mut matching_seconds = 0.;
    for (losses, indices) in groups {
        let stage = Instant::now();
        let mut active = vec![false; graph.edges.len()];
        for loss in losses {
            for &edge in &graph.loss_edges[loss] {
                active[edge] = true;
            }
        }
        let mut matching = Matching::new();
        for (i, e) in graph.edges.iter().enumerate() {
            let weight = if active[i] {
                e.loss_factor * graph.mean_weight
            } else {
                e.weight
            } / scale;
            let p = 1. / (1. + weight.exp());
            if let Some(v) = e.v {
                matching.add_edge(e.u, v, weight, &e.observables, p);
            } else {
                matching.add_boundary_edge(e.u, weight, &e.observables, p);
            }
        }
        matching.prepare();
        graph_seconds += stage.elapsed().as_secs_f64();
        let rows: Vec<_> = indices
            .iter()
            .map(|&i| graph.syndromes[i].clone())
            .collect();
        let stage = Instant::now();
        let values = matching.decode_batch(&rows);
        matching_seconds += stage.elapsed().as_secs_f64();
        for (row, value) in indices.into_iter().zip(values) {
            predictions[row] = value.first().copied().unwrap_or(0);
        }
    }
    let write_started = Instant::now();
    let mut writer = BufWriter::new(File::create(&args[2])?);
    writer.write_all(&predictions)?;
    writer.flush()?;
    let write_seconds = write_started.elapsed().as_secs_f64();
    let decode_seconds = started.elapsed().as_secs_f64();
    serde_json::to_writer_pretty(
        File::create(&args[3])?,
        &serde_json::json!({
            "decode_seconds":decode_seconds,"graph_build_seconds":graph_seconds,
            "matching_seconds":matching_seconds,"write_seconds":write_seconds,
            "batch_overhead_seconds":decode_seconds-graph_seconds-matching_seconds-write_seconds,
            "graph_builds":graph_builds,"shots":predictions.len(),"policy":"offline groups; one graph per pattern",
            "boundary":"JSON transport excluded; grouping, graph build, batch decode, reorder, b8 write and flush included; no fsync"
        }),
    )?;
    Ok(())
}
