//! Feature-gated export for comparing matching backends on identical public inputs.
//! This contains no private targets and is not a supported production API.
use super::*;
use serde_json::json;

pub fn export_matching_dataset(path: &Path) -> Result<serde_json::Value, DecodeFailure> {
    let dataset = read_dataset(path)?;
    let started = Instant::now();
    let circuit = compile_circuit(&dataset, DecoderKind::EnvelopeMatching)?;
    let matching = CompiledMatching::new(&circuit)?;
    let compile_seconds = started.elapsed().as_secs_f64();
    let mut reader = ResultBlockReader::new(
        BufReader::new(
            File::open(&dataset.shots_path).map_err(|e| DecodeFailure::new("io", e.to_string()))?,
        ),
        dataset.manifest.row.bits,
        dataset.manifest.shots as u64,
        OutputFormat::B8,
        BATCH_SIZE,
    )
    .map_err(|e| DecodeFailure::new("input", e.to_string()))?;
    let mut syndromes = Vec::new();
    let mut losses = Vec::new();
    let transform_started = Instant::now();
    while let Some(rows) = reader
        .next_block()
        .map_err(|e| DecodeFailure::new("input", e.to_string()))?
    {
        syndromes.extend(circuit.loss_aware_syndromes(&rows)?.into_iter().map(|s| {
            s.canonical_detector_values
                .into_iter()
                .map(u8::from)
                .collect::<Vec<_>>()
        }));
        losses.extend(circuit.loss_patterns(&rows));
    }
    let transform_seconds = transform_started.elapsed().as_secs_f64();
    let edges: Vec<_> = matching
        .edges
        .iter()
        .map(|edge| {
            json!({
                "u": edge.node1, "v": edge.node2, "observables": edge.observables,
                "weight": edge.weight, "loss_factor": match edge.kind {
                    matching::EdgeKind::TimeLike => 0.25,
                    matching::EdgeKind::SpaceLike | matching::EdgeKind::Boundary => 0.5,
                }
            })
        })
        .collect();
    Ok(json!({"schema_version":"rustqec.matching-benchmark.v1",
        "source": "public dataset only; graph/compiler shared with RustQEC",
        "edges":edges,"loss_edges":matching.loss_edges,"mean_weight":matching.mean_weight,
        "num_observables":matching.num_observables,"syndromes":syndromes,"losses":losses,
        "compile_seconds":compile_seconds,"transform_seconds":transform_seconds}))
}
