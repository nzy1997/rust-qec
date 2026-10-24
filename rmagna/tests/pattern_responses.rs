use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::costs::ExecutionCosts;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{TaskEnumeration, TaskSectorMasses};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-pattern-responses-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, n: &str, s: &str) {
        std::fs::write(self.0.join(n), s).unwrap();
    }
    fn load(
        &self,
        compiler: CompilerStrategy,
        causal: CausalStrategy,
        calls: Arc<AtomicUsize>,
        numeric: u64,
    ) -> FileTask {
        let mut registry = ControllerRegistry::default();
        registry
            .register("copy", "1", move |bits: &[bool]| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(bits.to_vec())
            })
            .unwrap();
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = compiler;
        options.evaluation.runtime.strategy = causal;
        options.numeric.max_work_bits = numeric;
        FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
            &registry,
            NoiseSampling::Physical,
            options,
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
fn hex(v: &Value) -> BigInt {
    BigInt::parse_bytes(v.as_str().unwrap().replace("0x", "").as_bytes(), 16).unwrap()
}
fn scalar(v: &Value) -> ExactField {
    let power = v["exponent2"].as_i64().unwrap();
    let scale = if power >= 0 {
        BigRational::from_integer(BigInt::one() << (power as usize))
    } else {
        BigRational::new(BigInt::one(), BigInt::one() << ((-power) as usize))
    };
    FieldArithmetic::default()
        .value(std::array::from_fn(|j| {
            BigRational::from_integer(hex(&v["coefficients_hex"][j])) * &scale
        }))
        .unwrap()
}
fn counts(a: ExecutionCosts, b: ExecutionCosts) {
    for (a, b) in [
        (a.compilation, b.compilation),
        (a.query, b.query),
        (a.decoder, b.decoder),
        (a.support, b.support),
    ] {
        assert_eq!(
            (a.attempted, a.succeeded, a.failed),
            (b.attempted, b.succeeded, b.failed)
        );
    }
}
fn combine(result: &TaskEnumeration) {
    let f = FieldArithmetic::default();
    let records = result.pattern_responses.as_ref().unwrap();
    let mut total = vec![ExactField::zero(); 3 + result.masses.moments.len()];
    let mut weighted = vec![total.clone(); result.by_weight.len()];
    let mut probability = BigRational::zero();
    let mut histories = 0;
    for p in records {
        let mut values = vec![&p.response.accepted, &p.response.error];
        let target = p.response.target_success.as_ref().unwrap();
        values.push(target);
        values.extend(&p.response.moments);
        for (j, v) in values.into_iter().enumerate() {
            let value = f.scale(v, p.probability.value()).unwrap();
            total[j] = f.add(&total[j], &value).unwrap();
            weighted[p.choices.len()][j] = f.add(&weighted[p.choices.len()][j], &value).unwrap();
        }
        probability += p.probability.value();
        histories += p.histories;
    }
    let check = |v: &[ExactField], m: &TaskSectorMasses| {
        assert_eq!(v[0], m.accepted);
        assert_eq!(v[1], m.error);
        assert_eq!(v[2], *m.target_success.as_ref().unwrap());
        assert_eq!(&v[3..], m.moments);
    };
    check(&total, &result.masses);
    for sector in &result.by_weight {
        check(&weighted[sector.weight], &sector.masses);
    }
    assert_eq!(probability, *result.covered_probability.value());
    assert_eq!(histories, result.histories);
}
#[test]
fn retained_responses_match_native_and_recombine_without_extra_queries_or_callbacks() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/pattern_response_native.json")).unwrap();
    assert_eq!(
        fixture["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        fixture["native_sha256"],
        "24ec8f3c108ca3a624ca5b0fd0b536ceb01111736f5d737643eb8f44a6d4bd77"
    );
    assert_eq!(fixture["vectors"].as_array().unwrap().len(), 6);
    for v in fixture["vectors"].as_array().unwrap() {
        let files = Files::new();
        files.put("physical.magna", v["source"].as_str().unwrap());
        files.put("ideal.magna", v["ideal"].as_str().unwrap());
        let mut task = v["task"].clone();
        task["ideal_state"] = if v.get("target_outputs").is_some() {
            json!({"circuit":"ideal.magna","outputs":v["target_outputs"],"postselection":v["target_postselection"]})
        } else {
            json!("ideal.magna")
        };
        files.put("task.json", &task.to_string());
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                let calls = Arc::new(AtomicUsize::new(0));
                let file = files.load(compiler, causal, calls.clone(), 16 << 20);
                let mut registry = ControllerRegistry::default();
                registry
                    .register("copy", "1", |b: &[bool]| Ok(b.to_vec()))
                    .unwrap();
                let parsed = rmagna::circuit::parse_circuit(
                    v["source"].as_str().unwrap(),
                    &rmagna::circuit::ParseOptions {
                        controllers: registry.names(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let mut runtime = rmagna::runtime::RuntimeOptions {
                    strategy: causal,
                    ..Default::default()
                };
                runtime.compile.compiler_strategy = compiler;
                let site_map: BTreeMap<usize, usize> = if parsed.needs_history_sampling() {
                    let program = rmagna::runtime::CausalProgram::new(
                        &parsed,
                        &registry,
                        runtime,
                        &rmagna::scalar::ScalarArithmetic::default(),
                    )
                    .unwrap();
                    parsed
                        .operations()
                        .iter()
                        .filter_map(|op| program.potential_site(op.id).map(|site| (op.id, site)))
                        .collect()
                } else {
                    let compiled = rmagna::instrument::compile_affine(
                        &parsed,
                        &runtime.compile,
                        &rmagna::scalar::ScalarArithmetic::default(),
                    )
                    .unwrap();
                    compiled
                        .noise_sites()
                        .iter()
                        .enumerate()
                        .flat_map(|(site, category)| {
                            category
                                .source_operations()
                                .iter()
                                .map(move |op| (*op, site))
                        })
                        .collect()
                };

                let mut options = FileEnumerationOptions::default();
                options.enumeration.noise.max_weight = v["max_weight"].as_u64().unwrap() as usize;
                options.enumeration.max_retained_response_values = 0;
                let baseline = file.enumerate(&options).unwrap();
                let baseline_calls = calls.load(Ordering::Relaxed);
                options.enumeration.include_patterns = true;
                options.enumeration.max_retained_response_values =
                    v["patterns"].as_array().unwrap().len()
                        * (3 + v["task"]["observables"].as_array().map_or(0, Vec::len));
                let report = file.enumerate(&options).unwrap();
                let raw = report.raw_result().unwrap();
                counts(baseline.execution_costs(), report.execution_costs());
                assert_eq!(calls.load(Ordering::Relaxed), 2 * baseline_calls);
                let mut without = raw.clone();
                without.pattern_responses = None;
                assert_eq!(&without, baseline.raw_result().unwrap());
                combine(raw);
                let records = raw.pattern_responses.as_ref().unwrap();
                assert_eq!(
                    records.len(),
                    v["patterns"].as_array().unwrap().len(),
                    "{}",
                    v["case"]
                );
                assert_eq!(
                    raw.histories,
                    v["histories"].as_u64().unwrap() as usize,
                    "{}",
                    v["case"]
                );
                let f = FieldArithmetic::default();
                let norm = scalar(&v["target_normalization"]);
                for native in v["patterns"].as_array().unwrap() {
                    let choices = native["choices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| {
                            let category = c["category"].as_u64().unwrap() as usize;
                            let channel = c["channel"].as_u64().unwrap() as usize;
                            let operation = v["category_source_operations"][category][channel]
                                .as_u64()
                                .unwrap() as usize;
                            (site_map[&operation], channel)
                        })
                        .collect::<Vec<_>>();
                    let record = records
                        .iter()
                        .find(|p| {
                            p.choices
                                .iter()
                                .map(|c| (c.site, c.alternative))
                                .collect::<Vec<_>>()
                                == choices
                        })
                        .unwrap_or_else(|| panic!("missing native pattern case={} choices={choices:?}; Rust={records:?}", v["case"]));
                    assert_eq!(
                        *record.probability.value(),
                        BigRational::new(
                            hex(&native["probability"]["numerator_hex"]),
                            hex(&native["probability"]["denominator_hex"])
                        )
                    );
                    if v["case"] == "joint_report_selection" {
                        let chosen = |site| record.choices.iter().any(|c| c.site == site);
                        let (x0, x1, flip) = (chosen(0), chosen(1), chosen(2));
                        let accepted = u8::from(!(x0 ^ flip));
                        let expected = |n: i64| {
                            f.from_rational(BigRational::from_integer(n.into()))
                                .unwrap()
                        };
                        assert_eq!(record.response.accepted, expected(i64::from(accepted)));
                        assert_eq!(
                            record.response.error,
                            expected(i64::from(accepted) * i64::from(x0 || x1))
                        );
                        for (moment, negative) in
                            record.response.moments.iter().zip([x0, x1, x0 ^ x1])
                        {
                            assert_eq!(
                                *moment,
                                expected(i64::from(accepted) * if negative { -1 } else { 1 })
                            );
                        }
                    }
                    let response = &native["response"];
                    assert_eq!(scalar(&response[0]), ExactField::one());
                    let accepted = scalar(&response[1]);
                    let error = scalar(&response[2]);
                    assert_eq!(
                        record.response.accepted, accepted,
                        "case={} choices={:?}",
                        v["case"], choices
                    );
                    assert_eq!(
                        record.response.error,
                        f.divide(&error, &norm).unwrap(),
                        "case={} choices={:?} native={}",
                        v["case"],
                        choices,
                        native
                    );
                    assert_eq!(
                        record.response.target_success,
                        Some(
                            f.subtract(&accepted, &f.divide(&error, &norm).unwrap())
                                .unwrap()
                        )
                    );
                    for (j, m) in record.response.moments.iter().enumerate() {
                        assert_eq!(*m, scalar(&response[j + 3]));
                    }
                }
                let json = report.to_json().unwrap();
                assert_eq!(json["result"]["patterns"], records.len());
                assert_eq!(
                    json["result"]["pattern_responses"]
                        .as_array()
                        .unwrap()
                        .len(),
                    records.len()
                );
                assert_eq!(json["options"]["enumeration"]["include_patterns"], true);
            }
        }
    }
}
#[test]
fn response_cap_is_preflighted_and_numeric_failure_retains_owned_exact_records() {
    let files = Files::new();
    files.put(
        "physical.magna",
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1\nI 2",
    );
    files.put("ideal.magna", "I 0");
    files.put(
        "task.json",
        r#"{"outputs":[1],"ideal_state":"ideal.magna","observables":[[0,1]]}"#,
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let file = files.load(
        CompilerStrategy::SharedCommutingLayer,
        CausalStrategy::FixedTemplate,
        calls.clone(),
        0,
    );
    let mut options = FileEnumerationOptions::default();
    options.enumeration.include_patterns = true;
    options.enumeration.max_retained_response_values = 7;
    let failed = file.enumerate(&options).unwrap();
    assert!(!failed.is_complete());
    assert!(
        failed
            .raw_result()
            .as_ref()
            .err()
            .unwrap()
            .field
            .contains("max_retained_response_values")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(failed.execution_costs().decoder.attempted, 0);
    assert_eq!(failed.execution_costs().query.attempted, 1);
    options.enumeration.max_retained_response_values = 8;
    let report = file.enumerate(&options).unwrap();
    assert!(report.is_complete());
    assert_eq!(
        report
            .raw_result()
            .unwrap()
            .pattern_responses
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    let exact = report.raw_result().unwrap().clone();
    assert!(report.to_json().is_err());
    drop(file);
    drop(files);
    assert_eq!(report.raw_result(), Ok(&exact));
    assert!(report.to_json().is_err());
}

#[test]
fn cli_retains_one_saved_report_and_rejects_invalid_retention_before_decoder_import() {
    use std::process::Command;
    let files = Files::new();
    files.put(
        "physical.magna",
        "X_ERROR(1/3) 0\nX_ERROR(1/5) 1\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2] rec[-1]",
    );
    files.put("task.json", r#"{"error_observables":[0]}"#);
    let run = |args: &[&str], code| {
        let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&files.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(code),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let report = run(
        &[
            "enumerate",
            "physical.magna",
            "--task",
            "task.json",
            "--include-patterns",
            "--max-pattern-response-values",
            "6",
            "--output",
            "result.json",
            "--json",
        ],
        0,
    );
    assert_eq!(
        report,
        serde_json::from_slice::<Value>(&std::fs::read(files.0.join("result.json")).unwrap())
            .unwrap()
    );
    assert_eq!(report["result"]["patterns"], 3);
    assert_eq!(
        report["result"]["pattern_responses"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        report["options"]["enumeration"]["max_retained_response_values"],
        6
    );
    for pattern in report["result"]["pattern_responses"].as_array().unwrap() {
        assert_eq!(pattern["response"]["accepted"]["value"], 1.0);
        assert_eq!(
            pattern["response"]["error"]["value"],
            if pattern["weight"] == 0 { 0.0 } else { 1.0 }
        );
        assert!(pattern["response"]["target_success"].is_null());
    }
    let failed = run(
        &[
            "enumerate",
            "physical.magna",
            "--task",
            "task.json",
            "--include-patterns",
            "--max-pattern-response-values",
            "5",
            "--json",
        ],
        1,
    );
    assert!(failed["result"].is_null());
    assert_eq!(failed["execution_costs"]["query"]["attempted"], 0);
    assert!(
        failed["failure"]["field"]
            .as_str()
            .unwrap()
            .contains("max_retained_response_values")
    );
    let numeric = run(
        &[
            "enumerate",
            "physical.magna",
            "--task",
            "task.json",
            "--include-patterns",
            "--numeric-work-bits",
            "0",
            "--json",
        ],
        1,
    );
    assert_eq!(numeric["engine_status"], "complete");
    assert_eq!(numeric["traversal_counts"]["patterns"], 3);
    assert_eq!(numeric["options"]["enumeration"]["include_patterns"], true);
    assert_eq!(numeric["numeric_export"]["exact_results_persisted"], false);
    files.put("decoder.py", "raise RuntimeError('decoder imported')\n");
    files.put(
        "controllers.json",
        r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"1"}}}"#,
    );
    for mode in ["run", "validate", "inspect", "prepare", "compare-samplers"] {
        for flags in [
            vec!["--include-patterns"],
            vec!["--max-pattern-response-values", "8"],
        ] {
            let mut args = vec![
                mode,
                "physical.magna",
                "--controllers",
                "controllers.json",
                "--json",
            ];
            args.extend(flags);
            let rejected = run(&args, 2);
            assert_eq!(rejected["failures"][0]["phase"], "arguments");
        }
    }
    for cap in ["-1", "NaN", "1.5", "18446744073709551616"] {
        let rejected = run(
            &[
                "enumerate",
                "physical.magna",
                "--controllers",
                "controllers.json",
                "--max-pattern-response-values",
                cap,
                "--json",
            ],
            2,
        );
        assert_eq!(rejected["failures"][0]["phase"], "arguments");
    }
    run(
        &[
            "enumerate",
            "physical.magna",
            "--controllers",
            "controllers.json",
            "--include-patterns",
            "--include-patterns",
            "--json",
        ],
        2,
    );
}

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn real_quadratic(a: BigRational, b: BigRational) -> ExactField {
    FieldArithmetic::default()
        .value([a, b.clone(), BigRational::zero(), -b])
        .unwrap()
}

#[test]
fn independent_categorical_laws_cover_guarded_latents_selection_and_reused_plans() {
    use rmagna::circuit::{NoiseKind, Operation, ParseOptions, parse_circuit};
    use rmagna::runtime::{CausalProgram, RuntimeOptions};
    use std::collections::BTreeSet;
    let f = FieldArithmetic::default();
    for fixture in 0..3 {
        let majority = fixture == 0;
        let selected = fixture == 2;
        let files = Files::new();
        let mut source = if majority {
            "H 0\nT 0\nH 0\nH 1 2\nM(1/7) 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nX_ERROR(1/5) ctrl[0] 3\n".to_owned()
        } else {
            "H 0\nT 0\nH 0\nCORRELATED_ERROR(1/4) X0\nELSE_CORRELATED_ERROR(1/3) Z0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\n".to_owned()
        };
        if selected {
            source.push_str("POSTSELECT det[0]\n");
        }
        files.put("physical.magna", &source);
        files.put("ideal.magna", "I 0\n");
        files.put(
            "task.json",
            &json!({"outputs":[if majority {3} else {1}],"ideal_state":"ideal.magna","observables":[[0,1]]}).to_string(),
        );
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                let calls = Arc::new(AtomicUsize::new(0));
                let observed = calls.clone();
                let mut registry = ControllerRegistry::default();
                registry
                    .register(
                        if majority { "majority" } else { "copy" },
                        "independent-law-v1",
                        move |bits: &[bool]| {
                            observed.fetch_add(1, Ordering::Relaxed);
                            Ok(vec![if majority {
                                bits.iter().filter(|b| **b).count() >= 2
                            } else {
                                bits[0]
                            }])
                        },
                    )
                    .unwrap();
                let mut runtime = RuntimeOptions {
                    strategy: causal,
                    ..Default::default()
                };
                runtime.compile.compiler_strategy = compiler;
                let parsed = parse_circuit(
                    &source,
                    &ParseOptions {
                        controllers: registry.names(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let program =
                    CausalProgram::new(&parsed, &registry, runtime, &Default::default()).unwrap();
                let mut sites = BTreeMap::new();
                for op in parsed.operations() {
                    let semantic = match &op.operation {
                        Operation::Measure { record, .. } => {
                            Some(if majority { *record } else { 1 })
                        }
                        Operation::Noise {
                            kind: NoiseKind::X,
                            guard: Some(_),
                            ..
                        } if majority => Some(3),
                        Operation::Noise {
                            kind: NoiseKind::Correlated { alternative: 0, .. },
                            ..
                        } => Some(0),
                        _ => None,
                    };
                    if let Some(semantic) = semantic {
                        assert!(
                            sites
                                .insert(program.potential_site(op.id).unwrap(), semantic)
                                .is_none()
                        );
                    }
                }
                assert_eq!(sites.len(), if majority { 4 } else { 2 });
                let mut options = FileTaskOptions::default();
                options.evaluation.runtime = runtime;
                let file = FileTask::load(
                    &files.0.join("physical.magna"),
                    Some(&files.0.join("task.json")),
                    &registry,
                    NoiseSampling::Physical,
                    options,
                    &FileLoadOptions::default(),
                    BTreeMap::new(),
                )
                .unwrap();
                let compiled = file.compile().unwrap();
                assert_eq!(calls.load(Ordering::Relaxed), 0);
                for weight in [0, if majority { 4 } else { 2 }] {
                    let patterns = if weight == 0 {
                        1
                    } else if majority {
                        16
                    } else {
                        6
                    };
                    let histories = if majority { 8 } else { 2 };
                    // Guarded noise is a second, deterministic observation group.
                    // Each completed majority history replays its controller twice.
                    let expected_calls = patterns * histories * if majority { 2 } else { 1 };
                    let mut options = FileEnumerationOptions::default();
                    options.enumeration.noise.max_weight = weight;
                    options.enumeration.max_retained_response_values = 0;
                    let before = calls.load(Ordering::Relaxed);
                    let baseline = compiled.enumerate(&options).unwrap();
                    assert!(baseline.is_complete());
                    assert_eq!(calls.load(Ordering::Relaxed) - before, expected_calls);
                    assert!(baseline.raw_result().unwrap().pattern_responses.is_none());
                    options.enumeration.include_patterns = true;
                    options.enumeration.max_retained_response_values = 4 * patterns;
                    let before = calls.load(Ordering::Relaxed);
                    let report = compiled.enumerate(&options).unwrap();
                    assert!(report.is_complete());
                    assert_eq!(calls.load(Ordering::Relaxed) - before, expected_calls);
                    counts(baseline.execution_costs(), report.execution_costs());
                    let raw = report.raw_result().unwrap();
                    let mut plain = raw.clone();
                    plain.pattern_responses = None;
                    assert_eq!(&plain, baseline.raw_result().unwrap());
                    assert_eq!(raw.patterns, patterns);
                    assert_eq!(raw.histories, patterns * histories);
                    combine(raw);
                    let mut seen = BTreeSet::new();
                    for record in raw.pattern_responses.as_ref().unwrap() {
                        let mut choices = [0_usize; 4];
                        for choice in &record.choices {
                            let semantic = sites[&choice.site];
                            assert_eq!(choices[semantic], 0);
                            choices[semantic] = choice.alternative + 1;
                        }
                        assert!(seen.insert(choices));
                        let probability;
                        let error;
                        if majority {
                            assert!(choices.iter().all(|v| *v <= 1));
                            probability = choices.iter().enumerate().fold(
                                BigRational::one(),
                                |p, (j, value)| {
                                    let rate = rational(1, if j < 3 { 7 } else { 5 });
                                    p * if *value == 0 {
                                        BigRational::one() - rate
                                    } else {
                                        rate
                                    }
                                },
                            );
                            // Two fair reports make majority conditionally3/4
                            // for first report1 and1/4 for first report0. HTH's
                            // first true bit has probability1/2-sqrt(2)/4.
                            error = if choices[3] == 1 {
                                ExactField::zero()
                            } else {
                                real_quadratic(
                                    rational(1, 2),
                                    rational(if choices[0] == 0 { -1 } else { 1 }, 8),
                                )
                            };
                        } else {
                            assert!(choices[0] <= 2 && choices[1] <= 1);
                            probability = rational(if choices[0] == 0 { 2 } else { 1 }, 4)
                                * rational(if choices[1] == 0 { 6 } else { 1 }, 7);
                            let toggled = (choices[0] == 1) ^ (choices[1] == 1);
                            error = real_quadratic(
                                rational(1, 2),
                                rational(if toggled { 1 } else { -1 }, 4),
                            );
                        }
                        let accepted = if selected {
                            f.subtract(&ExactField::one(), &error).unwrap()
                        } else {
                            ExactField::one()
                        };
                        let error = if selected { ExactField::zero() } else { error };
                        assert_eq!(*record.probability.value(), probability);
                        assert_eq!(record.histories, histories);
                        assert_eq!(record.response.accepted, accepted);
                        assert_eq!(record.response.error, error);
                        assert_eq!(
                            record.response.target_success,
                            Some(f.subtract(&accepted, &error).unwrap())
                        );
                        assert_eq!(
                            record.response.moments,
                            [
                                f.subtract(&accepted, &f.scale(&error, &rational(2, 1)).unwrap())
                                    .unwrap()
                            ]
                        );
                    }
                    assert_eq!(seen.len(), patterns);
                    options.enumeration.max_retained_response_values -= 1;
                    let before = calls.load(Ordering::Relaxed);
                    let failed = compiled.enumerate(&options).unwrap();
                    assert!(
                        failed
                            .raw_result()
                            .unwrap_err()
                            .field
                            .contains("max_retained_response_values")
                    );
                    assert_eq!(calls.load(Ordering::Relaxed), before);
                    assert_eq!(failed.execution_costs().query.attempted, 0);
                    assert_eq!(failed.execution_costs().decoder.attempted, 0);
                }
            }
        }
    }
}
