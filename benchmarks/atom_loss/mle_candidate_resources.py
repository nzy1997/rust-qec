"""MLE candidate-domain workload campaign (issue #722).

Executes the finite real-circuit MLE workload matrix declared by the scope
plan (``docs/envelope-mle-scope.json``, issue #721) against the real CLI:
the measured d=3/r=2 loss-0.002 points, the bounded follow-up d=3/r=1
loss-0.01 points that complete the declared cross-product, the synthetic
cache-eviction corpus (cache behavior only, never real-circuit coverage) and
the MLE failure-semantics controls. Budgets are fixed before measurement and
mirrored by the release gate, which fails loudly when they drift.

Campaign (a few minutes after build):

    python3 -m benchmarks.atom_loss.mle_candidate_resources \
      --binary target/release/rustqec \
      --plan docs/envelope-mle-scope.json \
      --out drafts/envelope-mle-candidate/mle-resources.json

Fast replay of a produced report:

    python3 -m benchmarks.atom_loss.mle_candidate_resources \
      --verify drafts/envelope-mle-candidate/mle-resources.json
"""
from .shot_data import require
import argparse
import json
from pathlib import Path
import sys
import tempfile
import time

from . import readiness_resources as resources
from .run import ROOT, save, digest

SCHEMA = 'rustqec.envelope-mle-candidate-resources.v1'
PASS_LINE = 'PASS envelope MLE candidate resources'
PLAN_PATH = ROOT/'docs/envelope-mle-scope.json'

# Budgets declared before any measurement; the release gate mirrors these
# values and fails when the report's declared budget drifts from them.
BUDGET = {'per_case_wall_seconds': 900, 'total_wall_seconds': 3600,
          'peak_rss_bytes': 4 * 1024**3}

# The declared finite workload matrix; nothing is measured outside it. The
# d=3/r=2 points reuse the issue #715 seeds so the candidate measurements are
# directly comparable to the retained full report; the d=3/r=1 points are the
# bounded follow-up this plan adds (issue #721).
WORKLOADS = (
    {'id': 'mle-d3r2-p002-b1024', 'kind': 'repeated-patterns', 'real_circuit': True,
     'distance': 3, 'rounds': 2, 'loss': 0.002, 'shots': 1024, 'seed': 715_401},
    {'id': 'mle-d3r2-p002-b16384', 'kind': 'repeated-patterns', 'real_circuit': True,
     'distance': 3, 'rounds': 2, 'loss': 0.002, 'shots': 16384, 'seed': 715_402},
    {'id': 'mle-d3r1-p010-b1024', 'kind': 'repeated-patterns', 'real_circuit': True,
     'distance': 3, 'rounds': 1, 'loss': 0.01, 'shots': 1024, 'seed': 721_401},
    {'id': 'mle-d3r1-p010-b16384', 'kind': 'repeated-patterns', 'real_circuit': True,
     'distance': 3, 'rounds': 1, 'loss': 0.01, 'shots': 16384, 'seed': 721_402},
    {'id': 'mle-eviction-wires24', 'kind': 'cache-eviction', 'real_circuit': False,
     'distance': None, 'rounds': None, 'loss': None, 'shots': None, 'seed': None},
)

FAILURE_CASE_IDS = ('fail-mle-candidate-limit', 'fail-mle-solve-timeout', 'fail-mle-infeasible')
EXPECTED_FAILURE_CODES = {
    'fail-mle-candidate-limit': ('unsupported_circuit', 2),
    'fail-mle-solve-timeout': ('decode_timeout', 3),
    'fail-mle-infeasible': ('decode_infeasible', 3),
}

EXCLUSIONS = [
    {'case': 'mle-batches-above-16384',
     'reason': 'not measured in this campaign; the supported batch ceiling is the largest '
               'measured successful batch on each declared circuit shape'},
    {'case': 'mle-scales-beyond-d3',
     'reason': 'larger distances/rounds widen the ILP model per pattern; only the declared '
               'd=3 points are measured, so larger scales stay outside the candidate domain'},
    {'case': 'synthetic-eviction-as-real-circuit-coverage',
     'reason': 'the 24-wire synthetic corpus is cache-behavior evidence only and never '
               'counts as real-circuit MLE coverage'},
]


