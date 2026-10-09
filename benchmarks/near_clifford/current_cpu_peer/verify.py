"""Seal first, then replay every fixed CPU-peer observation without live imports."""
import argparse
import collections
import hashlib
import itertools
import json
import math
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
from peer_evidence import inventory, preparation_pids, read, receipt_pids, require, seal_bundle, sha, verify_absence, verify_files
from provenance import verify_prepared
from commands import HOST_SCRIPT, THREADS, package_script
from validation import compact_equal, compare, counts, masks, replay, validate_observations


def options(backend):
    if backend == 'symft':
        return [{'cpu_backend':'legacy','batch':b} for b in ['scalar',1,64,256,1024,'auto']]+[{'cpu_backend':'compiled','batch':'auto'}]
    return [{'batch':b} for b in [1,64,256,1024,'auto']]


def schedule(manifest, selected):
    events = []
    for backend in ['clifft','symft']:
        events.extend([dict(kind='package-inspection',backend=backend),dict(kind='import-inspection',backend=backend)])
    events.append(dict(kind='host-inspection'))
    cases = list(itertools.product(manifest['names'],manifest['shots'],manifest['policies']))
    for name, shots in itertools.product(manifest['names'],manifest['shots']):
        for policy in manifest['policies']:
            events.append(dict(kind='counts-validation',backend='rstim',name=name,shots=shots,policy=policy,pair=None))
        for backend in ['clifft','clifft-scheduled','symft']:
            for selection in options(backend):
                events.append(dict(kind='tuning',backend=backend,name=name,shots=shots,selection=selection,pair=None,policy=None))
            selection = selected[name,shots][backend]
            events.extend([dict(kind='counts-validation',backend=backend,name=name,shots=shots,selection=selection,pair=None,policy=None),
                           dict(kind='raw-validation',backend=backend,name=name,shots=shots,selection=selection)])
    for pair in range(manifest['pairs']):
        ordered = cases[pair%len(cases):]+cases[:pair%len(cases)]
        if pair%2:
            ordered.reverse()
        roles = manifest['roles'][pair%len(manifest['roles']):]+manifest['roles'][:pair%len(manifest['roles'])]
        if pair%2:
            roles.reverse()
        for name,shots,policy in ordered:
            for role in roles:
                item = dict(kind='timing',backend=role,name=name,shots=shots,policy=policy,pair=pair)
                if role not in ['rstim','control']:
                    item['selection'] = selected[name,shots][role]
                events.append(item)
    for backend in ['clifft','symft']:
        events.extend([dict(kind='package-inspection',backend=backend),dict(kind='import-inspection',backend=backend)])
    return cases, events


def geomean(values):
    return math.exp(sum(map(math.log, values))/len(values))


