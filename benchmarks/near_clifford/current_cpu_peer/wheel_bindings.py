"""Bind a wheel's complete code inventory to retained installed bytes."""
import hashlib
from pathlib import Path, PurePosixPath
import zipfile


def require(value, message):
    if not value:
        raise ValueError(message)


def safe_relative(value):
    path = PurePosixPath(value)
    require(not path.is_absolute() and value == str(path) and '..' not in path.parts
            and '\\' not in value and bool(path.parts), 'unsafe wheel relative path')
    return path


def bind_wheel(wheel, metadata, *, original_site_packages, installed_root):
    """The installed root can be the live directory or its immutable snapshot."""
    wheel = Path(wheel)
    original = PurePosixPath(original_site_packages)
    require(original.is_absolute() and '..' not in original.parts, 'invalid site-packages root')
    installed = {}
    for filename, digest in metadata['files'].items():
        absolute = PurePosixPath(filename)
        require(absolute.is_absolute() and '..' not in absolute.parts, 'unsafe installed path')
        try:
            relative = str(absolute.relative_to(original))
        except ValueError as error:
            raise ValueError('installed file outside original site-packages') from error
        path = safe_relative(relative)
        if relative.endswith(('.py', '.so', '.pyd')) and not any(part.endswith('.dist-info') for part in path.parts):
            require(relative not in installed, 'duplicate installed code path')
            require(isinstance(digest, str) and len(digest) == 64 and all(c in '0123456789abcdef' for c in digest), 'invalid installed code digest')
            installed[relative] = digest
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), 'duplicate wheel members')
        for name in names:
            safe_relative(name.removesuffix('/'))
        code = {name for name in names if name.endswith(('.py', '.so', '.pyd'))
                and not any(part.endswith('.dist-info') for part in PurePosixPath(name).parts)}
        require(bool(code) and code == set(installed), 'wheel/installed code inventory mismatch')
        for relative in sorted(code):
            data = archive.read(relative)
            require(hashlib.sha256(data).hexdigest() == installed[relative], 'wheel/installed code digest mismatch')
            require((Path(installed_root)/relative).read_bytes() == data, 'installed code bytes mismatch')
    return dict(wheel=wheel.name, bytes=wheel.stat().st_size,
                sha256=hashlib.sha256(wheel.read_bytes()).hexdigest(), code_files=installed)