def measure_workload(binary, work, spec):
    """Execute one declared workload and return its raw observation record."""
    if spec['kind'] == 'cache-eviction':
        case = {'id': spec['id'], 'decoder': 'envelope-mle', 'kind': spec['kind'],
                'circuit_text': resources.decoder_reference.circuit_for(24),
                'source': 'benchmarks.atom_loss.decoder_reference.circuit_for(24)',
                'loss_rate': None, 'corpus': 'synthetic-eviction'}
    else:
        text, source = resources.generated_circuit(
            binary, work, spec['distance'], spec['rounds'], spec['loss'])
        case = {'id': spec['id'], 'decoder': 'envelope-mle', 'kind': spec['kind'],
                'circuit_text': text, 'source': source, 'loss_rate': spec['loss'],
                'shots': spec['shots'], 'seed': spec['seed']}
    record = resources.measure_case(binary, work, case)
    record['real_circuit'] = spec['real_circuit']
    record['circuit_params'] = {
        'distance': spec['distance'], 'rounds': spec['rounds'],
        'loss_rate': spec['loss'], 'batch': spec['shots']}
    return record


def mle_failure_cases(binary, work):
    """The MLE failure-semantics controls from the shared failure runner."""
    records = [r for r in resources.failure_cases(binary, work)
               if r['id'] in FAILURE_CASE_IDS]
    require(len(records) == len(FAILURE_CASE_IDS),
            'shared failure runner no longer produces the MLE controls: '
            + ', '.join(FAILURE_CASE_IDS))
    return records


def run_campaign(binary, plan_path, out_path):
    plan = json.loads(Path(plan_path).read_text())
    require(plan.get('schema_version') == 'rustqec.envelope-mle-scope.v1',
            'MLE candidate campaign requires the scope plan schema')
    budget = dict(BUDGET)
    started = time.perf_counter()
    records = []
    with tempfile.TemporaryDirectory(prefix='envelope-mle-candidate-') as tmp:
        work = Path(tmp)
        for spec in WORKLOADS:
            records.append(measure_workload(binary, work, spec))
        records += mle_failure_cases(binary, work)
    total_wall = time.perf_counter() - started
    problems = validate_records(records, budget, total_wall)
    result = {
        'schema_version': SCHEMA,
        'checkout_revision': resources.checkout_revision(),
        'generated_at': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
        'machine': resources.machine_identity(),
        'build': resources.build_identity(binary),
        'scope_plan': {'path': str(plan_path), 'sha256': digest(Path(plan_path)),
                       'source_revision': plan.get('applies_to', {}).get('source_revision')},
        'stress_budget': {'declared_before_run': True, 'values': budget},
        'total_wall_seconds': total_wall,
        'cases': records,
        'exclusions': EXCLUSIONS,
        'problems': problems,
        'status': 'pass' if not problems else 'fail',
    }
    save(out_path, result)
    return result


