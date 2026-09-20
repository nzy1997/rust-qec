#!/usr/bin/env python3
"""Generate the website CLI reference directly from ``rustqec capabilities``."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess


SCHEMA = "rustqec.cli.v1"
REPO = Path(__file__).resolve().parent.parent
ERROR_CHANNELS = {"stdout", "stderr"}


def _string_list(value: object, field: str, scope: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{scope}.{field} must be a list of non-empty strings")
    return value


def _non_empty_string(value: object, field: str, scope: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{scope}.{field} must be a non-empty string")
    return value


def _normalize_argument(argument: object, scope: str, flags: set[str]) -> dict:
    if not isinstance(argument, dict):
        raise ValueError(f"{scope} must be an object")
    name = _non_empty_string(argument.get("name"), "name", scope)
    flag = _non_empty_string(argument.get("flag"), "flag", scope)
    if flag in flags:
        raise ValueError(f"{scope}.flag must be a unique non-empty string")
    flags.add(flag)
    required = argument.get("required")
    if not isinstance(required, bool):
        raise ValueError(f"{scope}.required must be a boolean")
    default = argument.get("default")
    if isinstance(default, (dict, list)):
        raise ValueError(f"{scope}.default must be a scalar or null")
    return {
        "name": name,
        "flag": flag,
        "required": required,
        "values": _string_list(argument.get("values"), "values", scope),
        "default": default,
    }


def normalize(document: object) -> dict:
    if not isinstance(document, dict) or document.get("schema_version") != SCHEMA:
        raise ValueError(f"capabilities must declare schema_version {SCHEMA}")
    commands = document.get("commands")
    globals_ = document.get("global_arguments")
    if not isinstance(commands, list) or not commands or not isinstance(globals_, list):
        raise ValueError("capabilities must include commands and global_arguments arrays")

    global_flags: set[str] = set()
    normalized_globals = [
        _normalize_argument(argument, f"global_arguments[{index}]", global_flags)
        for index, argument in enumerate(globals_)
    ]

    normalized_commands = []
    names: set[str] = set()
    exit_codes: dict[int, dict[str, set[str]]] = {}

    def record_exit(code: int, command: str, error: str | None = None,
                    channel: str | None = None) -> None:
        detail = exit_codes.setdefault(code, {"commands": set(), "errors": set(), "channels": set()})
        detail["commands"].add(command)
        if error:
            detail["errors"].add(error)
        if channel:
            detail["channels"].add(channel)

    for index, command in enumerate(commands):
        scope = f"commands[{index}]"
        if not isinstance(command, dict):
            raise ValueError(f"{scope} must be an object")
        name = command.get("name")
        if not isinstance(name, str) or not name or name in names:
            raise ValueError(f"{scope}.name must be a unique non-empty string")
        names.add(name)
        argv = _string_list(command.get("argv"), "argv", scope)
        if not argv:
            raise ValueError(f"{scope}.argv must not be empty")
        arguments = command.get("arguments")
        errors = command.get("errors")
        artifacts = command.get("artifacts", [])
        decoders = command.get("decoders", [])
        if not isinstance(arguments, list) or not isinstance(errors, list) or not isinstance(artifacts, list):
            raise ValueError(f"{scope} arguments, errors, and artifacts must be arrays")
        if not isinstance(decoders, list):
            raise ValueError(f"{scope}.decoders must be an array")

        normalized_arguments = []
        flags: set[str] = set()
        for arg_index, argument in enumerate(arguments):
            arg_scope = f"{scope}.arguments[{arg_index}]"
            normalized_arguments.append(_normalize_argument(argument, arg_scope, flags))

        normalized_errors = []
        error_codes: set[str] = set()
        for error_index, error in enumerate(errors):
            error_scope = f"{scope}.errors[{error_index}]"
            if not isinstance(error, dict):
                raise ValueError(f"{error_scope} must be an object")
            error_code = _non_empty_string(error.get("code"), "code", error_scope)
            if error_code in error_codes:
                raise ValueError(f"{error_scope}.code must be unique within the command")
            error_codes.add(error_code)
            exit_code = error.get("exit_code")
            if not isinstance(exit_code, int) or isinstance(exit_code, bool):
                raise ValueError(f"{error_scope}.exit_code must be an integer")
            channel = error.get("channel")
            if channel not in ERROR_CHANNELS:
                raise ValueError(f"{error_scope}.channel must be stdout or stderr")
            record_exit(exit_code, name, error_code, channel)
            normalized_errors.append({
                "code": error_code,
                "exit_code": exit_code,
                "channel": channel,
            })

        normalized_artifacts = []
        artifact_names: set[str] = set()
        artifact_flags: set[str] = set()
        for artifact_index, artifact in enumerate(artifacts):
            artifact_scope = f"{scope}.artifacts[{artifact_index}]"
            if not isinstance(artifact, dict):
                raise ValueError(f"{artifact_scope} must be an object")
            artifact_name = _non_empty_string(artifact.get("name"), "name", artifact_scope)
            artifact_flag = _non_empty_string(artifact.get("flag"), "flag", artifact_scope)
            artifact_format = _non_empty_string(artifact.get("format"), "format", artifact_scope)
            if artifact_name in artifact_names or artifact_flag in artifact_flags:
                raise ValueError(f"{artifact_scope} name and flag must be unique within the command")
            artifact_names.add(artifact_name)
            artifact_flags.add(artifact_flag)
            normalized_artifacts.append({
                "name": artifact_name,
                "flag": artifact_flag,
                "format": artifact_format,
            })

        success_exit_code = command.get("success_exit_code")
        if not isinstance(success_exit_code, int) or isinstance(success_exit_code, bool):
            raise ValueError(f"{scope}.success_exit_code must be an integer")
        record_exit(success_exit_code, name)
        normalized_commands.append({
            "name": name,
            "anchor": "command-" + name.replace(".", "-"),
            "command_line": "rustqec " + " ".join(argv),
            "input_sources": _string_list(command.get("input_sources"), "input_sources", scope),
            "formats": _string_list(command.get("formats"), "formats", scope),
            "output_schema": _non_empty_string(command.get("output_schema"), "output_schema", scope),
            "success_exit_code": success_exit_code,
            "arguments": normalized_arguments,
            "errors": normalized_errors,
            "artifacts": normalized_artifacts,
            "decoders": _string_list(decoders, "decoders", scope),
        })

    normalized_exit_codes = []
    for value, detail in sorted(exit_codes.items()):
        normalized_exit_codes.append({
            "value": value,
            "status": "success" if not detail["errors"] else "error",
            "commands": sorted(detail["commands"]),
            "errors": sorted(detail["errors"]),
            "channels": sorted(detail["channels"]),
        })

    return {
        "schema_version": SCHEMA,
        "source_command": "rustqec capabilities --format json",
        "global_arguments": normalized_globals,
        "commands": normalized_commands,
        "exit_codes": normalized_exit_codes,
    }


def capabilities(binary: Path | None, repo_root: Path) -> dict:
    command = ([str(binary), "capabilities", "--format", "json"] if binary else [
        "cargo", "run", "--quiet", "--locked", "-p", "rustqec-cli", "--",
        "capabilities", "--format", "json",
    ])
    result = subprocess.run(command, cwd=repo_root, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"capabilities command failed: {result.stderr.strip()}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("capabilities command did not return JSON") from error


def write_reference(document: object, output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(normalize(document), indent=2, ensure_ascii=False) + "\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path, default=REPO / "site/generated/cli-reference.json")
    args = parser.parse_args()
    write_reference(capabilities(args.binary, args.repo_root), args.output)


if __name__ == "__main__":
    main()
