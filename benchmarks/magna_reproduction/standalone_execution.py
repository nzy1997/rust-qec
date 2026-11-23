"""Capture and validate compiler-bound Rust standalone fixture executions.

Receipts validate retained observations and identities, following coverage_rust's
producer trust model. Keep the actual compiler/list/run records for review.
"""
import json
from pathlib import Path
import subprocess

import coverage_rust as rust

SCHEMA = 'rmagna.standalone-execution.v1'


def command(targets):
    return ['cargo', 'test', '--locked', '-p', 'rmagna', '--no-run', '--message-format=json',
            *[arg for target in sorted(targets) for arg in ('--test', target)]]


def process(argv, root):
    result = subprocess.run(argv, cwd=root, capture_output=True, text=True, timeout=600)
    return dict(stdout=result.stdout, stderr=result.stderr, exit_code=result.returncode)


def artifacts(compiler):
    rows = []
    for line in compiler['stdout'].splitlines():
        value = json.loads(line)
        rust.require(isinstance(value, dict), 'invalid standalone compiler JSON record')
        if value.get('reason') == 'compiler-artifact' and value.get('executable'):
            rows.append(value)
    return rows


def capture(root, targets):
    before = rust.source_identity(root)
    argv = command(targets)
    compiler = process(argv, root)
    rust.require(compiler['exit_code'] == 0, 'standalone Rust compilation failed: '+compiler['stderr'])
    runs = []
    for artifact in artifacts(compiler):
        target = artifact['target']['name']
        if target not in targets:
            continue
        binary = Path(artifact['executable']).resolve()
        runs.append(dict(package='rmagna', target=target,
                         binary=dict(path=str(binary), sha256=rust.sha(binary)),
                         list=process([str(binary), '--list'], root),
                         run=process([str(binary), '--test-threads=1'], root)))
    rust.require(rust.source_identity(root) == before, 'standalone Rust sources changed during execution')
    return dict(schema=SCHEMA, compiler=dict(argv=argv, **compiler),
                tests=dict(schema='rstim.magna-rust-tests.v1', source_files=before, targets=runs))


def validate(root, receipt, witnesses):
    rust.fields(receipt, {'schema', 'compiler', 'tests'}, 'standalone execution receipt')
    rust.require(receipt['schema'] == SCHEMA, 'unsupported standalone execution receipt')
    required = {rust.witness(root, witness) for witness in witnesses}
    targets = {target for target, _ in required}
    compiler = rust.fields(receipt['compiler'], {'argv', 'stdout', 'stderr', 'exit_code'}, 'standalone compiler')
    rust.process({key: compiler[key] for key in ('stdout', 'stderr', 'exit_code')}, 'standalone compiler')
    rust.require(compiler['argv'] == command(targets), 'standalone compiler command changed')
    rust.require(compiler['exit_code'] == 0, 'standalone compiler process failed')
    compiled = artifacts(compiler)
    tests = rust.fields(receipt['tests'], {'schema', 'source_files', 'targets'}, 'standalone Rust tests')
    runs = tests['targets']
    rust.require(isinstance(runs, list), 'standalone execution targets must be an array')
    for run in runs:
        rust.fields(run, {'package', 'target', 'binary', 'list', 'run'}, 'standalone Rust target')
    rust.require({row['target'] for row in runs} == targets and len(runs) == len(targets),
                 'standalone execution target inventory changed')
    for run in runs:
        target = run['target']
        source = rust.safe_file(root, f'rmagna/tests/{target}.rs')
        matches = [row for row in compiled if row['target']['name'] == target]
        rust.require(len(matches) == 1, f'missing or duplicate compiler artifact: {target}')
        artifact = matches[0]
        rust.require(artifact['target']['kind'] == ['test']
                     and Path(artifact['manifest_path']).resolve() == root/'rmagna/Cargo.toml'
                     and Path(artifact['target']['src_path']).resolve() == source.resolve()
                     and Path(artifact['executable']).resolve() == Path(run['binary']['path']),
                     f'standalone compiler artifact does not bind the fixture: {target}')
        required.update((target, name) for name in rust.flat_tests(source.read_text()))
    observed = rust.execution(root, receipt['tests'], required)
    rust.require(observed['source_matches'] and not observed['errors']
                 and not observed['failed'] and not observed['missing'],
                 'standalone fixture execution failed: '+str(observed['errors']+observed['failed']+observed['missing']))
    return dict(status='PASS', targets=sorted(targets), tests=len(observed['passed']),
                source_bound=True, compiler_bound=True)
