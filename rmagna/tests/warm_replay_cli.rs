//! Exercise the diagnostic's actual argument/file/evaluator/process/replay path.
#[path = "../examples/evaluate_task.rs"]
mod diagnostic;

use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(changing: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-warm-replay-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        fixture.put(
            "physical.magna",
            "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\n",
        );
        fixture.put("ideal.magna", "I 0\n");
        fixture.put(
            "task.json",
            r#"{"outputs":[1],"ideal_state":"ideal.magna"}"#,
        );
        fixture.put("history.json", "[[false]]\n");
        fixture.put(
            "plugin.py",
            r#"import json
import os
from pathlib import Path
Path(__file__).with_suffix('.imported').write_text('imported')
calls = 0

def decode(bits, config):
    global calls
    calls += 1
    output = bool(bits[0]) ^ (config['changing'] and calls % 2 == 0)
    with Path(config['log']).open('a', encoding='utf-8') as stream:
        stream.write(json.dumps(dict(pid=os.getpid(), inputs=[bool(bit) for bit in bits], outputs=[output])) + '\n')
    return [output]
"#,
        );
        fixture.put(
            "controllers.json",
            &json!({"schema_version":1,"bindings":{
                "copy":{"import":"plugin:decode","version":"warm-replay-test-v1",
                        "config":{"changing":changing,"log":fixture.path("calls.jsonl")}}
            }})
            .to_string(),
        );
        fixture
    }
    fn put(&self, name: &str, value: &str) {
        std::fs::write(self.path(name), value).unwrap();
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn execute(&self, flags: &[&str]) -> (Result<(), String>, Vec<u8>) {
        let mut arguments = vec![
            self.path("physical.magna").display().to_string(),
            self.path("task.json").display().to_string(),
            "--controllers".into(),
            self.path("controllers.json").display().to_string(),
            "--module-root".into(),
            self.0.display().to_string(),
            "--python".into(),
            "python3".into(),
        ];
        arguments.extend(flags.iter().map(|value| value.to_string()));
        let mut output = Vec::new();
        let result = diagnostic::run(arguments.into_iter(), &mut output).map_err(|e| e.to_string());
        (result, output)
    }
    fn success(&self, flags: &[&str]) -> Value {
        let (result, bytes) = self.execute(flags);
        result.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
    fn calls(&self) -> Vec<Value> {
        match std::fs::read_to_string(self.path("calls.jsonl")) {
            Ok(text) => text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("{error}"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn scalar(value: &Value) -> ExactScalar {
    let coefficients = value["coefficients"].as_array().unwrap();
    assert_eq!(coefficients.len(), 4);
    ScalarArithmetic::default()
        .scalar(
            std::array::from_fn(|i| coefficients[i].as_str().unwrap().parse().unwrap()),
            value["power"].as_i64().unwrap(),
        )
        .unwrap()
}
fn check_result(fixture: &Fixture, value: &Value, count: usize) {
    assert_eq!(value["configuration"]["warm_replays"], count);
    for field in ["compile_ms", "query_ms"] {
        let duration = value[field].as_f64().unwrap();
        assert!(duration.is_finite() && duration >= 0.0);
    }
    if count == 0 {
        assert!(value["warm_replays"].is_null());
    } else {
        let warm = &value["warm_replays"];
        assert_eq!(warm["count"], count);
        assert_eq!(warm["cache_unchanged"], true);
        assert_eq!(
            warm["scope"],
            "same-assignment replay with reset entropy/history; not independent Monte Carlo or formal performance rounds"
        );
        let times = warm["query_ms"].as_array().unwrap();
        assert_eq!(times.len(), count);
        assert!(
            times
                .iter()
                .all(|v| v.as_f64().is_some_and(|t| t.is_finite() && t >= 0.0))
        );
    }
    let bit = check_decoder(fixture, value, count);
    check_masses(value, bit);
    check_cache(value);
}
fn check_decoder(fixture: &Fixture, value: &Value, count: usize) -> bool {
    let calls = fixture.calls();
    assert_eq!(
        calls.len(),
        count + 1,
        "the actual decoder must run on every call"
    );
    let first = calls.first().unwrap();
    for call in &calls {
        assert_eq!(
            call, first,
            "replays must reuse one process and reset the same inputs"
        );
    }
    assert!(first["pid"].as_u64().unwrap() > 0);
    assert_eq!(first["inputs"], first["outputs"]);
    assert_eq!(value["controller_calls"].as_array().unwrap().len(), 1);
    assert_eq!(value["controller_calls"][0]["name"], "copy");
    assert_eq!(value["controller_calls"][0]["inputs"], first["inputs"]);
    assert_eq!(value["controller_calls"][0]["outputs"], first["outputs"]);
    first["inputs"][0].as_bool().unwrap()
}
fn check_masses(value: &Value, bit: bool) {
    // A literal H-T-H measurement has P(0/1) = (2 +/- sqrt(2))/4.
    // The copied bit controls X on the target, so target error is its true bit.
    let sign = if bit { -1 } else { 1 };
    let a = ScalarArithmetic::default();
    let mass = a
        .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
        .unwrap();
    assert_eq!(scalar(&value["history_mass"]), mass);
    assert_eq!(scalar(&value["raw_acceptance"]), mass);
    assert_eq!(
        scalar(&value["raw_target_success"]),
        if bit {
            ExactScalar::zero()
        } else {
            mass.clone()
        }
    );
    assert_eq!(
        scalar(&value["raw_error"]),
        if bit { mass } else { ExactScalar::zero() }
    );
    assert_eq!(scalar(&value["target_normalization"]), ExactScalar::one());
    if value["configuration"]["history_sampler"] == "AffineSupport" {
        assert_eq!(
            scalar(&value["history_proposal_mass"]),
            a.omega(0, -2).unwrap()
        );
    } else {
        assert!(value["history_proposal_mass"].is_null());
    }
}
fn check_cache(value: &Value) {
    if value["configuration"]["causal_strategy"] == "ColdPrefix" {
        assert!(value["prepared_probability_cache"].is_null());
    } else {
        let cache = &value["prepared_probability_cache"];
        match value["configuration"]["probability_preparation"]
            .as_str()
            .unwrap()
        {
            "cold" => assert_eq!(cache, &json!({"cached_queries":0,"fixed_resources":0})),
            "trace" => {
                assert!(cache["cached_queries"].as_u64().unwrap() > 0);
                assert_eq!(cache["fixed_resources"], 0);
            }
            "fixed" => {
                assert!(cache["cached_queries"].as_u64().unwrap() > 0);
                assert_eq!(cache["cached_queries"], cache["fixed_resources"]);
            }
            _ => panic!("unknown preparation mode"),
        }
    }
}
#[test]
fn actual_warm_replays_preserve_irrational_masses_inputs_process_and_cache_modes() {
    for compiler in ["parity_copies", "shared"] {
        for mode in ["cold", "trace", "fixed"] {
            for causal in ["fixed", "cold"] {
                for sampler in ["exact_marginal", "affine_support"] {
                    let fixture = Fixture::new(false);
                    let value = fixture.success(&[
                        "--compiler",
                        compiler,
                        "--probability-preparation",
                        mode,
                        "--causal-strategy",
                        causal,
                        "--history-sampler",
                        sampler,
                        "--warm-replays",
                        "2",
                    ]);
                    assert_eq!(
                        value["compilation"]["strategy"],
                        if compiler == "shared" {
                            "SharedCommutingLayer"
                        } else {
                            "ParityCopies"
                        }
                    );
                    assert_eq!(
                        value["compilation"]["scope"],
                        if causal == "fixed" {
                            "initial_fixed_template"
                        } else {
                            "initial_potential"
                        }
                    );
                    check_result(&fixture, &value, 2);
                }
            }
        }
    }
}
#[test]
fn actual_warm_replays_reject_changed_stateful_decoder_outputs() {
    for compiler in ["parity_copies", "shared"] {
        for mode in ["cold", "trace", "fixed"] {
            let fixture = Fixture::new(true);
            let (result, output) = fixture.execute(&[
                "--compiler",
                compiler,
                "--probability-preparation",
                mode,
                "--constant-word",
                "0",
                "--warm-replays",
                "1",
            ]);
            assert_eq!(
                result.unwrap_err(),
                "warm replay changed an exact evaluation field or controller transcript"
            );
            assert!(
                output.is_empty(),
                "a failed replay must not emit success JSON"
            );
            let calls = fixture.calls();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0]["pid"], calls[1]["pid"]);
            assert_eq!(calls[0]["inputs"], calls[1]["inputs"]);
            assert_ne!(calls[0]["outputs"], calls[1]["outputs"]);
        }
    }
}
#[test]
fn actual_warm_replay_defaults_bounds_seeds_and_prescribed_histories_are_enforced() {
    for count in [None, Some("0"), Some("1"), Some("16")] {
        for seed in ["123", "18446744073709551615"] {
            let fixture = Fixture::new(false);
            let mut flags = vec![seed];
            if let Some(count) = count {
                flags.extend(["--warm-replays", count]);
            }
            let value = fixture.success(&flags);
            assert_eq!(value["configuration"]["seed"], seed.parse::<u64>().unwrap());
            check_result(&fixture, &value, count.unwrap_or("0").parse().unwrap());
        }
    }
    let fixture = Fixture::new(false);
    let history = fixture.path("history.json");
    let value = fixture.success(&[
        "--history",
        history.to_str().unwrap(),
        "--warm-replays",
        "2",
    ]);
    check_result(&fixture, &value, 2);
    assert_eq!(value["controller_calls"][0]["inputs"], json!([false]));
    for count in [Some("17"), Some("-1"), Some("1.5"), Some("invalid"), None] {
        let fixture = Fixture::new(false);
        let mut flags = vec!["--warm-replays"];
        if let Some(count) = count {
            flags.push(count);
        }
        let (result, output) = fixture.execute(&flags);
        assert!(result.is_err());
        assert!(output.is_empty());
        assert!(fixture.calls().is_empty());
        assert!(
            !fixture.path("plugin.imported").exists(),
            "invalid limits must fail before importing the decoder"
        );
    }
}
