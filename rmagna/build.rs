// Bind the reusable paper probe to the source and build files read by Cargo.
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn collect(root: &Path, path: &Path, files: &mut Vec<String>) {
    if !path.exists() {
        return;
    }
    let metadata = fs::symlink_metadata(path).expect("read probe input metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "probe inputs must not be symlinks"
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(path).expect("read probe input directory") {
            collect(root, &entry.expect("read probe input entry").path(), files);
        }
    } else {
        assert!(metadata.is_file(), "probe input must be a regular file");
        files.push(
            path.strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/"),
        );
    }
}

fn main() {
    let package = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&package).parent().unwrap();
    let inputs = [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo",
        "rmagna/Cargo.toml",
        "rmagna/build.rs",
        "rmagna/src",
        "rmagna/examples/paper_proposal_probe.rs",
        "rmagna/examples/evaluate_task.rs",
        "benchmarks/magna_reproduction/paper_query_work.py",
        "rmagna/tests/support/paper_proposal_probe.rs",
        "rmagna/tests/data/paper-proposal-runtime.json",
        "benchmarks/magna_reproduction/paper_proposals.py",
        "benchmarks/magna_reproduction/paper_proposal_draws.py",
        "benchmarks/magna_reproduction/paper_proposal_runtime_vectors.py",
    ];
    let mut files = Vec::new();
    for name in inputs {
        let path = root.join(name);
        // Directory tracking catches newly added implementation/config files.
        println!("cargo:rerun-if-changed={}", path.display());
        assert!(
            name == ".cargo" || path.exists(),
            "required probe input is missing"
        );
        collect(root, &path, &mut files);
    }
    files.sort();
    let mut tree = Sha256::new();
    for name in files {
        tree.update(name.as_bytes());
        tree.update([0]);
        tree.update(format!("{:x}", Sha256::digest(fs::read(root.join(name)).unwrap())).as_bytes());
        tree.update(b"\n");
    }
    println!(
        "cargo:rustc-env=RMAGNA_PAPER_SOURCE_SHA256={:x}",
        tree.finalize()
    );
}
