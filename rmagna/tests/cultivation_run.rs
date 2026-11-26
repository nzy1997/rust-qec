#[allow(dead_code)]
#[path = "../examples/paper_cultivation_run.rs"]
mod runner;

use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

const TARGET_TASK: &str = r#"{"schema_version":1,"outputs":[1],"ideal_state":"ideal.magna"}"#;

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
    run_source(
        directory,
        mode,
        start,
        shots,
        "R 0 1
H 1
T 1
X_ERROR(1/8) 0
X_ERROR(1/8) 0
Z_ERROR(1/8) 1
Z_ERROR(1/8) 1
M 0
DETECTOR rec[-1]
POSTSELECT det[0]",
    )
}

fn run_source(directory: &Directory, mode: &str, start: u32, shots: u32, physical: &str) -> Value {
    run_source_with_task(directory, mode, start, shots, physical, TARGET_TASK, 1024)
}

fn run_source_with_task(
    directory: &Directory,
    mode: &str,
    start: u32,
    shots: u32,
    physical: &str,
    task: &str,
    histogram_limit: usize,
) -> Value {
    let path = &directory.0;
    std::fs::write(path.join("physical.magna"), physical).unwrap();
    std::fs::write(path.join("ideal.magna"), "H 0\nT 0").unwrap();
    std::fs::write(path.join("task.json"), task).unwrap();
    std::fs::write(
        path.join("config.json"),
        json!({"schema":"rmagna.paper-cultivation-configuration.v1",
        "physical_rate":"1/8","mode":mode,"closing_sites":if mode=="physical" {0} else {2},
        "seed":2026101000,"start":start,"shots":shots,"max_signature_work":1<<26,
        "max_entropy_words":4096,"max_histogram_entries":histogram_limit})
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

fn reference_word(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mixed = (*state ^ (*state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d049bb133111eb);
    mixed ^ (mixed >> 31)
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
    let mut no_fault = 0;
    let mut detector_rejections = 0;
    for index in 0..64 {
        let mut address = (2026101000u64 << 32) | index;
        let mut stream = reference_word(&mut address);
        let faults = std::array::from_fn::<_, 4, _>(|_| reference_word(&mut stream) & 7 == 0);
        let accepted = !(faults[0] ^ faults[1]);
        no_fault += u64::from(faults.iter().all(|&bit| !bit));
        detector_rejections += u64::from(!accepted);
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
    let work = &full["score_work"];
    assert_eq!(work["reused_no_fault"], no_fault);
    assert_eq!(work["rejected_by_detectors"], detector_rejections);
    let reused = work["reused_signature"].as_u64().unwrap();
    assert!(
        reused > 0,
        "equivalent nonempty assignments should reuse exact masses"
    );
    let evaluated = work["evaluated_full_t"].as_u64().unwrap();
    assert!(
        evaluated > 0,
        "a new target-affecting assignment must use full T"
    );
    assert_eq!(no_fault + detector_rejections + reused + evaluated, 64);
    assert_eq!(
        full["inventory"]["after_preparation"]["source_assignments"],
        1
    );
    assert_eq!(full["inventory"]["after_warmup"]["source_assignments"], 2);
    assert_eq!(
        full["inventory"]["after_sampling"]["source_assignments"],
        evaluated + 2
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
fn a_zero_full_t_acceptance_skips_target_density_after_the_detector_screen() {
    let directory = Directory::new();
    let shots = 64;
    // The backward Pauli detector screen stops at T_DAG. In the full
    // circuit T and T_DAG cancel, so a Z fault makes selection impossible.
    let result = run_source(
        &directory,
        "physical",
        0,
        shots,
        "RX 0
R 1
H 1
T 1
T 0
T_DAG 0
Z_ERROR(1/8) 0
MX 0
DETECTOR rec[-1]
POSTSELECT det[0]",
    );
    assert_eq!(result["status"], "complete");
    assert_eq!(result["law"]["flow"]["unsafe_t"], json!([0]));
    let work = &result["score_work"];
    assert_eq!(work["rejected_by_detectors"], 0);
    assert_eq!(work["evaluated_full_t"], 1);
    assert_eq!(result["skipped_zero_targets"], 1);
    let fault_count = (0..shots)
        .filter(|&index| {
            let mut address = (2026101000u64 << 32) | u64::from(index);
            let mut stream = reference_word(&mut address);
            reference_word(&mut stream) & 7 == 0
        })
        .count();
    assert_eq!(work["reused_signature"], fault_count - 1);
    for row in result["histogram"].as_array().unwrap() {
        let faulted = row["weight"]["occurrences"] == 1;
        assert_eq!(
            row["accepted"],
            json!([if faulted { "0" } else { "1" }, "0", "0", "0"])
        );
        assert_eq!(row["error"], json!(["0", "0", "0", "0"]));
        assert_eq!(
            row["count"],
            if faulted {
                fault_count
            } else {
                shots as usize - fault_count
            }
        );
    }
    assert_eq!(
        result["inventory"]["after_sampling"]["work"]["source"]["closure_evaluations"],
        5
    );
}

#[test]
fn histogram_limit_preserves_the_committed_score_prefix() {
    let directory = Directory::new();
    let physical = "RX 0\nR 1\nH 1\nT 1\nT 0\nT_DAG 0\nZ_ERROR(1/8) 0\nMX 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let first_fault = (0..64)
        .find(|&index| {
            let mut address = (2026101000u64 << 32) | index;
            let mut stream = reference_word(&mut address);
            reference_word(&mut stream) & 7 == 0
        })
        .unwrap();
    assert!(first_fault > 0);
    let value = run_source_with_task(&directory, "physical", 0, 64, physical, TARGET_TASK, 1);
    assert_eq!(value["status"], "failed");
    assert_eq!(value["completed"], first_fault);
    assert_eq!(value["failure"]["index"], first_fault);
    assert_eq!(
        value["failure"]["reason"],
        "cultivation max_histogram_entries exceeded"
    );
    assert_eq!(
        value["score_work"],
        json!({
            "rejected_by_detectors": 0, "reused_no_fault": first_fault, "reused_signature": 0,
            "evaluated_full_t": 0
        })
    );
    assert_eq!(value["skipped_zero_targets"], 0);
    assert_eq!(
        value["inventory"]["after_sampling"]["source_assignments"],
        3
    );
    assert_eq!(histogram(&value).into_values().sum::<u64>(), first_fault);
}

#[test]
fn tasks_with_observables_keep_exact_target_scoring() {
    let directory = Directory::new();
    let physical = "R 0 1\nH 1\nT 1\nZ_ERROR(1/8) 1\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let with_observables = run_source_with_task(
        &directory,
        "physical",
        0,
        64,
        physical,
        r#"{"schema_version":1,"outputs":[1],"ideal_state":"ideal.magna","observables":[[0,1]]}"#,
        1024,
    );
    let target_only = run_source(&directory, "physical", 0, 64, physical);
    assert_eq!(with_observables["status"], "complete", "{with_observables}");
    assert_eq!(with_observables["skipped_zero_targets"], 0);
    assert_eq!(with_observables["score_work"]["evaluated_full_t"], 64);
    assert_eq!(histogram(&with_observables), histogram(&target_only));
}
