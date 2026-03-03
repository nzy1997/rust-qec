use std::path::Path;
use crate::task_stats::TaskStats;

pub fn plot_error_rate(
    _stats: &[TaskStats],
    _x_func: impl Fn(&TaskStats) -> f64,
    _group_func: impl Fn(&TaskStats) -> String,
    _output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_stat(p: f64, d: u64, shots: u64, errors: u64) -> TaskStats {
        TaskStats {
            strong_id: String::new(),
            decoder: String::new(),
            metadata: serde_json::json!({"p": p, "d": d}),
            shots,
            errors,
            discards: 0,
            seconds: 0.0,
            custom_counts: HashMap::new(),
        }
    }

    #[test]
    fn test_plot_svg_created() {
        let stats = vec![
            make_stat(0.001, 3, 10000, 10),
            make_stat(0.005, 3, 10000, 100),
            make_stat(0.01,  3, 10000, 500),
            make_stat(0.001, 5, 10000, 1),
            make_stat(0.005, 5, 10000, 20),
            make_stat(0.01,  5, 10000, 150),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot.svg");
        plot_error_rate(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            &out,
        ).unwrap();
        assert!(out.exists());
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("<svg"), "output should be SVG");
    }

    #[test]
    fn test_plot_png_created() {
        let stats = vec![
            make_stat(0.001, 3, 10000, 10),
            make_stat(0.01,  3, 10000, 500),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot.png");
        plot_error_rate(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            &out,
        ).unwrap();
        assert!(out.exists());
        // PNG magic bytes: 0x89 0x50 0x4E 0x47
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[0..4], b"\x89PNG");
    }
}
