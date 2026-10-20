"""Verified native imports for one-shot numerical witness generators."""

from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
import sys
import tempfile

import reference


@dataclass(frozen=True)
class NativeReference:
    native: object
    manifest: dict
    manifest_sha256: str
    native_sha256: str


@contextmanager
def native_reference(source):
    if any(name == "magna" or name.startswith("magna.") for name in sys.modules):
        raise reference.ReferenceError("run witness generators in a fresh Python process")
    manifest = reference.load_manifest()
    source = source.resolve()
    before = reference.verify_source(source, manifest)
    previous_path, previous_cache = sys.path[:], sys.pycache_prefix
    with tempfile.TemporaryDirectory(prefix="magna-witness-cache-") as cache:
        sys.pycache_prefix = cache
        sys.path.insert(0, str(source / "core"))
        try:
            from magna import _native

            binary = Path(_native.__file__).resolve()
            if not binary.is_relative_to(source / "core/magna"):
                raise reference.ReferenceError("native extension is outside the pinned source tree")
            binary_hash = reference.digest(binary)
            yield NativeReference(_native, manifest, reference.digest(reference.MANIFEST), binary_hash)
            if reference.verify_source(source, manifest) != before:
                raise reference.ReferenceError("reference source evidence changed during witness generation")
            if reference.digest(binary) != binary_hash:
                raise reference.ReferenceError("native extension changed during witness generation")
        finally:
            sys.path[:] = previous_path
            sys.pycache_prefix = previous_cache
