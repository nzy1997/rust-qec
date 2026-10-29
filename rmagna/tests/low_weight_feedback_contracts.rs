//! Complete finite feedback/failure contracts from pinned
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_low_weight.py. No native binding or cursor ABI claim.
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-low-feedback-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let files = Self(path);
        files.put("target.magna", "I 0\n");
        files
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn load(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        compiler: CompilerStrategy,
        causal: CausalStrategy,
    ) -> FileTask {
        self.put("physical.magna", source);
        self.put("task.json", task);
        let mut configured = FileTaskOptions::default();
        configured.evaluation.runtime.strategy = causal;
        configured.evaluation.runtime.compile.compiler_strategy = compiler;
        FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
            registry,
            NoiseSampling::Physical,
            configured,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
type Calls = Arc<Mutex<Vec<Vec<bool>>>>;
fn controller(name: &str, and: bool) -> (ControllerRegistry, Calls) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let saved = seen.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register(name, "1", move |bits: &[bool]| {
            saved.lock().unwrap().push(bits.to_vec());
            Ok(if and {
                vec![bits[0] & bits[1]]
            } else {
                bits.to_vec()
            })
        })
        .unwrap();
    (registry, seen)
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn magic(sign: i64) -> ExactField {
    // Literal (2 +/- sqrt(2))/4, using sqrt(2)=omega-omega^3.
    let scalar = ScalarArithmetic::default()
        .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
        .unwrap();
    FieldArithmetic::default().from_scalar(&scalar).unwrap()
}
fn pair(n: i64, d: i64) -> Value {
    json!({"numerator":n.to_string(),"denominator":d.to_string()})
}
fn exact(value: &Value, expected: &ExactField) {
    assert_eq!(value["exact"]["coefficients"],Value::Array(expected.coefficients().iter().map(|v|json!({"numerator":v.numer().to_string(),"denominator":v.denom().to_string()})).collect()));
}
fn enumeration(weight: usize) -> FileEnumerationOptions {
    let mut options = FileEnumerationOptions::default();
    options.enumeration.noise.max_weight = weight;
    options.enumeration.include_patterns = true;
    options
}
fn configurations() -> impl Iterator<Item = (CompilerStrategy, CausalStrategy)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix]
            .into_iter()
            .map(move |causal| (compiler, causal))
    })
}
fn route(causal: CausalStrategy) -> &'static str {
    match causal {
        CausalStrategy::FixedTemplate => "causal_fixed_template",
        CausalStrategy::ColdPrefix => "causal_cold_prefix",
    }
}
const MAGIC: &str = "H 0\nT 0\nH 0\nCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCX ctrl[0] 2";
const ECHO: &str = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0";

