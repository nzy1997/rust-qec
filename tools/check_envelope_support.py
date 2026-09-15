#!/usr/bin/env python3
"""Verify the executable envelope-decoder support matrix against a real CLI.

Runs every control declared in ``docs/envelope-support.json`` against the
requested ``rustqec`` binary: acceptance controls must succeed with the pinned
predictions/statistics, rejection controls must fail with the declared
structured error code, exit code and output-file rules. Missing ILP support in
the tested binary fails the run instead of silently skipping MLE controls.

Usage:
    python3 tools/check_envelope_support.py \
        --binary target/release/rustqec \
        --matrix docs/envelope-support.json \
        --out drafts/envelope-readiness/support.json
    python3 tools/check_envelope_support.py --self-test
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "rustqec.envelope-support.v1"
RESULT_SCHEMA_VERSION = "rustqec.envelope-support-result.v1"
DEFAULT_BINARY = Path("target/release/rustqec")
DEFAULT_MATRIX = Path("docs/envelope-support.json")

# Structural completeness: every decoder must keep these controls. Removing
# one of them invalidates the matrix before any execution (self-test control).
REQUIRED_CONTROLS: dict[str, tuple[str, ...]] = {
    "envelope-matching": (
        "mini-circuit-known-answer",
        "midswap-fixture-matching-acceptance",
        "conventional-fixture-matching-acceptance",
        "repeat-block-rejection",
        "terminal-ml-rejection",
        "observable-limit-rejection",
    ),
    "envelope-mle": (
        "mini-circuit-known-answer",
        "midswap-canonical-mle-known-answer",
        "conventional-mle-candidate-explosion-rejection",
        "mle-solve-timeout",
        "mle-infeasible-shot",
    ),
}


class MatrixError(Exception):
    """The matrix itself is invalid; raised before executing controls."""


class ControlMismatch(Exception):
    """One executed control disagreed with its declared expectation."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load_matrix(path: Path) -> dict[str, Any]:
    matrix = json.loads(path.read_text(encoding="utf-8"))
    validate_matrix(matrix)
    return matrix


def validate_matrix(matrix: dict[str, Any]) -> None:
    if matrix.get("schema_version") != SCHEMA_VERSION:
        raise MatrixError(f"unsupported matrix schema_version: {matrix.get('schema_version')!r}")
    decoders = matrix.get("decoders")
    if not isinstance(decoders, dict) or not decoders:
        raise MatrixError("matrix must declare at least one decoder")
    for name, decoder in decoders.items():
        for field in ("required_build_features", "current_maturity", "objective"):
            if field not in decoder:
                raise MatrixError(f"decoder {name}: missing field {field}")
    controls = matrix.get("controls")
    if not isinstance(controls, list) or not controls:
        raise MatrixError("matrix must declare controls")
    ids: set[str] = set()
    for control in controls:
        cid = control.get("id")
        if not cid or cid in ids:
            raise MatrixError(f"duplicate or missing control id: {cid!r}")
        ids.add(cid)
        declared = control.get("decoders")
        if not declared or any(name not in decoders for name in declared):
            raise MatrixError(f"control {cid}: declares unknown decoder(s) {declared!r}")
        expected = control.get("expected", {})
        outcome = expected.get("outcome")
        if outcome not in ("success", "rejection"):
            raise MatrixError(f"control {cid}: expected.outcome must be success|rejection")
        if outcome == "success" and not (
            "predictions_hex" in expected or "predictions_sha256" in expected
        ):
            raise MatrixError(f"control {cid}: success controls must pin predictions")
        if outcome == "rejection":
            for field in ("error_code", "exit_code", "predictions_written", "stats_written"):
                if field not in expected:
                    raise MatrixError(f"control {cid}: rejection controls must declare {field}")
    for decoder, required in REQUIRED_CONTROLS.items():
        if decoder not in decoders:
            raise MatrixError(f"required decoder missing from matrix: {decoder}")
        missing = [
            cid
            for cid in required
            if not any(c["id"] == cid and decoder in c["decoders"] for c in controls)
        ]
        if missing:
            raise MatrixError(
                f"decoder {decoder}: required control(s) missing from matrix: {', '.join(missing)}"
            )


