#![cfg(all(feature = "unified-cli", not(feature = "qasm")))]

use std::io::Cursor;

#[test]
fn disabled_conversion_is_absent_from_capabilities_and_has_structured_errors() {
    let mut stdout = Vec::new();
    rstim::unified_cli::run(
        ["rstim", "capabilities", "--format", "json"],
        &mut Cursor::new(""),
        &mut stdout,
    )
    .unwrap();
    let capabilities: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    let commands = capabilities["commands"].as_array().unwrap();
    assert!(
        commands
            .iter()
            .all(|command| !command["name"].as_str().unwrap().contains("qasm"))
    );
    let sample = commands
        .iter()
        .find(|command| command["name"] == "circuit.sample")
        .unwrap();
    assert!(
        sample["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|arg| arg["flag"] != "--in-format")
    );

    for (verb, command) in [
        ("import-qasm", "circuit.import-qasm"),
        ("export-qasm", "circuit.export-qasm"),
    ] {
        stdout.clear();
        let error = rstim::unified_cli::run(
            ["rstim", "circuit", verb, "--out", "unused.qasm"],
            &mut Cursor::new(""),
            &mut stdout,
        )
        .unwrap_err();
        assert!(stdout.is_empty());
        assert_eq!(rstim::unified_cli::exit_code(&error), 2);
        let mut stderr = Vec::new();
        rstim::unified_cli::write_error(&error, &mut stdout, &mut stderr);
        let value: serde_json::Value = serde_json::from_slice(&stderr).unwrap();
        assert_eq!(value["command"], command);
        assert_eq!(value["status"], "error");
        assert_eq!(value["error"]["code"], "invalid_arguments");
    }
}