def validate_records(records, budget, total_wall):
    """Budget/output-rule/coverage validation shared by run and verify."""
    problems = []
    for record in records:
        problems += [f"{record['id']}: {p}" for p in record['output_rule_problems']]
        if record['wall_seconds'] > budget['per_case_wall_seconds']:
            problems.append(f"{record['id']}: wall {record['wall_seconds']:.1f}s "
                            'exceeds the declared budget')
        if record['peak_rss_watermark_bytes'] > budget['peak_rss_bytes']:
            problems.append(f"{record['id']}: peak RSS exceeds the declared budget")
    if total_wall > budget['total_wall_seconds']:
        problems.append(f'total wall {total_wall:.1f}s exceeds the declared budget')
    workloads = {r['id']: r for r in records if r['kind'] != 'failure-semantics'}
    for spec in WORKLOADS:
        record = workloads.get(spec['id'])
        if record is None:
            problems.append(f"missing declared workload {spec['id']}")
            continue
        if record['exit_code'] != 0:
            problems.append(f"{spec['id']}: declared workload did not succeed")
        if record.get('real_circuit') != spec['real_circuit']:
            problems.append(f"{spec['id']}: real_circuit flag drifted")
        if spec['real_circuit']:
            params = record.get('circuit_params') or {}
            if (params.get('distance'), params.get('rounds')) != (spec['distance'], spec['rounds']):
                problems.append(f"{spec['id']}: measured circuit shape drifted from the plan")
            if record['completed_shots'] != spec['shots']:
                problems.append(f"{spec['id']}: completed shots != declared batch")
    eviction = workloads.get('mle-eviction-wires24')
    if eviction and eviction['cache']['eviction_rebuilds_observed'] <= 0:
        problems.append('mle-eviction-wires24: cache-eviction case recorded no eviction')
    repeated = [r for r in workloads.values() if r['kind'] == 'repeated-patterns']
    for record in repeated:
        if record['cache']['hits'] <= 0:
            problems.append(f"{record['id']}: repeated-pattern case recorded no cache hits")
    failures = {r['id']: r for r in records if r['kind'] == 'failure-semantics'}
    for case_id, (code, exit_code) in EXPECTED_FAILURE_CODES.items():
        record = failures.get(case_id)
        if record is None:
            problems.append(f'missing failure-semantics case {case_id}')
        elif record['error_code'] != code or record['exit_code'] != exit_code \
                or record['completed_shots'] != 0:
            problems.append(f'{case_id}: expected {code} (exit {exit_code}) with zero '
                            'completed shots')
    timeout = failures.get('fail-mle-solve-timeout')
    if timeout and not timeout.get('compilation_outside_timeout'):
        problems.append('fail-mle-solve-timeout: compilation must stay outside the '
                        'solve-phase timeout (compile_seconds recorded, one attempted shot)')
    return problems


def verify_report(report_path):
    """Fast structural replay of a produced report; no decoder execution."""
    report = json.loads(Path(report_path).read_text())
    problems = []
    if report.get('schema_version') != SCHEMA:
        return [f"unsupported report schema: {report.get('schema_version')!r}"]
    for field in ('machine', 'build', 'scope_plan', 'stress_budget', 'cases', 'status'):
        if field not in report:
            problems.append(f'report missing {field}')
    if problems:
        return problems
    if not report['stress_budget'].get('declared_before_run'):
        problems.append('stress budget was not declared before the run')
    if report['stress_budget'].get('values') != BUDGET:
        problems.append('declared budget drifted from the campaign constants')
    if not report['build'].get('sha256') or not report['machine'].get('platform'):
        problems.append('machine/build identity incomplete')
    problems += validate_records(report['cases'], report['stress_budget']['values'],
                                 report['total_wall_seconds'])
    if report.get('status') != 'pass' or report.get('problems'):
        problems.append('report does not record a passing campaign')
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/release/rustqec')
    parser.add_argument('--plan', type=Path, default=PLAN_PATH)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--verify', type=Path)
    args = parser.parse_args()
    if args.verify is not None:
        problems = verify_report(args.verify)
        if not problems:
            print(f'{PASS_LINE} verify')
            raise SystemExit(0)
        print('FAIL envelope MLE candidate resources verify', file=sys.stderr)
        for problem in problems:
            print('  - ' + problem, file=sys.stderr)
        raise SystemExit(1)
    binary = args.binary.resolve()
    require(binary.is_file(), f'missing binary: {binary}')
    require(args.out is not None, '--out is required for a campaign run')
    result = run_campaign(binary, args.plan, args.out)
    if result['status'] == 'pass':
        print(f"{PASS_LINE} cases={len(result['cases'])} "
              f"wall={result['total_wall_seconds']:.1f}s")
        raise SystemExit(0)
    print('FAIL envelope MLE candidate resources', file=sys.stderr)
    for problem in result['problems']:
        print('  - ' + problem, file=sys.stderr)
    raise SystemExit(1)


if __name__ == '__main__':
    main()
