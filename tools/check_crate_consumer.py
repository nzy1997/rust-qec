#!/usr/bin/env python3
"""Build and exercise the crates.io surface from freshly packaged sources."""

from __future__ import annotations

import argparse
import hashlib
import json
import selectors
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request
from pathlib import Path


class ConsumerCheckError(Exception):
    pass


def run(command: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        rendered = " ".join(command)
        detail = result.stderr.strip() or result.stdout.strip()
        raise ConsumerCheckError(f"command failed ({rendered}): {detail}")
    return result


def load_package_order(path: Path) -> list[str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ConsumerCheckError(f"cannot read package policy {path}: {error}") from error
    order = document.get("publish_order")
    if document.get("schema_version") != 1 or not isinstance(order, list) or not order:
        raise ConsumerCheckError("package policy must have schema_version 1 and a non-empty publish_order")
    if any(not isinstance(name, str) or not name for name in order) or len(order) != len(set(order)):
        raise ConsumerCheckError("publish_order must contain unique non-empty package names")
    return order


def workspace_packages(repo_root: Path) -> dict[str, dict[str, object]]:
    result = run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=repo_root
    )
    document = json.loads(result.stdout)
    return {package["name"]: package for package in document["packages"]}


def snapshot_workspace(repo_root: Path, destination: Path) -> Path:
    repo_root = repo_root.resolve()
    metadata = workspace_packages(repo_root)
    destination.mkdir(parents=True)
    for filename in ("Cargo.toml", "Cargo.lock", "LICENSE"):
        source = repo_root / filename
        if not source.is_file():
            raise ConsumerCheckError(f"workspace snapshot requires {source}")
        shutil.copy2(source, destination / filename)

    def ignored(_directory: str, names: list[str]) -> set[str]:
        blocked = {"target", ".git", ".worktrees", "__pycache__", ".pytest_cache"}
        return set(names) & blocked

    for package in metadata.values():
        source = Path(package["manifest_path"]).resolve().parent
        relative = source.relative_to(repo_root)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target, ignore=ignored)
    return destination


def package_and_unpack(
    repo_root: Path, target_dir: Path, package_order: list[str], destination: Path
) -> dict[str, Path]:
    metadata = workspace_packages(repo_root)
    unpacked: dict[str, Path] = {}
    package_dir = target_dir / "package"
    for name in package_order:
        package = metadata.get(name)
        if package is None:
            raise ConsumerCheckError(f"package policy names unknown workspace package {name!r}")
        patch_args = [
            item
            for patched_name, source in unpacked.items()
            for item in (
                "--config",
                f"patch.crates-io.{json.dumps(patched_name)}.path={json.dumps(str(source))}",
            )
        ]
        run(
            [
                "cargo",
                "package",
                "-p",
                name,
                "--no-verify",
                "--allow-dirty",
                "--target-dir",
                str(target_dir),
                *patch_args,
            ],
            cwd=repo_root,
        )
        version = package["version"]
        archive = package_dir / f"{name}-{version}.crate"
        if not archive.is_file():
            raise ConsumerCheckError(f"cargo package did not create {archive}")
        root = destination / f"{name}-{version}"
        extract_crate(archive, destination)
        if not (root / "Cargo.toml").is_file():
            raise ConsumerCheckError(f"{archive.name} lacks its normalized Cargo.toml")
        unpacked[name] = root
    return unpacked


def extract_crate(archive: Path, destination: Path) -> None:
    with tarfile.open(archive, "r:gz") as bundle:
        for member in bundle.getmembers():
            member_path = Path(member.name)
            if member_path.is_absolute() or ".." in member_path.parts:
                raise ConsumerCheckError(f"unsafe archive member in {archive.name}: {member.name}")
        bundle.extractall(destination, filter="data")


