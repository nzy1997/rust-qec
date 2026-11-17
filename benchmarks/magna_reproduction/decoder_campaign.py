"""Source-bound batch capture shared by the decoder workload experiments."""

from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
from dataclasses import dataclass
import fcntl
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
from typing import Callable
import zipfile

from cultivation_benchmark import atomic, digest_json, sha
from paper_probe_identity import source_identity, source_inputs
import reference
from remaining_contracts import read_json, require


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
COMMON_FILES = (
    'benchmarks/magna_reproduction/decoder_campaign.py',
    'benchmarks/magna_reproduction/cultivation_benchmark.py',
    'benchmarks/magna_reproduction/cultivation_statistics.py',
    'benchmarks/magna_reproduction/decoder_workloads_reference.py',
    'benchmarks/magna_reproduction/paper_benchmark.py',
    'benchmarks/magna_reproduction/paper_probe_identity.py',
    'benchmarks/magna_reproduction/reference.py',
    'rmagna/python/decoder_worker.py',
)


@dataclass(frozen=True)
class Campaign:
    schema: str
    label: str
    runner_files: tuple[str, ...]
    input_pins: dict[str, str]
    historical_pins: dict[str, str]
    cases: Callable
    configuration: Callable
    command: Callable
    validate_stage: Callable
    results_from_batches: Callable
    summarize: Callable
    derived_inputs: Callable | None = None
    after_worker: Callable | None = None

    def verify_identity(self, plan, out, receipt):
        require(receipt['schema'] == self.schema and receipt['manifest_sha256'] == digest_json(plan)
                and receipt['runner_sha256'] == self.runner_identity()
                and receipt['rust_source_sha256'] == source_identity(ROOT)
                and receipt['rust_source_inputs'] == source_inputs(ROOT)
                and sha(receipt['rust_binary_sha256']) and sha(receipt['python_environment']['sha256'])
                and isinstance(receipt['python_environment']['launcher'], str),
                f'{self.label} capture identity changed')
        require(type(receipt['precision_workers']) is int and 1 <= receipt['precision_workers'] <= 64
                and type(receipt['timing_workers']) is int and receipt['timing_workers'] == 1,
                f'invalid {self.label} worker policy')
        reference.verify_files(out/'inputs', self.input_pins)
        reference.verify_files(out/'historical', self.historical_pins)
        if self.derived_inputs is not None:
            derived = self.derived_inputs(out/'inputs')
            actual = {}
            for name, expected in derived.items():
                path = reference.relative_file(out/'inputs', name)
                require(digest_json(read_json(path)) == digest_json(expected), f'{self.label} derived input changed: {name}')
                actual[name] = reference.digest(path)
            require(receipt['derived_input_sha256'] == actual, f'{self.label} derived input identity changed')

    def runner_identity(self):
        return {name: reference.digest(ROOT/name) for name in (*COMMON_FILES, *self.runner_files)}

    def capture_inputs(self, upstream, archive, archive_sha, out):
        for name in self.input_pins:
            source = reference.relative_file(upstream, name)
            path = out/'inputs'/name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, path)
        require(reference.digest(archive) == archive_sha, f'{self.label} paper archive changed')
        with zipfile.ZipFile(archive) as source:
            for name in self.historical_pins:
                path = out/'historical'/name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(source.read('magna-data-deposit/' + name))


def stage_path(out, case):
    return out/'batches'/(case['id']+'.json')


def run_stage(campaign, case, plan, out, executable, python, receipt, run_process):
    path = stage_path(out, case)
    config = out/'config'/path.name
    atomic(config, campaign.configuration(case, plan))
    command = campaign.command(case, out, executable, python, config)
    try:
        stage = run_process(list(map(str, command)), case['timeout_seconds'], out/'config')
    except OSError as error:
        stage = dict(status='failed', launch_error=str(error))
    stage['case'] = case
    if stage['status'] == 'complete':
        try:
            if campaign.after_worker is not None:
                campaign.after_worker(stage, case, out, python, run_process)
            campaign.validate_stage(stage, case, plan, receipt)
        except (ValueError, KeyError, TypeError, OSError) as error:
            stage.update(status='invalid', validation_error=str(error))
    atomic(path, stage)
    print(case['id'] + ' ' + stage['status'], flush=True)
    return stage['status']


