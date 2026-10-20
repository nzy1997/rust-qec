#!/usr/bin/env python3
"""Observation-only decoder host for rmagna.controller.v1; no quantum backend."""
import argparse
import contextlib
import copy
import hashlib
import importlib
import importlib.abc
import importlib.machinery
import importlib.util
import json
import operator
from pathlib import Path
import sys

MAX_BYTES = 16 * 1024 * 1024
MAX_BITS = 1_000_000


class NoNativeBackend(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname == "magna" or fullname.startswith("magna."):
            raise ImportError("the rmagna decoder host cannot import a Magna quantum backend")
        return None


def digest(path):
    with path.open("rb") as source:
        content = source.read(MAX_BYTES + 1)
    if len(content) > MAX_BYTES:
        raise ValueError(f"decoder source exceeds byte budget: {path}")
    return hashlib.sha256(content).hexdigest()


class VerifiedSourceLoader(importlib.machinery.SourceFileLoader):
    def __init__(self, fullname, path, expected):
        super().__init__(fullname, str(path))
        self.expected = expected

    def get_code(self, fullname):
        path = Path(self.path)
        with path.open("rb") as source:
            content = source.read(MAX_BYTES + 1)
        if len(content) > MAX_BYTES or hashlib.sha256(content).hexdigest() != self.expected:
            raise ImportError(f"decoder source changed before import: {path}")
        # Execute verified source directly; existing bytecode caches are irrelevant.
        return compile(content, str(path), "exec", dont_inherit=True)


class DecoderSources(importlib.abc.MetaPathFinder):
    def __init__(self, root, hashes):
        self.root, self.hashes = root, hashes

    def find_spec(self, fullname, path=None, target=None):
        spec = importlib.machinery.PathFinder.find_spec(fullname, path)
        if spec is None or spec.origin is None:
            return None
        source = Path(spec.origin).resolve()
        if not source.is_relative_to(self.root):
            return None
        key = source.relative_to(self.root).as_posix()
        if key not in self.hashes:
            raise ImportError(f"decoder module has no pinned Python source: {source}")
        spec.loader = VerifiedSourceLoader(fullname, source, self.hashes[key])
        return spec


def read_json(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError(f"duplicate JSON field {key!r}")
            result[key] = value
        return result
    def invalid(value):
        raise ValueError(f"non-finite JSON value {value!r}")
    return json.loads(text, object_pairs_hook=pairs, parse_constant=invalid)


def send(value):
    frame = json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode() + b"\n"
    if len(frame) > MAX_BYTES:
        raise ValueError("response exceeds frame budget")
    sys.stdout.buffer.write(frame)
    sys.stdout.buffer.flush()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--controllers", type=Path, required=True)
    parser.add_argument("--module-root", type=Path, required=True)
    args = parser.parse_args()
    sys.meta_path.insert(0, NoNativeBackend())
    root = args.module_root.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("module root must be a directory")
    files = {}
    for path in root.rglob("*.py"):
        if path.is_file():
            if len(files) >= 4096:
                raise ValueError("decoder source file budget exceeded")
            files[path.relative_to(root).as_posix()] = digest(path)
    sys.meta_path.insert(1, DecoderSources(root, files))
    sys.path.insert(0, str(root))
    with args.controllers.open("rb") as source:
        raw = source.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError("controller configuration exceeds byte budget")
    data = read_json(raw)
    if not isinstance(data, dict) or set(data) - {"schema_version", "bindings"}:
        raise ValueError("controller configuration must contain schema_version and bindings")
    if type(data.get("schema_version", 1)) is not int or data.get("schema_version", 1) != 1:
        raise ValueError("unsupported controller schema version")
    bindings = data.get("bindings", {})
    if not isinstance(bindings, dict) or len(bindings) > MAX_BITS:
        raise ValueError("bindings must be a bounded object")
    callbacks, versions, identities = {}, {}, {}
    # Decoder import/callback prints are diagnostics, never protocol frames.
    with contextlib.redirect_stdout(sys.stderr):
        for name, spec in bindings.items():
            if not name or name in {"XOR", "NOT"} or not isinstance(spec, dict):
                raise ValueError("binding name/specification is invalid")
            if set(spec) - {"import", "version", "config"}:
                raise ValueError(f"unknown binding fields for {name!r}")
            version = spec.get("version")
            imported = spec.get("import")
            if not isinstance(version, str) or not version or not isinstance(imported, str):
                raise ValueError("binding needs nonempty version and import strings")
            module, separator, function = imported.partition(":")
            if not separator or not module or not function.isidentifier():
                raise ValueError("controller import must be package.module:function")
            loaded = importlib.import_module(module)
            callback = getattr(loaded, function)
            if not callable(callback):
                raise ValueError(f"binding {name!r} is not callable")
            config = copy.deepcopy(spec.get("config", {}))
            if not isinstance(config, dict):
                raise ValueError("controller config must be an object")
            callbacks[name] = callback, config
            versions[name] = version
            identities[name] = dict(import_name=imported, version=version, config_sha256=hashlib.sha256(
                json.dumps(config, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()).hexdigest())
    for key, expected in files.items():
        if digest(root / key) != expected:
            raise ValueError(f"decoder source changed during imports: {key}")
    manifest = dict(controllers_sha256=hashlib.sha256(raw).hexdigest(), bindings=identities,
                    module_sources=files, native_backend="unavailable")
    send(dict(protocol="rmagna.controller.v1", bindings=versions, manifest=manifest))
    while True:
        frame = sys.stdin.buffer.readline(MAX_BYTES + 1)
        if not frame:
            return
        request_id = None
        try:
            if len(frame) > MAX_BYTES or not frame.endswith(b"\n"):
                raise ValueError("request exceeds frame budget or lacks newline")
            request = read_json(frame)
            if not isinstance(request, dict) or set(request) != {"id", "name", "bits"}:
                raise ValueError("request must contain id, name and bits")
            request_id = request["id"]
            if type(request_id) is not int or not 0 <= request_id < 2**64:
                raise ValueError("request ID must be an unsigned 64-bit integer")
            bits = request["bits"]
            if not isinstance(bits, list) or len(bits) > MAX_BITS or any(type(b) not in (int, bool) or b not in (0, 1) for b in bits):
                raise ValueError("inputs must be a bounded bit list")
            callback, config = callbacks[request["name"]]
            with contextlib.redirect_stdout(sys.stderr):
                # A fresh configuration prevents one invocation mutating the next.
                result = iter(callback(tuple(map(int, bits)), copy.deepcopy(config)))
                outputs = []
                for bit in result:
                    if len(outputs) >= MAX_BITS:
                        raise ValueError("decoder output exceeds bit budget")
                    if isinstance(bit, float):
                        raise ValueError("decoder outputs must be integer bits")
                    value = operator.index(bit)
                    if value not in (0, 1):
                        raise ValueError("decoder returned a non-bit value")
                    outputs.append(value)
            send(dict(id=request_id, outputs=outputs))
        except Exception as error:
            # A callback may have changed hidden state. Fail once, then exit.
            send(dict(id=request_id, error=f"{type(error).__name__}: {error}"[:4096]))
            raise SystemExit(1)


if __name__ == "__main__":
    main()