def validate_package_contents(unpacked: dict[str, Path], repo_root: Path) -> None:
    repo_text = str(repo_root.resolve())
    for name, root in unpacked.items():
        manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
        if repo_text in manifest:
            raise ConsumerCheckError(f"normalized {name} manifest retains source repository path")

    rstim = unpacked.get("rstim")
    if rstim is None:
        raise ConsumerCheckError("publish order must include rstim")
    asset_manifest = rstim / "assets/shot-viewer/asset-manifest.json"
    if not asset_manifest.is_file():
        raise ConsumerCheckError("packaged rstim is missing the shot-viewer asset manifest")
    try:
        assets = json.loads(asset_manifest.read_text(encoding="utf-8"))["files"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise ConsumerCheckError("packaged shot-viewer asset manifest is invalid") from error
    for relative, record in assets.items():
        asset = asset_manifest.parent / relative
        if not asset.is_file():
            raise ConsumerCheckError(f"packaged rstim is missing viewer asset {relative}")
        observed = hashlib.sha256(asset.read_bytes()).hexdigest()
        if not isinstance(record, dict) or observed != record.get("sha256"):
            raise ConsumerCheckError(f"packaged viewer asset checksum differs for {relative}")


def append_patches(manifest: Path, unpacked: dict[str, Path]) -> None:
    lines = ["", "[patch.crates-io]"]
    for name, source in unpacked.items():
        lines.append(f"{json.dumps(name)} = {{ path = {json.dumps(str(source))} }}")
    with manifest.open("a", encoding="utf-8") as stream:
        stream.write("\n".join(lines) + "\n")


def validate_resolved_sources(manifest: Path, unpacked: dict[str, Path], features: str | None = None) -> None:
    result = run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            str(manifest),
            *(["--features", features] if features else []),
        ],
        cwd=manifest.parent,
    )
    packages = {package["name"]: package for package in json.loads(result.stdout)["packages"]}
    for name, expected_root in unpacked.items():
        package = packages.get(name)
        if package is None:
            continue
        observed = Path(package["manifest_path"]).resolve().parent
        if observed != expected_root.resolve():
            raise ConsumerCheckError(
                f"{name} resolved outside unpacked package: {observed}"
            )


def exercise_consumer(repo_root: Path, target_dir: Path, unpacked: dict[str, Path], work: Path) -> None:
    source = repo_root / "examples/rust-consumer"
    consumer = work / "consumer"
    shutil.copytree(source, consumer)
    lockfile = consumer / "Cargo.lock"
    lockfile.unlink(missing_ok=True)
    append_patches(consumer / "Cargo.toml", unpacked)
    validate_resolved_sources(consumer / "Cargo.toml", unpacked)
    validate_minimal_graph(consumer / "Cargo.toml", {"clap", "qec-code", "qec-ilp-core", "highs", "highs-sys"})
    result = run(
        [
            "cargo",
            "run",
            "--manifest-path",
            str(consumer / "Cargo.toml"),
            "--target-dir",
            str(target_dir),
            "-j",
            "4",
        ],
        cwd=consumer,
    )
    expected = "validated Bell parity and 128/128 observable predictions"
    if result.stdout.strip() != expected:
        raise ConsumerCheckError(f"consumer result differs: {result.stdout.strip()!r}")


def validate_minimal_graph(manifest: Path, forbidden: set[str]) -> None:
    result = run(["cargo", "tree", "--manifest-path", str(manifest), "--edges", "normal,build",
                  "--prefix", "none"], cwd=manifest.parent)
    found = {line.split()[0] for line in result.stdout.splitlines() if line.split()} & forbidden
    if found:
        raise ConsumerCheckError(f"minimal consumer unexpectedly builds {sorted(found)}")


def exercise_model_consumer(target_dir: Path, unpacked: dict[str, Path], work: Path) -> None:
    consumer = work / "model-consumer"
    (consumer / "src").mkdir(parents=True)
    manifest = consumer / "Cargo.toml"
    dependencies = {}
    for name in ("qec-code", "qec-ilp-core"):
        dependencies[name] = tomllib.loads((unpacked[name] / "Cargo.toml").read_text())["package"]["version"]
    manifest.write_text('[workspace]\n[package]\nname="minimal-model-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\n'
                        + "\n".join(f'{name} = "{version}"' for name, version in dependencies.items()) + "\n")
    append_patches(manifest, unpacked)
    (consumer / "src/main.rs").write_text('''fn main() {
    assert_eq!(qec_code::binary::try_binary_rank(&[vec![1, 0], vec![0, 1]]).unwrap(), 2);
    let mut model = qec_ilp_core::BinaryIlpModel {
        binary_vars: vec![], integer_vars: vec![], constraints: vec![], solution_binary_prefix_len: 0,
    };
    model.validate().unwrap();
    model.solution_binary_prefix_len = 1;
    assert!(model.validate().is_err());
    println!("validated model-only libraries");
}
''')
    validate_resolved_sources(manifest, unpacked)
    validate_minimal_graph(manifest, {"clap", "highs", "highs-sys", "gurobi", "bindgen", "cmake"})
    result = run(["cargo", "run", "--locked", "--manifest-path", str(manifest), "--target-dir", str(target_dir), "-j", "4"], cwd=consumer)
    if result.stdout.strip() != "validated model-only libraries":
        raise ConsumerCheckError("model-only consumer did not validate its result")


