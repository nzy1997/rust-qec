use crate::task_stats::TaskStats;
use std::io::Write;

pub fn write_csv(stats: &[TaskStats], out: &mut dyn Write) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(out);
    wtr.write_record(&[
        "shots", "errors", "discards", "seconds",
        "decoder", "strong_id", "json_metadata", "custom_counts",
    ]).map_err(|e| e.to_string())?;
    for s in stats {
        wtr.write_record(&[
            s.shots.to_string(),
            s.errors.to_string(),
            s.discards.to_string(),
            format!("{:.4}", s.seconds),
            s.decoder.clone(),
            s.strong_id.clone(),
            serde_json::to_string(&s.metadata).unwrap_or_default(),
            serde_json::to_string(&s.custom_counts).unwrap_or_default(),
        ]).map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_csv(data: &[u8]) -> Result<Vec<TaskStats>, String> {
    let mut rdr = csv::Reader::from_reader(data);
    let mut results = Vec::new();
    for record in rdr.records() {
        let r = record.map_err(|e| e.to_string())?;
        results.push(TaskStats {
            shots: r[0].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            errors: r[1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            discards: r[2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            seconds: r[3].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?,
            decoder: r[4].to_string(),
            strong_id: r[5].to_string(),
            metadata: serde_json::from_str(&r[6]).unwrap_or(serde_json::Value::Null),
            custom_counts: serde_json::from_str(&r[7]).unwrap_or_default(),
        });
    }
    Ok(results)
}
