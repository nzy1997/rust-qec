//! Exercise the real paper runner with the pinned complete two-output T/S tasks.
#[allow(dead_code)]
#[path = "../examples/paper_bh_run.rs"]
mod runner;

use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rmagna-paper-bh-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(
        &self,
        variant: &str,
        configuration: &Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let fixture: Value = serde_json::from_str(include_str!("fixtures/bh-native-inputs.json"))?;
        for (name, text) in fixture["cases"][variant]["inputs"].as_object().unwrap() {
            std::fs::write(self.0.join(name), text.as_str().unwrap())?;
        }
        std::fs::write(self.0.join("configuration.json"), configuration.to_string())?;
        runner::run(
            ["physical.magna", "task.json", "configuration.json"]
                .into_iter()
                .map(|name| self.0.join(name).to_string_lossy().into_owned()),
        )
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn configuration() -> Value {
    json!({"schema":"rmagna.paper-bh-configuration.v1","outputs":2,"shots":64,
        "seed":2026101200,"proposal_scale":"1000/7","max_signature_work":17179869184_u64})
}

#[test]
fn complete_t_and_matched_s_tasks_share_exact_joint_totals_with_one_excluded_warmup() {
    let t = Directory::new().run("T", &configuration()).unwrap();
    let s = Directory::new().run("S", &configuration()).unwrap();
    for result in [&t, &s] {
        assert_eq!(result["status"], "complete");
        assert_eq!(result["widths"]["physical"], 30);
        assert_eq!(result["metrics"]["completed"], 64);
        assert_eq!(result["checkpoint"]["committed"], "64");
        assert_eq!(result["excluded_warmup"]["metrics"]["completed"], 1);
        assert_eq!(result["inventory"]["after_warmup"]["source_assignments"], 1);
        assert_eq!(
            result["inventory"]["after_sampling"]["source_assignments"],
            65
        );
        assert_eq!(
            result["inventory"]["after_preparation"]["storage"],
            result["inventory"]["after_sampling"]["storage"]
        );
        for family in ["acceptance", "target"] {
            assert_eq!(result["resources"][family]["maximum_quotient_rank"], 0);
            assert_eq!(result["resources"][family]["maximum_terms"], 1);
        }
    }
    assert_eq!(t["checkpoint"]["statistics"], s["checkpoint"]["statistics"]);
    assert_eq!(
        t["checkpoint"]["random_state"],
        s["checkpoint"]["random_state"]
    );
    assert_eq!(t["metrics"]["estimates"], s["metrics"]["estimates"]);
    assert_ne!(
        t["inputs"]["physical_sha256"],
        s["inputs"]["physical_sha256"]
    );
}

#[test]
fn bad_paper_configuration_joint_target_and_resource_admission_fail_before_production() {
    for (field, value) in [
        ("shots", json!(1)),
        ("outputs", json!(1)),
        ("unknown_option", json!(true)),
        ("max_signature_work", json!(0)),
    ] {
        let mut options = configuration();
        options[field] = value;
        assert!(
            Directory::new().run("T", &options).is_err(),
            "accepted invalid {field}"
        );
    }
    let mut options = configuration();
    options["outputs"] = json!(4);
    assert!(
        Directory::new()
            .run("T", &options)
            .unwrap_err()
            .to_string()
            .contains("complete joint target")
    );
}