def verify_bundle(root, *, git_sources=False):
    seal = read(root/'original-seal.json')
    verify_files(root,seal['files'])
    actual = inventory(root,bundle=True)
    actual.pop('original-seal.json',None)
    require(actual == seal['files'], 'complete original inventory changed')
    prep, output = root/'preparation',root/'output'
    prepared = verify_prepared(prep,ROOT,git_sources=git_sources)
    meta, manifest = prepared['meta'], prepared['manifest']
    original = Path(meta['preparation_directory'])
    repository = Path(meta['repository_directory'])
    protocol = Path(meta['protocol_directory'])
    outer = read(root/'control/closure.json')
    require(outer['child_waited'] is True and type(outer['exit_code']) is int and outer['exit_code'] == 0
            and outer['timed_out'] is False and outer['cancellation'] is None and outer['post_run_seal_error'] is None
            and outer['preparation_seal_before'] == outer['preparation_seal_after'] == prepared['preparation_sha256']
            and sha(root/'control/producer.log') == outer['producer_log_sha256'], 'actual outer closure differs')
    original_output = Path(outer['command'][-1])
    require(original_output.is_absolute() and len(outer['command']) == 7 and outer['command'][1:] ==
            ['-I',str(protocol/'run.py'),'--preparation',str(original),'--out',str(original_output)], 'actual producer command differs')
    header, closure = read(output/'header.json'),read(output/'closure.json')
    require(header['schema'] == manifest['schema'] and header['manifest'] == manifest
            and header['before'] == closure['before'] == closure['after']
            and header['preparation_seal_sha256'] == closure['preparation_seal_after'] == prepared['preparation_sha256']
            and header['controller_pid'] == closure['controller_pid'] == outer['child_pid']
            and closure['all_children_waited'] is True, 'producer source/closure differs')
    packages = prepared['packages']
    require(header['packages'] == closure['packages_after'] == packages['packages']
            and header['identities'] == closure['identities_after'] == packages['identities'], 'package/import closure differs')
    before = header['before']
    rust_build = read(prep/'rust/baseline/native-build-counts.receipt.json')
    require(before['protocol_revision'] == meta['protocol_revision'] and before['rust_build_head'] == meta['rust_source_head']
            and before['rust_sources'] == prepared['rust_sources'] and before['peer_sources'] == {n:m['sha256'] for n,m in prepared['peer_sources'].items()}
            and before['rust_binary_sha256'] == rust_build['binary']['sha256'] and before['inputs'] == manifest['inputs'], 'full measured source bindings differ')
    prefix = 'benchmarks/near_clifford/'
    protocol_names = {name.removeprefix('protocol/') for name in read(prep/'seal.json')['files']
                      if name.startswith('protocol/'+prefix+'current_cpu_peer/')}
    helpers = {prefix+name for name in ['diagnostics/source_contract.py','application_counts/common.py',
                'compiled_sota/run.py','compiled_sota/worker.py','compiled_sota/projection.py','compiled_sota/evidence.py']}
    expected_harness = {str(repository/name) for name in protocol_names|helpers}
    expected_harness |= {str(original/name) for name in ['manifest.json','expected-environment.json']}
    require(set(before['harness']) == expected_harness, 'complete executed harness inventory differs')
    for filename, digest in before['harness'].items():
        path = Path(filename)
        if path.is_relative_to(original):
            retained = prep/path.relative_to(original)
        else:
            retained = prep/'protocol'/path.relative_to(repository)
        require(sha(retained) == digest, 'executed harness binding differs')
    host = header['host']
    threads = THREADS
    require(host['uname'][0] == 'Linux' and host['uname'][4] == 'x86_64'
            and host['python'].startswith('3.12.') and all(outer['thread_environment'][k] == '1' for k in threads)
            and len(host['affinity']) == 1 and type(host['affinity'][0]) is int
            and all(host['thread_environment'][k] == '1' for k in threads), 'native host/affinity/threads differ')
    events = [json.loads(line) for line in (output/'events.jsonl').read_text().splitlines()]
    require(len(events) == 759 == closure['events'] and sha(output/'events.jsonl') == closure['events_sha256']
            and [e['index'] for e in events] == list(range(759)) and len({e['child_pid'] for e in events}) == 759, 'complete event ledger differs')
    frozen = read(output/'frozen-selections.json')
    keys = list(itertools.product(manifest['names'],manifest['shots']))
    require([(f['name'],f['shots']) for f in frozen] == keys, 'complete frozen selection inventory differs')
    selected = {(f['name'],f['shots']):f['backends'] for f in frozen}
    cases, expected = schedule(manifest,selected)
    require(len(expected) == 759 and all(all(e.get(k) == v for k,v in s.items()) for e,s in zip(events,expected)), 'exact finite/tuning/comparison schedule differs')
    fixtures = prep/'protocol/benchmarks/near_clifford/application_counts/fixtures'
    parsed = {}
    for name in manifest['names']:
        fixture = fixtures/(name+'.stim')
        require(sha(fixture) == manifest['inputs'][name], 'original fixture hash differs')
        parsed[name], projection = masks(fixture.read_text())
        require((output/(name+'.records.stim')).read_text() == projection, 'raw-record projection differs')
    payloads, parity = {},{}
    known = preparation_pids(prep)+receipt_pids(outer)
    observations = collections.Counter()
    for e in events:
        index = e['index']
        require(e['child_waited'] is True and e['controller_pid'] == header['controller_pid'] and e['cancellation'] is None
                and e['result_compaction_error'] is None and e['start'] <= e['end']
                and (index == 0 or events[index-1]['end'] <= e['start']), 'worker lifecycle/serial chronology differs')
        known.extend(receipt_pids(e))
        for stream in ['stdout','stderr']:
            require(sha(output/f'{index:05d}.{stream}') == e[stream+'_sha256'], 'actual worker streams differ')
        successful = type(e['exit_code']) is int and e['exit_code'] == 0 and e['timed_out'] is False
        require(type(e['exit_code']) is int and type(e['timed_out']) is bool
                and e['process_status'] == ('timeout-group-killed-and-waited' if e['timed_out'] else 'closed'), 'actual worker closure status differs')
        if e['kind'] != 'tuning':
            require(successful and e['process_status'] == 'closed', 'required native worker did not successfully close')
        try:
            payload = read(output/f'{index:05d}.stdout')
        except (ValueError,UnicodeDecodeError):
            require(e['kind'] == 'tuning' and not successful and e['result'] is None, 'invalid successful JSON worker')
            payload = None
        payloads[index] = payload
        if payload and 'measurements' in payload:
            compact_equal(payload,e['result'])
            parity[index] = replay(payload,parsed[e['name']])
        else:
            require(e['result'] == payload, 'original worker payload differs')
        kind = e['kind']
        backend = e.get('backend')
        if kind in ['timing','tuning','counts-validation','raw-validation']:
            name, shots = e['name'],e['shots']
            circuit = str(repository/'benchmarks/near_clifford/application_counts/fixtures'/(name+'.stim'))
            if backend in ['rstim','control']:
                command = [str(original/'rust/baseline/near-clifford-application-counts.bin'),circuit,str(shots),'1' if kind == 'counts-validation' else '7',e['policy'],'validate' if kind == 'counts-validation' else 'bench','native']
            else:
                interpreter = str(original/('symft' if backend == 'symft' else 'clifft')/'bin/python')
                selection = e['selection']
                if kind == 'raw-validation':
                    total = ((8192+shots-1)//shots)*shots
                    command = [interpreter,'-I',str(repository/'benchmarks/near_clifford/compiled_sota/worker.py'),backend,str(original_output/(name+'.records.stim')),str(shots),'--batch',str(selection['batch']),'--mode','dump','--dump-total',str(total)]
                else:
                    command = [interpreter,'-I',str(protocol/'worker.py'),backend,circuit,str(shots),'--batch',str(selection['batch']),'--cpu-backend',selection.get('cpu_backend','legacy'),'--repetitions','1' if kind == 'counts-validation' else '7']
                    if kind == 'counts-validation':
                        command.append('--validate')
            require(e['command'] == command, 'actual worker executable/input/native route differs')
            if successful and payload and payload.get('status') == 'ok' and kind != 'raw-validation':
                validate_observations(payload,e)
                observations[kind] += len(payload['observations'])
                require(payload['input_sha256'] == manifest['inputs'][name], 'actual original input differs')
            else:
                require(kind in ['raw-validation','tuning'], 'selected candidate silently declined')
            if backend not in ['rstim','control'] and successful and payload:
                module = 'symft' if backend == 'symft' else 'clifft'
                require(payload['isolated'] is True and payload['loaded_files'] == header['identities'][module], 'actual pinned imported peer differs')
                if kind != 'raw-validation' and payload.get('status') == 'ok' and module == 'symft':
                    info = payload['sampler_info']
                    require(payload['cpu_backend'] == selection['cpu_backend'] and info['cpu_compiled'] == (selection['cpu_backend'] == 'compiled')
                            and info['threads'] == 1 and info['detector_postselection'] is True and info['reference_normalized'] is False, 'requested compiled CPU/threads/raw counts semantics differ')
            if kind == 'counts-validation' and backend == 'rstim':
                require(parity[index] == counts(payload), 'Rust literal raw parity differs')
            if kind == 'raw-validation':
                require(payload['input_sha256'] == sha(output/(name+'.records.stim')), 'actual raw projected input differs')
        elif kind == 'host-inspection':
            require(payload == host and e['command'] == [outer['command'][0],'-I','-c',HOST_SCRIPT], 'actual host worker differs')
        elif kind == 'package-inspection':
            require(e['command'] == [str(original/backend/'bin/python'),'-I','-c',package_script([backend,'numpy'])], 'actual package inspection executable differs')
            environment = header['packages'] if backend == 'symft' else header['packages']['clifft_environment']
            require(payload == {n:environment[n] for n in [backend,'numpy']}, 'actual package worker differs')
        elif kind == 'import-inspection':
            require(e['command'] == [str(original/backend/'bin/python'),'-I',str(repository/'benchmarks/near_clifford/compiled_sota/worker.py'),backend,str(repository/'benchmarks/near_clifford/compiled_sota/manifest.json'),'1','--mode','identity'], 'actual import inspection executable differs')
            require(payload['isolated'] is True and payload['loaded_files'] == header['identities'][backend], 'actual import worker differs')
    require(observations['timing'] == 4200 and observations['counts-validation'] == 30 and closure['timing_children'] == 600, 'complete required observation counts differ')
    for name,shots in keys:
        for backend in ['clifft','clifft-scheduled','symft']:
            trials = [e for e in events if e['kind'] == 'tuning' and (e['name'],e['shots'],e['backend']) == (name,shots,backend)]
            valid = [e for e in trials if e['exit_code'] == 0 and not e['timed_out'] and payloads[e['index']] and payloads[e['index']].get('status') == 'ok']
            require(bool(valid), 'no validated tuning candidate')
            best = min(valid,key=lambda e:statistics.median(o['ns_per_call'] for o in payloads[e['index']]['observations']))
            require(best['selection'] == selected[name,shots][backend], 'selection differs from prespecified tuning minimum')
    checks = [json.loads(line) for line in (output/'validation-checks.jsonl').read_text().splitlines()]
    require([(r['name'],r['shots'],r['backend']) for r in checks] == [(n,s,b) for n,s in keys for b in ['clifft','clifft-scheduled','symft']], 'full finite comparison inventory differs')
    alpha = .001/(12*3*12)
    for row in checks:
        name, shots, backend = row['name'],row['shots'],row['backend']
        match = lambda e,b,k: e['kind'] == k and (e['name'],e['shots'],e['backend']) == (name,shots,b)
        native = counts(payloads[next(e['index'] for e in events if match(e,backend,'counts-validation'))])
        own = parity[next(e['index'] for e in events if match(e,backend,'raw-validation'))]
        references = {p:parity[next(e['index'] for e in events if match(e,'rstim','counts-validation') and e['policy'] == p)] for p in manifest['policies']}
        expected_checks = dict(own_records=compare(native,own,alpha),against_rust={p:compare(native,r,alpha) for p,r in references.items()})
        require(row['selection'] == selected[name,shots][backend] and row['alpha_per_population'] == alpha and row['counts'] == native
                and row['own_records'] == own and row['rust'] == references and row['checks'] == expected_checks
                and expected_checks['own_records']['passed'] and all(v['passed'] for v in expected_checks['against_rust'].values()), 'finite raw-count/Hoeffding replay differs')
    validate_end = max(e['end'] for e in events if e['kind'] in ['counts-validation','tuning','raw-validation'])
    require(validate_end < min(e['start'] for e in events if e['kind'] == 'timing'), 'selection/finite validation did not precede comparisons')
    summary = []
    for name,shots,policy in cases:
        timing = [e for e in events if e['kind'] == 'timing' and (e['name'],e['shots'],e['policy']) == (name,shots,policy)]
        values = {role:[statistics.median(o['ns_per_call'] for o in payloads[next(e['index'] for e in timing if e['backend'] == role and e['pair'] == pair)]['observations']) for pair in range(10)] for role in manifest['roles']}
        ratios = {role:[p/r for p,r in zip(values[role],values['rstim'])] for role in ['clifft','clifft-scheduled','symft','control']}
        summary.append(dict(name=name,shots=shots,policy=policy,per_round_ns=values,per_round_over_rust=ratios,geomean_over_rust={r:geomean(v) for r,v in ratios.items()},median_ns={r:statistics.median(v) for r,v in values.items()}))
    verify_absence(read(root/'control/preparation-process-absence.json'),preparation_pids(prep))
    verify_absence(read(root/'process-absence.json'),known)
    verify_files(root,seal['files'])
    return dict(cases=summary,events=759,finite_comparisons=18,timing_workers=600,timing_observations=4200,
                successful_tuning_observations=observations['tuning'],original_seal_sha256=sha(root/'original-seal.json'),
                scope='Fixed12 native CPU counts cells, all observations/outliers retained; finite unconditional bounds only; counts RNG literal words absent; no general SOTA/full19 admission')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle',type=Path)
    parser.add_argument('--seal',action='store_true')
    parser.add_argument('--git-sources',action='store_true')
    parser.add_argument('--analysis',type=Path)
    args = parser.parse_args()
    root = args.bundle.resolve()
    if args.seal:
        require(args.analysis is None, 'seal before analysis')
        seal_bundle(root)
        print('original native peer bundle sealed')
    else:
        result = verify_bundle(root,git_sources=args.git_sources)
        if args.analysis:
            require(args.analysis.resolve().is_relative_to(root/'analysis'), 'analysis output must be inside excluded bundle analysis directory')
            args.analysis.parent.mkdir(parents=True,exist_ok=True)
            args.analysis.write_text(json.dumps(result,indent=2)+'\n')
        print('verified 759 events, 18 finite comparisons, 600 timing workers, 4200 timing observations')


if __name__ == '__main__':
    main()
