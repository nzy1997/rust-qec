use rmagna::circuit::{Circuit, ParseOptions, parse_circuit};
use rmagna::fibers::FiberOptions;
use rmagna::field::FieldArithmetic;
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ScalarArithmetic;
use rmagna::task::{
    ContentFingerprints, EvaluationOptions, FingerprintError, FingerprintInputs,
    FingerprintOptions, LoadedModules, TaskOptions, TaskRunOptions, canonical_fingerprint,
    canonical_raw_fingerprint, fingerprint_content, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Environment {
    physical: Circuit,
    modules: LoadedModules,
    registry: ControllerRegistry,
    identities: BTreeMap<String, Value>,
    evaluation: EvaluationOptions,
    run: TaskRunOptions,
    sampling: NoiseSampling,
    numeric: NumericOptions,
}
impl Environment {
    fn new(physical: &str, task: &str, path: &Path) -> Self {
        let physical = parse_circuit(physical, &ParseOptions::default()).unwrap();
        let modules = parse_task(task, path, &TaskOptions::default())
            .unwrap()
            .load_modules(&physical, &ParseOptions::default())
            .unwrap();
        Self {
            physical,
            modules,
            registry: ControllerRegistry::default(),
            identities: BTreeMap::new(),
            evaluation: EvaluationOptions::default(),
            run: TaskRunOptions::default(),
            sampling: NoiseSampling::Physical,
            numeric: NumericOptions::default(),
        }
    }
    fn inputs(&self) -> FingerprintInputs<'_> {
        FingerprintInputs {
            physical: &self.physical,
            modules: &self.modules,
            controllers: &self.registry,
            controller_identities: &self.identities,
            evaluation: self.evaluation,
            run: self.run,
            sampling: &self.sampling,
            numeric: self.numeric,
        }
    }
    fn fingerprints(&self) -> ContentFingerprints {
        fingerprint_content(&self.inputs(), FingerprintOptions::default()).unwrap()
    }
}
#[test]
fn canonical_sha256_matches_independent_python_hashlib_witnesses_and_limits() {
    // Independently generated with hashlib.sha256(prefix + json.dumps(value,
    // ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).
    let witnesses = [
        (
            "null",
            "49eb7bb0538ce6cb8eaa3bf72afef0591e24a260d50422f0e585b8cd815288a3",
        ),
        (
            r#"{"z":[true,null,{"b":"λ\n\t\r\b\f\u0001\"\\","a":123456789012345678901234567890}],"a":-3}"#,
            "aa4e9cce08d5f4c9927b2b45f79d2097999ce1a3581e2a4af0c0b1d0e510341d",
        ),
        (
            "[1,2]",
            "b00347a0691f00397e24359028bce79b1fd45075779bc8812c3559fe0a8c0aaa",
        ),
        (
            "[2,1]",
            "c9825c6f0041c3e9b2718918ead1a61f7b444408ae73d7ed7fc445d29e4ffba5",
        ),
        (
            r#"{"mode":"importance","proposal_scale":{"numerator":"3","denominator":"2"}}"#,
            "9ecc110542dc1ffca055b3436bdf31fee2f7e77f60d169b4eeeadafe2b5fa2f8",
        ),
    ];
    for (index, (text, hash)) in witnesses.into_iter().enumerate() {
        let raw: Box<serde_json::value::RawValue> = serde_json::from_str(text).unwrap();
        assert_eq!(
            canonical_raw_fingerprint(&raw, FingerprintOptions::default()).unwrap(),
            hash
        );
        // The second witness requires exact number tokens. The Value entry
        // fingerprints already-parsed numbers in serde_json's Display form.
        if index != 1 {
            let value = serde_json::from_str(text).unwrap();
            assert_eq!(
                canonical_fingerprint(&value, FingerprintOptions::default()).unwrap(),
                hash
            );
        }
    }
    let first: Value = serde_json::from_str(r#"{"z":{"b":1,"a":2},"a":[{"d":3,"c":4}]}"#).unwrap();
    let second: Value = serde_json::from_str(r#"{"a":[{"c":4,"d":3}],"z":{"a":2,"b":1}}"#).unwrap();
    let hash = |v: &Value| canonical_fingerprint(v, FingerprintOptions::default()).unwrap();
    assert_eq!(hash(&first), hash(&second));
    assert_ne!(hash(&json!(1)), hash(&json!(1.0))); // No mathematical number normalization.
    for (value, bytes, depth) in [
        (Value::Null, 4, 0),
        (json!([]), 2, 0),
        (json!([[]]), 4, 1),
        (json!({"a":["λ\n"]}), 14, 2),
    ] {
        let limits = FingerprintOptions {
            max_json_bytes: bytes,
            max_depth: depth,
        };
        assert_eq!(canonical_fingerprint(&value, limits).unwrap(), hash(&value));
        assert_eq!(
            canonical_fingerprint(
                &value,
                FingerprintOptions {
                    max_json_bytes: bytes - 1,
                    ..limits
                }
            ),
            Err(FingerprintError::ByteLimit { limit: bytes - 1 })
        );
        if depth > 0 {
            assert_eq!(
                canonical_fingerprint(
                    &value,
                    FingerprintOptions {
                        max_depth: depth - 1,
                        ..limits
                    }
                ),
                Err(FingerprintError::DepthLimit { limit: depth - 1 })
            );
        }
    }
}
#[test]
fn module_domains_source_snapshots_and_effective_widths_are_distinct() {
    let base = Environment::new("I 0", "{}", Path::new("/tmp"));
    let original = base.fingerprints();
    assert_eq!(
        original.circuit,
        "9e4af4e94a9839d9427f6cdba5ce9fb421d15443622cb089d520936cffe10920"
    );
    assert_ne!(original.ideal_state, original.recovery); // Null has domain separation.
    let same = Environment::new(
        "I 0",
        r#"{"outputs":[0]}"#,
        Path::new("/different/location"),
    );
    assert_eq!(same.fingerprints(), original);
    let mut relocated = Environment::new("I 0", "{}", Path::new("/tmp"));
    relocated.physical = parse_circuit(
        "I 0",
        &ParseOptions {
            source: Some("/another/host/physical.magna".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(relocated.fingerprints(), original);
    let comment = Environment::new("I 0\n# snapshot", "{}", Path::new("/tmp"));
    assert_ne!(comment.fingerprints().circuit, original.circuit);
    assert_eq!(comment.fingerprints().task, original.task);
    let files = Files::new();
    files.put("empty.magna", "");
    let one = Environment::new(
        "I 0",
        r#"{"ideal_state":"empty.magna","recovery":"empty.magna"}"#,
        &files.0,
    );
    let two = Environment::new(
        "I 1",
        r#"{"ideal_state":"empty.magna","recovery":"empty.magna"}"#,
        &files.0,
    );
    assert_eq!(
        one.modules.ideal_state().unwrap().circuit().to_text(),
        two.modules.ideal_state().unwrap().circuit().to_text()
    );
    assert_ne!(one.fingerprints().ideal_state, original.ideal_state);
    assert_ne!(one.fingerprints().recovery, original.recovery);
    assert_ne!(
        one.fingerprints().ideal_state,
        two.fingerprints().ideal_state
    );
    assert_ne!(one.fingerprints().recovery, two.fingerprints().recovery);
    let snapshot = one.fingerprints();
    files.put("empty.magna", "X 0");
    assert_eq!(one.fingerprints(), snapshot);
    let reloaded = Environment::new(
        "I 0",
        r#"{"ideal_state":"empty.magna","recovery":"empty.magna"}"#,
        &files.0,
    );
    assert_ne!(reloaded.fingerprints().ideal_state, snapshot.ideal_state);
    assert_ne!(reloaded.fingerprints().recovery, snapshot.recovery);
}
#[test]
fn ordered_task_and_projected_target_content_ignore_filesystem_locations() {
    let root = Files::new();
    let other = Files::new();
    for files in [&root, &other] {
        files.put("target.magna", "H 0\nCX 0 1");
        files.put("recovery.magna", "I 0");
    }
    let spec = r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}},"recovery":"recovery.magna"}"#;
    let original = Environment::new("I 1", spec, &root.0).fingerprints();
    assert_eq!(
        original,
        Environment::new("I 1", spec, &other.0).fingerprints()
    );
    for changed in [
        spec.replace("\"0\":0", "\"0\":1"),
        spec.replace(
            "[1],\"postselection\":{\"0\":0}",
            "[0],\"postselection\":{\"1\":0}",
        ),
    ] {
        let next = Environment::new("I 1", &changed, &root.0).fingerprints();
        assert_ne!(next.ideal_state, original.ideal_state);
        assert_eq!(next.circuit, original.circuit);
        assert_eq!(next.task, original.task);
    }
    let task = |text| {
        Environment::new(
            "I 128\nM 0 1\nOBSERVABLE_INCLUDE(3) rec[-1]\nOBSERVABLE_INCLUDE(5) rec[-2]",
            text,
            Path::new("/tmp"),
        )
        .fingerprints()
        .task
    };
    for (first, second) in [
        (r#"{"outputs":[0,128]}"#, r#"{"outputs":[128,0]}"#),
        (
            r#"{"sample_records":[0,1,0]}"#,
            r#"{"sample_records":[1,0,0]}"#,
        ),
        (
            r#"{"observables":[[1,0],[0,2]]}"#,
            r#"{"observables":[[0,2],[1,0]]}"#,
        ),
        (r#"{"observables":[[1,1]]}"#, r#"{"observables":[[1,0]]}"#),
        (
            r#"{"error_observables":[3,5]}"#,
            r#"{"error_observables":[5,3]}"#,
        ),
        (r#"{"error_observables":[]}"#, r#"{"outputs":[]}"#),
    ] {
        assert_ne!(task(first), task(second));
    }
    let high = rmagna::BigInt::from(1) << 128;
    assert_ne!(
        task(&format!(r#"{{"observables":[[{high},0]]}}"#)),
        task(r#"{"observables":[[1,0]]}"#)
    );
}
#[test]
fn every_resolved_execution_and_export_setting_changes_the_options_digest() {
    let baseline = || Environment::new("I 0", "{}", Path::new("/tmp"));
    let original = baseline().fingerprints();
    let mutations: &[fn(&mut Environment)] = &[
        |e| e.evaluation.runtime.history_sampler = HistorySampler::AffineSupport,
        |e| e.evaluation.runtime.strategy = CausalStrategy::ColdPrefix,
        |e| e.evaluation.runtime.max_history_queries += 1,
        |e| e.evaluation.runtime.max_history_inputs += 1,
        |e| e.evaluation.runtime.max_prefix_compiles += 1,
        |e| e.evaluation.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer,
        |e| e.evaluation.runtime.compile.probability_preparation = QueryPreparation::Trace,
        |e| {
            e.evaluation.runtime.compile.probability_preparation =
                QueryPreparation::FixedFibers(FiberOptions::default())
        },
        |e| e.evaluation.runtime.compile.max_prepared_probabilities += 1,
        |e| e.evaluation.runtime.compile.density_preparation = QueryPreparation::Trace,
        |e| {
            e.evaluation.runtime.compile.density_preparation =
                QueryPreparation::FixedFibers(FiberOptions::default())
        },
        |e| e.evaluation.runtime.compile.max_prepared_densities += 1,
        |e| e.evaluation.runtime.compile.max_coherent_qubits += 1,
        |e| e.evaluation.runtime.compile.max_gates += 1,
        |e| e.evaluation.runtime.compile.max_noise_sites += 1,
        |e| e.evaluation.runtime.noise.max_denominator_bits += 1,
        |e| e.evaluation.runtime.noise.max_entropy_words += 1,
        |e| e.evaluation.runtime.sampling.max_terms += 1,
        |e| e.evaluation.runtime.sampling.max_entropy_words += 1,
        |e| e.evaluation.scalars = ScalarArithmetic::new(12345).unwrap(),
        |e| e.evaluation.fields = FieldArithmetic::new(12345),
        |e| e.run.shots += 1,
        |e| e.run.seed += 1,
        |e| e.run.max_retained_record_values += 1,
        |e| e.run.statistics.max_dimensions += 1,
        |e| e.run.statistics.arithmetic = FieldArithmetic::new(12345),
        |e| e.numeric.max_work_bits += 1,
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let mut env = baseline();
        mutation(&mut env);
        let result = env.fingerprints();
        assert_ne!(result.options, original.options, "mutation {index}");
        assert_eq!(
            ContentFingerprints {
                options: original.options.clone(),
                ..result
            },
            original
        );
    }
    // Both fixed modes must include both nested search parameters.
    for density in [false, true] {
        let mut hashes = std::collections::BTreeSet::new();
        for fiber in [
            FiberOptions::default(),
            FiberOptions {
                max_search_rank: 17,
                ..Default::default()
            },
            FiberOptions {
                attempts: 255,
                ..Default::default()
            },
        ] {
            let mut env = baseline();
            if density {
                env.evaluation.runtime.compile.density_preparation =
                    QueryPreparation::FixedFibers(fiber);
            } else {
                env.evaluation.runtime.compile.probability_preparation =
                    QueryPreparation::FixedFibers(fiber);
            }
            assert!(hashes.insert(env.fingerprints().options));
        }
    }
}
#[test]
fn controllers_and_exact_proposals_are_declared_without_invoking_callbacks() {
    let baseline = || Environment::new("X_ERROR(1/10) 0", "{}", Path::new("/tmp"));
    let mut env = baseline();
    let unbound = env.fingerprints();
    env.registry
        .register("decoder", "v1", |_: &[bool]| -> Result<Vec<bool>, String> {
            panic!("fingerprinting must not invoke callbacks")
        })
        .unwrap();
    let declared = env.fingerprints();
    assert_ne!(declared.controllers, unbound.controllers);
    env.identities
        .insert("decoder".into(), json!({"manifest":{"z":2,"a":"λ"}}));
    let identity = env.fingerprints();
    assert_ne!(identity.controllers, declared.controllers);
    let mut other = baseline();
    other
        .registry
        .register("decoder", "v1", |_: &[bool]| Ok(vec![true]))
        .unwrap();
    other
        .identities
        .insert("decoder".into(), json!({"manifest":{"a":"λ","z":2}}));
    assert_eq!(identity, other.fingerprints()); // Closures are deliberately not attested.
    let mut version = baseline();
    version
        .registry
        .register("decoder", "v2", |_: &[bool]| Ok(vec![]))
        .unwrap();
    assert_ne!(version.fingerprints().controllers, declared.controllers);
    let manifest = identity.to_json();
    assert_eq!(
        manifest["controller_attestation"],
        "caller_declared_unverified"
    );
    assert_eq!(manifest["encoding"], "rmagna-provenance-v1");
    assert_eq!(manifest["algorithm"], "sha256");
    let mut proposals = std::collections::BTreeSet::new();
    proposals.insert(identity.sampling.clone());
    for scale in [
        "1",
        "3/2",
        "1/100000000000000000000000000000000000000000000000000",
    ] {
        env.sampling = NoiseSampling::Importance {
            proposal_scale: scale.parse().unwrap(),
        };
        let next = env.fingerprints();
        assert!(proposals.insert(next.sampling.clone()));
        assert_eq!(
            ContentFingerprints {
                sampling: identity.sampling.clone(),
                ..next
            },
            identity
        );
    }
    env.identities.insert("unknown".into(), Value::Null);
    assert_eq!(
        fingerprint_content(&env.inputs(), FingerprintOptions::default()),
        Err(FingerprintError::UnknownController {
            name: "unknown".into()
        })
    );
    env.identities.remove("unknown");
    for limits in [
        FingerprintOptions {
            max_json_bytes: 0,
            ..Default::default()
        },
        FingerprintOptions {
            max_depth: 0,
            ..Default::default()
        },
    ] {
        assert!(fingerprint_content(&env.inputs(), limits).is_err());
    }
    // Find the exact complete-set byte and depth boundaries independently of
    // which category is largest. Limits do not change a successful identity.
    let expected = env.fingerprints();
    let boundary = (1..4096)
        .find(|&n| {
            fingerprint_content(
                &env.inputs(),
                FingerprintOptions {
                    max_json_bytes: n,
                    ..Default::default()
                },
            )
            .is_ok()
        })
        .unwrap();
    assert_eq!(
        fingerprint_content(
            &env.inputs(),
            FingerprintOptions {
                max_json_bytes: boundary,
                ..Default::default()
            }
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        fingerprint_content(
            &env.inputs(),
            FingerprintOptions {
                max_json_bytes: boundary - 1,
                ..Default::default()
            }
        ),
        Err(FingerprintError::ByteLimit {
            limit: boundary - 1
        })
    );
    let depth = (1..16)
        .find(|&n| {
            fingerprint_content(
                &env.inputs(),
                FingerprintOptions {
                    max_depth: n,
                    ..Default::default()
                },
            )
            .is_ok()
        })
        .unwrap();
    assert_eq!(
        fingerprint_content(
            &env.inputs(),
            FingerprintOptions {
                max_depth: depth,
                ..Default::default()
            }
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        fingerprint_content(
            &env.inputs(),
            FingerprintOptions {
                max_depth: depth - 1,
                ..Default::default()
            }
        ),
        Err(FingerprintError::DepthLimit { limit: depth - 1 })
    );
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-provenance-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn raw_json_numbers_preserve_spelling_without_workspace_feature_changes() {
    // Independent literal canonical bytes and Python hashlib; no numeric cast.
    let cases = [
        (
            r##"123456789012345678901234567890"##,
            r##"123456789012345678901234567890"##,
            "ddc435295ffd70726362a3172eabfb4d8f1fba8684a1cd00a884a20e63dba35d",
        ),
        (
            r##"123456789012345678901234567891"##,
            r##"123456789012345678901234567891"##,
            "ec854ff481363da8f861108edc076baa1611ab6b90dc3d59262e9545a34149fc",
        ),
        (
            r##"1e400"##,
            r##"1e400"##,
            "1abf56ebf542ec6081eafd6c28015f2d784675e9cc0b856b669aacf5499e7cb0",
        ),
        (
            r##"-0"##,
            r##"-0"##,
            "f5fa17937f779329a3b97e6b4a6c927021afb023d2aa3e6e4d7cfb88577af271",
        ),
        (
            r##"1.0"##,
            r##"1.0"##,
            "12341dcbc8e90d56b7a9f29e7333c7ea552b706094388869159b038366a87e57",
        ),
        (
            r##"1e0"##,
            r##"1e0"##,
            "0fedbdfe9dafd77441c270de8fcc532ba51589c45144ccea54b0196745de47b8",
        ),
        (
            r##"{ "b": [1e400, -0], "a": 123456789012345678901234567890 }"##,
            r##"{"a":123456789012345678901234567890,"b":[1e400,-0]}"##,
            "361f1ff2a1a24011a02abff103db6941703fc56dada14c110e17b861d5dc56f5",
        ),
        (
            r##"{"$serde_json::private::Number":"1234"}"##,
            r##"{"$serde_json::private::Number":"1234"}"##,
            "a399c96a67b5db1f4100f7c680abc36bbb0ddb8c3d3a0046805ab2df45d78775",
        ),
    ];
    for (source, canonical, expected) in cases {
        let raw: Box<serde_json::value::RawValue> = serde_json::from_str(source).unwrap();
        let options = FingerprintOptions {
            max_json_bytes: canonical.len(),
            max_depth: 2,
        };
        assert_eq!(canonical_raw_fingerprint(&raw, options).unwrap(), expected);
        assert_eq!(
            canonical_raw_fingerprint(
                &raw,
                FingerprintOptions {
                    max_json_bytes: canonical.len() - 1,
                    ..options
                }
            ),
            Err(FingerprintError::ByteLimit {
                limit: canonical.len() - 1
            })
        );
    }
    let array: Box<serde_json::value::RawValue> = serde_json::from_str("[[]]").unwrap();
    assert!(
        canonical_raw_fingerprint(
            &array,
            FingerprintOptions {
                max_json_bytes: 4,
                max_depth: 1
            }
        )
        .is_ok()
    );
    assert_eq!(
        canonical_raw_fingerprint(
            &array,
            FingerprintOptions {
                max_json_bytes: 4,
                max_depth: 0
            }
        ),
        Err(FingerprintError::DepthLimit { limit: 0 })
    );
    let object: Box<serde_json::value::RawValue> =
        serde_json::from_str(r#"{"$serde_json::private::Number":"1234"}"#).unwrap();
    let number: Box<serde_json::value::RawValue> = serde_json::from_str("1234").unwrap();
    assert_ne!(
        canonical_raw_fingerprint(&object, Default::default()).unwrap(),
        canonical_raw_fingerprint(&number, Default::default()).unwrap()
    );
    let duplicate: Box<serde_json::value::RawValue> =
        serde_json::from_str(r#"{"a":0,"a":1}"#).unwrap();
    assert_eq!(
        canonical_raw_fingerprint(&duplicate, Default::default()).unwrap(),
        canonical_fingerprint(&json!({"a":1}), Default::default()).unwrap()
    );
    let text = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    let deep: Box<serde_json::value::RawValue> = serde_json::from_str(&text).unwrap();
    assert_eq!(
        canonical_raw_fingerprint(&deep, Default::default()),
        Err(FingerprintError::DepthLimit { limit: 64 })
    );
}
