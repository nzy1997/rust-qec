from __future__ import annotations

import hashlib
import re
import tomllib
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any, Iterable


SCHEMA_VERSION = 2
SUITE = "rstim_vs_stim_simulator"
EXPECTED_BUNDLE_IDS = (
    "fair-cli-release",
    "compiled-steady-release",
    "reference-build-release",
    "frame-instruction-wide-release",
)

_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_POSIX_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])/(?!/)")
_WINDOWS_ABSOLUTE_RE = re.compile(r"(^|[\s\"'=,:\[\(\{;|&<>])([A-Za-z]:[\\/]|\\\\)")
_RUNTIME_IDENTITY_FIELDS = frozenset({"role", "version", "basename", "sha256"})
_CHECKED_COMMAND_FIELDS = frozenset({"name", "argv"})
_CHECKED_PROVENANCE_FIELDS = frozenset({"name", "value"})


def load_catalog(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        catalog = tomllib.load(handle)
    if not isinstance(catalog, dict):
        raise ValueError("catalog root must be a TOML table")
    return catalog


def validate_catalog(catalog: dict[str, Any], catalog_path: Path) -> list[str]:
    errors: list[str] = []
    catalog_path = catalog_path.resolve()
    repo_root = catalog_path.parents[2]

    if catalog.get("schema") != SCHEMA_VERSION:
        errors.append(f"catalog schema must be {SCHEMA_VERSION}")
    if catalog.get("suite") != SUITE:
        errors.append(f'suite must be "{SUITE}"')

    bundles = catalog.get("bundles")
    if not isinstance(bundles, list):
        errors.append('catalog field "bundles" must be an array')
        return errors

    bundle_ids = tuple(bundle.get("id") for bundle in bundles if isinstance(bundle, dict))
    if bundle_ids != EXPECTED_BUNDLE_IDS:
        errors.append(f"bundle IDs must be exactly: {', '.join(EXPECTED_BUNDLE_IDS)}")

    for index, raw_bundle in enumerate(bundles):
        bundle_label = f"bundle[{index}]"
        if not isinstance(raw_bundle, dict):
            errors.append(f"{bundle_label} must be a TOML table")
            continue
        bundle_id = raw_bundle.get("id")
        if isinstance(bundle_id, str) and bundle_id:
            bundle_label = f'bundle "{bundle_id}"'
        else:
            errors.append(f'{bundle_label} field "id" must be a non-empty string')

        bundle_root = _validate_catalog_path(
            raw_bundle.get("bundle_path"),
            repo_root,
            "bundle path",
            errors,
        )
        _validate_path_hash_entries(
            raw_bundle.get("repository_inputs"),
            repo_root,
            "repository_inputs",
            "repository",
            errors,
        )
        artifact_paths = _validate_path_hash_entries(
            raw_bundle.get("artifacts"),
            bundle_root,
            "artifacts",
            "bundle artifact",
            errors,
        )
        _validate_artifact_completeness(artifact_paths, bundle_root, errors)

        logical_roles = _validate_logical_executables(raw_bundle.get("logical_executables"), bundle_label, errors)
        runtime_roles = _validate_runtime_identities(raw_bundle.get("runtime_identities"), bundle_label, errors)
        _validate_checked_commands(raw_bundle.get("checked_commands"), logical_roles, runtime_roles, bundle_label, errors)
        _validate_checked_provenance(raw_bundle.get("checked_provenance"), bundle_label, errors)

    return errors


def _validate_catalog_path(
    raw_path: object,
    root: Path,
    path_label: str,
    errors: list[str],
) -> Path | None:
    path = _validate_relative_posix_path(raw_path, path_label, errors)
    if path is None:
        return None
    return root / path


def _validate_relative_posix_path(
    raw_path: object,
    path_label: str,
    errors: list[str],
) -> PurePosixPath | None:
    if not isinstance(raw_path, str) or raw_path == "":
        errors.append(f"{path_label} path must be relative POSIX path")
        return None

    posix_path = PurePosixPath(raw_path)
    parts = raw_path.split("/")
    if (
        posix_path.is_absolute()
        or PureWindowsPath(raw_path).drive
        or "\\" in raw_path
        or any(part in {"", ".", ".."} for part in parts)
    ):
        errors.append(f"{path_label} path must be relative POSIX path")
        return None

    return posix_path


def _validate_path_hash_entries(
    raw_entries: object,
    base_dir: Path | None,
    field: str,
    path_label: str,
    errors: list[str],
) -> set[PurePosixPath]:
    relative_paths: set[PurePosixPath] = set()
    entries = list(_path_hash_entries(raw_entries, field, errors))
    if raw_entries is None:
        errors.append(f'field "{field}" must be an array or table')
        return relative_paths

    for entry_label, raw_path, raw_sha256 in entries:
        relative_path = _validate_relative_posix_path(raw_path, path_label, errors)
        _validate_sha256(raw_sha256, f"{entry_label} sha256", errors)
        if relative_path is not None:
            relative_paths.add(relative_path)
        if relative_path is None or not isinstance(raw_sha256, str):
            continue

        if base_dir is None:
            continue
        target = base_dir / relative_path
        try:
            actual_sha256 = _sha256_file(target)
        except OSError as error:
            errors.append(f"{entry_label} file could not be read: {error}")
            continue
        if actual_sha256 != raw_sha256:
            errors.append(f"{entry_label} sha256 mismatch")

    return relative_paths


def _validate_artifact_completeness(
    catalog_paths: set[PurePosixPath],
    bundle_root: Path | None,
    errors: list[str],
) -> None:
    if bundle_root is None:
        return
    if not bundle_root.is_dir():
        errors.append("bundle path must identify an existing artifact directory")
        return

    try:
        bundle_files = {
            path.relative_to(bundle_root).as_posix()
            for path in bundle_root.rglob("*")
            if path.is_file()
        }
    except OSError as error:
        errors.append(f"bundle artifact directory could not be read: {error}")
        return

    catalog_files = {path.as_posix() for path in catalog_paths}
    for missing_path in sorted(bundle_files - catalog_files):
        errors.append(f"artifact catalog missing bundle file: {missing_path}")


def _path_hash_entries(
    raw_entries: object,
    field: str,
    errors: list[str],
) -> Iterable[tuple[str, object, object]]:
    if isinstance(raw_entries, list):
        for index, raw_entry in enumerate(raw_entries):
            entry_label = f'{field}[{index}]'
            if not isinstance(raw_entry, dict):
                errors.append(f"{entry_label} must be a TOML table")
                continue
            yield entry_label, raw_entry.get("path"), raw_entry.get("sha256")
        return

    if isinstance(raw_entries, dict):
        for raw_path, raw_digest in raw_entries.items():
            if isinstance(raw_digest, dict):
                yield f'{field}["{raw_path}"]', raw_path, raw_digest.get("sha256")
            else:
                yield f'{field}["{raw_path}"]', raw_path, raw_digest
        return

    if raw_entries is not None:
        errors.append(f'field "{field}" must be an array or table')


def _validate_logical_executables(raw_entries: object, bundle_label: str, errors: list[str]) -> set[str]:
    roles: set[str] = set()
    if raw_entries is None:
        errors.append(f'{bundle_label} field "logical_executables" must be an array or table')
        return roles

    if isinstance(raw_entries, list):
        for index, entry in enumerate(raw_entries):
            role = entry.get("role") if isinstance(entry, dict) else entry
            if isinstance(entry, dict):
                _validate_allowed_keys(entry, {"role"}, f'{bundle_label} logical_executables[{index}]', errors)
            if _validate_tool_role(role, f'{bundle_label} logical_executables[{index}] role', errors):
                roles.add(role)
        return roles

    if isinstance(raw_entries, dict):
        for name, entry in raw_entries.items():
            role = entry.get("role") if isinstance(entry, dict) else entry
            if isinstance(entry, dict):
                _validate_allowed_keys(entry, {"role"}, f'{bundle_label} logical_executables["{name}"]', errors)
            if _validate_tool_role(role, f'{bundle_label} logical_executables["{name}"] role', errors):
                roles.add(role)
        return roles

    errors.append(f'{bundle_label} field "logical_executables" must be an array or table')
    return roles


def _validate_runtime_identities(raw_entries: object, bundle_label: str, errors: list[str]) -> set[str]:
    roles: set[str] = set()
    if not isinstance(raw_entries, list):
        errors.append(f'{bundle_label} field "runtime_identities" must be an array')
        return roles
    if not raw_entries:
        errors.append(f'{bundle_label} field "runtime_identities" must contain at least one identity')
        return roles

    required_fields = ("role", "version", "basename", "sha256")
    for index, identity in enumerate(raw_entries):
        identity_label = f"{bundle_label} runtime_identities[{index}]"
        if not isinstance(identity, dict):
            errors.append(f"{identity_label} must be a TOML table")
            continue

        _validate_allowed_keys(identity, _RUNTIME_IDENTITY_FIELDS, f"{identity_label} runtime identity", errors)
        missing = [field for field in required_fields if field not in identity]
        if missing:
            errors.append(f"{identity_label} missing required field(s): {', '.join(missing)}")

        if _validate_tool_role(identity.get("role"), f"{identity_label} role", errors):
            roles.add(identity["role"])
        if not isinstance(identity.get("version"), str) or not identity.get("version"):
            errors.append(f'{identity_label} field "version" must be a non-empty string')
        basename = identity.get("basename")
        if not isinstance(basename, str) or not basename:
            errors.append(f'{identity_label} field "basename" must be a non-empty string')
        elif "/" in basename or "\\" in basename:
            errors.append(f'{identity_label} field "basename" must not contain path separators')
        _validate_sha256(identity.get("sha256"), f"{identity_label} sha256", errors)

        if identity.get("required_live_path") is True:
            errors.append("checked evidence must not require a live runtime path")

    return roles


def _validate_checked_commands(
    raw_entries: object,
    logical_roles: set[str],
    runtime_roles: set[str],
    bundle_label: str,
    errors: list[str],
) -> None:
    if not isinstance(raw_entries, list):
        errors.append(f'{bundle_label} field "checked_commands" must be an array')
        return

    for index, command in enumerate(raw_entries):
        command_label = f"{bundle_label} checked_commands[{index}]"
        if not isinstance(command, dict):
            errors.append(f"{command_label} must be a TOML table")
            continue
        _validate_allowed_keys(command, _CHECKED_COMMAND_FIELDS, f"{command_label} checked command", errors)
        if _contains_host_absolute_path(command):
            errors.append("checked command contains host-absolute path")
        if "argv" not in command:
            errors.append(f'{command_label} field "argv" is required')
            continue
        argv = command["argv"]
        if not isinstance(argv, list) or not argv or not all(isinstance(arg, str) for arg in argv):
            errors.append(f'{command_label} field "argv" must be a non-empty array of strings')
            continue
        if argv[0] not in logical_roles:
            errors.append("checked command executable must be declared tool:// role")
        if argv[0] not in runtime_roles:
            errors.append("checked command executable must have runtime identity")


def _validate_checked_provenance(raw_entries: object, bundle_label: str, errors: list[str]) -> None:
    if not isinstance(raw_entries, list):
        errors.append(f'{bundle_label} field "checked_provenance" must be an array')
        return

    for index, provenance in enumerate(raw_entries):
        provenance_label = f"{bundle_label} checked_provenance[{index}]"
        if not isinstance(provenance, dict):
            errors.append(f"{provenance_label} must be a TOML table")
            continue
        _validate_allowed_keys(
            provenance,
            _CHECKED_PROVENANCE_FIELDS,
            f"{provenance_label} checked provenance",
            errors,
        )
        if _contains_host_absolute_path(provenance):
            errors.append("checked provenance contains host-absolute path")
        if "value" not in provenance:
            errors.append(f'{provenance_label} field "value" is required')
            continue


def _validate_tool_role(raw_role: object, label: str, errors: list[str]) -> bool:
    if not isinstance(raw_role, str) or not raw_role.startswith("tool://") or not raw_role[7:]:
        errors.append(f"{label} must be a non-empty tool:// role")
        return False
    return True


def _validate_allowed_keys(
    table: dict[str, object],
    allowed_fields: frozenset[str] | set[str],
    label: str,
    errors: list[str],
) -> None:
    unsupported = sorted(set(table) - set(allowed_fields))
    if unsupported:
        errors.append(f"{label} unsupported field(s): {', '.join(unsupported)}")


def _validate_sha256(raw_sha256: object, label: str, errors: list[str]) -> None:
    if not isinstance(raw_sha256, str) or _SHA256_RE.fullmatch(raw_sha256) is None:
        errors.append(f"{label} must be a lowercase SHA-256 digest")


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _contains_host_absolute_path(value: object) -> bool:
    if isinstance(value, str):
        return _string_contains_host_absolute_path(value)
    if isinstance(value, list):
        return any(_contains_host_absolute_path(item) for item in value)
    if isinstance(value, tuple):
        return any(_contains_host_absolute_path(item) for item in value)
    if isinstance(value, dict):
        return any(
            _contains_host_absolute_path(key) or _contains_host_absolute_path(item)
            for key, item in value.items()
        )
    return False


def _string_contains_host_absolute_path(value: str) -> bool:
    return (
        PurePosixPath(value).is_absolute()
        or bool(PureWindowsPath(value).drive and PureWindowsPath(value).is_absolute())
        or _POSIX_ABSOLUTE_RE.search(value) is not None
        or _WINDOWS_ABSOLUTE_RE.search(value) is not None
    )
