#[allow(dead_code)]
#[path = "../examples/support/cultivation_rng.rs"]
mod rng;
#[allow(dead_code)]
#[path = "../examples/paper_cultivation_run.rs"]
mod runner;

use rmagna::noise::RandomSource;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-cultivation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn run(directory: &Directory, mode: &str, start: u32, shots: u32) -> Value {
    let path = &directory.0;
    std::fs::write(path.join("physical.magna"), "R 0 1\nH 1\nT 1\nX_ERROR(1/8) 0\nX_ERROR(1/8) 0\nZ_ERROR(1/8) 1\nZ_ERROR(1/8) 1\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]").unwrap();
    std::fs::write(path.join("ideal.magna"), "H 0\nT 0").unwrap();
    std::fs::write(
        path.join("task.json"),
        r#"{"schema_version":1,"outputs":[1],"ideal_state":"ideal.magna"}"#,
    )
    .unwrap();
    std::fs::write(
        path.join("config.json"),
        json!({"schema":"rmagna.paper-cultivation-configuration.v1",
        "physical_rate":"1/8","mode":mode,"closing_sites":if mode=="physical" {0} else {2},
        "seed":2026101000,"start":start,"shots":shots,"max_signature_work":1<<26,
        "max_entropy_words":4096,"max_histogram_entries":1024})
        .to_string(),
    )
    .unwrap();
    runner::run(
        ["physical.magna", "task.json", "config.json"]
            .map(|name| path.join(name).to_str().unwrap().to_owned())
            .into_iter(),
    )
    .unwrap()
}

fn histogram(payload: &Value) -> std::collections::BTreeMap<String, u64> {
    payload["histogram"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let mut key = row.clone();
            key.as_object_mut().unwrap().remove("count");
            (key.to_string(), row["count"].as_u64().unwrap())
        })
        .collect()
}

#[test]
fn physical_full_t_draws_match_independent_fault_parities_and_resume_exactly() {
    let directory = Directory::new();
    let full = run(&directory, "physical", 0, 64);
    assert_eq!(full["status"], "complete", "{full}");
    assert_eq!(full["completed"], 64);
    assert_eq!(full["law"]["original_sites"], 4);
    assert_eq!(full["inputs"].as_object().unwrap().len(), 3);
    let mut expected = std::collections::BTreeMap::new();
    for index in 0..64 {
        let mut random = rng::draw_stream(2026101000, index);
        let faults: Vec<_> = (0..4).map(|_| (random.next_u64() & 7) == 0).collect();
        let accepted = !(faults[0] ^ faults[1]);
        let error = accepted && (faults[2] ^ faults[3]);
        let key=json!({"weight":{"occurrences":faults.iter().filter(|&&v|v).count(),"tail_candidates":0,
            "passing_candidates":0,"passes":false},"accepted":[if accepted {"1"} else {"0"},"0","0","0"],
            "error":[if error {"1"} else {"0"},"0","0","0"]}).to_string();
        *expected.entry(key).or_insert(0) += 1;
    }
    assert_eq!(histogram(&full), expected);
    let first = run(&directory, "physical", 0, 17);
    let second = run(&directory, "physical", 17, 47);
    let mut resumed = histogram(&first);
    for (key, count) in histogram(&second) {
        *resumed.entry(key).or_default() += count;
    }
    assert_eq!(histogram(&full), resumed);
    assert_eq!(
        full["inventory"]["after_preparation"]["source_assignments"],
        0
    );
    assert_eq!(full["inventory"]["after_warmup"]["source_assignments"], 1);
    assert_eq!(
        full["inventory"]["after_sampling"]["source_assignments"],
        65
    );
    assert_eq!(full["excluded_warmup"]["index"], u32::MAX);
}

#[test]
fn full_support_closing_retains_failed_constraints_and_exact_quantum_error() {
    let directory = Directory::new();
    let value = run(&directory, "closing", 0, 256);
    assert_eq!(value["status"], "complete", "{value}");
    assert_eq!(value["law"]["proposal_rate"], "7/8");
    let rows = value["histogram"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row["count"].as_u64().unwrap())
            .sum::<u64>(),
        256
    );
    assert!(rows.iter().any(|row| row["accepted"][0] == "0"));
    assert!(rows.iter().any(|row| row["error"][0] == "1"));
    assert_eq!(value["law"]["flow"]["retained"], json!([0]));
    assert_eq!(value["law"]["flow"]["unsafe_t"], json!([]));
}

#[test]
fn addressed_streams_match_known_words_and_preserve_draw_partition() {
    fn reference(mut state: u64) -> u64 {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        state = (state ^ (state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        state = (state ^ (state >> 27)).wrapping_mul(0x94d049bb133111eb);
        state ^ (state >> 31)
    }
    let mut initial_states = std::collections::BTreeSet::new();
    for seed in [0, 1, 2026101000, u32::MAX] {
        for index in [0, 1, 17, u32::MAX - 1, u32::MAX] {
            let child = reference((u64::from(seed) << 32) | u64::from(index));
            assert!(initial_states.insert(child));
            let mut actual = rng::draw_stream(seed, index);
            assert_eq!(actual.state(), child);
            assert_eq!(actual.next_u64(), reference(child));
        }
    }
}
