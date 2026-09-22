"""Bind evidence to a clean source commit, allowing a later artifact-only commit.

The inventory is derived from Git and Cargo workspace membership, never from a
bundle's own list. Historical timing is not revalidated by prediction replay.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
BUILD_COMMANDS = [
    ['cargo', 'build', '--release', '--locked', '-p', 'rustqec-cli', '--features',
     'benchmark-tools,ilp', '--bin', 'rustqec', '--example', 'export_matching_benchmark',
     '--example', 'export_decoder_oracle', '--example', 'offline_matching_benchmark'],
    ['cargo', 'build', '--release', '--locked', '-p', 'rstim', '--example', 'atom_loss_sampling_benchmark'],
]
BINARIES = ['target/release/rustqec'] + ['target/release/examples/'+name for name in
    ['export_matching_benchmark', 'export_decoder_oracle', 'offline_matching_benchmark', 'atom_loss_sampling_benchmark']]


BUILD_POLICY = 'isolated-cargo-home; allowlisted-environment; no-external-ancestor-config-v2'
BUILD_ENV_KEYS = ('PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'RUSTUP_HOME',
                  'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT', 'LIBCLANG_PATH')
NETWORK_ENV_KEYS = ('HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NO_PROXY',
                    'http_proxy', 'https_proxy', 'all_proxy', 'no_proxy',
                    'SSL_CERT_FILE', 'SSL_CERT_DIR')
CAPTURE_DEPENDENCIES = ('stim', 'numpy', 'pymatching', 'scipy', 'matplotlib')
THREAD_ENVIRONMENT = {'OMP_NUM_THREADS': '1', 'OPENBLAS_NUM_THREADS': '1', 'RAYON_NUM_THREADS': '1'}


def verify_local_cargo_config(repo):
    # git status omits ignored files, but Cargo still consumes these two names.
    for name in ('.cargo/config', '.cargo/config.toml'):
        path = repo/name
        if path.exists() or path.is_symlink():
            tracked = git(repo, 'ls-tree', '--name-only', 'HEAD', '--', name).decode().strip()
            if tracked != name or not path.is_file() or path.is_symlink():
                raise ValueError('Cargo configuration must be tracked in the source commit: '+name)


def build_environment(repo, cargo_home):
    """No inherited Cargo/profile/compiler flags or ambient Cargo configuration.

    Fresh CARGO_HOME isolates user registry/config state. Cargo also searches
    ancestor directories independently of CARGO_HOME, so reject those configs.
    The checkout's own .cargo files remain part of the source inventory.
    """
    for parent in repo.resolve().parents:
        for name in ('config', 'config.toml'):
            path = parent/'.cargo'/name
            if path.exists():
                raise ValueError('External ancestor Cargo configuration: '+str(path))
    verify_local_cargo_config(repo)
    if cargo_home.exists():
        raise ValueError('Use a fresh isolated Cargo home')
    env = {key: os.environ[key] for key in BUILD_ENV_KEYS + NETWORK_ENV_KEYS if key in os.environ}
    env.update(CARGO_HOME=str(cargo_home.resolve()), LANG='C', LC_ALL='C', **THREAD_ENVIRONMENT)
    return env


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args])


def inventory(repo, revision):
    cargo = tomllib.loads(git(repo, 'show', revision+':Cargo.toml').decode())
    members = cargo['workspace']['members']
    if any('*' in member or '..' in Path(member).parts for member in members):
        raise ValueError('Source contract requires explicit workspace member paths')
    prefixes = tuple(member+'/' for member in members) + ('benchmarks/atom_loss/', '.cargo/')
    entries = {}
    for entry in git(repo, 'ls-tree', '-rz', '--full-tree', revision).split(b'\0'):
        if not entry:
            continue
        header, name = entry.split(b'\t', 1)
        mode, kind, oid = header.decode().split()
        name = name.decode()
        # All files in every workspace member include build.rs, include_bytes!
        # assets and local dependency configuration, not just decoder .rs files.
        if name.startswith(prefixes) or name in {
            'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml',
            'build.rs', '.github/workflows/atom-loss-reference.yml'}:
            if kind != 'blob' or mode not in {'100644', '100755'}:
                raise ValueError('Unsupported source entry: '+name)
            entries[name] = {'mode': mode, 'git_blob': oid}
    return entries


def input_digest(entries):
    return hashlib.sha256(json.dumps(entries, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def check_worktree(repo, entries):
    for name, entry in entries.items():
        path = repo/name
        if path.is_symlink() or not path.is_file():
            raise ValueError('Missing or redirected source input: '+name)
        data = path.read_bytes()
        blob = hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
        executable = bool(path.stat().st_mode & 0o111)
        if blob != entry['git_blob'] or executable != (entry['mode'] == '100755'):
            raise ValueError('Dirty source input: '+name)


def clean_source(repo=ROOT):
    verify_local_cargo_config(repo)
    if git(repo, 'status', '--porcelain', '--untracked-files=all').strip():
        raise ValueError('Evidence generation requires a clean checkout; use a detached worktree and external output directories')
    commit = git(repo, 'rev-parse', 'HEAD').decode().strip()
    entries = inventory(repo, commit)
    check_worktree(repo, entries)
    return {'schema': 1, 'source_commit': commit, 'working_tree_dirty': False,
            'inputs': entries, 'input_digest': input_digest(entries), 'build_commands': BUILD_COMMANDS, 'build_policy': BUILD_POLICY}


def verify_source(record, repo=ROOT):
    commit = record['source_commit']
    if record.get('schema') != 1 or record.get('working_tree_dirty') is not False:
        raise ValueError('Missing clean source provenance')
    if len(commit) != 40 or any(c not in '0123456789abcdef' for c in commit):
        raise ValueError('Invalid source commit')
    subprocess.run(['git', '-C', str(repo), 'merge-base', '--is-ancestor', commit, 'HEAD'], check=True)
    expected = inventory(repo, commit)
    if record['inputs'] != expected or record['input_digest'] != input_digest(expected):
        raise ValueError('Incomplete or altered source inventory')
    if record.get('build_policy') != BUILD_POLICY:
        raise ValueError('Unexpected clean-build environment policy')
    if record['build_commands'] != BUILD_COMMANDS:
        raise ValueError('Unexpected evidence build commands')
    if inventory(repo, 'HEAD') != expected:
        raise ValueError('Current source/build inputs differ from measured source commit; regenerate evidence')
    # Catch newly staged/untracked build inputs before a local verification too.
    members = tomllib.loads(git(repo, 'show', 'HEAD:Cargo.toml').decode())['workspace']['members']
    prefixes = tuple(member+'/' for member in members) + ('benchmarks/atom_loss/', '.cargo/')
    added = git(repo, 'diff', 'HEAD', '--name-only', '--diff-filter=A', '-z') + git(repo, 'ls-files', '--others', '--exclude-standard', '-z')
    for name in added.decode().split('\0'):
        if name and (name.startswith(prefixes) or name in {'rust-toolchain', 'rust-toolchain.toml', 'build.rs'}):
            raise ValueError('Uncommitted source input: '+name)
    verify_local_cargo_config(repo)
    check_worktree(repo, expected)
    return commit


def capture(out, stage, extra=None):
    """Record only sources and binaries produced by the clean-build entry point."""
    import importlib.metadata
    import platform
    import sys
    from datetime import datetime, timezone
    from .run import cpu_model, save, digest
    binding = json.loads((out/'source-manifest.json').read_text())
    commit = verify_source(binding)
    if clean_source()['source_commit'] != commit:
        raise ValueError('Measurements must execute at the source commit itself')
    binaries = {name: digest(ROOT/name) for name in BINARIES}
    if binaries != binding['binaries']:
        raise ValueError('Measured binaries differ from clean build')
    sources = {name: (ROOT/name).read_text() for name in binding['inputs']
               if name.startswith('benchmarks/atom_loss/') and (ROOT/name).suffix in {'.py', '.stim', '.txt', '.md'}}
    suffix = '' if stage == 'all' else '-'+stage
    save(out/('source-snapshot'+suffix+'.json'), {'base_commit': commit, 'files': sources})
    record = {'source_commit': commit, 'working_tree_dirty': False,
              'input_digest': binding['input_digest'], 'started_utc': datetime.now(timezone.utc).isoformat(),
              'command': sys.argv, 'os': platform.platform(), 'cpu': cpu_model(), 'python': sys.version,
              'rustc': binding['rustc'], 'binaries': binaries,
              'dependencies': {p: importlib.metadata.version(p) for p in CAPTURE_DEPENDENCIES},
              'environment': {k: os.environ.get(k) for k in THREAD_ENVIRONMENT},
              'sources': {name: hashlib.sha256(data.encode()).hexdigest() for name, data in sources.items()}}
    record.update(extra or {})
    save(out/('provenance-'+stage+'.json'), record)


def verify_bundle_source(out, repo=ROOT):
    binding = json.loads((out/'source-manifest.json').read_text())
    commit = verify_source(binding, repo)
    env = binding.get('build_environment', {})
    fixed = {'LANG': 'C', 'LC_ALL': 'C', **THREAD_ENVIRONMENT}
    if (not {'PATH', 'HOME', 'CARGO_HOME'}.issubset(env)
            or set(env) - (set(BUILD_ENV_KEYS) | set(fixed) | {'CARGO_HOME'})
            or any(not isinstance(value, str) or not value for value in env.values())
            or any(env.get(key) != value for key, value in fixed.items())):
        raise ValueError('Invalid effective build environment')
    if set(binding['binaries']) != set(BINARIES) or any(len(h) != 64 for h in binding['binaries'].values()):
        raise ValueError('Incomplete clean-build binary manifest')
    if not isinstance(binding.get('rustc'), str) or not binding['rustc'].strip():
        raise ValueError('Missing recorded build compiler')
    dependencies = {}
    for line in (repo/'benchmarks/atom_loss/requirements.txt').read_text().splitlines():
        package, separator, version = line.strip().partition('==')
        if package in CAPTURE_DEPENDENCIES:
            if not separator or not version.strip() or package in dependencies:
                raise ValueError('Invalid pinned provenance dependency: '+package)
            dependencies[package] = version
    if set(dependencies) != set(CAPTURE_DEPENDENCIES):
        raise ValueError('Missing pinned provenance dependency')
    runtime = None
    for path in sorted(out.glob('provenance-*.json')):
        record = json.loads(path.read_text())
        if (record['source_commit'] != commit or record.get('working_tree_dirty') is not False
                or record.get('input_digest') != binding['input_digest'] or record['binaries'] != binding['binaries']):
            raise ValueError('Stage provenance differs from clean build: '+path.name)
        if record.get('dependencies') != dependencies:
            raise ValueError('Stage dependencies differ from pinned requirements: '+path.name)
        if record.get('rustc') != binding['rustc']:
            raise ValueError('Stage compiler differs from clean build: '+path.name)
        if record.get('environment') != THREAD_ENVIRONMENT:
            raise ValueError('Stage threading environment differs from fixed run contract: '+path.name)
        host = {key: record.get(key) for key in ['cpu', 'os', 'python']}
        if any(not isinstance(value, str) or not value.strip() for value in host.values()):
            raise ValueError('Missing recorded stage runtime: '+path.name)
        # Compare the stages of this historical run, never the verifier's host.
        if runtime is not None and host != runtime:
            raise ValueError('Stage runtime differs within the recorded run: '+path.name)
        runtime = host
        suffix = '' if path.stem == 'provenance-all' else path.stem.removeprefix('provenance')
        snapshot = json.loads((out/('source-snapshot'+suffix+'.json')).read_text())
        expected = {name: (repo/name).read_text() for name in binding['inputs']
                    if name.startswith('benchmarks/atom_loss/') and (repo/name).suffix in {'.py', '.stim', '.txt', '.md'}}
        if record['sources'] != {name: hashlib.sha256(text.encode()).hexdigest() for name, text in expected.items()}:
            raise ValueError('Stage source inventory differs from current source: '+path.name)
        if snapshot != {'base_commit': commit, 'files': expected}:
            raise ValueError('Snapshot differs from measured/current source: '+path.name)
    return commit
