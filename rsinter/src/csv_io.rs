use crate::failure::{classify_completed, FailureKind};
use crate::task_stats::TaskStats;
use std::io::Write;

pub fn write_csv(stats: &[TaskStats], out: &mut dyn Write) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(out);
    wtr.write_record(&[
        "shots",
        "errors",
        "discards",
        "seconds",
        "failure_kind",
        "decoder",
        "strong_id",
        "json_metadata",
        "custom_counts",
    ])
    .map_err(|e| e.to_string())?;
    for s in stats {
        wtr.write_record(&[
            s.shots.to_string(),
            s.errors.to_string(),
            s.discards.to_string(),
            format!("{:.4}", s.seconds),
            s.failure_kind.to_string(),
            s.decoder.clone(),
            s.strong_id.clone(),
            serde_json::to_string(&s.metadata).unwrap_or_default(),
            serde_json::to_string(&s.custom_counts).unwrap_or_default(),
        ])
        .map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_csv(data: &[u8]) -> Result<Vec<TaskStats>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut rdr = csv::Reader::from_reader(data);
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let required_index = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| format!("missing required CSV column: {name}"))
    };

    let shots_idx = required_index("shots")?;
    let errors_idx = required_index("errors")?;
    let discards_idx = required_index("discards")?;
    let seconds_idx = required_index("seconds")?;
    let decoder_idx = required_index("decoder")?;
    let strong_id_idx = required_index("strong_id")?;
    let metadata_idx = required_index("json_metadata")?;
    let custom_counts_idx = required_index("custom_counts")?;
    let failure_kind_idx = headers.iter().position(|header| header == "failure_kind");

    let mut results = Vec::new();
    for record in rdr.records() {
        let r = record.map_err(|e| e.to_string())?;
        let get = |idx: usize, name: &str| {
            r.get(idx)
                .ok_or_else(|| format!("missing CSV value for column: {name}"))
        };
        let shots = get(shots_idx, "shots")?
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let errors = get(errors_idx, "errors")?
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let discards = get(discards_idx, "discards")?
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;
        let seconds = get(seconds_idx, "seconds")?
            .parse()
            .map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let failure_kind = match failure_kind_idx {
            Some(idx) => get(idx, "failure_kind")?.parse::<FailureKind>()?,
            None => classify_completed(errors, false),
        };

        results.push(TaskStats {
            shots,
            errors,
            discards,
            seconds,
            decoder: get(decoder_idx, "decoder")?.to_string(),
            strong_id: get(strong_id_idx, "strong_id")?.to_string(),
            metadata: serde_json::from_str(get(metadata_idx, "json_metadata")?)
                .unwrap_or(serde_json::Value::Null),
            failure_kind,
            custom_counts: serde_json::from_str(get(custom_counts_idx, "custom_counts")?)
                .unwrap_or_default(),
        });
    }
    Ok(results)
}
