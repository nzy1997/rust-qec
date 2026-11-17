use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
use std::{error::Error, path::Path, process::Command};

pub(crate) fn worker(path: &Path, python: &str) -> Result<ControllerProcess, Box<dyn Error>> {
    let root = path
        .parent()
        .ok_or("controller file is missing a module root")?;
    let mut command = Command::new(python);
    command.args([
        "-I",
        "-B",
        "-u",
        concat!(env!("CARGO_MANIFEST_DIR"), "/python/decoder_worker.py"),
    ]);
    command
        .arg("--controllers")
        .arg(path)
        .arg("--module-root")
        .arg(root);
    Ok(ControllerProcess::spawn(
        command,
        ProcessOptions::default(),
    )?)
}
