use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::noise::SplitMix64;
use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
use rmagna::runtime::{CausalProgram, ControllerRegistry, RuntimeOptions};
use rmagna::scalar::ScalarArithmetic;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-controller-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }
    fn python(&self, script: &str) -> Command {
        let mut command = Command::new("python3");
        command.args(["-I", "-B", "-u", "-c", script]);
        command.current_dir(&self.0);
        command
    }
    fn host(&self, source: &str) -> ControllerProcess {
        self.write("plugin.py", source);
        self.host_existing()
    }
    fn host_existing(&self) -> ControllerProcess {
        self.write("controllers.json", &json!({"schema_version":1,"bindings":{"majority":{"import":"plugin:decode","version":"toy-v1","config":{"threshold":2}}}}).to_string());
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
        ControllerProcess::spawn(command, ProcessOptions::default()).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const HELLO: &str = "import sys,json\nprint(json.dumps({'protocol':'rmagna.controller.v1','bindings':{'toy':'1'}}),flush=True)\n";
fn fake(script: &str, options: ProcessOptions) -> ControllerProcess {
    let directory = Directory::new();
    ControllerProcess::spawn(directory.python(&format!("{HELLO}{script}")), options).unwrap()
}
#[test]
fn persistent_python_decoder_runs_real_causal_histories_without_native_backend() {
    let directory = Directory::new();
    let worker = directory.host(
        "def decode(bits,config):\n print('decoder diagnostic')\n threshold=config.pop('threshold')\n return (int(sum(bits)>=threshold),)\n",
    );
    assert_eq!(worker.versions()["majority"], "toy-v1");
    assert_eq!(worker.manifest()["native_backend"], "unavailable");
    assert_eq!(
        worker.manifest()["module_sources"]["plugin.py"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        worker.evaluate("majority", &[true, true, false]).unwrap(),
        [true]
    );
    assert_eq!(
        worker.evaluate("majority", &[false, true, false]).unwrap(),
        [false]
    );
    let mut registry = ControllerRegistry::default();
    worker.register_all(&mut registry).unwrap();
    let physical = parse_circuit("H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3", &ParseOptions {controllers:registry.names(), ..Default::default()}).unwrap();
    let a = ScalarArithmetic::default();
    let program = CausalProgram::new(&physical, &registry, RuntimeOptions::default(), &a).unwrap();
    let mut random = SplitMix64::new(78);
    let mut seen = [false; 2];
    for _ in 0..24 {
        let history = program.resolve(&[], &mut random, &a).unwrap();
        let call = &history.controller_calls()[0];
        let expected = call.inputs.iter().filter(|&&v| v).count() >= 2;
        assert_eq!(call.outputs, [expected]);
        seen[usize::from(expected)] = true;
        assert_eq!(*history.history_mass(), a.omega(0, -6).unwrap());
    }
    assert_eq!(seen, [true, true]);
}
#[test]
fn configuration_does_not_mutate_between_calls_and_native_import_fails_explicitly() {
    let directory = Directory::new();
    let worker = directory.host("def decode(bits,config):\n import magna\n return (0,)\n");
    let failure = worker.evaluate("majority", &[]).unwrap_err().to_string();
    assert!(
        failure.contains("cannot import a Magna quantum backend"),
        "{failure}"
    );
    assert!(
        worker
            .evaluate("majority", &[])
            .unwrap_err()
            .to_string()
            .contains("previously failed")
    );
}
#[test]
fn protocol_failures_and_callback_errors_poison_the_session_without_retry() {
    for (body, expected) in [
        ("print('not-json',flush=True)", "invalid JSON"),
        (
            "print(json.dumps({'id':77,'outputs':[0]}),flush=True)",
            "ID does not match",
        ),
        (
            "print(json.dumps({'id':0,'outputs':[2]}),flush=True)",
            "non-bit",
        ),
        (
            "print(json.dumps({'id':0,'outputs':[0.0]}),flush=True)",
            "non-bit",
        ),
        (
            "print(json.dumps({'id':0,'error':'toy failure'}),flush=True)",
            "toy failure",
        ),
        (
            "print(json.dumps({'id':0,'outputs':[],'extra':1}),flush=True)",
            "only id and outputs",
        ),
        ("sys.exit(3)", "closed stdout"),
    ] {
        let worker = fake(
            &format!("sys.stdin.readline()\n{body}"),
            ProcessOptions::default(),
        );
        let failure = worker.evaluate("toy", &[]).unwrap_err().to_string();
        assert!(failure.contains(expected), "{failure}");
        assert!(
            worker
                .evaluate("toy", &[])
                .unwrap_err()
                .to_string()
                .contains("previously failed")
        );
    }
}
#[test]
fn whole_exchange_timeouts_and_frame_limits_fail_closed() {
    let options = ProcessOptions {
        timeout: Duration::from_millis(500),
        max_frame_bytes: 256,
        ..Default::default()
    };
    let worker = fake("sys.stdin.readline()\nimport time\ntime.sleep(3)", options);
    assert!(
        worker
            .evaluate("toy", &[])
            .unwrap_err()
            .to_string()
            .contains("response unavailable")
    );
    assert!(worker.evaluate("toy", &[]).is_err());
    let worker = fake("sys.stdin.readline()\nprint('x'*300,flush=True)", options);
    assert!(
        worker
            .evaluate("toy", &[])
            .unwrap_err()
            .to_string()
            .contains("frame budget")
    );
    let directory = Directory::new();
    let missing = ControllerProcess::spawn(directory.python("pass"), options);
    assert!(missing.err().unwrap().to_string().contains("closed stdout"));
    let timeout = ControllerProcess::spawn(directory.python("import time; time.sleep(3)"), options);
    assert!(
        timeout
            .err()
            .unwrap()
            .to_string()
            .contains("response unavailable")
    );
}
#[test]
fn registration_is_atomic_and_input_rejection_does_not_touch_the_worker() {
    let worker = fake(
        "for line in sys.stdin:\n request=json.loads(line)\n print(json.dumps({'id':request['id'],'outputs':request['bits']}),flush=True)",
        ProcessOptions {
            max_bits: 2,
            ..Default::default()
        },
    );
    assert!(worker.evaluate("missing", &[]).is_err());
    assert!(worker.evaluate("toy", &[false; 3]).is_err());
    assert_eq!(
        worker.evaluate("toy", &[false, true]).unwrap(),
        [false, true]
    );
    let mut registry = ControllerRegistry::default();
    registry
        .register("toy", "existing", |_bits: &[bool]| Ok(vec![]))
        .unwrap();
    assert!(worker.register_all(&mut registry).is_err());
    assert_eq!(registry.versions()["toy"], "existing");
}
#[test]
fn worker_rejects_float_outputs_and_bad_shape_fails_the_draw() {
    let directory = Directory::new();
    let worker = directory.host("def decode(bits,config):\n return (0.0,)\n");
    assert!(
        worker
            .evaluate("majority", &[])
            .unwrap_err()
            .to_string()
            .contains("integer bits")
    );
    let directory = Directory::new();
    let worker = directory.host("def decode(bits,config):\n return ()\n");
    let mut registry = ControllerRegistry::default();
    worker.register_all(&mut registry).unwrap();
    let source = parse_circuit(
        "CONTROL[majority] -> ctrl[0]",
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let program = CausalProgram::new(
        &source,
        &registry,
        RuntimeOptions::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    assert!(
        program
            .resolve(&[], &mut SplitMix64::new(0), &ScalarArithmetic::default())
            .err()
            .unwrap()
            .to_string()
            .contains("returned 0")
    );
}

#[test]
fn decoder_source_executes_instead_of_a_matching_poisoned_bytecode_cache() {
    let directory = Directory::new();
    directory.write("plugin.py", "def decode(bits,config):\n return (0,)\n");
    let script = "import py_compile,pathlib,os\np=pathlib.Path('plugin.py'); stat=p.stat(); original=p.read_bytes()\np.write_bytes(original.replace(b'(0,)',b'(1,)')); os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))\npy_compile.compile(str(p),doraise=True)\np.write_bytes(original); os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))\n";
    assert!(directory.python(script).status().unwrap().success());
    // Prove the ordinary source loader accepts and executes the poisoned cache.
    assert!(directory.python("import sys,os;sys.path.insert(0,os.getcwd());import plugin;assert plugin.decode((),{})==(1,)").status().unwrap().success());
    let worker = directory.host_existing();
    assert_eq!(worker.evaluate("majority", &[]).unwrap(), [false]);
}
#[test]
fn changed_lazy_decoder_sources_are_rejected_before_execution() {
    let directory = Directory::new();
    directory.write("helper.py", "VALUE = 0\n");
    let worker =
        directory.host("def decode(bits,config):\n import helper\n return (helper.VALUE,)\n");
    directory.write("helper.py", "VALUE = 1\n");
    let failure = worker.evaluate("majority", &[]).unwrap_err().to_string();
    assert!(
        failure.contains("source changed before import"),
        "{failure}"
    );
}
#[test]
fn malformed_startup_and_excessive_output_counts_are_rejected() {
    let directory = Directory::new();
    for greeting in [
        "{'protocol':'another','bindings':{}}",
        "{'protocol':'rmagna.controller.v1','bindings':{'toy':''}}",
        "{'protocol':'rmagna.controller.v1','bindings':{'XOR':'1'}}",
        "{'protocol':'rmagna.controller.v1','bindings':{},'extra':1}",
    ] {
        assert!(
            ControllerProcess::spawn(
                directory.python(&format!(
                    "import json; print(json.dumps({greeting}),flush=True)"
                )),
                ProcessOptions::default()
            )
            .is_err()
        );
    }
    let worker = fake(
        "sys.stdin.readline()\nprint(json.dumps({'id':0,'outputs':[0,1,0]}),flush=True)",
        ProcessOptions {
            max_bits: 2,
            ..Default::default()
        },
    );
    assert!(
        worker
            .evaluate("toy", &[])
            .unwrap_err()
            .to_string()
            .contains("output bit budget")
    );
}
