"""Compile a Python-free Rust consumer using the workspace's exact lock closure."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib


def require(condition, message):
    if not condition:
        raise ValueError(message)


def consumer_lock(workspace_lock):
    """Select, without resolving versions, the existing rmagna dependency closure."""
    lock = tomllib.loads(workspace_lock)
    packages = lock['package']
    pending = ['rmagna']
    selected = {}
    while pending:
        dependency = pending.pop()
        parts = dependency.split(' ', 2)
        name = parts[0]
        version = parts[1] if len(parts) > 1 else None
        source = parts[2].strip('()') if len(parts) > 2 else None
        matches = [p for p in packages if p['name'] == name
                   and (version is None or p['version'] == version)
                   and (source is None or p.get('source') == source)]
        require(len(matches) == 1, f'ambiguous or missing locked consumer dependency: {dependency}')
        package = matches[0]
        key = (package['name'], package['version'], package.get('source'))
        if key in selected:
            continue
        require(set(package) <= {'name', 'version', 'source', 'checksum', 'dependencies'},
                f'unsupported consumer lock metadata for {name}')
        selected[key] = package
        pending.extend(package.get('dependencies', []))
    require(all('python' not in p['name'].lower() and p['name'] != 'pyo3'
                for p in selected.values()), 'Python dependency in standalone consumer')
    name = 'rmagna-standalone-consumer'
    require(all(p['name'] != name for p in selected.values()), 'consumer package name collision')
    selected[(name, '0.0.0', None)] = dict(name=name, version='0.0.0', dependencies=['rmagna'])
    lines = ['# Generated from the workspace lock; no version resolution.',
             f"version = {lock['version']}", '']
    for package in sorted(selected.values(), key=lambda p: (p['name'], p['version'], p.get('source', ''))):
        lines.append('[[package]]')
        for key in ('name', 'version', 'source', 'checksum'):
            if key in package:
                lines.append(f'{key} = {json.dumps(package[key])}')
        if 'dependencies' in package:
            lines += ['dependencies = [']
            lines += [f' {json.dumps(d)},' for d in package['dependencies']]
            lines += [']']
        lines += ['']
    return '\n'.join(lines), len(selected)


def check_consumer(root, fixture, *, timeout=180):
    """Create only a temporary manifest, source and projected lock; reuse Cargo."""
    root = root.resolve()
    before = (root/'Cargo.lock').read_bytes()
    source = fixture.read_bytes()
    locked, count = consumer_lock(before.decode())
    manifest = ('[package]\nname = "rmagna-standalone-consumer"\nversion = "0.0.0"\n'
                'edition = "2024"\n[dependencies]\nrmagna = { path = '
                + json.dumps(str(root/'rmagna')) + ' }\n[workspace]\n')
    # Resolve the platform temporary path so the consumer is not accidentally
    # rooted through macOS /var or /tmp symlinks. No environment is copied.
    temporary_root = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix='magna-standalone-consumer-', dir=temporary_root) as temporary:
        directory = Path(temporary)
        (directory/'src').mkdir()
        (directory/'Cargo.toml').write_text(manifest)
        (directory/'Cargo.lock').write_text(locked)
        (directory/'src/main.rs').write_bytes(source)
        command = ['cargo', 'check', '--locked', '--offline', '--manifest-path',
                   str(directory/'Cargo.toml')]
        try:
            result = subprocess.run(command, cwd=root, capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            raise ValueError(f"standalone consumer Cargo check exceeded {timeout} seconds") from error
        require((directory/'Cargo.lock').read_text() == locked, 'locked consumer rewrote its lockfile')
    require((root/'Cargo.lock').read_bytes() == before, 'consumer changed workspace lockfile')
    require(fixture.read_bytes() == source, 'consumer source changed during compilation')
    require(result.returncode == 0, 'standalone consumer compilation failed:\n' + result.stderr)
    sha = lambda value: hashlib.sha256(value).hexdigest()
    return dict(status='PASS', scope='compile_only_four_public_facades_no_python_runtime',
                command=command, returncode=result.returncode, packages=count,
                workspace_lock_sha256=sha(before), consumer_source_sha256=sha(source),
                generated_lock_sha256=sha(locked.encode()),
                temporary_consumer_removed=not directory.exists())