def dataset_id(circuit_sha: str, shots_sha: str, shots: int, row_bits: int) -> str:
    material = (
        "format=rstim_decoder_dataset\n"
        "schema_version=1\n"
        "mode=measurements_blinded\n"
        f"circuit_sha256={circuit_sha}\n"
        f"shots={shots}\n"
        f"row_bits={row_bits}\n"
        f"shots_b8_sha256={shots_sha}\n"
    )
    return sha256_bytes(material.encode())


def circuit_stats(binary: Path, circuit_path: Path, work: Path) -> dict[str, int]:
    result = subprocess.run(
        [str(binary), "circuit", "stats", "--format", "json", "--in", str(circuit_path)],
        capture_output=True,
        text=True,
        cwd=work,
        check=False,
    )
    if result.returncode:
        raise MatrixError(f"circuit stats failed for {circuit_path}: {result.stderr.strip()}")
    payload = json.loads(result.stdout)
    return payload["result"]


def apply_manifest_patch(manifest: dict[str, Any], patch: dict[str, Any]) -> None:
    for dotted, value in patch.items():
        target = manifest
        keys = dotted.split(".")
        for key in keys[:-1]:
            target = target[key]
        target[keys[-1]] = value


def materialize_dataset(
    binary: Path, spec: dict[str, Any], root: Path
) -> tuple[Path, dict[str, Any]]:
    """Build a dataset directory; returns (path, file hash record)."""
    kind = spec.get("type")
    dataset = root / "dataset"
    if kind == "fixture":
        source = REPO_ROOT / spec["path"]
        if not source.is_dir():
            raise MatrixError(f"fixture dataset missing: {source}")
        hashes = {
            name: sha256_file(source / name)
            for name in ("circuit.stim", "shots.b8", "manifest.json")
        }
        return source, {"source": str(source.relative_to(REPO_ROOT)), "sha256": hashes}

    dataset.mkdir(parents=True)
    variants: list[tuple[str, str]] = []
    if kind == "inline":
        circuit_text = spec["circuit"]
        shots = bytes.fromhex(spec["shots_hex"])
    elif kind == "derived":
        circuit_text = (REPO_ROOT / spec["circuit_file"]).read_text(encoding="utf-8")
        circuit_text += spec.get("append", "")
        shot_count = spec.get("shots", 1)
        stats = circuit_stats(binary, write_text(root, "probe.stim", circuit_text), root)
        stride = (stats["num_measurements"] + 7) // 8
        shots = bytes(stride * shot_count)
    elif kind == "variants":
        raise MatrixError("variants datasets are expanded by the caller")
    else:
        raise MatrixError(f"unknown dataset type: {kind!r}")

    circuit_path = write_text(dataset, "circuit.stim", circuit_text)
    stats = circuit_stats(binary, circuit_path, root)
    bits = stats["num_measurements"]
    stride = (bits + 7) // 8
    if len(shots) % stride:
        raise MatrixError("shots payload is not a whole number of rows")
    (dataset / "shots.b8").write_bytes(shots)
    circuit_sha, shots_sha = sha256_file(circuit_path), sha256_bytes(shots)
    manifest = {
        "format": "rstim_decoder_dataset",
        "schema_version": 1,
        "dataset_id": dataset_id(circuit_sha, shots_sha, len(shots) // stride, bits),
        "mode": "measurements_blinded",
        "shots": len(shots) // stride,
        "row": {
            "kind": "measurements",
            "bits": bits,
            "encoding": "b8",
            "bit_order": "lsb_first",
            "bytes_per_shot": stride,
        },
        "circuit": {
            "file": "circuit.stim",
            "sha256": circuit_sha,
            "measurements": stats["num_measurements"],
            "detectors": stats["num_detectors"],
            "observables": stats["num_observables"],
            "sweep_bits": stats["num_sweep_bits"],
        },
        "shots_file": {
            "file": "shots.b8",
            "sha256": shots_sha,
            "bits": bits,
            "bytes_per_shot": stride,
        },
    }
    apply_manifest_patch(manifest, spec.get("manifest_patch", {}))
    (dataset / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    del variants
    return dataset, {
        "source": "inline",
        "sha256": {
            "circuit.stim": circuit_sha,
            "shots.b8": shots_sha,
            "manifest.json": sha256_file(dataset / "manifest.json"),
        },
    }


def write_text(root: Path, name: str, text: str) -> Path:
    path = root / name
    path.write_text(text, encoding="utf-8")
    return path


def expand_controls(matrix: dict[str, Any]) -> list[dict[str, Any]]:
    """Expand variant datasets into per-variant controls with stable ids."""
    expanded: list[dict[str, Any]] = []
    for control in matrix["controls"]:
        dataset = control["dataset"]
        if dataset.get("type") != "variants":
            expanded.append(control)
            continue
        for variant in dataset["variants"]:
            old, new = variant["replace"]
            if old not in dataset["base_circuit"]:
                raise MatrixError(f"control {control['id']}: variant {variant['id']} base mismatch")
            expanded.append(
                {
                    **{k: v for k, v in control.items() if k != "dataset"},
                    "id": f"{control['id']}:{variant['id']}",
                    "dataset": {
                        "type": "inline",
                        "circuit": dataset["base_circuit"].replace(old, new),
                        "shots_hex": dataset["shots_hex"],
                    },
                }
            )
    return expanded


def advertised_decoders(binary: Path, work: Path) -> list[str]:
    result = subprocess.run(
        [str(binary), "capabilities", "--format", "json"],
        capture_output=True,
        text=True,
        cwd=work,
        check=False,
    )
    if result.returncode:
        raise MatrixError(f"capabilities failed: {result.stderr.strip()}")
    commands = json.loads(result.stdout)["commands"]
    decode = next((c for c in commands if c.get("name") == "decode"), None)
    if decode is None:
        raise MatrixError("binary does not advertise the decode command")
    return list(decode.get("decoders", []))


def run_decode(
    binary: Path, decoder: str, dataset: Path, arguments: dict[str, Any], work: Path
) -> dict[str, Any]:
    out_dir = work / "out"
    out_dir.mkdir(exist_ok=True)
    predictions = out_dir / f"{decoder}.b8"
    stats = out_dir / f"{decoder}.json"
    argv = [
        str(binary),
        "decode",
        "--decoder",
        decoder,
        "--dataset",
        str(dataset),
        "--out",
        str(predictions),
        "--stats-out",
        str(stats),
    ]
    if "shot_timeout_ms" in arguments:
        argv += ["--shot-timeout-ms", str(arguments["shot_timeout_ms"])]
    result = subprocess.run(argv, capture_output=True, text=True, cwd=work, check=False)
    envelope: dict[str, Any] | None = None
    if result.stderr.strip():
        try:
            envelope = json.loads(result.stderr)
        except json.JSONDecodeError:
            envelope = None
    observed: dict[str, Any] = {
        "exit_code": result.returncode,
        "stdout_bytes": len(result.stdout),
        "predictions_written": predictions.is_file(),
        "stats_written": stats.is_file(),
    }
    if predictions.is_file():
        payload = predictions.read_bytes()
        observed["predictions_sha256"] = sha256_bytes(payload)
        observed["predictions_hex"] = payload.hex()
    if stats.is_file():
        observed["stats"] = json.loads(stats.read_text(encoding="utf-8"))
    if envelope is not None:
        observed["error"] = envelope.get("error", {})
    return observed


def compare_stats(
    declared: dict[str, Any], observed: dict[str, Any], label: str
) -> list[str]:
    problems = []
    for key, value in declared.items():
        if key not in observed:
            problems.append(f"{label}: stats field {key} missing")
        elif observed[key] != value:
            problems.append(f"{label}: stats field {key}: expected {value!r}, observed {observed[key]!r}")
    return problems


def evaluate(
    control: dict[str, Any], decoder: str, observed: dict[str, Any]
) -> tuple[str, list[str]]:
    expected = control["expected"]
    problems: list[str] = []
    label = f"{control['id']} [{decoder}]"
    if expected["outcome"] == "success":
        if observed["exit_code"] != 0:
            problems.append(f"{label}: expected success, observed exit {observed['exit_code']}")
        if observed["stdout_bytes"]:
            problems.append(f"{label}: unexpected stdout on success")
        if "error" in observed:
            problems.append(f"{label}: unexpected error envelope on success")
        if not observed["predictions_written"]:
            problems.append(f"{label}: predictions missing on success")
        else:
            if "predictions_hex" in expected and observed["predictions_hex"] != expected["predictions_hex"]:
                problems.append(
                    f"{label}: known answer mismatch: expected {expected['predictions_hex']}, "
                    f"observed {observed['predictions_hex']}"
                )
            if "predictions_sha256" in expected and observed["predictions_sha256"] != expected["predictions_sha256"]:
                problems.append(
                    f"{label}: prediction hash mismatch: expected {expected['predictions_sha256']}, "
                    f"observed {observed['predictions_sha256']}"
                )
        if not observed["stats_written"]:
            problems.append(f"{label}: stats missing on success")
        else:
            stats = observed["stats"]
            if stats.get("schema_version") != "rustqec.decode-stats.v1":
                problems.append(f"{label}: unexpected stats schema {stats.get('schema_version')!r}")
            if stats.get("decoder") != decoder:
                problems.append(f"{label}: stats decoder field {stats.get('decoder')!r}")
            problems += compare_stats(expected.get("stats", {}), stats, label)
            problems += compare_stats(
                expected.get("decoder_stats", {}).get(decoder, {}), stats, label
            )
    else:
        if observed["exit_code"] != expected["exit_code"]:
            problems.append(
                f"{label}: expected exit {expected['exit_code']}, observed {observed['exit_code']}"
            )
        error = observed.get("error")
        if error is None:
            problems.append(f"{label}: missing structured error envelope on stderr")
        else:
            if error.get("code") != expected["error_code"]:
                problems.append(
                    f"{label}: expected error code {expected['error_code']}, "
                    f"observed {error.get('code')!r}"
                )
            needle = expected.get("message_contains")
            if needle and needle not in str(error.get("message", "")):
                problems.append(f"{label}: error message does not contain {needle!r}")
        for artifact in ("predictions", "stats"):
            want = expected[f"{artifact}_written"]
            got = observed[f"{artifact}_written"]
            if want != got:
                problems.append(f"{label}: {artifact}_written expected {want}, observed {got}")
        if observed.get("stats_written") and "stats" in expected:
            problems += compare_stats(expected["stats"], observed["stats"], label)
    return ("pass" if not problems else "fail"), problems


def run_checks(
    binary: Path, matrix: dict[str, Any], only: str | None = None
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="envelope-support-") as temporary:
        work = Path(temporary)
        available = advertised_decoders(binary, work)
        missing = [name for name in matrix["decoders"] if name not in available]
        if missing:
            raise MatrixError(
                "binary does not advertise decoder(s) required by the matrix: "
                + ", ".join(missing)
                + "; build with the declared features (e.g. --features ilp) "
                "instead of silently skipping controls"
            )
        binary_record = {
            "path": str(binary),
            "sha256": sha256_file(binary),
            "version": subprocess.run(
                [str(binary), "--version"], capture_output=True, text=True, check=False
            ).stdout.strip(),
            "advertised_decoders": available,
        }
        results = []
        problems: list[str] = []
        for control in expand_controls(matrix):
            if only is not None and control["id"] != only and not control["id"].startswith(only + ":"):
                continue
            for decoder in control["decoders"]:
                run_root = work / f"run-{len(results)}"
                run_root.mkdir()
                dataset_path, dataset_record = materialize_dataset(
                    binary, control["dataset"], run_root
                )
                observed = run_decode(
                    binary, decoder, dataset_path, control.get("arguments", {}), run_root
                )
                status, issues = evaluate(control, decoder, observed)
                problems += issues
                record = {
                    "control": control["id"],
                    "decoder": decoder,
                    "dataset": dataset_record,
                    "expected_outcome": control["expected"]["outcome"],
                    "observed": observed,
                    "status": status,
                }
                if issues:
                    record["problems"] = issues
                if "known_answer" in control["expected"]:
                    record["known_answer_source"] = control["expected"]["known_answer"]["source"]
                results.append(record)
        return {
            "schema_version": RESULT_SCHEMA_VERSION,
            "matrix": {
                "schema_version": matrix["schema_version"],
                "applies_to": matrix["applies_to"],
                "controls_declared": len(matrix["controls"]),
                "controls_executed": len({r['control'] for r in results}),
            },
            "binary": binary_record,
            "source_revision": matrix["applies_to"].get("source_revision"),
            "results": results,
            "status": "pass" if not problems else "fail",
            "problems": problems,
        }


def write_report(report: dict[str, Any], out: Path) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def self_test(binary: Path, matrix_path: Path) -> int:
    """Prove the ordinary checker rejects three mutated matrices/results."""
    matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
    observations: list[dict[str, str]] = []

    def expect_rejection(name: str, mutated: dict[str, Any], only: str | None, needle: str) -> bool:
        with tempfile.TemporaryDirectory(prefix="envelope-support-selftest-") as temporary:
            mutated_path = Path(temporary) / "matrix.json"
            mutated_path.write_text(json.dumps(mutated, indent=2), encoding="utf-8")
            try:
                validate_matrix(mutated)
                report = run_checks(binary, mutated, only=only)
                ok = report["status"] == "fail" and any(
                    needle in problem for problem in report["problems"]
                )
                detail = "; ".join(report["problems"][:2])
            except MatrixError as error:
                ok = needle in str(error)
                detail = str(error)
        observations.append({"mutation": name, "rejected": str(ok), "detail": detail})
        return ok

    # Mutation 1: declare a rejection control as an acceptance.
    mutated = copy.deepcopy(matrix)
    target = next(c for c in mutated["controls"] if c["id"] == "conventional-mle-candidate-explosion-rejection")
    target["expected"] = {"outcome": "success", "predictions_sha256": "0" * 64}
    first = expect_rejection(
        "rejection-declared-as-acceptance",
        mutated,
        "conventional-mle-candidate-explosion-rejection",
        "expected success",
    )

    # Mutation 2: remove one decoder's required control.
    mutated = copy.deepcopy(matrix)
    mutated["controls"] = [
        c for c in mutated["controls"] if c["id"] != "midswap-canonical-mle-known-answer"
    ]
    second = expect_rejection(
        "required-control-removed", mutated, None, "required control(s) missing"
    )

    # Mutation 3: change an expected known answer.
    mutated = copy.deepcopy(matrix)
    target = next(c for c in mutated["controls"] if c["id"] == "mini-circuit-known-answer")
    target["expected"]["predictions_hex"] = "00000000"
    third = expect_rejection(
        "expected-answer-changed", mutated, "mini-circuit-known-answer", "known answer mismatch"
    )

    passed = first and second and third
    print(json.dumps({"self_test_mutations": observations}, indent=2))
    if passed:
        print("PASS envelope support self-test")
        return 0
    print("FAIL envelope support self-test: not all mutated matrices were rejected")
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--only", help="run a single control id (prefix for variant controls)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    binary = args.binary.resolve()
    if args.self_test:
        if not binary.is_file():
            print(f"missing binary for self-test: {binary}", file=sys.stderr)
            return 2
        return self_test(binary, args.matrix)

    try:
        matrix = load_matrix(args.matrix)
        report = run_checks(binary, matrix, only=args.only)
    except MatrixError as error:
        print(f"FAIL envelope support matrix: {error}", file=sys.stderr)
        if args.out:
            write_report(
                {
                    "schema_version": RESULT_SCHEMA_VERSION,
                    "status": "fail",
                    "problems": [str(error)],
                },
                args.out,
            )
        return 1
    if args.out:
        write_report(report, args.out)
    if report["status"] == "pass":
        executed = len(report["results"])
        print(f"PASS envelope support matrix controls={executed}")
        return 0
    print("FAIL envelope support matrix", file=sys.stderr)
    for problem in report["problems"]:
        print(f"  - {problem}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