def exercise_viewer(binary: Path, expected_html: bytes) -> None:
    with subprocess.Popen([str(binary), "shot_viewer", "--no_open", "--serve-once"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as process:
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                if not selector.select(timeout=10):
                    raise ConsumerCheckError("installed viewer did not start within 10 seconds")
            line = process.stdout.readline().strip()
            prefix = "rstim shot viewer: http://127.0.0.1:"
            if not line.startswith(prefix):
                raise ConsumerCheckError(f"installed viewer did not announce loopback URL: {line!r}")
            with urllib.request.urlopen(line.removeprefix("rstim shot viewer: "), timeout=10) as response:
                if response.read() != expected_html:
                    raise ConsumerCheckError("installed viewer served different package HTML")
            _, stderr = process.communicate(timeout=10)
            if process.returncode:
                raise ConsumerCheckError(f"installed viewer failed: {stderr}")
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


def exercise_compatibility_cli(target_dir: Path, unpacked: dict[str, Path], work: Path) -> None:
    rstim = unpacked["rstim"]
    append_patches(rstim / "Cargo.toml", unpacked)
    features = "cli,codegen-css,shot-viewer"
    validate_resolved_sources(rstim / "Cargo.toml", unpacked, features=features)
    install_root = work / "compatibility-install"
    run(["cargo", "install", "--locked", "--path", str(rstim), "--features", features,
         "--root", str(install_root), "--target-dir", str(target_dir), "-j", "4"], cwd=work)
    bins = sorted(path.name for path in (install_root / "bin").iterdir() if path.is_file())
    if bins != ["rstim"]:
        raise ConsumerCheckError(f"compatibility install exposed unexpected binaries: {bins}")
    exercise_viewer(install_root / "bin/rstim", (rstim / "assets/shot-viewer/index.html").read_bytes())


def exercise_installed_cli(target_dir: Path, unpacked: dict[str, Path], work: Path) -> None:
    cli = unpacked.get("rustqec-cli")
    if cli is None:
        raise ConsumerCheckError("publish order must include rustqec-cli")
    append_patches(cli / "Cargo.toml", unpacked)
    validate_resolved_sources(cli / "Cargo.toml", unpacked)
    install_root = work / "install"
    run(
        [
            "cargo",
            "install",
            "--path",
            str(cli),
            "--root",
            str(install_root),
            "--locked",
            "--target-dir",
            str(target_dir),
            "-j",
            "4",
        ],
        cwd=work,
    )
    bin_dir = install_root / "bin"
    installed = sorted(path.name for path in bin_dir.iterdir() if path.is_file())
    if installed != ["rustqec"]:
        raise ConsumerCheckError(f"installed binary set differs: expected ['rustqec'], got {installed}")

    repo_root = Path(__file__).resolve().parents[1]
    if str(repo_root) not in sys.path:
        sys.path.insert(0, str(repo_root))
    from tools.check_installed_quickstart import QuickstartError, validate

    try:
        validate(bin_dir)
    except (QuickstartError, OSError) as error:
        raise ConsumerCheckError(f"installed rustqec quickstart failed: {error}") from error

    # Validate the remaining published dependency through the opt-in solver path.
    # The installed binary above intentionally remains the default lightweight build.
    validate_resolved_sources(cli / "Cargo.toml", unpacked, features="ilp")
    run(["cargo", "check", "--locked", "--manifest-path", str(cli / "Cargo.toml"),
         "--features", "ilp", "--target-dir", str(target_dir), "-j", "4"], cwd=work)


def check(repo_root: Path, target_dir: Path, policy: Path) -> None:
    order = load_package_order(policy)
    with tempfile.TemporaryDirectory(prefix="crate-consumer-") as temporary:
        work = Path(temporary)
        snapshot = snapshot_workspace(repo_root, work / "source-workspace")
        unpacked = package_and_unpack(snapshot, target_dir, order, work / "packages")
        validate_package_contents(unpacked, repo_root)
        # A pure library consumer must compile even without viewer files on disk.
        assets = unpacked["rstim"] / "assets/shot-viewer"
        backup = work / "viewer-assets"
        shutil.move(assets, backup)
        try:
            exercise_consumer(repo_root, target_dir, unpacked, work)
        finally:
            shutil.move(backup, assets)
        exercise_model_consumer(target_dir, unpacked, work)
        exercise_installed_cli(target_dir, unpacked, work)
        exercise_compatibility_cli(target_dir, unpacked, work)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--package-policy", type=Path)
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    target_dir = (args.target_dir or repo_root / "target").resolve()
    policy = (args.package_policy or repo_root / "tools/crates_io_packages.json").resolve()
    try:
        check(repo_root, target_dir, policy)
    except (ConsumerCheckError, OSError, KeyError, json.JSONDecodeError) as error:
        print(f"FAIL crate consumer: {error}", file=sys.stderr)
        return 1
    print("PASS crate consumer from packaged sources")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