#[test]
fn original_nonlinear_magic_feedback_integrates_both_irrational_positive_branches() {
    let files = Files::new();
    let minus = magic(-1);
    let plus = magic(1);
    assert_eq!(
        FieldArithmetic::default().add(&minus, &plus).unwrap(),
        ExactField::one()
    );
    for (compiler, causal) in configurations() {
        let (registry, seen) = controller("and", true);
        let file = files.load(
            MAGIC,
            r#"{"outputs":[2],"ideal_state":"target.magna"}"#,
            &registry,
            compiler,
            causal,
        );
        let prepared = file.compile().unwrap();
        let options = enumeration(0);
        let mut previous = None;
        for reused in [false, true, true] {
            seen.lock().unwrap().clear();
            let report = if reused {
                prepared.enumerate(&options)
            } else {
                file.enumerate(&options)
            }
            .unwrap();
            assert!(report.is_complete());
            assert_eq!(report.metadata()["route"], route(causal));
            let raw = report.raw_result().unwrap();
            assert_eq!(raw.patterns, 1);
            assert_eq!(raw.histories, 2);
            assert_eq!(raw.covered_probability, Probability::one());
            assert_eq!(raw.omitted_probability, Probability::zero());
            assert_eq!(raw.masses.accepted, ExactField::one());
            assert_eq!(raw.masses.error, minus);
            assert_eq!(raw.masses.target_success, Some(plus.clone()));
            let calls = seen.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(
                calls.iter().cloned().collect::<BTreeSet<_>>(),
                BTreeSet::from([vec![false, false], vec![true, true]])
            );
            drop(calls);
            let patterns = raw.pattern_responses.as_ref().unwrap();
            assert_eq!(patterns.len(), 1);
            assert!(patterns[0].choices.is_empty());
            assert_eq!(patterns[0].probability, Probability::one());
            assert_eq!(patterns[0].histories, 2);
            assert_eq!(patterns[0].response, raw.masses);
            let value = report.to_json().unwrap();
            assert_eq!(value["status"], "complete");
            exact(&value["result"]["masses"]["error"], &minus);
            exact(&value["result"]["masses"]["target_success"], &plus);
            let ordinary = value["result"]["masses"]["error"]["value"]
                .as_f64()
                .unwrap();
            assert!((ordinary - (2.0 - 2.0_f64.sqrt()) / 4.0).abs() < 1e-15);
            assert!(
                value["original_noise_categories"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            if let Some(old) = &previous {
                assert_eq!(raw, old);
            }
            previous = Some(raw.clone());
        }
    }
}

#[test]
fn original_terminal_feedback_and_guarded_noise_preserve_physical_weights() {
    let files = Files::new();
    files.put(
        "recovery.magna",
        "H 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0",
    );
    for (compiler, causal) in configurations() {
        let (registry, seen) = controller("echo", false);
        for recovery in [true, false] {
            let (source, task, p, error, success) = if recovery {
                (
                    "X_ERROR(1/10) 0",
                    r#"{"outputs":[0],"recovery":"recovery.magna","ideal_state":"target.magna"}"#,
                    (1, 10),
                    (1, 2),
                    (1, 2),
                )
            } else {
                (
                    "H 1\nM 1\nX_ERROR(1/5) rec[-1] 0",
                    r#"{"outputs":[0],"ideal_state":"target.magna"}"#,
                    (1, 5),
                    (1, 10),
                    (9, 10),
                )
            };
            let file = files.load(source, task, &registry, compiler, causal);
            let prepared = file.compile().unwrap();
            let options = enumeration(1);
            let mut previous = None;
            for reused in [false, true, true] {
                seen.lock().unwrap().clear();
                let report = if reused {
                    prepared.enumerate(&options)
                } else {
                    file.enumerate(&options)
                }
                .unwrap();
                assert!(report.is_complete());
                let raw = report.raw_result().unwrap();
                assert_eq!(raw.patterns, 2);
                assert_eq!(raw.covered_probability, Probability::one());
                assert_eq!(raw.omitted_probability, Probability::zero());
                assert_eq!(raw.masses.accepted, ExactField::one());
                assert_eq!(raw.masses.error, field(error.0, error.1));
                assert_eq!(raw.masses.target_success, Some(field(success.0, success.1)));
                let patterns = raw.pattern_responses.as_ref().unwrap();
                assert_eq!(patterns.len(), 2);
                assert_eq!(
                    patterns[0].probability.value(),
                    &BigRational::new((p.1 - p.0).into(), p.1.into())
                );
                assert_eq!(
                    patterns[1].probability.value(),
                    &BigRational::new(p.0.into(), p.1.into())
                );
                assert!(patterns[0].choices.is_empty());
                assert_eq!(patterns[1].choices.len(), 1);
                for (i, row) in patterns.iter().enumerate() {
                    assert_eq!(row.response.accepted, ExactField::one());
                    let conditional = if recovery || i == 1 {
                        field(1, 2)
                    } else {
                        ExactField::zero()
                    };
                    assert_eq!(row.response.error, conditional);
                }
                if recovery {
                    assert_eq!(report.metadata()["route"], route(causal));
                    assert_eq!(raw.histories, 4);
                    assert!(patterns.iter().all(|row| row.histories == 2));
                    let calls = seen.lock().unwrap();
                    assert_eq!(calls.len(), 4);
                    assert_eq!(
                        calls.iter().cloned().collect::<BTreeSet<_>>(),
                        BTreeSet::from([vec![false], vec![true]])
                    );
                } else {
                    // A guarded physical assignment retains both positive M1
                    // histories, even though no decoder callback is needed.
                    assert_eq!(report.metadata()["route"], route(causal));
                    assert_eq!(raw.histories, 4);
                    assert!(patterns.iter().all(|row| row.histories == 2));
                    assert!(seen.lock().unwrap().is_empty());
                }
                let value = report.to_json().unwrap();
                assert_eq!(value["status"], "complete");
                exact(
                    &value["result"]["masses"]["error"],
                    &field(error.0, error.1),
                );
                let categories = value["original_noise_categories"].as_array().unwrap();
                assert_eq!(categories.len(), 1);
                let category = &categories[0];
                assert_eq!(category["original_category"], 0);
                assert_eq!(category["channels"].as_array().unwrap().len(), 1);
                assert_eq!(category["channels"][0]["probability"], pair(p.0, p.1));
                assert_eq!(category["identity_probability"], pair(p.1 - p.0, p.1));
                assert_eq!(
                    category["channels"][0]["action"],
                    json!({"kind":"pauli","factors":[{"qubit":0,"basis":"X"}]})
                );
                if recovery {
                    assert!(category["guard"].is_null());
                    assert_eq!(category["internal_site"], 0);
                    assert_eq!(category["source_operations"][0]["line"], 1);
                } else {
                    assert_eq!(
                        category["guard"],
                        json!({"constant":false,"terms":[{"kind":"record","record":0}]})
                    );
                    assert_eq!(category["internal_site"], 1);
                    assert_eq!(category["source_operations"][0]["line"], 3);
                }
                let retained = value["result"]["pattern_responses"].as_array().unwrap();
                assert_eq!(retained[1]["choices"][0]["original_category"], 0);
                if let Some(old) = &previous {
                    assert_eq!(raw, old);
                }
                previous = Some(raw.clone());
            }
        }
    }
}

#[test]
fn original_pattern_and_branch_caps_fail_without_estimates_and_allow_prepared_retry() {
    let files = Files::new();
    for (compiler, causal) in configurations() {
        let (registry, seen) = controller("echo", false);
        for branches in [false, true] {
            let source = if branches {
                ECHO
            } else {
                "X_ERROR(1/10) 0\nX_ERROR(1/10) 0"
            };
            let file = files.load(
                source,
                r#"{"outputs":[0],"ideal_state":"target.magna"}"#,
                &registry,
                compiler,
                causal,
            );
            let prepared = file.compile().unwrap();
            let mut limited = enumeration(if branches { 0 } else { 2 });
            let option = if branches {
                limited.enumeration.histories.max_branches = 1;
                "max_branches"
            } else {
                limited.enumeration.noise.max_patterns = 3;
                "max_patterns"
            };
            for reused in [false, true] {
                let failed = if reused {
                    prepared.enumerate(&limited)
                } else {
                    file.enumerate(&limited)
                }
                .unwrap();
                assert!(!failed.is_complete());
                assert!(
                    failed
                        .raw_result()
                        .unwrap_err()
                        .to_string()
                        .contains(option)
                );
                let value = failed.to_json().unwrap();
                assert_eq!(value["status"], "failed");
                assert_eq!(value["failure_phase"], "enumeration");
                assert!(value["result"].is_null());
                assert!(
                    value["failure"]["message"]
                        .as_str()
                        .unwrap()
                        .contains(option)
                );
                // Rust returns no partial TaskEnumeration: no completed-pattern
                // estimate can escape, rather than exposing a native zero cursor.
                assert!(value["result"]["patterns"].is_null());
                assert!(value["result"]["masses"].is_null());
            }
            let mut adequate = limited;
            if branches {
                adequate.enumeration.histories.max_branches = 2;
            } else {
                adequate.enumeration.noise.max_patterns = 4;
            }
            seen.lock().unwrap().clear();
            let again = prepared.enumerate(&adequate).unwrap();
            assert!(again.is_complete());
            let raw = again.raw_result().unwrap();
            assert_eq!(raw.covered_probability, Probability::one());
            assert_eq!(raw.omitted_probability, Probability::zero());
            assert_eq!(raw.masses.accepted, ExactField::one());
            if branches {
                assert_eq!(raw.patterns, 1);
                assert_eq!(raw.histories, 2);
                assert_eq!(raw.masses.error, ExactField::zero());
                assert_eq!(raw.masses.target_success, Some(ExactField::one()));
                assert_eq!(
                    seen.lock()
                        .unwrap()
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from([vec![false], vec![true]])
                );
            } else {
                assert_eq!(raw.patterns, 4);
                assert_eq!(raw.masses.error, field(9, 50));
                assert_eq!(raw.masses.target_success, Some(field(41, 50)));
                let patterns = raw.pattern_responses.as_ref().unwrap();
                let mut masses = patterns
                    .iter()
                    .map(|row| row.probability.value().clone())
                    .collect::<Vec<_>>();
                masses.sort();
                assert_eq!(
                    masses,
                    vec![
                        BigRational::new(1.into(), 100.into()),
                        BigRational::new(9.into(), 100.into()),
                        BigRational::new(9.into(), 100.into()),
                        BigRational::new(81.into(), 100.into())
                    ]
                );
            }
            let fresh = file.enumerate(&adequate).unwrap();
            assert_eq!(fresh.raw_result(), again.raw_result());
        }
    }
}
