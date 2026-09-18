#!/usr/bin/env python3
"""Build and verify the version-bound envelope support evidence bundle.

Every native release that advertises a Supported envelope decoder publishes one
self-contained, checksummed evidence bundle next to its archives. The bundle
freezes the exact inputs the per-decoder release gate consumed — the support
matrix, the compatibility policy, the support/correctness/resource evidence,
one installed-envelope report per official target, the passing gate report and
the retained resource raw observations — and binds them to the release tag,
the immutable source commit and both published archive identities.

``build`` assembles the bundle inside the candidate release run and refuses
mismatched tags, commits, archive hashes, missing platforms, failed evidence
or promotion claims the gate did not earn. The default mode verifies a
directory of already downloaded release assets read-only; it never reruns the
evidence campaigns:

    python3 tools/check_envelope_publication.py --self-test
    python3 tools/check_envelope_publication.py \
      --release-dir drafts/envelope-release-audit \
      --expect-decoder envelope-matching \
      --marker-out drafts/envelope-release-audit/envelope-support-verification-v0.3.2.json

``--expect-decoder`` may be repeated to require more than one Supported
decoder. Releases that predate the bundle (such as v0.3.0) contain no evidence
asset and are reported as lacking a verified promotion, never as Supported.
``--marker-out`` is valid only in release-directory verification mode and
requires at least one expected decoder; the release workflow uses it only
after downloading the published assets and verifying them again.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import re
import shutil
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_envelope_release as gate  # noqa: E402
from tools import check_envelope_support as support  # noqa: E402
from tools import envelope_mle_scope as mle_scope  # noqa: E402
from tools import verify_release_archive  # noqa: E402

PUBLICATION_SCHEMA = "rustqec.envelope-publication.v1"
VERIFICATION_SCHEMA = "rustqec.envelope-publication-verification.v1"
MATRIX_SCHEMA = support.SCHEMA_VERSION
REQUIRED_TARGETS = gate.DEFAULT_TARGETS
PASS_LINE = "PASS envelope published support"
BUNDLE_PREFIX = "envelope-support-evidence-"
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
BUNDLE_SUMS_LINE = re.compile(r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._/-]*)")

GENERATED_EVIDENCE = {
    "support": ("support.json", support.RESULT_SCHEMA_VERSION),
    "correctness": ("correctness.json", "rustqec.envelope-readiness-correctness.v1"),
    "resources": ("resources.json", "rustqec.envelope-readiness-resources.v1"),
    "mle-resources": ("mle-resources.json", gate.MLE_RESOURCES_SCHEMA),
}

# Machine-local absolute paths recorded by the candidate run are dropped when
# the installed reports are republished; archive identity is kept as
# filename + SHA-256 so each report still matches its published archive.
REDACTED_INSTALLED_PATHS = ("bin_dir", "binary.path", "archive.path", "matrix.path")


class PublicationError(Exception):
    pass


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise PublicationError(f"missing {label}: {path}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise PublicationError(f"invalid {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise PublicationError(f"{label} root must be an object: {path}")
    return value


def require_schema(document: dict[str, Any], schema: str, label: str) -> None:
    if document.get("schema_version") != schema:
        raise PublicationError(
            f"{label} schema mismatch: expected {schema}, "
            f"observed {document.get('schema_version')!r}"
        )


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PublicationError(message)


# ---------------------------------------------------------------------------
# Deterministic publication claims
# ---------------------------------------------------------------------------


def derive_decoders(
    matrix: dict[str, Any],
    gate_report: dict[str, Any],
    mle_plan: dict[str, Any] | None,
) -> dict[str, Any]:
    """The only maturity/scope a bundle may claim, derived from the evidence.

    Maturity is the gate decision. Matching scope comes from the consumed
    matrix, while MLE scope comes from the separately gated finite-domain
    plan; a hand-edited claim therefore never reconciles.
    """
    contract = matrix["circuit_contract"]
    families = [
        {"id": family["id"], "status": family["status"], "domain_note": family["domain_note"]}
        for family in contract["circuit_families"]
    ]
    decoders: dict[str, Any] = {}
    for name, declaration in matrix["decoders"].items():
        decision = gate_report.get("decoders", {}).get(name)
        require(
            isinstance(decision, dict) and decision.get("decision") in ("supported", "beta"),
            f"gate report does not record a supported/beta decision for {name}",
        )
        scope_statement = matrix["release_candidate_scope"]["statement"]
        scope_plan = None
        if name == "envelope-mle" and mle_plan is not None:
            scope_statement = mle_plan["domain"]["no_interpolation"]
            scope_plan = "scope/envelope-mle-scope.json"
        record = {
            "maturity": decision["decision"],
            "objective": declaration["objective"],
            "required_build_features": declaration.get("required_build_features", []),
            "circuit_families": families,
            "scope_statement": scope_statement,
            "numeric_operating_limits": declaration["numeric_operating_limits"],
            "known_limitations": declaration["known_limitations"],
        }
        # Matching-only legacy bundles (v0.3.1) predate the MLE scope-plan
        # member. Preserve their exact publication shape while requiring the
        # plan for every bundle that actually carries one or promotes MLE.
        if scope_plan is not None:
            record["scope_plan"] = scope_plan
        decoders[name] = record
    return decoders


def supported_decoders(decoders: dict[str, Any]) -> list[str]:
    return sorted(name for name, record in decoders.items() if record["maturity"] == "supported")


# ---------------------------------------------------------------------------
# Bundle assembly (candidate release run)
# ---------------------------------------------------------------------------


def sanitize_installed_report(report: dict[str, Any]) -> dict[str, Any]:
    portable = gate.portable_installed_report(report)
    portable["redacted_machine_local_paths"] = list(REDACTED_INSTALLED_PATHS)
    return portable


def check_gate_report(gate_report: dict[str, Any], source_sha: str,
                      matrix_sha: str, policy_sha: str, label: str) -> None:
    require_schema(gate_report, gate.SCHEMA_VERSION, label)
    require(
        gate_report.get("status") == "pass",
        f"{label} does not record a passing gate decision "
        f"(status={gate_report.get('status')!r}); a release cannot publish support "
        "claims from a failed gate",
    )
    require(
        gate_report.get("candidate_revision") == source_sha,
        f"{label} candidate {gate_report.get('candidate_revision')!r} "
        f"does not match the release source commit {source_sha}",
    )
    matrix_record = gate_report.get("matrix") or {}
    require(
        matrix_record.get("sha256") == matrix_sha,
        f"{label} consumed a different support matrix "
        f"(gate {matrix_record.get('sha256')!r}, bundled {matrix_sha})",
    )
    policy_record = gate_report.get("compatibility_policy") or {}
    require(
        policy_record.get("sha256") == policy_sha,
        f"{label} consumed a different compatibility policy",
    )
    retained = gate_report.get("retained_resource_report") or {}
    require(
        retained.get("present") is True and retained.get("binding") == "source-equivalent",
        f"{label} does not record a source-equivalent retained resource report",
    )
    require(
        SHA256.fullmatch(str(retained.get("sha256", ""))) is not None,
        f"{label} does not bind the retained resource report by SHA-256",
    )
    for name in GENERATED_EVIDENCE:
        record = (gate_report.get("evidence") or {}).get(name) or {}
        if (name == "mle-resources" and not record
                and (gate_report.get("decoders", {}).get("envelope-mle") or {}).get(
                    "decision") != "supported"):
            continue
        require(
            SHA256.fullmatch(str(record.get("sha256", ""))) is not None,
            f"{label} does not bind {name} evidence by SHA-256",
        )
    for target in REQUIRED_TARGETS:
        record = (gate_report.get("installed") or {}).get(target) or {}
        require(
            SHA256.fullmatch(str(record.get("sha256", ""))) is not None,
            f"{label} does not bind the installed report for {target} by SHA-256",
        )
        require(
            SHA256.fullmatch(str(record.get("portable_sha256", ""))) is not None,
            f"{label} does not bind the portable installed report for {target}",
        )


def check_generated_evidence_files(
    evidence_dir: Path, source_sha: str, gate_report: dict[str, Any]
) -> tuple[dict[str, bytes], dict[str, dict[str, Any]]]:
    payloads: dict[str, bytes] = {}
    documents: dict[str, dict[str, Any]] = {}
    for name, (filename, schema) in GENERATED_EVIDENCE.items():
        path = evidence_dir / filename
        evidence = load_json(path, f"{name} evidence")
        require_schema(evidence, schema, f"{name} evidence")
        require(
            evidence.get("checkout_revision") == source_sha,
            f"{name} evidence revision {evidence.get('checkout_revision')!r} "
            f"does not match the release source commit {source_sha}",
        )
        require(
            evidence.get("status") == "pass",
            f"{name} evidence status is {evidence.get('status')!r}, not 'pass'",
        )
        gate_record = (gate_report.get("evidence") or {}).get(name) or {}
        require(
            gate_record.get("sha256") == sha256_file(path),
            f"{name} evidence does not match the exact input inspected by the release gate",
        )
        payloads[f"evidence/{filename}"] = path.read_bytes()
        documents[name] = evidence
    return payloads, documents


def load_installed_report(evidence_dir: Path, target: str, source_sha: str,
                          matrix_sha: str, archives: dict[str, Any],
                          gate_report: dict[str, Any]) -> dict[str, Any]:
    path = evidence_dir / f"installed-{target}.json"
    report = load_json(path, f"installed-envelope report for {target}")
    require_schema(report, gate.INSTALLED_SCHEMA, f"installed report for {target}")
    require(
        report.get("target") == target,
        f"installed report {path.name} is for target {report.get('target')!r}, not {target}",
    )
    require(
        report.get("source_revision") == source_sha,
        f"installed report for {target} tested revision "
        f"{report.get('source_revision')!r}, not the release source commit {source_sha}",
    )
    require(
        report.get("status") == "pass",
        f"installed report for {target} status is {report.get('status')!r}",
    )
    ilp = report.get("ilp") or {}
    require(
        ilp.get("expected") is True and ilp.get("available") is True,
        f"installed report for {target} is not a passing ILP-capable archive report",
    )
    archive = report.get("archive") or {}
    filename = Path(str(archive.get("path", ""))).name
    identity = archives.get(filename)
    require(
        isinstance(identity, dict) and identity.get("target") == target,
        f"installed report for {target} names archive {filename!r}, which is not the "
        f"published {target} archive",
    )
    require(
        archive.get("sha256") == identity.get("sha256"),
        f"installed report for {target} tested archive SHA-256 {archive.get('sha256')!r} "
        f"but the published {filename} hashes to {identity.get('sha256')!r}",
    )
    require(
        (report.get("matrix") or {}).get("sha256") == matrix_sha,
        f"installed report for {target} consumed a different support matrix",
    )
    gate_record = (gate_report.get("installed") or {}).get(target) or {}
    require(
        gate_record.get("sha256") == sha256_file(path),
        f"installed report for {target} does not match the exact input inspected by the release gate",
    )
    require(
        gate_record.get("portable_sha256")
        == gate.json_document_sha256(gate.portable_installed_report(report)),
        f"installed report for {target} does not match the portable content inspected by the release gate",
    )
    return sanitize_installed_report(report)


def check_evidence_completeness(
    matrix: dict[str, Any],
    gate_report: dict[str, Any],
    generated: dict[str, dict[str, Any]],
    installed: dict[str, dict[str, Any]],
) -> None:
    """Re-run the release gate's structural checks on the bundled evidence.

    Hash binding proves these are the files the gate consumed; this second
    check prevents a stale or hand-edited passing gate report from blessing a
    structurally incomplete bundle.
    """
    gaps = gate.correctness_summary_gaps(generated["correctness"])
    wrapped_generated = {
        name: {"evidence": evidence} for name, evidence in generated.items()
    }
    wrapped_installed = {
        target: {"report": report} for target, report in installed.items()
    }
    for decoder, decision in (gate_report.get("decoders") or {}).items():
        if decision.get("decision") == "supported":
            gaps += gate.decoder_coverage(
                decoder, wrapped_generated, wrapped_installed, matrix
            )
    require(
        not gaps,
        "bundled evidence fails the release gate's completeness checks: "
        + "; ".join(gaps[:4]),
    )


def collect_retained_resources(retained_dir: Path, expected_revision: str) -> dict[str, bytes]:
    manifest_path = retained_dir / "manifest.json"
    manifest = load_json(manifest_path, "retained resource manifest")
    require_schema(manifest, gate.RETAINED_MANIFEST_SCHEMA, "retained resource manifest")
    require(
        manifest.get("status") == "pass",
        "retained resource manifest does not record a passing campaign",
    )
    require(
        manifest.get("checkout_revision") == expected_revision,
        f"retained resource manifest revision {manifest.get('checkout_revision')!r} does "
        f"not match the gate's retained report revision {expected_revision}",
    )
    payloads: dict[str, bytes] = {}
    for path in sorted(retained_dir.rglob("*")):
        if path.is_file() and not path.is_symlink():
            relative = path.relative_to(retained_dir).as_posix()
            require(".." not in PurePosixPath(relative).parts, f"unsafe retained path {relative}")
            payloads[f"retained-resources/{relative}"] = path.read_bytes()
    case_raws = {
        f"retained-resources/{case['raw']}": case.get("raw_sha256")
        for case in manifest.get("cases", [])
        if case.get("raw")
    }
    for relative, recorded in case_raws.items():
        require(
            relative in payloads,
            f"retained resource manifest names a missing raw observation {relative}",
        )
        if recorded is not None:
            require(
                sha256_bytes(payloads[relative]) == recorded,
                f"retained raw observation {relative} does not match its recorded SHA-256",
            )
    return payloads


def render_summary(publication: dict[str, Any]) -> str:
    lines = [
        f"# Envelope support evidence — {publication['tag']}",
        "",
        f"- Version: {publication['version']}",
        f"- Source commit: {publication['source_sha']}",
        f"- Release gate: `{gate.SCHEMA_VERSION}`, status pass",
        "",
        "## Decoder maturity",
        "",
        "| Decoder | Maturity |",
        "| --- | --- |",
    ]
    for name, record in sorted(publication["decoders"].items()):
        lines.append(f"| `{name}` | {record['maturity'].capitalize()} |")
    lines += [
        "",
        "## Scope",
        "",
    ]
    for name, record in sorted(publication["decoders"].items()):
        if record["maturity"] != "supported":
            continue
        lines.append(f"### `{name}` (Supported)")
        lines.append("")
        lines.append(record["scope_statement"])
        lines.append("")
        lines.append(f"Operating limits: {record['numeric_operating_limits']}")
        lines.append("")
        if record["known_limitations"]:
            lines.append("Known limitations:")
            for limitation in record["known_limitations"]:
                lines.append(f"- {limitation}")
            lines.append("")
    lines += [
        "## Platforms",
        "",
        "| Target | Archive | SHA-256 |",
        "| --- | --- | --- |",
    ]
    for platform in publication["platforms"]:
        archive = platform["archive"]
        lines.append(f"| `{platform['target']}` | `{archive['filename']}` | `{archive['sha256']}` |")
    lines += [
        "",
        "## Verification",
        "",
        "Download every asset of this release into one directory and run:",
        "",
        "```sh",
        "python3 tools/check_envelope_publication.py \\",
        "  --release-dir <download-dir>" + "".join(
            f" --expect-decoder {name}" for name in publication["supported_decoders"]
        ),
        "```",
        "",
        "The verifier reconciles the tag, source commit, both archive identities, the",
        "per-platform installed reports and the gate decision without rerunning the",
        "evidence campaigns. Releases without this bundle (for example v0.3.0) lack a",
        "verified envelope promotion.",
        "",
    ]
    return "\n".join(lines)


def _add_bytes(archive: tarfile.TarFile, name: str, payload: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(payload)
    info.mode = 0o644
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    archive.addfile(info, io.BytesIO(payload))


def write_bundle(path: Path, root: str, files: dict[str, bytes]) -> None:
    with path.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for relative in sorted(files):
                    _add_bytes(archive, f"{root}/{relative}", files[relative])


def build_bundle(
    *,
    tag: str,
    source_sha: str,
    release_manifest: Path,
    archives_dir: Path,
    evidence_dir: Path,
    gate_report_path: Path,
    matrix_path: Path,
    policy_path: Path,
    mle_scope_path: Path,
    retained_dir: Path,
    out_dir: Path,
) -> tuple[Path, Path, dict[str, Any]]:
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", tag):
        raise PublicationError(f"invalid release tag: {tag!r}")
    if not FULL_SHA.fullmatch(source_sha):
        raise PublicationError("source SHA must be a full lowercase Git commit ID")

    manifest = load_json(release_manifest, "release manifest")
    try:
        verify_release_archive.validate_manifest(manifest, tag)
    except verify_release_archive.VerificationError as error:
        raise PublicationError(f"release manifest rejected: {error}") from error
    require(
        manifest.get("source_sha") == source_sha,
        f"release manifest source SHA {manifest.get('source_sha')!r} "
        f"does not match {source_sha}",
    )
    archives = manifest["archives"]
    for filename, identity in archives.items():
        archive_path = archives_dir / filename
        require(archive_path.is_file(), f"missing native archive {filename} in {archives_dir}")
        require(
            sha256_file(archive_path) == identity.get("sha256"),
            f"native archive {filename} does not match the release manifest SHA-256",
        )

    matrix_bytes = matrix_path.read_bytes()
    matrix = json.loads(matrix_bytes.decode("utf-8"))
    require_schema(matrix, MATRIX_SCHEMA, "support matrix")
    matrix_sha = sha256_bytes(matrix_bytes)
    policy_bytes = policy_path.read_bytes()
    policy_sha = sha256_bytes(policy_bytes)

    gate_report = load_json(gate_report_path, "envelope release gate report")
    check_gate_report(gate_report, source_sha, matrix_sha, policy_sha, "gate report")
    try:
        mle_plan = mle_scope.load_plan(mle_scope_path)
    except (mle_scope.ScopeError, json.JSONDecodeError) as error:
        raise PublicationError(f"invalid MLE scope plan: {error}") from error
    mle_scope_bytes = mle_scope_path.read_bytes()
    mle_scope_sha = sha256_bytes(mle_scope_bytes)
    gate_scope = gate_report.get("mle_scope_plan") or {}
    require(
        gate_scope.get("sha256") == mle_scope_sha,
        "MLE scope plan does not match the plan consumed by the release gate",
    )
    retained_revision = gate_report["retained_resource_report"]["checkout_revision"]

    payloads, generated_documents = check_generated_evidence_files(
        evidence_dir, source_sha, gate_report
    )
    platforms: list[dict[str, Any]] = []
    installed_documents: dict[str, dict[str, Any]] = {}
    for target in REQUIRED_TARGETS:
        sanitized = load_installed_report(
            evidence_dir, target, source_sha, matrix_sha, archives, gate_report
        )
        installed_documents[target] = sanitized
        archive_identity = next(
            identity for identity in archives.values() if identity["target"] == target
        )
        relative = f"evidence/installed-{target}.json"
        payload = (json.dumps(sanitized, indent=2) + "\n").encode("utf-8")
        payloads[relative] = payload
        platforms.append({
            "target": target,
            "archive": {
                "filename": archive_identity["filename"],
                "sha256": archive_identity["sha256"],
                "size": archive_identity["size"],
            },
            "installed_report": relative,
            "installed_report_sha256": sha256_bytes(payload),
        })
    payloads["matrix/envelope-support.json"] = matrix_bytes
    payloads["policy/envelope-compatibility-policy.md"] = policy_bytes
    payloads["scope/envelope-mle-scope.json"] = mle_scope_bytes
    gate_bytes = gate_report_path.read_bytes()
    payloads["release-gate.json"] = gate_bytes
    retained_payloads = collect_retained_resources(retained_dir, retained_revision)
    require(
        sha256_bytes(retained_payloads["retained-resources/manifest.json"])
        == gate_report["retained_resource_report"].get("sha256"),
        "retained resource manifest does not match the exact input inspected by the release gate",
    )
    payloads.update(retained_payloads)
    check_evidence_completeness(
        matrix, gate_report, generated_documents, installed_documents
    )

    decoders = derive_decoders(matrix, gate_report, mle_plan)
    publication: dict[str, Any] = {
        "schema_version": PUBLICATION_SCHEMA,
        "tag": tag,
        "version": tag[1:] if tag.startswith("v") else tag,
        "source_sha": source_sha,
        "generator": "tools/check_envelope_publication.py build",
        "decoders": decoders,
        "supported_decoders": supported_decoders(decoders),
        "platforms": platforms,
        "matrix": {"path": "matrix/envelope-support.json", "sha256": matrix_sha},
        "policy": {"path": "policy/envelope-compatibility-policy.md", "sha256": policy_sha},
        "mle_scope": {
            "path": "scope/envelope-mle-scope.json",
            "sha256": mle_scope_sha,
            "source_revision": mle_plan.get("applies_to", {}).get("source_revision"),
        },
        "gate_report": {"path": "release-gate.json", "sha256": sha256_bytes(gate_bytes)},
        "evidence": {
            name: {"path": f"evidence/{filename}", "sha256": sha256_bytes(payloads[f"evidence/{filename}"])}
            for name, (filename, _schema) in GENERATED_EVIDENCE.items()
        },
        "retained_resources": {
            "manifest": {
                "path": "retained-resources/manifest.json",
                "sha256": sha256_bytes(retained_payloads["retained-resources/manifest.json"]),
                "checkout_revision": retained_revision,
            },
            "files": {
                relative: sha256_bytes(payload)
                for relative, payload in sorted(retained_payloads.items())
            },
        },
    }
    payloads["publication.json"] = (json.dumps(publication, indent=2) + "\n").encode("utf-8")
    payloads["SUMMARY.md"] = render_summary(publication).encode("utf-8")
    sums = "".join(
        f"{sha256_bytes(payload)}  {relative}\n" for relative, payload in sorted(payloads.items())
    )
    payloads["SHA256SUMS"] = sums.encode("utf-8")

    out_dir.mkdir(parents=True, exist_ok=True)
    bundle_name = f"{BUNDLE_PREFIX}{tag}.tar.gz"
    bundle_path = out_dir / bundle_name
    write_bundle(bundle_path, bundle_name.removesuffix(".tar.gz"), payloads)
    sidecar_path = out_dir / f"{bundle_name}.sha256"
    sidecar_path.write_text(f"{sha256_file(bundle_path)}  {bundle_name}\n", encoding="utf-8")
    return bundle_path, sidecar_path, publication


# ---------------------------------------------------------------------------
# Read-only verification of downloaded release assets
# ---------------------------------------------------------------------------


def parse_bundle_checksums(path: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        match = BUNDLE_SUMS_LINE.fullmatch(line)
        if not match:
            raise PublicationError(f"invalid bundle SHA256SUMS line {line_number}")
        digest, filename = match.groups()
        parts = PurePosixPath(filename).parts
        if ".." in parts or filename.endswith("/"):
            raise PublicationError(f"unsafe bundle SHA256SUMS entry: {filename}")
        if filename in checksums:
            raise PublicationError(f"duplicate bundle SHA256SUMS entry: {filename}")
        checksums[filename] = digest
    return checksums


def extract_bundle(bundle_path: Path, destination: Path, root: str) -> Path:
    try:
        with tarfile.open(bundle_path, "r:gz") as archive:
            members = archive.getmembers()
            for member in members:
                relative = PurePosixPath(member.name)
                if relative.is_absolute() or ".." in relative.parts or not relative.parts:
                    raise PublicationError(f"unsafe evidence bundle member: {member.name}")
                if relative.parts[0] != root:
                    raise PublicationError(f"evidence bundle member outside its root: {member.name}")
                if not member.isfile() and not member.isdir():
                    raise PublicationError(f"unsupported evidence bundle member type: {member.name}")
            for member in members:
                if member.isdir():
                    continue
                source = archive.extractfile(member)
                if source is None:
                    raise PublicationError(f"cannot read evidence bundle member: {member.name}")
                output = destination.joinpath(*PurePosixPath(member.name).parts)
                output.parent.mkdir(parents=True, exist_ok=True)
                with output.open("wb") as handle:
                    shutil.copyfileobj(source, handle, 1024 * 1024)
    except tarfile.TarError as error:
        raise PublicationError(f"cannot read the evidence bundle: {error}") from error
    extracted = (destination / root).resolve()
    if destination.resolve() not in extracted.parents:
        raise PublicationError("evidence bundle extraction escaped the staging directory")
    return extracted


def verify_release_dir(release_dir: Path, expect_decoders: tuple[str, ...]) -> dict[str, Any]:
    if not release_dir.is_dir():
        raise PublicationError(f"release directory does not exist: {release_dir}")
    bundles = sorted(release_dir.glob(f"{BUNDLE_PREFIX}*.tar.gz"))
    if not bundles:
        raise PublicationError(
            f"no {BUNDLE_PREFIX}*.tar.gz asset found in {release_dir}: this release "
            "lacks a verified envelope promotion (legacy releases such as v0.3.0 "
            "predate the evidence bundle and must not be treated as Supported)"
        )
    require(len(bundles) == 1, f"multiple envelope evidence bundles in {release_dir}")
    bundle_path = bundles[0]

    sidecar_path = bundle_path.with_name(bundle_path.name + ".sha256")
    require(sidecar_path.is_file(), f"missing evidence bundle checksum {sidecar_path.name}")
    match = re.fullmatch(
        r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._-]*)", sidecar_path.read_text().strip()
    )
    require(match is not None, f"invalid evidence bundle checksum file {sidecar_path.name}")
    recorded, named = match.groups()
    require(
        named == bundle_path.name,
        f"evidence bundle checksum names {named!r}, expected {bundle_path.name!r}",
    )
    require(
        sha256_file(bundle_path) == recorded,
        f"evidence bundle SHA-256 mismatch: {bundle_path.name} was modified after publication",
    )

    with tempfile.TemporaryDirectory(prefix="envelope-publication-") as temporary:
        root = bundle_path.name.removesuffix(".tar.gz")
        bundle = extract_bundle(bundle_path, Path(temporary), root)
        return _verify_extracted(bundle, release_dir, bundle_path.name, expect_decoders)


def _verify_extracted(bundle: Path, release_dir: Path, bundle_name: str,
                      expect_decoders: tuple[str, ...]) -> dict[str, Any]:
    sums = parse_bundle_checksums(bundle / "SHA256SUMS")
    actual_files = {
        path.relative_to(bundle).as_posix()
        for path in bundle.rglob("*")
        if path.is_file()
    } - {"SHA256SUMS"}
    require(
        set(sums) == actual_files,
        "bundle SHA256SUMS does not cover exactly the bundled files: "
        + ", ".join(sorted(set(sums) ^ actual_files)[:3]),
    )
    for relative, digest in sums.items():
        require(
            sha256_file(bundle / relative) == digest,
            f"bundle member {relative} does not match its recorded SHA-256",
        )

    publication = load_json(bundle / "publication.json", "publication record")
    require_schema(publication, PUBLICATION_SCHEMA, "publication record")
    tag = publication.get("tag")
    require(
        isinstance(tag, str) and bundle_name == f"{BUNDLE_PREFIX}{tag}.tar.gz",
        f"publication tag {tag!r} does not match the bundle filename {bundle_name}",
    )
    source_sha = str(publication.get("source_sha", ""))
    require(FULL_SHA.fullmatch(source_sha), "publication record source SHA is invalid")

    manifest = load_json(release_dir / "release-manifest.json", "release manifest")
    try:
        verify_release_archive.validate_manifest(manifest, tag)
    except verify_release_archive.VerificationError as error:
        raise PublicationError(f"release manifest rejected: {error}") from error
    require(
        manifest.get("source_sha") == source_sha,
        f"release manifest source SHA {manifest.get('source_sha')!r} does not match "
        f"the evidence bundle {source_sha}",
    )
    archives = manifest["archives"]
    try:
        release_sums = verify_release_archive.parse_checksums(release_dir / "SHA256SUMS")
    except verify_release_archive.VerificationError as error:
        raise PublicationError(f"release SHA256SUMS rejected: {error}") from error
    require(
        set(release_sums) == set(archives),
        "release SHA256SUMS does not contain exactly the manifested archives",
    )
    actual_archive_hashes: dict[str, str] = {}
    for filename, identity in archives.items():
        archive_path = release_dir / filename
        require(archive_path.is_file(), f"published archive missing from the directory: {filename}")
        actual = sha256_file(archive_path)
        require(
            actual == identity.get("sha256") == release_sums.get(filename),
            f"published archive {filename} does not match the release manifest/checksums",
        )
        actual_archive_hashes[filename] = actual

    matrix_bytes = (bundle / "matrix/envelope-support.json").read_bytes()
    matrix = json.loads(matrix_bytes.decode("utf-8"))
    require_schema(matrix, MATRIX_SCHEMA, "bundled support matrix")
    matrix_sha = sha256_bytes(matrix_bytes)
    require(
        matrix_sha == (publication.get("matrix") or {}).get("sha256"),
        "bundled support matrix does not match the publication record",
    )
    policy_sha = sha256_file(bundle / "policy/envelope-compatibility-policy.md")
    require(
        policy_sha == (publication.get("policy") or {}).get("sha256"),
        "bundled compatibility policy does not match the publication record",
    )
    scope_record = publication.get("mle_scope") or {}
    scope_path = bundle / "scope/envelope-mle-scope.json"
    mle_plan: dict[str, Any] | None = None
    if scope_record:
        require(scope_path.is_file(), "publication records an MLE scope plan but the bundle omits it")
        try:
            mle_plan = mle_scope.load_plan(scope_path)
        except (mle_scope.ScopeError, json.JSONDecodeError) as error:
            raise PublicationError(f"bundled MLE scope plan is invalid: {error}") from error
        require(
            scope_record.get("path") == "scope/envelope-mle-scope.json"
            and scope_record.get("sha256") == sha256_file(scope_path)
            and scope_record.get("source_revision")
            == mle_plan.get("applies_to", {}).get("source_revision"),
            "bundled MLE scope plan does not match the publication record",
        )
    else:
        require(
            not scope_path.exists(),
            "bundle carries an unrecorded MLE scope plan",
        )

    gate_report = load_json(bundle / "release-gate.json", "bundled gate report")
    check_gate_report(gate_report, source_sha, matrix_sha, policy_sha, "bundled gate report")
    require(
        sha256_bytes(bundle.joinpath("release-gate.json").read_bytes())
        == (publication.get("gate_report") or {}).get("sha256"),
        "bundled gate report does not match the publication record",
    )
    require(
        tuple(gate_report.get("required_targets", ())) == tuple(REQUIRED_TARGETS),
        "gate report does not cover both official release targets",
    )
    mle_decision = (gate_report.get("decoders", {}).get("envelope-mle") or {}).get(
        "decision"
    )
    if mle_plan is not None:
        require(
            (gate_report.get("mle_scope_plan") or {}).get("sha256")
            == scope_record.get("sha256"),
            "bundled MLE scope plan does not match the plan consumed by the gate",
        )
    else:
        require(
            mle_decision != "supported" and "envelope-mle" not in expect_decoders,
            "an MLE Supported publication requires a bundled, gate-bound MLE scope plan",
        )

    generated_documents: dict[str, dict[str, Any]] = {}
    for name, (filename, schema) in GENERATED_EVIDENCE.items():
        recorded = (publication.get("evidence") or {}).get(name) or {}
        if name == "mle-resources" and not recorded:
            require(
                mle_decision != "supported",
                "an MLE Supported publication requires bundled mle-resources evidence",
            )
            continue
        path = bundle / "evidence" / filename
        evidence = load_json(path, f"bundled {name} evidence")
        require_schema(evidence, schema, f"bundled {name} evidence")
        require(
            evidence.get("checkout_revision") == source_sha,
            f"bundled {name} evidence revision {evidence.get('checkout_revision')!r} "
            f"does not match the release source commit {source_sha}",
        )
        require(
            evidence.get("status") == "pass",
            f"bundled {name} evidence status is {evidence.get('status')!r}",
        )
        if (name == "mle-resources"
                and (gate_report.get("decoders", {}).get("envelope-mle") or {}).get(
                    "decision") == "supported"):
            problems = gate.mle_resource_campaign.verify_document(evidence, scope_path)
            require(
                not problems,
                "bundled MLE resource evidence failed replay: " + "; ".join(problems),
            )
        require(
            recorded.get("sha256") == sha256_file(path),
            f"bundled {name} evidence does not match the publication record",
        )
        require(
            ((gate_report.get("evidence") or {}).get(name) or {}).get("sha256")
            == sha256_file(path),
            f"bundled {name} evidence does not match the exact input inspected by the release gate",
        )
        generated_documents[name] = evidence

    retained = publication.get("retained_resources") or {}
    retained_manifest = load_json(
        bundle / "retained-resources/manifest.json", "bundled retained resource manifest"
    )
    require_schema(retained_manifest, gate.RETAINED_MANIFEST_SCHEMA,
                   "bundled retained resource manifest")
    require(
        retained_manifest.get("status") == "pass",
        "bundled retained resource manifest does not record a passing campaign",
    )
    require(
        retained_manifest.get("checkout_revision")
        == gate_report["retained_resource_report"].get("checkout_revision")
        == (retained.get("manifest") or {}).get("checkout_revision"),
        "retained resource report revision disagreement between the bundle, the "
        "publication record and the gate report",
    )
    require(
        sha256_file(bundle / "retained-resources/manifest.json")
        == gate_report["retained_resource_report"].get("sha256"),
        "bundled retained resource manifest does not match the exact input inspected by the release gate",
    )
    for case in retained_manifest.get("cases", []):
        raw = case.get("raw")
        if not raw:
            continue
        require(
            ".." not in PurePosixPath(raw).parts,
            f"unsafe retained raw observation path {raw}",
        )
        raw_path = bundle / "retained-resources" / raw
        require(raw_path.is_file(), f"missing retained raw observation {raw}")
        require(
            SHA256.fullmatch(str(case.get("raw_sha256", ""))) is not None,
            f"retained raw observation {raw} lacks a valid manifest SHA-256",
        )
        require(
            sha256_file(raw_path) == case["raw_sha256"],
            f"retained raw observation {raw} does not match the gate-bound manifest",
        )
    for relative, digest in (retained.get("files") or {}).items():
        path = bundle / relative
        require(path.is_file(), f"missing retained raw observation {relative}")
        require(
            sha256_file(path) == digest,
            f"retained raw observation {relative} does not match the publication record",
        )

    platforms = publication.get("platforms") or []
    require(
        sorted(platform.get("target") for platform in platforms) == sorted(REQUIRED_TARGETS),
        "publication record does not cover exactly the two official release targets",
    )
    installed_documents: dict[str, dict[str, Any]] = {}
    for platform in platforms:
        target = platform["target"]
        report = load_json(bundle / platform["installed_report"],
                           f"bundled installed-envelope report for {target}")
        require_schema(report, gate.INSTALLED_SCHEMA, f"installed report for {target}")
        require(
            report.get("target") == target,
            f"bundled installed report for {target} describes target "
            f"{report.get('target')!r}; installed reports cannot be swapped between archives",
        )
        require(
            report.get("source_revision") == source_sha,
            f"bundled installed report for {target} tested a different source revision",
        )
        require(
            report.get("status") == "pass",
            f"bundled installed report for {target} status is {report.get('status')!r}",
        )
        report_archive = report.get("archive") or {}
        published = platform.get("archive") or {}
        require(
            report_archive.get("filename") == published.get("filename"),
            f"installed report for {target} names archive "
            f"{report_archive.get('filename')!r}, not the published "
            f"{published.get('filename')!r}",
        )
        require(
            report_archive.get("sha256")
            == published.get("sha256")
            == actual_archive_hashes.get(str(published.get("filename"))),
            f"installed report for {target} does not hash-match the published archive "
            f"{published.get('filename')!r}; the report and the archive come from "
            "different builds",
        )
        require(
            (report.get("matrix") or {}).get("sha256") == matrix_sha,
            f"installed report for {target} consumed a different support matrix",
        )
        ilp = report.get("ilp") or {}
        require(
            ilp.get("expected") is True and ilp.get("available") is True,
            f"installed report for {target} is not ILP-capable",
        )
        require(
            sha256_file(bundle / platform["installed_report"])
            == platform.get("installed_report_sha256"),
            f"bundled installed report for {target} does not match the publication record",
        )
        require(
            ((gate_report.get("installed") or {}).get(target) or {}).get("portable_sha256")
            == gate.json_document_sha256(gate.portable_installed_report(report)),
            f"bundled installed report for {target} does not match the portable content inspected by the release gate",
        )
        installed_documents[target] = report

    check_evidence_completeness(
        matrix, gate_report, generated_documents, installed_documents
    )

    expected_claims = derive_decoders(matrix, gate_report, mle_plan)
    require(
        publication.get("decoders") == expected_claims,
        "publication decoder claims do not match the gate decision and the consumed "
        "matrix; the advertised maturity or scope was edited without matching evidence",
    )
    require(
        publication.get("supported_decoders") == supported_decoders(expected_claims),
        "publication supported-decoder list does not match the gate decision",
    )
    for decoder in expect_decoders:
        record = expected_claims.get(decoder)
        require(record is not None, f"unknown decoder {decoder!r} for this release")
        require(
            record["maturity"] == "supported",
            f"decoder {decoder} is not Supported in {tag}: the gate decision for this "
            f"release is {record['maturity']!r}",
        )

    return {
        "tag": tag,
        "version": publication.get("version"),
        "source_sha": source_sha,
        "evidence_bundle": {
            "asset": bundle_name,
            "sha256": sha256_file(release_dir / bundle_name),
        },
        "decoders": expected_claims,
        "supported_decoders": supported_decoders(expected_claims),
        "platforms": platforms,
    }


def print_summary(result: dict[str, Any], expect_decoders: tuple[str, ...]) -> None:
    print(f"envelope published support: version={result['version']} tag={result['tag']} "
          f"source={result['source_sha']}")
    for platform in result["platforms"]:
        archive = platform["archive"]
        print(f"platform {platform['target']}: {archive['filename']} sha256={archive['sha256']}")
    for name, record in sorted(result["decoders"].items()):
        maturity = record["maturity"].capitalize()
        print(f"decoder {name}: {maturity}")
        if name in expect_decoders:
            print(f"  scope: {record['scope_statement']}")
            print(f"  operating limits: {record['numeric_operating_limits']}")
    print(PASS_LINE)


def write_verification_marker(result: dict[str, Any], path: Path) -> None:
    """Write the browser-readable marker only after downloaded assets verify."""
    marker = {
        "schema_version": VERIFICATION_SCHEMA,
        "tag": result["tag"],
        "version": result["version"],
        "source_sha": result["source_sha"],
        "evidence_bundle": result["evidence_bundle"],
        "supported_decoders": result["supported_decoders"],
        "verification": "pass",
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(marker, indent=2) + "\n", encoding="utf-8")


# ---------------------------------------------------------------------------
# Offline self-test with synthetic fixtures
# ---------------------------------------------------------------------------

SELFTEST_TAG = "v9.9.9"
SELFTEST_SHA = "a" * 40
SELFTEST_RETAINED_SHA = "b" * 40


def _fixture_matrix() -> dict[str, Any]:
    return {
        "schema_version": MATRIX_SCHEMA,
        "applies_to": {"source_revision": SELFTEST_SHA},
        "decoders": {
            "envelope-matching": {
                "required_build_features": [],
                "current_maturity": "beta",
                "proposed_release_maturity": "supported-candidate",
                "objective": "Minimum-weight perfect matching on a loss-conditioned graph.",
                "known_limitations": ["Not the exact declared fault-configuration objective."],
                "numeric_operating_limits": "Measured operating envelope: retained resource report.",
            },
            "envelope-mle": {
                "required_build_features": ["ilp"],
                "current_maturity": "beta",
                "proposed_release_maturity": "beta",
                "objective": "Exact most-likely fault configuration for the declared envelope model.",
                "known_limitations": ["Envelope candidate count is bounded."],
                "numeric_operating_limits": "Measured operating envelope: retained resource report.",
            },
        },
        "circuit_contract": {
            "circuit_families": [
                {"id": "midswap", "description": "Flat loss-visible Mid-SWAP memory-Z circuits.",
                 "status": "supported-domain-candidate",
                 "domain_note": "Finite witnesses of the declared domain."},
                {"id": "conventional-stim-annotated",
                 "description": "Conventional Stim-generated circuits annotated loss-visible.",
                 "status": "isolated-checked-examples",
                 "domain_note": "Only the pinned fixtures are checked."},
            ],
        },
        "release_candidate_scope": {
            "statement": "First supported-release candidate scope: envelope-matching on flat "
                         "loss-visible Mid-SWAP memory-Z circuits within the measured envelope.",
            "promotion_gate": "per-decoder release-readiness gate",
        },
        "controls": [],
    }


def _fixture_gate_report(
    matrix_sha: str,
    policy_sha: str,
    scope_sha: str,
    matching_decision: str,
    mle_decision: str,
    evidence_dir: Path,
    retained_manifest: Path,
) -> dict[str, Any]:
    proposed = "supported-candidate" if matching_decision == "supported" else "beta"
    blocking = [] if matching_decision == "supported" else ["envelope-matching held at beta: synthetic"]
    return {
        "schema_version": gate.SCHEMA_VERSION,
        "candidate_revision": SELFTEST_SHA,
        "required_targets": list(REQUIRED_TARGETS),
        "matrix": {
            "path": "docs/envelope-support.json",
            "sha256": matrix_sha,
            "matrix_source_revision": SELFTEST_SHA,
            "candidate_revision": SELFTEST_SHA,
            "binding": "ancestor-or-equal",
        },
        "compatibility_policy": {
            "path": "docs/envelope-compatibility-policy.md",
            "present": True,
            "sha256": policy_sha,
            "required_sections_missing": [],
        },
        "retained_resource_report": {
            "path": "benchmarks/atom_loss/readiness/resources/manifest.json",
            "present": True,
            "sha256": sha256_file(retained_manifest),
            "checkout_revision": SELFTEST_RETAINED_SHA,
            "changed_equivalence_paths": [],
            "binding": "source-equivalent",
        },
        "mle_scope_plan": {
            "path": "docs/envelope-mle-scope.json",
            "present": True,
            "enforced": mle_decision == "supported",
            "sha256": scope_sha,
            "source_revision": "359fc656d6a8d7150489fd6735fdf6f9f5e236c2",
        },
        "evidence": {
            name: {
                "path": f"evidence/{filename}",
                "sha256": sha256_file(evidence_dir / filename),
                "schema_version": schema,
                "checkout_revision": SELFTEST_SHA,
                "revision_bound": True,
                "equivalence_waiver": False,
                "status": "pass",
            }
            for name, (filename, schema) in GENERATED_EVIDENCE.items()
        },
        "installed": {
            target: {
                "path": f"evidence/installed-{target}.json",
                "sha256": sha256_file(evidence_dir / f"installed-{target}.json"),
                "portable_sha256": gate.json_document_sha256(
                    gate.portable_installed_report(
                        load_json(
                            evidence_dir / f"installed-{target}.json",
                            f"installed fixture for {target}",
                        )
                    )
                ),
                "target": target,
                "source_revision": SELFTEST_SHA,
                "revision_bound": True,
                "equivalence_waiver": False,
                "status": "pass",
            }
            for target in REQUIRED_TARGETS
        },
        "decoders": {
            "envelope-matching": {
                "current_maturity": "beta",
                "proposed_release_maturity": proposed,
                "decision": matching_decision,
                "coverage_gaps": [],
                "blocking_gaps": list(blocking),
            },
            "envelope-mle": {
                "current_maturity": "beta",
                "proposed_release_maturity": (
                    "supported-candidate" if mle_decision == "supported" else "beta"
                ),
                "decision": mle_decision,
                "coverage_gaps": [],
                "blocking_gaps": [],
            },
        },
        "status": "pass",
        "blocking_gaps": [],
    }


def _fixture_correctness() -> dict[str, Any]:
    cases = []
    for index, prediction in enumerate((0, 1), 1):
        predictions = [prediction, prediction]
        case = {
            "name": f"synthetic-{index}",
            "evidence_level": "independent-end-to-end",
            "seed": index,
            "detectors": index,
            "histories": [
                {"category": category, "exercised": True}
                for category in (
                    "no_loss",
                    "single_loss",
                    "multiple_simultaneous_losses",
                    "loss_in_different_rounds",
                    "reset_restoring_wire",
                )
            ],
            "noise_channels_exercised": list(gate.REQUIRED_NOISE_CHANNELS),
            "rows": 2,
            "placeholder_pairs": 1,
            "allowed_answers": {
                "envelope-matching": [[prediction], [prediction]],
                "envelope-mle": [[prediction], [prediction]],
            },
            "backends": {
                "envelope-matching": {
                    "predictions": predictions,
                    "checked_rows": 2,
                    "unique_optimum_rows": 2,
                    "allowed_tie_rows": 0,
                    "placeholder_invariance": True,
                    "prediction_sha256": hashlib.sha256(bytes(predictions)).hexdigest(),
                    "rejected_rows": [],
                    "flipped_prediction_rejected": True,
                },
                "envelope-mle": {
                    "predictions": predictions,
                    "checked_rows": 2,
                    "unique_optimum_rows": 2,
                    "allowed_tie_rows": 0,
                    "placeholder_invariance": True,
                    "prediction_sha256": hashlib.sha256(bytes(predictions)).hexdigest(),
                    "rejected_rows": [],
                    "flipped_prediction_rejected": True,
                },
            },
            "mismatches": [],
        }
        cases.append(case)
    return {"cases": cases, "coverage": gate.recomputed_coverage(cases)}


def _fixture_installed(target: str, archive_name: str, archive_sha: str,
                       matrix_sha: str) -> dict[str, Any]:
    return {
        "schema_version": gate.INSTALLED_SCHEMA,
        "target": target,
        "bin_dir": f"/ci/work/extracted/{archive_name.removesuffix('.tar.gz')}/bin",
        "binary": {
            "path": "/ci/work/extracted/bin/rustqec",
            "sha256": "c" * 64,
            "version": f"rustqec {SELFTEST_TAG[1:]}",
            "advertised_decoders": ["envelope-matching", "envelope-mle"],
        },
        "archive": {"path": f"/ci/work/staged-release/{archive_name}", "sha256": archive_sha},
        "source_revision": SELFTEST_SHA,
        "ilp": {"expected": True, "available": True},
        "matrix": {
            "path": "/ci/work/tooling/docs/envelope-support.json",
            "sha256": matrix_sha,
            "schema_version": MATRIX_SCHEMA,
        },
        "controls": [
            {"decoder": "envelope-matching", "control": "mini", "status": "pass"},
            {"decoder": "envelope-mle", "control": "mini", "status": "pass"},
        ],
        "previous_fixture_semantics": "synthetic fixture",
        "status": "pass",
        "problems": [],
    }


def _fixture_mle_resources(scope_path: Path) -> dict[str, Any]:
    cases = []
    for workload in gate.mle_resource_campaign.WORKLOADS:
        shots = workload["shots"] or 1601
        cases.append({
            "id": workload["id"],
            "kind": workload["kind"],
            "real_circuit": workload["real_circuit"],
            "circuit_params": {
                "distance": workload["distance"],
                "rounds": workload["rounds"],
                "loss_rate": workload["loss"],
                "batch": workload["shots"],
            },
            "expected": {"outcome": "success"},
            "output_rule_problems": [],
            "wall_seconds": 1.0,
            "peak_rss_watermark_bytes": 1024,
            "exit_code": 0,
            "shots": shots,
            "completed_shots": shots,
            "predictions": {"installed": True},
            "stats_written": True,
            "cache": {
                "hits": 1,
                "eviction_rebuilds_observed": (
                    1 if workload["kind"] == "cache-eviction" else 0
                ),
            },
        })
    for case_id, (error_code, exit_code) in \
            gate.mle_resource_campaign.EXPECTED_FAILURE_CODES.items():
        stats_written = case_id != "fail-mle-candidate-limit"
        cases.append({
            "id": case_id,
            "kind": "failure-semantics",
            "expected": {
                "outcome": "rejection",
                "exit_code": exit_code,
                "error_code": error_code,
                "stats_written": stats_written,
            },
            "output_rule_problems": [],
            "wall_seconds": 1.0,
            "peak_rss_watermark_bytes": 1024,
            "exit_code": exit_code,
            "error_code": error_code,
            "completed_shots": 0,
            "predictions": {
                "installed": False,
                "pre_existing": False,
                "unchanged": False,
            },
            "stats_written": stats_written,
            "compilation_outside_timeout": True,
        })
    plan = json.loads(scope_path.read_text(encoding="utf-8"))
    return {
        "schema_version": gate.MLE_RESOURCES_SCHEMA,
        "checkout_revision": SELFTEST_SHA,
        "machine": {"platform": "synthetic"},
        "build": {"sha256": "f" * 64},
        "scope_plan": {
            "sha256": sha256_file(scope_path),
            "source_revision": plan["applies_to"]["source_revision"],
        },
        "stress_budget": {
            "declared_before_run": True,
            "values": gate.MLE_EXPECTED_BUDGET,
        },
        "total_wall_seconds": float(len(cases)),
        "cases": cases,
        "status": "pass",
        "problems": [],
    }


def make_selftest_fixture(
    base: Path,
    matching_decision: str = "supported",
    mle_decision: str = "beta",
) -> dict[str, Any]:
    """A complete synthetic candidate run: archives, manifest, evidence, gate."""
    for sub in ("release", "evidence", "retained/raw", "out"):
        (base / sub).mkdir(parents=True, exist_ok=True)

    archives: dict[str, Any] = {}
    checksum_lines = []
    for target, payload_bytes in (
        (REQUIRED_TARGETS[0], b"synthetic linux archive payload"),
        (REQUIRED_TARGETS[1], b"synthetic macos archive payload"),
    ):
        name = f"rustqec-{SELFTEST_TAG}-{target}.tar.gz"
        (base / "release" / name).write_bytes(payload_bytes)
        digest = sha256_bytes(payload_bytes)
        archives[name] = {
            "filename": name,
            "root_directory": name.removesuffix(".tar.gz"),
            "sha256": digest,
            "size": len(payload_bytes),
            "target": target,
            "compiler": {"host": target, "release": "1.90.0", "commit_hash": "d" * 40},
            "runtime": {"baseline": f"synthetic {target}", "linkage": ["system-only"]},
        }
        checksum_lines.append(f"{digest}  {name}")

    manifest = {
        "schema_version": verify_release_archive.SCHEMA,
        "tag": SELFTEST_TAG,
        "source_sha": SELFTEST_SHA,
        "packages": [
            {"name": "rustqec-cli", "version": SELFTEST_TAG[1:], "rust_version": "1.88"},
            {"name": "rstim", "version": SELFTEST_TAG[1:], "rust_version": "1.88"},
        ],
        "shot_lab_assets": {
            "rebuilt_from_tag": True,
            "manifest_sha256": "e" * 64,
            "manifest": {"format_version": "rstim-shot-assets-v1"},
        },
        "archives": archives,
    }
    (base / "release" / "release-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    (base / "release" / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n",
                                                 encoding="utf-8")

    matrix_path = base / "matrix.json"
    matrix_path.write_text(json.dumps(_fixture_matrix(), indent=2) + "\n", encoding="utf-8")
    matrix_sha = sha256_file(matrix_path)
    policy_path = base / "policy.md"
    policy_path.write_text("# synthetic compatibility policy\n", encoding="utf-8")
    policy_sha = sha256_file(policy_path)
    mle_scope_path = base / "envelope-mle-scope.json"
    shutil.copyfile(REPO_ROOT / gate.DEFAULT_MLE_SCOPE, mle_scope_path)
    scope_sha = sha256_file(mle_scope_path)

    for name, (filename, schema) in GENERATED_EVIDENCE.items():
        if name == "mle-resources":
            evidence = _fixture_mle_resources(mle_scope_path)
        else:
            evidence = {
                "schema_version": schema,
                "checkout_revision": SELFTEST_SHA,
                "status": "pass",
                "results": [],
                "cases": [],
            }
        if name == "correctness":
            evidence.update(_fixture_correctness())
        elif name == "resources":
            evidence["cases"] = [
                {
                    "id": f"{decoder}-{kind}",
                    "decoder": decoder,
                    "kind": kind,
                    "exit_code": 0,
                    "output_rule_problems": [],
                }
                for decoder in ("envelope-matching", "envelope-mle")
                for kind in ("workload", "cache-eviction")
            ]
        (base / "evidence" / filename).write_text(json.dumps(evidence, indent=2) + "\n",
                                                  encoding="utf-8")
    for target in REQUIRED_TARGETS:
        name = f"rustqec-{SELFTEST_TAG}-{target}.tar.gz"
        report = _fixture_installed(target, name, archives[name]["sha256"], matrix_sha)
        (base / "evidence" / f"installed-{target}.json").write_text(
            json.dumps(report, indent=2) + "\n", encoding="utf-8")

    raw_payload = b'{"synthetic": "raw observation"}'
    retained_manifest = {
        "schema_version": gate.RETAINED_MANIFEST_SCHEMA,
        "status": "pass",
        "checkout_revision": SELFTEST_RETAINED_SHA,
        "cases": [
            {"id": "matching-synthetic", "decoder": "envelope-matching",
             "raw": "raw/matching-synthetic.json", "raw_sha256": sha256_bytes(raw_payload)},
        ],
    }
    (base / "retained" / "manifest.json").write_text(
        json.dumps(retained_manifest, indent=2) + "\n", encoding="utf-8")
    (base / "retained" / "raw" / "matching-synthetic.json").write_bytes(raw_payload)
    (base / "retained" / "report.md").write_text("# synthetic resource report\n",
                                                encoding="utf-8")

    gate_path = base / "release-gate.json"
    gate_path.write_text(
        json.dumps(
            _fixture_gate_report(
                matrix_sha,
                policy_sha,
                scope_sha,
                matching_decision,
                mle_decision,
                base / "evidence",
                base / "retained" / "manifest.json",
            ),
            indent=2,
        ) + "\n",
        encoding="utf-8",
    )

    return {
        "tag": SELFTEST_TAG,
        "source_sha": SELFTEST_SHA,
        "release_manifest": base / "release" / "release-manifest.json",
        "archives_dir": base / "release",
        "evidence_dir": base / "evidence",
        "gate_report_path": gate_path,
        "matrix_path": matrix_path,
        "policy_path": policy_path,
        "mle_scope_path": mle_scope_path,
        "retained_dir": base / "retained",
        "out_dir": base / "out",
        "release_dir": base / "release",
    }


def stage_release_dir(fixture: dict[str, Any], destination: Path) -> Path:
    """A downloaded-assets directory: release files plus the evidence bundle."""
    destination.mkdir(parents=True, exist_ok=True)
    for path in fixture["release_dir"].iterdir():
        if path.is_file():
            shutil.copy2(path, destination / path.name)
    for path in fixture["out_dir"].iterdir():
        if path.is_file():
            shutil.copy2(path, destination / path.name)
    return destination


def repack_bundle(staged: Path, bundle_name: str, transform) -> None:
    """Mutate an extracted bundle like an attacker with full asset control.

    Internal checksums, the tarball and the sidecar are all regenerated, so
    only cross-record reconciliation can catch the edit.
    """
    root = bundle_name.removesuffix(".tar.gz")
    with tempfile.TemporaryDirectory(prefix="envelope-publication-repack-") as temporary:
        extracted = extract_bundle(staged / bundle_name, Path(temporary), root)
        transform(extracted)
        files = {
            path.relative_to(extracted).as_posix(): path.read_bytes()
            for path in extracted.rglob("*")
            if path.is_file() and path.name != "SHA256SUMS"
        }
        sums = "".join(
            f"{sha256_bytes(payload)}  {relative}\n"
            for relative, payload in sorted(files.items())
        )
        files["SHA256SUMS"] = sums.encode("utf-8")
        write_bundle(staged / bundle_name, root, files)
    (staged / f"{bundle_name}.sha256").write_text(
        f"{sha256_file(staged / bundle_name)}  {bundle_name}\n", encoding="utf-8")


def self_test() -> int:
    """The verifier must reject defective or over-claiming publications."""
    observations: list[dict[str, Any]] = []

    def observe(name: str, rejected: bool, detail: Any) -> bool:
        observations.append({"mutation": name, "rejected": rejected, "detail": detail})
        return rejected

    def expect_failure(name: str, staged: Path, needle: str,
                       expect: tuple[str, ...] = ("envelope-matching",)) -> bool:
        try:
            verify_release_dir(staged, expect)
        except PublicationError as error:
            return observe(name, needle in str(error), str(error))
        return observe(name, False, "verification unexpectedly passed")

    with tempfile.TemporaryDirectory(prefix="envelope-publication-selftest-") as temporary:
        work = Path(temporary)
        fixture = make_selftest_fixture(work / "known-good")
        try:
            bundle_path, _sidecar, publication = build_bundle(
                tag=fixture["tag"],
                source_sha=fixture["source_sha"],
                release_manifest=fixture["release_manifest"],
                archives_dir=fixture["archives_dir"],
                evidence_dir=fixture["evidence_dir"],
                gate_report_path=fixture["gate_report_path"],
                matrix_path=fixture["matrix_path"],
                policy_path=fixture["policy_path"],
                mle_scope_path=fixture["mle_scope_path"],
                retained_dir=fixture["retained_dir"],
                out_dir=fixture["out_dir"],
            )
            result = verify_release_dir(
                stage_release_dir(fixture, work / "audit-known-good"),
                ("envelope-matching",),
            )
            baseline = observe(
                "baseline",
                result["tag"] == SELFTEST_TAG
                and result["supported_decoders"] == ["envelope-matching"]
                and publication["supported_decoders"] == ["envelope-matching"],
                {"supported": result["supported_decoders"]},
            )
        except PublicationError as error:
            baseline = observe("baseline", False, str(error))
        if not baseline:
            print(json.dumps({"self_test_mutations": observations}, indent=2))
            print("FAIL envelope publication self-test: the known-good bundle did not verify",
                  file=sys.stderr)
            return 1
        bundle_name = bundle_path.name

        def fresh_stage(label: str) -> Path:
            return stage_release_dir(fixture, work / label)

        def swap_installed(bundle_root: Path) -> None:
            first = bundle_root / f"evidence/installed-{REQUIRED_TARGETS[0]}.json"
            second = bundle_root / f"evidence/installed-{REQUIRED_TARGETS[1]}.json"
            first.write_bytes(second.read_bytes())

        staged = fresh_stage("audit-swapped")
        repack_bundle(staged, bundle_name, swap_installed)
        swapped = expect_failure("swapped-installed-report", staged,
                                 "cannot be swapped between archives")

        def drop_installed(bundle_root: Path) -> None:
            (bundle_root / f"evidence/installed-{REQUIRED_TARGETS[1]}.json").unlink()

        staged = fresh_stage("audit-missing")
        repack_bundle(staged, bundle_name, drop_installed)
        missing = expect_failure("missing-platform-report", staged,
                                 "missing bundled installed-envelope report")

        def drop_mle_resources(bundle_root: Path) -> None:
            (bundle_root / "evidence/mle-resources.json").unlink()

        staged = fresh_stage("audit-missing-mle-resources")
        repack_bundle(staged, bundle_name, drop_mle_resources)
        missing_mle_resources = expect_failure(
            "missing-mle-resource-report", staged, "missing bundled mle-resources evidence"
        )

        def invalidate_mle_scope(bundle_root: Path) -> None:
            path = bundle_root / "scope/envelope-mle-scope.json"
            plan = json.loads(path.read_text(encoding="utf-8"))
            plan["schema_version"] = "invalid"
            path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-invalid-mle-scope")
        repack_bundle(staged, bundle_name, invalidate_mle_scope)
        invalid_mle_scope = expect_failure(
            "invalid-mle-scope-plan", staged, "bundled MLE scope plan is invalid"
        )

        def widen_scope(bundle_root: Path) -> None:
            path = bundle_root / "publication.json"
            record = json.loads(path.read_text(encoding="utf-8"))
            record["decoders"]["envelope-mle"]["maturity"] = "supported"
            record["supported_decoders"].append("envelope-mle")
            path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-scope")
        repack_bundle(staged, bundle_name, widen_scope)
        scope = expect_failure("scope-claim-without-evidence", staged,
                               "do not match the gate decision")

        def fail_gate(bundle_root: Path) -> None:
            path = bundle_root / "release-gate.json"
            report = json.loads(path.read_text(encoding="utf-8"))
            report["status"] = "fail"
            path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-failed-gate")
        repack_bundle(staged, bundle_name, fail_gate)
        failed_gate = expect_failure("failed-gate-with-supported-claim", staged,
                                     "passing gate decision")

        # The same publication path must accept the narrow MLE domain when the
        # release gate has actually promoted it and its scoped evidence verifies.
        mle_fixture = make_selftest_fixture(
            work / "mle-supported", mle_decision="supported"
        )
        try:
            _mle_bundle, _mle_sidecar, mle_publication = build_bundle(
                tag=mle_fixture["tag"],
                source_sha=mle_fixture["source_sha"],
                release_manifest=mle_fixture["release_manifest"],
                archives_dir=mle_fixture["archives_dir"],
                evidence_dir=mle_fixture["evidence_dir"],
                gate_report_path=mle_fixture["gate_report_path"],
                matrix_path=mle_fixture["matrix_path"],
                policy_path=mle_fixture["policy_path"],
                mle_scope_path=mle_fixture["mle_scope_path"],
                retained_dir=mle_fixture["retained_dir"],
                out_dir=mle_fixture["out_dir"],
            )
            mle_result = verify_release_dir(
                stage_release_dir(mle_fixture, work / "audit-mle-supported"),
                ("envelope-matching", "envelope-mle"),
            )
            mle_supported = observe(
                "mle-supported-baseline",
                mle_result["supported_decoders"]
                == ["envelope-matching", "envelope-mle"]
                and mle_publication["supported_decoders"]
                == ["envelope-matching", "envelope-mle"]
                and "exactly four measured workload points"
                in mle_publication["decoders"]["envelope-mle"]["scope_statement"],
                {
                    "supported": mle_result["supported_decoders"],
                    "scope": mle_publication["decoders"]["envelope-mle"]["scope_statement"],
                },
            )
        except PublicationError as error:
            mle_supported = observe("mle-supported-baseline", False, str(error))

        # v0.3.1 is a valid Matching-only publication from before the MLE
        # scope-plan and MLE-resource members existed. New verifiers must keep
        # accepting that immutable bundle for Matching, while refusing an MLE
        # expectation against it.
        def make_matching_only_legacy(bundle_root: Path) -> None:
            (bundle_root / "scope/envelope-mle-scope.json").unlink()
            (bundle_root / "evidence/mle-resources.json").unlink()
            gate_path = bundle_root / "release-gate.json"
            gate_record = json.loads(gate_path.read_text(encoding="utf-8"))
            gate_record.pop("mle_scope_plan", None)
            gate_record["evidence"].pop("mle-resources", None)
            gate_path.write_text(json.dumps(gate_record, indent=2) + "\n", encoding="utf-8")
            publication_path = bundle_root / "publication.json"
            publication_record = json.loads(publication_path.read_text(encoding="utf-8"))
            publication_record.pop("mle_scope", None)
            publication_record["evidence"].pop("mle-resources", None)
            mle_claim = publication_record["decoders"]["envelope-mle"]
            mle_claim.pop("scope_plan", None)
            mle_claim["scope_statement"] = _fixture_matrix()["release_candidate_scope"][
                "statement"
            ]
            publication_record["gate_report"]["sha256"] = sha256_file(gate_path)
            publication_path.write_text(
                json.dumps(publication_record, indent=2) + "\n", encoding="utf-8"
            )

        staged = fresh_stage("audit-matching-only-legacy")
        repack_bundle(staged, bundle_name, make_matching_only_legacy)
        try:
            legacy_result = verify_release_dir(staged, ("envelope-matching",))
            try:
                verify_release_dir(staged, ("envelope-mle",))
            except PublicationError as error:
                legacy_mle_rejected = "requires a bundled, gate-bound MLE scope" in str(error)
            else:
                legacy_mle_rejected = False
            matching_only_legacy = observe(
                "matching-only-legacy-bundle",
                legacy_result["supported_decoders"] == ["envelope-matching"]
                and legacy_mle_rejected,
                {"supported": legacy_result["supported_decoders"],
                 "mle_rejected": legacy_mle_rejected},
            )
        except PublicationError as error:
            matching_only_legacy = observe("matching-only-legacy-bundle", False, str(error))
        def hollow_correctness_keep_gate_hash(bundle_root: Path) -> None:
            evidence_path = bundle_root / "evidence/correctness.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["cases"] = []
            evidence["coverage"] = {}
            evidence_path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
            publication_path = bundle_root / "publication.json"
            record = json.loads(publication_path.read_text(encoding="utf-8"))
            record["evidence"]["correctness"]["sha256"] = sha256_file(evidence_path)
            publication_path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-hollow-correctness-bound")
        repack_bundle(staged, bundle_name, hollow_correctness_keep_gate_hash)
        correctness_bound = expect_failure(
            "hollow-correctness-does-not-match-gate-input",
            staged,
            "exact input inspected by the release gate",
        )

        def hollow_correctness_rewrite_gate_hash(bundle_root: Path) -> None:
            hollow_correctness_keep_gate_hash(bundle_root)
            evidence_path = bundle_root / "evidence/correctness.json"
            gate_path = bundle_root / "release-gate.json"
            report = json.loads(gate_path.read_text(encoding="utf-8"))
            report["evidence"]["correctness"]["sha256"] = sha256_file(evidence_path)
            gate_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            publication_path = bundle_root / "publication.json"
            record = json.loads(publication_path.read_text(encoding="utf-8"))
            record["gate_report"]["sha256"] = sha256_file(gate_path)
            publication_path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-hollow-correctness-rewritten-gate")
        repack_bundle(staged, bundle_name, hollow_correctness_rewrite_gate_hash)
        correctness_complete = expect_failure(
            "hollow-correctness-fails-completeness",
            staged,
            "completeness checks",
        )

        def rewrite_installed_report(bundle_root: Path) -> None:
            target = REQUIRED_TARGETS[0]
            report_path = bundle_root / f"evidence/installed-{target}.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["binary"]["version"] = "rustqec 0.0.0-rewritten"
            report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            publication_path = bundle_root / "publication.json"
            record = json.loads(publication_path.read_text(encoding="utf-8"))
            platform = next(item for item in record["platforms"] if item["target"] == target)
            platform["installed_report_sha256"] = sha256_file(report_path)
            publication_path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-rewritten-installed")
        repack_bundle(staged, bundle_name, rewrite_installed_report)
        installed_bound = expect_failure(
            "rewritten-installed-report-does-not-match-gate",
            staged,
            "portable content inspected by the release gate",
        )

        def rewrite_retained_raw(bundle_root: Path) -> None:
            raw_path = bundle_root / "retained-resources/raw/matching-synthetic.json"
            raw_path.write_bytes(b'{"synthetic": "rewritten observation"}')
            publication_path = bundle_root / "publication.json"
            record = json.loads(publication_path.read_text(encoding="utf-8"))
            relative = "retained-resources/raw/matching-synthetic.json"
            record["retained_resources"]["files"][relative] = sha256_file(raw_path)
            publication_path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")

        staged = fresh_stage("audit-rewritten-retained-raw")
        repack_bundle(staged, bundle_name, rewrite_retained_raw)
        retained_raw_bound = expect_failure(
            "rewritten-retained-raw-does-not-match-manifest",
            staged,
            "gate-bound manifest",
        )

        # A Beta-only legacy fixture must not satisfy --expect-decoder envelope-matching.
        legacy = make_selftest_fixture(work / "legacy-beta", matching_decision="beta")
        build_bundle(
            tag=legacy["tag"],
            source_sha=legacy["source_sha"],
            release_manifest=legacy["release_manifest"],
            archives_dir=legacy["archives_dir"],
            evidence_dir=legacy["evidence_dir"],
            gate_report_path=legacy["gate_report_path"],
            matrix_path=legacy["matrix_path"],
            policy_path=legacy["policy_path"],
            mle_scope_path=legacy["mle_scope_path"],
            retained_dir=legacy["retained_dir"],
            out_dir=legacy["out_dir"],
        )
        beta_only = expect_failure(
            "beta-only-legacy-fixture",
            stage_release_dir(legacy, work / "audit-legacy-beta"),
            "not Supported",
        )

        # A release directory without any evidence bundle (the v0.3.0 shape).
        no_bundle = work / "audit-no-bundle"
        no_bundle.mkdir()
        for path in fixture["release_dir"].iterdir():
            if path.is_file():
                shutil.copy2(path, no_bundle / path.name)
        legacy_missing = expect_failure("legacy-release-without-bundle", no_bundle,
                                        "lacks a verified envelope promotion")

    passed = all([
        baseline, swapped, missing, missing_mle_resources, invalid_mle_scope,
        scope, failed_gate, mle_supported, matching_only_legacy, beta_only, legacy_missing,
        correctness_bound,
        correctness_complete,
        installed_bound,
        retained_raw_bound,
    ])
    print(json.dumps({"self_test_mutations": observations}, indent=2))
    if passed:
        print("PASS envelope publication self-test")
        return 0
    print("FAIL envelope publication self-test: not all mutations were rejected",
          file=sys.stderr)
    return 1


# ---------------------------------------------------------------------------
# Command line
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-dir", type=Path,
                        help="directory of downloaded assets for one candidate/release")
    parser.add_argument("--expect-decoder", action="append", dest="expect_decoders",
                        default=[], help="decoder that must be Supported; repeatable")
    parser.add_argument(
        "--marker-out",
        type=Path,
        help="write a publication-verification marker after all downloaded assets pass",
    )
    parser.add_argument("--self-test", action="store_true")
    subparsers = parser.add_subparsers(dest="command")
    build_parser = subparsers.add_parser(
        "build", help="assemble the evidence bundle inside a candidate release run")
    build_parser.add_argument("--tag", required=True)
    build_parser.add_argument("--source-sha", required=True)
    build_parser.add_argument("--release-manifest", type=Path, required=True)
    build_parser.add_argument("--archives-dir", type=Path, required=True)
    build_parser.add_argument("--evidence-dir", type=Path, required=True)
    build_parser.add_argument("--gate-report", type=Path, required=True)
    build_parser.add_argument("--matrix", type=Path, default=gate.DEFAULT_MATRIX)
    build_parser.add_argument("--policy", type=Path, default=gate.DEFAULT_POLICY)
    build_parser.add_argument("--mle-scope", type=Path, default=gate.DEFAULT_MLE_SCOPE)
    build_parser.add_argument("--retained-resources", type=Path,
                              default=gate.DEFAULT_RETAINED_REPORT.parent)
    build_parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)

    if args.marker_out is not None:
        if args.self_test or args.command is not None:
            parser.error("--marker-out is valid only with --release-dir verification")
        if not args.expect_decoders:
            parser.error("--marker-out requires at least one --expect-decoder")

    if args.self_test:
        return self_test()

    if args.command == "build":
        try:
            bundle_path, sidecar_path, publication = build_bundle(
                tag=args.tag,
                source_sha=args.source_sha,
                release_manifest=args.release_manifest,
                archives_dir=args.archives_dir,
                evidence_dir=args.evidence_dir,
                gate_report_path=args.gate_report,
                matrix_path=args.matrix,
                policy_path=args.policy,
                mle_scope_path=args.mle_scope,
                retained_dir=args.retained_resources,
                out_dir=args.out_dir,
            )
        except PublicationError as error:
            print(f"FAIL envelope publication build: {error}", file=sys.stderr)
            return 1
        supported = ",".join(publication["supported_decoders"]) or "none"
        print(f"built {bundle_path.name} sha256={sha256_file(bundle_path)} "
              f"supported={supported}")
        print(f"checksum {sidecar_path.name}")
        return 0

    if args.release_dir is None:
        print("FAIL envelope published support: --release-dir is required "
              "(or use --self-test / the build subcommand)", file=sys.stderr)
        return 2
    try:
        result = verify_release_dir(args.release_dir, tuple(args.expect_decoders))
    except PublicationError as error:
        print(f"FAIL envelope published support: {error}", file=sys.stderr)
        return 1
    print_summary(result, tuple(args.expect_decoders))
    if args.marker_out is not None:
        write_verification_marker(result, args.marker_out)
        print(f"wrote publication verification marker {args.marker_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
