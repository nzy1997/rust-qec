"""Current-tree counterpart of rmagna/build.rs's embedded paper probe digest."""
import hashlib
from pathlib import Path

# Keep this explicit input scope equal to the build script. The verifier itself
# is not a compiled input; tests assert the two manifests agree on actual bytes.
INPUTS = (
    "Cargo.toml", "Cargo.lock", ".cargo", "rmagna/Cargo.toml", "rmagna/build.rs",
    "rmagna/src", "rmagna/examples/paper_proposal_probe.rs",
    "rmagna/examples/evaluate_task.rs",
    "rmagna/examples/paper_bh_run.rs",
    "rmagna/examples/paper_cultivation_run.rs",
    "rmagna/examples/support",
    "benchmarks/magna_reproduction/paper_query_work.py",
    "rmagna/tests/support/paper_proposal_probe.rs",
    "rmagna/tests/data/paper-proposal-runtime.json",
    "benchmarks/magna_reproduction/paper_proposals.py",
    "benchmarks/magna_reproduction/paper_proposal_draws.py",
    "benchmarks/magna_reproduction/paper_proposal_runtime_vectors.py",
)


def source_inputs(root):
    root = Path(root).resolve(strict=True)
    files = []
    def collect(path):
        if path.is_symlink():
            raise ValueError("probe inputs must not be symlinks")
        if path.is_dir():
            for child in path.iterdir():
                collect(child)
        elif path.is_file():
            files.append(path)
        elif path.exists():
            raise ValueError("probe input must be a regular file")
        else:
            raise ValueError("required probe input is missing")
    for name in INPUTS:
        path = root/name
        if name == ".cargo" and not path.exists() and not path.is_symlink():
            continue
        collect(path)
    return {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(files, key=lambda p: p.relative_to(root).as_posix())}


def source_identity(root):
    digest = hashlib.sha256()
    for name, sha256 in source_inputs(root).items():
        digest.update(name.encode()+b"\0"+sha256.encode()+b"\n")
    return digest.hexdigest()
