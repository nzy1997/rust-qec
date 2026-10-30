//! Complete original CircuitFileTests binding/config-isolation method from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
//! Rust owns a separate controllers fingerprint domain, not Python object ABI.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
use rmagna::runtime::{CausalProgram, ControllerRegistry, RuntimeOptions};
use rmagna::scalar::ScalarArithmetic;
use rmagna::task::TaskRunOptions;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-binding-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn bindings(&self, name: &str, version: &str, config: &Value) {
        let mut bindings = serde_json::Map::new();
        bindings.insert(
            name.to_owned(),
            json!({"import":"plugin:decode","version":version,"config":config}),
        );
        self.put(
            "controllers.json",
            &json!({"schema_version":1,"bindings":bindings}).to_string(),
        );
    }
    fn worker(&self, name: &str, version: &str, config: &Value, source: &str) -> ControllerProcess {
        self.put("plugin.py", source);
        self.bindings(name, version, config);
        let mut command = Command::new("python3");
        command.args(["-I", "-B", "-u"]);
        command.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/python/decoder_worker.py"
        ));
        command
            .arg("--controllers")
            .arg(self.0.join("controllers.json"));
        command.arg("--module-root").arg(&self.0);
        let worker = ControllerProcess::spawn(command, ProcessOptions::default()).unwrap();
        assert_eq!(worker.versions()[name], version);
        assert_eq!(worker.manifest()["native_backend"], "unavailable");
        worker
    }
    fn identity(&self, registry: &ControllerRegistry, worker: &ControllerProcess) -> Value {
        self.put("physical.magna", "CONTROL[decoder] -> ctrl[0]");
        let task = FileTask::load(
            &self.0.join("physical.magna"),
            None,
            registry,
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::from([("decoder".into(), worker.manifest().clone())]),
        )
        .unwrap();
        task.fingerprints(&TaskRunOptions {
            shots: 2,
            ..Default::default()
        })
        .unwrap()
        .to_json()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const MUTATING_CALLBACK: &str =
    "def decode(bits,c):\n result=c['a'][0]\n c['a'][0]=0\n return (result,)\n";

#[test]
fn original_bindings_isolate_nested_config_version_identity_and_named_return_errors() {
    let files = Files::new();
    let mut caller = json!({"a":[1]});
    let worker = files.worker("decoder", "r1", &caller, MUTATING_CALLBACK);
    let captured = worker.manifest().clone();
    // Capture has completed. Mutating both the caller value and its source file
    // cannot replace a persistent binding's original configuration.
    caller["a"][0] = json!(0);
    files.bindings("decoder", "r1", &caller);
    assert_eq!(caller["a"][0], 0);
    assert_eq!(worker.evaluate("decoder", &[]).unwrap(), [true]);
    assert_eq!(worker.evaluate("decoder", &[]).unwrap(), [true]);
    assert_eq!(worker.manifest(), &captured);

    let mut registry = ControllerRegistry::default();
    worker.register_all(&mut registry).unwrap();
    let names = registry.names();
    let failure = registry
        .register("decoder", "r2", |_: &[bool]| Ok(vec![false]))
        .unwrap_err()
        .to_string();
    assert!(failure.contains("already registered"), "{failure}");
    assert!(failure.contains("decoder"), "{failure}");
    assert_eq!(registry.names(), names);
    assert_eq!(registry.versions()["decoder"], "r1");

    let other = Files::new();
    let newer = other.worker("decoder", "r2", &json!({"a":[1]}), MUTATING_CALLBACK);
    let mut registry2 = ControllerRegistry::default();
    newer.register_all(&mut registry2).unwrap();
    assert_eq!(newer.versions()["decoder"], "r2");
    assert_eq!(
        captured["bindings"]["decoder"]["config_sha256"],
        newer.manifest()["bindings"]["decoder"]["config_sha256"]
    );
    assert_eq!(
        captured["module_sources"],
        newer.manifest()["module_sources"]
    );
    let r1 = files.identity(&registry, &worker);
    let r2 = other.identity(&registry2, &newer);
    // The independent Rust encoding binds versions/config in controllers.
    // Equal source circuits deliberately keep equal circuit-text identities.
    assert_eq!(r1["circuit"], r2["circuit"]);
    assert_ne!(r1["controllers"], r2["controllers"]);
    assert_eq!(r1["encoding"], "rmagna-provenance-v1");
    assert_eq!(r2["encoding"], r1["encoding"]);
    assert_eq!(r1["task"], r2["task"]);

    let arithmetic = ScalarArithmetic::default();
    let options = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit("CONTROL[decoder] -> ctrl[0]", &options).unwrap();
    let program =
        CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &arithmetic).unwrap();
    for seed in [0, 1] {
        let history = program
            .resolve(&[], &mut SplitMix64::new(seed), &arithmetic)
            .unwrap();
        assert_eq!(history.controller_calls().len(), 1);
        assert!(history.controller_calls()[0].inputs.is_empty());
        assert_eq!(history.controller_calls()[0].outputs, [true]);
    }

    for (returned, hint) in [
        ("(2,)", "non-bit"),
        ("(0.0,)", "integer bits"),
        ("()", "returned 0"),
        ("None", "iterable"),
    ] {
        let invalid = Files::new();
        let worker = invalid.worker(
            "broken",
            "1",
            &json!({}),
            &format!("def decode(bits,c):\n return {returned}\n"),
        );
        let mut registry = ControllerRegistry::default();
        worker.register_all(&mut registry).unwrap();
        let circuit = parse_circuit(
            "CONTROL[broken] -> ctrl[0]",
            &ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            },
        )
        .unwrap();
        let program =
            CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &arithmetic)
                .unwrap();
        let failure = match program.resolve(&[], &mut SplitMix64::new(0), &arithmetic) {
            Ok(_) => panic!("each original malformed callback must fail its one-output binding"),
            Err(error) => error.to_string(),
        };
        assert!(failure.contains("broken"), "{returned}: {failure}");
        assert!(failure.contains(hint), "{returned}: {failure}");
    }
}
