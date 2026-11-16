"""Decompose pinned aggregate acceptance; candidate owners never certify parity."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import coverage_rust as rust

import standalone_discovery as discovery
# Explicit semantic ranges in the pinned foundation main; helper context is retained whole.
MAIN = [(771, 'wide-clifford-elimination'),
 (833, 'word-shifts'),
 (853, 'history-efficiency'),
 (863, 'prepared-clifford'),
 (927, 'certificate-cache'),
 (939, 'categorical-law-interning'),
 (949, 'virtual-noise-actions'),
 (979, 'independent-proposal'),
 (988, 'paired-ratio'),
 (995, 'boundary-coordinate-rhs'),
 (999, 'literal-assignment-and-controls'),
 (1057, 'operation-sampling'),
 (1087, 'true-reported-records'),
 (1093, 'target-inverse'),
 (1099, 'recovery-ancilla'),
 (1108, 'file-target'),
 (1119, 'malformed-history-task'),
 (1137, 'file-causal-runtime'),
 (1183, 'online-classical-knowledge'),
 (1189, 'file-parser'),
 (1199, 'scaled-noise-model'),
 (1219, 'cached-noise-law-and-weight'),
 (1308, 'exact-categorical'),
 (1322, 'fixed-virtual-template'),
 (1333, 'selection-offset'),
 (1336, 'coherent-file-pipeline'),
 (1345, 'trace-purpose'),
 (1384, 'prepared-trace-runtime'),
 (1414, 'character-geometry'),
 (1471, 'reference-resource-transport'),
 (1498, 'strict-parity-representation'),
 (1532, 'dimension-changing-transport'),
 (1571, 'consuming-contraction'),
 (1599, 'finite-sum-contraction'),
 (1614, 'completed-pair-sampling'),
 (1636, 'sparse-noise-stream'),
 (1645, 'weight-sectors'),
 (1655, 'exact-statistics'),
 (1661, 'wide-exact-scalars'),
 (1672, 'caller-rng'),
 (1674, 'signed-affine-grouping'),
 (1678, 'coherent-expansion'),
 (1689, 'literal-callback-certificates'),
 (1717, 'affine-proposal-hull'),
 (1774, 'quadratic-radical-support')]

SCHEMA = 'rmagna.standalone-contracts.v1'
SOURCES = {'causal': 'tests/native/causal.cpp',
           'foundation': 'tests/native/foundation.cpp'}
AGGREGATES = {
    'causal': 'test_native_causal.NativeCausalTest.test_online_basis_without_cpython',
    'foundation': 'test_native_standalone.NativeStandaloneTest.test_foundation_without_cpython',
}
DISCOVERY_DIGESTS = {
    'causal': '613789a30c994716919d2c367b9925fab60392922b3042b24ac47ee3c266ebac',
    'foundation': '0aa6f936b04114564f1106dabc4618c3ef1a2f9af85368a2b7504a90df35d584',
}

ASSESSMENTS = {'validation_gap', 'implementation_gap', 'representation_difference', 'helper_context'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def compact_discovery(source):
    """Retain source identity, complete helper closure and every literal site."""
    found = discovery.discover(source)
    return dict(
        source_sha256=found['source_sha256'],
        helpers=found['functions'],
        assertions=[{key: value for key, value in a.items() if key != 'expression'}
                    for a in found['assertions']],
        counts=found['counts'],
    )


def bindings_from_facts(facts):
    """Explicit semantic subdivisions of the pinned main, full other helpers."""
    groups = []
    for name, found in facts.items():
        for helper in found['helpers']:
            spans = [(helper['line'], helper['end_line'], helper['name'])]
            if name == 'foundation' and helper['name'] == 'main':
                spans = [(line, MAIN[i+1][0]-1 if i+1 < len(MAIN) else helper['end_line'],
                          f'main.{label}') for i, (line, label) in enumerate(MAIN)]
            for lo, hi, label in spans:
                assertions = [a for a in found['assertions']
                              if a['function'] == helper['name'] and lo <= a['line'] <= hi]
                # A group's setup can reference other source groups, but its
                # assertion itself must never be cut across semantic ranges.
                require(all(a['end_line'] <= hi for a in assertions),
                        f'assertion crosses standalone group: {name}/{label}')
                groups.append(dict(
                    id=f'{name}/{label}', function=helper['name'], line=lo, end_line=hi,
                    assertions=[a['id'] for a in assertions],
                    loops=[i for i, loop in enumerate(helper['loops'])
                           if lo <= loop['line'] <= hi],
                ))
    return groups


def group_bindings(sources):
    facts = {name: compact_discovery(source) for name, source in sources.items()}
    return facts, bindings_from_facts(facts)


def check_catalogue(sources, catalogue, ledger, reference_manifest, reference_sha256, root):
    require(set(sources) == set(SOURCES), 'missing standalone source suite')
    facts, _ = group_bindings(sources)
    for name, path in SOURCES.items():
        require(facts[name]['source_sha256'] == reference_manifest['upstream']['files'][path],
                f'standalone pinned source changed: {path}')
    require(catalogue.get('sources') == facts, 'standalone helper/assertion/loop inventory changed')
    result = check_inventory(catalogue, ledger, reference_manifest, reference_sha256, root)
    result['upstream_verified'] = True
    return result


def check_inventory(catalogue, ledger, reference_manifest, reference_sha256, root):
    """Offline structural check of the exact retained discovery snapshot.

    Snapshot digests bind the complete original discovery, including empty
    helpers and loop-only oracles. The standalone command additionally parses
    the actual source again; this offline path never claims source execution
    or executable Rust coverage.
    """
    require(catalogue.get('schema') == SCHEMA, 'unsupported standalone schema')
    require(catalogue.get('claim') == 'decomposition_only_not_verified_parity',
            'standalone catalogue cannot certify aggregate parity')
    require(catalogue.get('reference_sha256') == reference_sha256,
            'standalone reference identity changed')
    require(catalogue.get('upstream_commit') == reference_manifest['upstream']['commit'],
            'standalone upstream commit changed')
    facts = catalogue.get('sources')
    require(isinstance(facts, dict) and set(facts) == set(SOURCES), 'missing standalone source inventory')
    for name, path in SOURCES.items():
        encoded = json.dumps(facts[name], sort_keys=True, separators=(',', ':')).encode()
        require(hashlib.sha256(encoded).hexdigest() == DISCOVERY_DIGESTS[name],
                f'standalone helper/assertion/loop inventory changed: {name}')
        require(facts[name]['source_sha256'] == reference_manifest['upstream']['files'][path],
                f'standalone pinned source changed: {path}')
    bindings = bindings_from_facts(facts)
    groups = catalogue.get('groups')
    require(isinstance(groups, list) and len(groups) == len(bindings),
            'missing or duplicate standalone assertion group')
    require(all(isinstance(g, dict) for g in groups), 'invalid standalone group')
    require(len({g.get('id') for g in groups}) == len(groups), 'duplicate standalone group')
    require({g.get('id') for g in groups} == {g['id'] for g in bindings},
            'missing or unknown standalone helper group')
    expected = {g['id']: g for g in bindings}
    for group in groups:
        require(all(group.get(key) == value for key, value in expected[group['id']].items()),
                f'standalone assertion/loop partition changed: {group["id"]}')
        require(group.get('status') == 'unresolved',
                'standalone groups require source-specific executable qualification before promotion')
        require(group.get('assessment') in ASSESSMENTS, 'unknown standalone assessment')
        require(type(group.get('follow_up_issue')) is int and group['follow_up_issue'] > 0,
                'missing standalone unresolved follow-up issue')
        require(isinstance(group.get('reason'), str) and group['reason'].strip(),
                'missing explicit unresolved standalone obligation')
        owners = group.get('candidate_owners')
        require(isinstance(owners, list), 'missing standalone owner assessment')
        if group['assessment'] == 'validation_gap':
            require(owners, 'validation gap must name existing Rust ownership candidate')
        for owner in owners:
            path = owner.get('path', '')
            parts = Path(path).parts
            require(path and not Path(path).is_absolute() and '..' not in parts
                    and '\\' not in path and path.endswith('.rs'),
                    'invalid relative standalone Rust owner')
            resolved = root / path
            require(resolved.is_file() and not resolved.is_symlink()
                    and resolved.resolve().is_relative_to(root.resolve())
                    and isinstance(owner.get('anchor'), str) and owner['anchor']
                    and owner['anchor'] in resolved.read_text(),
                    f'standalone Rust owner missing: {path}')
        witnesses = group.get('candidate_witnesses')
        require(isinstance(witnesses, list), 'missing standalone witness assessment')
        for witness in witnesses:
            # Declaration/source binding only. Reusing these already existing
            # tests never changes this group's unresolved executable status.
            rust.witness(root, witness)
    statuses = {row['id']: row['status'] for row in ledger['cases']}
    require(len(statuses) == len(ledger['cases']), 'duplicate ledger method')
    require(all(statuses.get(method) == 'unmapped' for method in AGGREGATES.values()),
            'unresolved standalone aggregate cannot be promoted')
    # Explicit partitions also protect assertion-free helpers and loop-only
    # oracles, which a require()-only scanner would otherwise silently lose.
    for name, found in facts.items():
        own = [g for g in groups if g['id'].startswith(name + '/')]
        actual = [a for g in own for a in g['assertions']]
        require(sorted(actual) == sorted(a['id'] for a in found['assertions']),
                'standalone assertion partition incomplete')
        helpers = {h['name']: h for h in found['helpers']}
        for helper in helpers.values():
            assigned = [i for g in own if g['function'] == helper['name'] for i in g['loops']]
            require(sorted(assigned) == list(range(len(helper['loops']))),
                    'standalone loop partition incomplete')
            require(set(helper['helper_calls']) <= set(helpers), 'missing standalone source helper')
    return dict(status='PASS', scope=catalogue['claim'], upstream_verified=False,
                helpers=sum(v['counts']['functions'] for v in facts.values()),
                groups=len(groups), covered_groups=0, unresolved_groups=len(groups),
                assertions=sum(v['counts']['assertions'] for v in facts.values()),
                covered_assertions=0,
                unresolved_assertions=sum(v['counts']['assertions'] for v in facts.values()),
                loop_headers=sum(v['counts']['loop_headers'] for v in facts.values()),
                assessments=dict(sorted(Counter(g['assessment'] for g in groups).items())),
                follow_up_issues=sorted({g['follow_up_issue'] for g in groups}),
                aggregate_statuses={name: statuses[method] for name, method in AGGREGATES.items()})
