"""Verify the complete independent decoder workloads from the paper deposit."""

from pathlib import Path

import code_switching_benchmark
import logical_distillation_benchmark
import reference
import tqd_paper_benchmark
from cultivation_benchmark import digest_json
from remaining_contracts import read_json, require


HERE = Path(__file__).resolve().parent
SCHEMA = 'rmagna.paper-decoder-workloads-plan.v1'
CAMPAIGNS = {
    'logical-distillation': ('figure5-serial-table1', logical_distillation_benchmark.check_capture),
    'code-switching': ('paper-code-switching', code_switching_benchmark.check_capture),
    'tqd': ('terminal-curve-table1', tqd_paper_benchmark.check_capture),
}


def manifest(pinned):
    return dict(schema=SCHEMA, paper=pinned['paper']['arxiv'],
                reference_sha256=reference.digest(HERE/'reference.json'),
                archive_sha256=pinned['paper']['archive_sha256'],
                upstream_commit=pinned['upstream']['commit'], historical_kernel=pinned['paper']['kernel_commit'],
                campaigns={name:dict(scope=scope, plan_sha256=reference.digest(HERE/'paper'/f'{name}.json'))
                           for name, (scope, _) in CAMPAIGNS.items()})


def check_capture(plan, out):
    require(digest_json(plan) == digest_json(manifest(reference.load_manifest())),
            'decoder workload plan omits or changes a full campaign')
    out = out.resolve(strict=True)
    require({entry.name for entry in out.iterdir()} == set(CAMPAIGNS),
            'decoder workload result directory must contain exactly the three full campaigns')
    results = {}
    for name, (scope, check) in CAMPAIGNS.items():
        subplan = read_json(HERE/'paper'/f'{name}.json')
        require(subplan['scope'] == scope, f'decoder workload requires the full {name} plan')
        results[name] = check(subplan, reference.relative_file(out, name))
    return dict(status='PASS', scope='paper-decoder-workloads', campaigns=results,
                precision_draws=sum(result['precision_draws'] for result in results.values()),
                completed_batches=sum(result['completed_batches'] for result in results.values()))
