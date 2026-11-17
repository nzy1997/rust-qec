#!/usr/bin/env python3
"""Verify and regenerate a sealed Magna paper evidence bundle offline."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tarfile
from tempfile import TemporaryDirectory

from remaining_contracts import read_json, reference, require


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SCHEMA = "rmagna.paper-evidence.v1"
INDEX_SCHEMA = "rmagna.paper-evidence-index.v1"
CAMPAIGNS = {
    "bh": "bh.json",
    "cultivation": "cultivation.json",
    "decoder-workloads": "decoder_workloads.json",
}
SOURCE_SCOPE = (
    "Cargo.toml", "Cargo.lock", ".cargo", "rmagna/Cargo.toml", "rmagna/build.rs",
    "rmagna/src", "rmagna/examples", "rmagna/python", "rmagna/tests/data",
    "rmagna/tests/support", "benchmarks/magna_reproduction",
)


def file_record(path):
    digest = hashlib.sha256()
    count = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
            count += len(chunk)
    return {"sha256": digest.hexdigest(), "bytes": count}


def artifact_inventory(root):
    require(root.is_dir() and not root.is_symlink(), "missing or symlinked evidence raw directory")
    required = {*CAMPAIGNS, "provenance"}
    require({path.name for path in root.iterdir()} == required, "evidence raw campaign inventory differs")
    files = {}

    def fail(error):
        raise error

    for directory, dirs, names in os.walk(root, onerror=fail, followlinks=False):
        directory = Path(directory)
        for name in dirs:
            require(not (directory/name).is_symlink(), f"symlink in evidence inventory: {name}")
        for name in names:
            path = directory/name
            relative = path.relative_to(root).as_posix()
            require(stat.S_ISREG(path.lstat().st_mode), f"special file in evidence inventory: {relative}")
            files[relative] = file_record(path)
    for campaign in required:
        require(any(name.startswith(campaign+"/") for name in files), f"empty evidence scope: {campaign}")
    return dict(schema=INDEX_SCHEMA, files=files)


def verify_inventory(raw, index):
    actual = artifact_inventory(raw)
    require(index["schema"] == INDEX_SCHEMA and index.keys() == actual.keys(), "unsupported evidence index")
    expected_files = index["files"]
    found_files = actual["files"]
    missing = sorted(expected_files.keys() - found_files.keys())
    extra = sorted(found_files.keys() - expected_files.keys())
    require(not missing and not extra, f"evidence raw inventory differs, missing {missing[:5]}, extra {extra[:5]}")
    for path, expected in expected_files.items():
        require(type(expected["bytes"]) is int and found_files[path] == expected, f"evidence raw bytes changed: {path}")
    return len(found_files)


def validate_manifest(plan, *, finalized=True):
    pinned = reference.load_manifest()
    require(plan["schema"] == SCHEMA
            and plan["reference_sha256"] == reference.digest(HERE/"reference.json")
            and plan["paper"] == pinned["paper"]["arxiv"]
            and plan["archive_sha256"] == pinned["paper"]["archive_sha256"]
            and plan["historical_kernel"] == pinned["paper"]["kernel_commit"]
            and plan["upstream_commit"] == pinned["upstream"]["commit"],
            "evidence paper or source identity differs")
    require(plan["captures"].keys() == CAMPAIGNS.keys(), "evidence plan omits a full campaign")
    require(re.fullmatch(r"[0-9a-f]{64}", plan["ablation_source_sha256"]) is not None,
            "invalid ablation source hash")
    if finalized:
        require(re.fullmatch(r"[0-9a-f]{64}", plan["index_sha256"]) is not None, "invalid finalized index hash")


def git(repo, *args):
    env = dict(os.environ, GIT_NO_LAZY_FETCH="1", GIT_ALLOW_PROTOCOL="")
    result = subprocess.run(["git", "--no-replace-objects", *args], cwd=repo, env=env, capture_output=True)
    require(result.returncode == 0,
            f"local Git source unavailable ({args[0]}): {result.stderr.decode(errors='replace').strip()}")
    return result.stdout


def check_campaign(repo, name, pin, path):
    commit, tree = pin["commit"], pin["tree"]
    require(all(re.fullmatch(r"[0-9a-f]{40}", value) for value in (commit, tree)),
            f"invalid evidence commit or tree for {name}")
    require(git(repo, "rev-parse", "--verify", commit+"^{commit}").decode().strip() == commit
            and git(repo, "rev-parse", "--verify", commit+"^{tree}").decode().strip() == tree,
            f"evidence verifier tree differs for {name}")
    tracked = set(git(repo, "ls-tree", "-rz", "--name-only", commit, "--", *SOURCE_SCOPE).decode().split("\0"))
    scope = [part for part in SOURCE_SCOPE if part in tracked or any(x.startswith(part+"/") for x in tracked)]
    archive = git(repo, "archive", "--format=tar", commit, *scope)
    with TemporaryDirectory(prefix="magna-paper-source-") as temporary:
        snapshot = Path(temporary)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as source:
            source.extractall(snapshot, filter="data")
        here = snapshot/"benchmarks/magna_reproduction"
        manifest = reference.relative_file(here, "paper/"+CAMPAIGNS[name])
        require(file_record(manifest)["sha256"] == pin["plan_sha256"], f"evidence plan bytes changed for {name}")
        worker = subprocess.run(
            [sys.executable, "-I", "-S", "-B", str(HERE/"paper_evidence_snapshot.py"), str(here), name,
             str(manifest), str(path)], cwd=snapshot, capture_output=True, text=True, timeout=1800)
        require(worker.returncode == 0, f"{name} offline verification failed: {worker.stderr.strip()}")
    return json.loads(worker.stdout)


def canonical_json(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False)+"\n").encode()


def checked_path(root, name):
    path = reference.relative_file(root, name)
    require(path.is_file(), f"missing evidence file: {name}")
    return path


def generate(plan, raw, artifact_count):
    from paper_evidence_ablations import check_capture as check_ablations
    import paper_evidence_deposit as deposit
    from paper_evidence_report import csv_table, report

    checked = {name: check_campaign(ROOT, name, plan["captures"][name], reference.relative_file(raw, name))
               for name in CAMPAIGNS}
    original = deposit.check(reference.relative_file(raw, "provenance/deposit"), reference.load_manifest(), checked["bh"])
    ablations = check_ablations(read_json(checked_path(raw, "provenance/ablations.json")))
    require(ablations["rust_source_sha256"] == plan["ablation_source_sha256"], "ablation source differs from the sealed plan")
    output = report(plan, checked, artifact_count, original, ablations)
    return output, {
        "regenerated.json": canonical_json(output),
        "figure-data.csv": csv_table(output["points"]),
        "timing-tables.csv": csv_table(output["timings"]),
        "cultivation-representation.csv": csv_table(output["cultivation_representation"]),
    }


def write_derived(out, derived):
    out.mkdir(parents=True, exist_ok=False)
    for name, content in derived.items():
        (out/name).write_bytes(content)


def check_bundle(plan, results, out=None):
    validate_manifest(plan)
    results = results.resolve(strict=True)
    sealed = reference.relative_file(results, "sealed")
    require({path.name for path in sealed.iterdir()} == {"plan.json", "index.json", "derived"},
            "sealed evidence inventory differs")
    require(checked_path(sealed, "plan.json").read_bytes() == canonical_json(plan), "sealed plan differs")
    index_file = checked_path(sealed, "index.json")
    require(file_record(index_file)["sha256"] == plan["index_sha256"], "sealed evidence index hash differs")
    raw = reference.relative_file(results, "raw")
    artifact_count = verify_inventory(raw, read_json(index_file))
    output, derived = generate(plan, raw, artifact_count)
    frozen_dir = reference.relative_file(sealed, "derived")
    require({path.name for path in frozen_dir.iterdir()} == derived.keys(), "derived evidence inventory differs")
    for name, content in derived.items():
        frozen = checked_path(frozen_dir, name)
        require(frozen.read_bytes() == content, f"sealed summary, timing rule, or weighted covariance differs: {name}")
    if out is not None:
        write_derived(out, derived)
    return dict(status="PASS", artifacts=artifact_count, **output["cells"])


def seal_bundle(plan, results, archive):
    import paper_evidence_deposit as deposit

    validate_manifest(plan, finalized=False)
    require("index_sha256" not in plan, "sealing requires an unsealed source manifest")
    results = results.resolve(strict=True)
    raw = reference.relative_file(results, "raw")
    target = reference.relative_file(results, "sealed")
    require(not target.exists(), "use an unsealed bundle directory")
    deposit.extract(reference.relative_file(raw, "provenance/deposit"), archive, reference.load_manifest(), plan["archive_sha256"])
    index = artifact_inventory(raw)
    index_content = canonical_json(index)
    sealed_plan = dict(plan, index_sha256=hashlib.sha256(index_content).hexdigest())
    output, derived = generate(sealed_plan, raw, len(index["files"]))
    with TemporaryDirectory(dir=results, prefix=".magna-seal-") as directory:
        temporary = Path(directory)/"sealed"
        temporary.mkdir()
        (temporary/"plan.json").write_bytes(canonical_json(sealed_plan))
        (temporary/"index.json").write_bytes(index_content)
        write_derived(temporary/"derived", derived)
        temporary.rename(target)
    return dict(status="SEALED", artifacts=output["artifacts"], index_sha256=sealed_plan["index_sha256"], **output["cells"])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=HERE/"paper/evidence.json")
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--seal", action="store_true")
    parser.add_argument("--archive", type=Path, help="paper deposit required when sealing a new bundle")
    args = parser.parse_args(argv)
    try:
        plan = read_json(args.manifest)
        if args.seal:
            require(args.archive is not None and args.out is None, "sealing requires --archive and uses the bundle directory")
            result = seal_bundle(plan, args.results, args.archive)
        else:
            require(args.archive is None, "checking a sealed bundle needs no archive")
            result = check_bundle(plan, args.results, args.out)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, TypeError, KeyError, OSError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(f"Magna paper evidence: FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