def python_environment(path):
    # Invoking a venv symlink by its resolved target bypasses pyvenv.cfg.
    executable = path.absolute()
    program = '''
import importlib.metadata
import json
import sys
packages = {}
for name in ('numpy', 'scipy', 'stim', 'pymatching'):
    try:
        packages[name] = importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        packages[name] = None
print(json.dumps(dict(prefix=sys.prefix, base_prefix=sys.base_prefix,
                      version=sys.version, packages=packages), sort_keys=True))
'''
    probe = subprocess.run([executable, '-I', '-B', '-c', program], capture_output=True, text=True, timeout=15)
    require(probe.returncode == 0, 'could not inspect the requested decoder Python environment')
    return executable, dict(launcher=str(executable), sha256=reference.digest(executable), **json.loads(probe.stdout))


def capture(campaign, args, plan, run_process):
    label = campaign.label
    require(args.python is not None and 1 <= args.jobs <= 64, f'{label} requires --python and --jobs 1..64')
    upstream, executable = (path.resolve(strict=True) for path in (args.upstream, args.rust_example))
    python, python_identity = python_environment(args.python)
    out = args.out.resolve()
    reference.verify_files(upstream, campaign.input_pins)
    if not args.resume:
        require(args.archive is not None, f'{label} requires the pinned paper deposit in --archive')
        out.mkdir(parents=True, exist_ok=False)
        (out/'batches').mkdir()
        (out/'config').mkdir()
    with (out/'capture.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError(f'{label} capture is already running') from error
        if args.resume:
            receipt = read_json(reference.relative_file(out, 'receipt.json'))
        else:
            campaign.capture_inputs(upstream, args.archive.resolve(strict=True), plan['archive_sha256'], out)
            receipt = dict(schema=campaign.schema, status='recording', manifest_sha256=digest_json(plan),
                           runner_sha256=campaign.runner_identity(), rust_source_sha256=source_identity(ROOT),
                           rust_source_inputs=source_inputs(ROOT), rust_binary_sha256=reference.digest(executable),
                           python_environment=python_identity, precision_workers=args.jobs, timing_workers=1,
                           host=dict(os=platform.system(), architecture=platform.machine(), logical_cpus=os.cpu_count()))
            if campaign.derived_inputs is not None:
                receipt['derived_input_sha256'] = {}
                for name, value in campaign.derived_inputs(out/'inputs').items():
                    path = reference.relative_file(out/'inputs',name)
                    atomic(path,value)
                    receipt['derived_input_sha256'][name] = reference.digest(path)
            atomic(out/'receipt.json', receipt)
        campaign.verify_identity(plan, out, receipt)
        require((reference.digest(executable), python_identity, args.jobs) ==
                (receipt['rust_binary_sha256'], receipt['python_environment'], receipt['precision_workers']),
                f'{label} resume changed the executable, Python, or worker count')
        unfinished = []
        for case in campaign.cases(plan):
            path = stage_path(out, case)
            if path.exists() or path.is_symlink():
                path = reference.relative_file(out, 'batches/' + path.name)
                campaign.validate_stage(read_json(path), case, plan, receipt)
            else:
                unfinished.append(case)

        def execute(case):
            return run_stage(campaign, case, plan, out, executable, python, receipt, run_process)

        for case in unfinished:
            if case['kind'] == 'timing':
                require(execute(case) == 'complete', f'{label} timing failed; batch receipt retained')
        pending = iter(case for case in unfinished if case['kind'] == 'precision')
        with ThreadPoolExecutor(max_workers=args.jobs) as workers:
            initial = (next(pending, None) for _ in range(args.jobs))
            active = {workers.submit(execute, case) for case in initial if case is not None}
            while active:
                completed, active = wait(active, return_when=FIRST_COMPLETED)
                require(all(job.result() == 'complete' for job in completed),
                        f'{label} precision failed; batch receipts retained')
                for _ in completed:
                    case = next(pending, None)
                    if case is not None:
                        active.add(workers.submit(execute, case))
        campaign.verify_identity(plan, out, receipt)
        require(reference.digest(executable) == receipt['rust_binary_sha256']
                and python_environment(python)[1] == receipt['python_environment'], f'{label} producer changed during capture')
        report = campaign.results_from_batches(plan, out, receipt)
        atomic(out/'report.json', report)
        receipt.update(status='recorded', report_sha256=reference.digest(out/'report.json'))
        atomic(out/'receipt.json', receipt)
    return campaign.summarize(report)


def check_capture(campaign, plan, out):
    out = out.resolve(strict=True)
    receipt = read_json(reference.relative_file(out, 'receipt.json'))
    campaign.verify_identity(plan, out, receipt)
    actual = campaign.results_from_batches(plan, out, receipt)
    report = reference.relative_file(out, 'report.json')
    require(receipt['status'] == 'recorded' and receipt['report_sha256'] == reference.digest(report)
            and digest_json(actual) == digest_json(read_json(report)),
            f'{campaign.label} report, paired covariance, or draw inventory differs')
    return campaign.summarize(actual)
