use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn rstim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rstim"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn copy_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) {
    let dst = dst.as_ref();
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(src, dst).unwrap();
}

fn copy_gallery_inputs(temp_root: &Path) {
    let root = repo_root();
    for rel in [
        "qp101-viz/examples/basic.stim",
        "qp101-viz/examples/repeat-detector.stim",
        "qp101-viz/examples/atom-loss-sample.stim",
        "tools/build_qp101_gallery.py",
    ] {
        copy_file(root.join(rel), temp_root.join(rel));
    }
}

#[cfg(unix)]
fn create_failing_typst(bin_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(bin_dir).unwrap();
    let typst = bin_dir.join("typst");
    fs::write(
        &typst,
        "#!/bin/sh\nprintf 'typst should not be used by gallery build\\n' >&2\nexit 127\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&typst).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&typst, perms).unwrap();
}

#[test]
fn qp101_gallery_builds_without_typst() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = temp.path().join("repo");
    copy_gallery_inputs(&temp_root);
    let gallery_dir = temp_root.join("_site/gallery");
    let mask_bin = temp.path().join("mask-bin");
    create_failing_typst(&mask_bin);

    let mut command = Command::new("python3");
    command
        .arg(temp_root.join("tools/build_qp101_gallery.py"))
        .arg("--repo-root")
        .arg(&temp_root)
        .arg("--out-dir")
        .arg(&gallery_dir)
        .arg("--rstim-cmd")
        .arg(rstim_bin());

    let path = std::env::var_os("PATH").unwrap();
    let joined_path = std::env::join_paths(
        std::iter::once(mask_bin.as_path().to_path_buf())
            .chain(std::env::split_paths(&path)),
    )
    .unwrap();
    command.env("PATH", joined_path);

    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, markers) in [
        ("basic-site.svg", vec!["q0", "H", "M"]),
        ("repeat-detector-site.svg", vec!["repeat x2", "iter 2", "DETECTOR"]),
        ("atom-loss-sample.svg", vec!["LOSS", "DETECTOR", "data-style-preset=\"danger\""]),
    ] {
        let svg = fs::read_to_string(gallery_dir.join(name)).unwrap();
        assert!(svg.starts_with("<svg"), "{name} should start with <svg: {svg}");
        for marker in markers {
            assert!(svg.contains(marker), "{name} missing marker {marker}: {svg}");
        }
    }
}

#[test]
fn qp101_gallery_invalid_fixture_does_not_replace_existing_svg() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = temp.path().join("repo");
    copy_gallery_inputs(&temp_root);
    fs::write(temp_root.join("qp101-viz/examples/basic.stim"), "REPEAT nope {\nM 0\n}\n").unwrap();
    let gallery_dir = temp_root.join("_site/gallery");
    fs::create_dir_all(&gallery_dir).unwrap();
    let protected_svg = gallery_dir.join("basic-site.svg");
    fs::write(&protected_svg, "existing gallery output should remain").unwrap();

    let output = Command::new("python3")
        .arg(temp_root.join("tools/build_qp101_gallery.py"))
        .arg("--repo-root")
        .arg(&temp_root)
        .arg("--out-dir")
        .arg(&gallery_dir)
        .arg("--rstim-cmd")
        .arg(rstim_bin())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid fixture should fail gallery build"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bad repeat count") || stderr.contains("line 1"),
        "stderr should include the render_svg parse error: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(protected_svg).unwrap(),
        "existing gallery output should remain"
    );
}
