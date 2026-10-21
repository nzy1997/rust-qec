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
        self.bindings();
        ControllerProcess::spawn(self.host_command(), ProcessOptions::default()).unwrap()
    }
    fn bindings(&self) {
        self.write("controllers.json", &json!({"schema_version":1,"bindings":{"majority":{"import":"plugin:decode","version":"toy-v1","config":{"threshold":2}}}}).to_string());
    }
    fn host_command(&self) -> Command {
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
        command
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
            "integer zero/one bit",
        ),
        (
            "print(json.dumps({'id':0,'error':'toy failure'}),flush=True)",
            "toy failure",
        ),
        (
            "print(json.dumps({'id':0,'outputs':[],'extra':1}),flush=True)",
            "unknown field",
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

#[test]
fn wire_tokens_duplicates_and_nulls_reject_and_poison_independently_of_serde_features() {
    for response in [
        r#"{"id":{"$serde_json::private::Number":"0"},"outputs":[1]}"#,
        r#"{"id":0,"outputs":[{"$serde_json::private::Number":"1"}]}"#,
        r#"{"id":999,"id":0,"outputs":[1]}"#,
        r#"{"id":0,"outputs":[2],"outputs":[0]}"#,
        r#"{"id":0,"\u0069d":0,"outputs":[1]}"#,
        r#"{"id":0,"outputs":[1],"error":null}"#,
        r#"{"id":0,"outputs":null}"#,
        r#"{"id":0,"error":"first","error":"second"}"#,
        r#"{"id":true,"outputs":[1]}"#,
        r#"{"id":0.0,"outputs":[1]}"#,
        r#"{"id":0e0,"outputs":[1]}"#,
        r#"{"id":-0,"outputs":[1]}"#,
        r#"{"id":0,"outputs":[1.0]}"#,
        r#"{"id":0,"outputs":["1"]}"#,
        r#"{"id":0,"outputs":[-1]}"#,
    ] {
        let worker = fake(
            &format!("sys.stdin.readline()\nprint({response:?},flush=True)"),
            ProcessOptions::default(),
        );
        assert!(worker.evaluate("toy", &[]).is_err(), "{response}");
        assert!(
            worker
                .evaluate("toy", &[])
                .unwrap_err()
                .to_string()
                .contains("previously failed")
        );
    }
    let worker = fake(
        "sys.stdin.readline()\nprint('{\"id\":0,\"outputs\":[true,false,0,1]}',flush=True)",
        ProcessOptions::default(),
    );
    assert_eq!(
        worker.evaluate("toy", &[]).unwrap(),
        [true, false, false, true]
    );
}

#[test]
fn greeting_duplicates_binding_types_and_metadata_shapes_are_checked_before_registration() {
    let directory = Directory::new();
    for greeting in [
        r#"{"protocol":"wrong","protocol":"rmagna.controller.v1","bindings":{"toy":"1"}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{"toy":"1","toy":"2"}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{"toy":"1","\u0074oy":"2"}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{"toy":{"$serde_json::private::Number":"1"}}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{},"manifest":null,"manifest":{}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{},"manifest":{"source":"a","source":"b"}}"#,
        r#"{"protocol":"rmagna.controller.v1","bindings":{},"manifest":{"nested":[{"a":1,"a":2}]}}"#,
    ] {
        let command = directory.python(&format!("print({greeting:?},flush=True)"));
        assert!(
            ControllerProcess::spawn(command, ProcessOptions::default()).is_err(),
            "{greeting}"
        );
    }
    let greeting = r#"{"protocol":"rmagna.controller.v1","bindings":{"toy":"1"},"manifest":{"tagged":{"$serde_json::private::Number":"7"},"nested":[{"$serde_json::private::Number":"0"}],"maximum":18446744073709551615}}"#;
    let command = directory.python(&format!(
        "import time\nprint({greeting:?},flush=True)\ntime.sleep(5)"
    ));
    let worker = ControllerProcess::spawn(command, ProcessOptions::default()).unwrap();
    assert!(worker.manifest()["tagged"].is_object());
    assert_eq!(
        worker.manifest()["tagged"]["$serde_json::private::Number"],
        "7"
    );
    assert!(worker.manifest()["nested"][0].is_object());
    assert_eq!(worker.manifest()["maximum"], json!(u64::MAX));
    let nested = format!(
        "{{\"protocol\":\"rmagna.controller.v1\",\"bindings\":{{}},\"manifest\":{}null{}}}",
        "[".repeat(129),
        "]".repeat(129)
    );
    let command = directory.python(&format!("print({nested:?},flush=True)"));
    let error = ControllerProcess::spawn(command, ProcessOptions::default())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("limit"), "{error}");
}

#[cfg(unix)]
#[test]
fn inventoried_source_symlinks_cannot_execute_matching_poisoned_bytecode() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    let external = Directory::new();
    external.write("source.py", "def decode(bits,config):\n return (0,)\n");
    symlink(external.0.join("source.py"), directory.0.join("plugin.py")).unwrap();
    let poison = "import py_compile,pathlib,os\np=pathlib.Path('plugin.py'); stat=p.stat(); original=p.read_bytes()\np.write_bytes(original.replace(b'(0,)',b'(1,)'));os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))\npy_compile.compile(str(p),doraise=True)\np.write_bytes(original);os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))\n";
    assert!(directory.python(poison).status().unwrap().success());
    // Negative control: ordinary Python executes the stale matching cache.
    assert!(directory.python("import sys,os;sys.path.insert(0,os.getcwd());import plugin;assert plugin.decode((),{})==(1,)").status().unwrap().success());
    directory.bindings();
    let out = directory.host_command().output().unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("source symbolic links are unsupported"));
}

#[cfg(unix)]
#[test]
fn symlinked_package_components_and_new_lazy_source_links_fail_before_execution() {
    use std::os::unix::fs::symlink;
    for outside in [false, true] {
        let directory = Directory::new();
        let external = Directory::new();
        let target = if outside {
            external.0.join("actual")
        } else {
            directory.0.join("actual")
        };
        std::fs::create_dir(&target).unwrap();
        std::fs::write(
            target.join("__init__.py"),
            "def decode(bits,config): return [1]\n",
        )
        .unwrap();
        symlink(&target, directory.0.join("linked")).unwrap();
        directory.write("plugin.py", "from linked import decode\n");
        directory.bindings();
        let out = directory.host_command().output().unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("source symbolic links are unsupported")
        );
    }
    let directory = Directory::new();
    let external = Directory::new();
    directory.write("helper.py", "VALUE=0\n");
    external.write("helper.py", "VALUE=0\n");
    let worker =
        directory.host("def decode(bits,config):\n import helper\n return (helper.VALUE,)\n");
    std::fs::remove_file(directory.0.join("helper.py")).unwrap();
    symlink(external.0.join("helper.py"), directory.0.join("helper.py")).unwrap();
    external.write("helper.py", "VALUE=1\n");
    let failure = worker.evaluate("majority", &[]).unwrap_err().to_string();
    assert!(
        failure.contains("source symbolic links are unsupported"),
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
