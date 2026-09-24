"""Git input inventory for the retained operating-envelope resource campaign."""

from pathlib import Path
import subprocess


PATHS = (
    # Include every Python helper, current and future, without pulling the
    # generated readiness/resources bundle into its own source inventory.
    "benchmarks/atom_loss",
    "benchmarks/atom_loss/readiness_resources.py",
    "benchmarks/atom_loss/retained_source.py",
    "benchmarks/atom_loss/chain_reference.py",
    "benchmarks/atom_loss/decoder_reference.py",
    "benchmarks/atom_loss/run.py",
    "benchmarks/atom_loss/reference.py",
    "benchmarks/atom_loss/artifacts.py",
    "benchmarks/atom_loss/correctness.py",
    "benchmarks/atom_loss/shot_data.py",
    "benchmarks/atom_loss/fixtures",
    "benchmarks/atom_loss/requirements.txt",
    "rustqec-cli/tests/fixtures/current_rstim_atom_loss",
    "docs/envelope-support.json",
    "Cargo.toml",
    "Cargo.lock",
    ".cargo",
    "rustqec-cli/src", "rustqec-cli/Cargo.toml",
    "renvelope/src", "renvelope/Cargo.toml",
    "rmatching/src", "rmatching/Cargo.toml",
    "qec-ilp-core/src", "qec-ilp-core/Cargo.toml",
    "rstim/src", "rstim/Cargo.toml",
)


def inventory(repo: Path, revision: str) -> dict[str, dict[str, str]]:
    entries = {}
    raw = subprocess.check_output(
        ["git", "-C", str(repo), "ls-tree", "-rz", "--full-tree", revision, "--", *PATHS]
    )
    for item in raw.split(b"\0"):
        if not item:
            continue
        header, name = item.split(b"\t", 1)
        mode, kind, oid = header.decode().split()
        path = name.decode()
        if path.startswith("benchmarks/atom_loss/") and not (
            path.endswith(".py")
            or path == "benchmarks/atom_loss/requirements.txt"
            or path.startswith("benchmarks/atom_loss/fixtures/")
        ):
            continue
        if kind != "blob" or mode not in {"100644", "100755"}:
            raise ValueError("Unsupported retained source input: " + path)
        entries[path] = {"mode": mode, "git_blob": oid}
    if not entries:
        raise ValueError("Missing retained source inputs")
    return entries


def clean_inventory(repo: Path, revision: str) -> dict[str, dict[str, str]]:
    """Require a clean, stable measurement checkout before retaining results."""
    from .source_contract import verify_local_cargo_config

    verify_local_cargo_config(repo)
    if subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip() != revision:
        raise ValueError("Measurement source revision changed during campaign")
    # Exclude generated output while checking all code, fixtures and build
    # inputs, including newly added or untracked helpers.
    status = subprocess.check_output([
        "git", "-C", str(repo), "status", "--porcelain", "--untracked-files=all",
        "--", *PATHS, ":(exclude)benchmarks/atom_loss/readiness/resources",
    ])
    if status.strip():
        raise ValueError("Measurement source checkout is dirty")
    return inventory(repo, revision)
