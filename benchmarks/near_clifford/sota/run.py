"""Freeze tuning, validate raw records, then rotate isolated timing processes."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
from lowering import lower, records_only, physical_width
from evidence import compact, parity_counts

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BACKENDS = ['rstim', 'clifft', 'clifft-scheduled', 'symft']
BATCHES = [1, 64, 256, 1024, 'auto']


def source_inventory():
    paths = list((ROOT/'rstim/src').rglob('*.rs'))
    paths += [ROOT/'Cargo.toml', ROOT/'Cargo.lock', ROOT/'rstim/Cargo.toml']
    return {str(path.relative_to(ROOT)): sha(path) for path in sorted(paths)}


def harness_inventory():
    return {str(path.relative_to(HERE)): sha(path) for path in sorted(HERE.rglob('*'))
        if path.is_file() and 'target' not in path.relative_to(HERE).parts
        and '__pycache__' not in path.relative_to(HERE).parts}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(command, timeout=180):
    result = subprocess.run(command, text=True, capture_output=True, timeout=timeout, check=True)
    return json.loads(result.stdout)


def masks(text, width):
    # Rich fixed set: every marginal, adjacent correlation, four contiguous
    # blocks, full parity, and each original detector/observable parity.
    result = {(i,) for i in range(width)}
    result.update((i, i+1) for i in range(width-1))
    result.update(tuple(range(i, min(i+4, width))) for i in range(0, width, 4))
    result.add(tuple(range(width)))
    record = 0
    for line in text.splitlines():
        words = line.split('#', 1)[0].split()
        if not words:
            continue
        gate = words[0].split('(')[0]
        if gate in ['M', 'MZ', 'MX', 'MY', 'MR', 'MRZ', 'MRX', 'MRY', 'MPP']:
            record += len(words)-1
        if gate in ['DETECTOR', 'OBSERVABLE_INCLUDE']:
            indices = []
            for token in words[1:]:
                if token.startswith('rec['):
                    index = record+int(token[4:-1])
                    if not 0 <= index < width:
                        raise ValueError('invalid record in validation mask')
                    if index in indices:
                        indices.remove(index)
                    else:
                        indices.append(index)
            result.add(tuple(sorted(indices)))
    if record != width:
        raise ValueError('record counter mismatch; this frozen corpus must not contain REPEAT')
    return sorted(result)


def histogram(payload, selected):
    return parity_counts(payload,selected)


def report(result):
    lines = ['# Exact near-Clifford raw-record comparison', '',
        f"Host: {result['host']}. Single CPU thread; full raw records, without postselection.",
        'Native competitor circuits; rstim uses disclosed exact MPP/readout lowering.',
        'Warm timings exclude output destruction; each raw observation accumulates at least 50 ms of public calls.',
        'Tuning and correctness are outside all measured windows. Ranges describe process medians, not confidence intervals.', '',
        '| Fixture | Shots | rstim ms | Fastest peer ms | rstim speedup | Paired speedup range | Fastest peer |',
        '| --- | ---: | ---: | ---: | ---: | --- | --- |']
    for case in result['cases']:
        if case.get('error'):
            lines.append(f"| {case['id']} | {case['shots']} | ERROR | ERROR | n/a | n/a | {case['error']} |")
            continue
        times = {backend: statistics.median(statistics.median(pair[backend]['warm_ns'])
            for pair in case['runs'])/1e6 for backend in BACKENDS}
        winner = min(BACKENDS[1:], key=times.get)
        ratios = [statistics.median(pair[winner]['warm_ns'])/statistics.median(pair['rstim']['warm_ns'])
            for pair in case['runs']]
        lines.append(f"| {case['id']} | {case['shots']} | {times['rstim']:.6f} | {times[winner]:.6f} | {times[winner]/times['rstim']:.4f}× | {min(ratios):.4f}–{max(ratios):.4f}× | {winner} |")
    return '\n'.join(lines)+'\n'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--python', type=Path, required=True, help='pinned Clifft/NumPy environment')
    p.add_argument('--symft-python', type=Path, required=True, help='pinned current SymFT source build')
    p.add_argument('--symft-source', type=Path, required=True, help='checkout used for that source build')
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--pairs', type=int, default=3)
    p.add_argument('--repetitions', type=int, default=7)
    p.add_argument('--validation-shots', type=int, default=8192)
    p.add_argument('--only', nargs='+')
    p.add_argument('--shots', nargs='+', type=int)
    args = p.parse_args()
    if args.pairs < 1 or args.repetitions < 1 or args.validation_shots < 1 or args.validation_shots % 1024:
        p.error('pairs/repetitions/validation shots must be positive')
    args.out = args.out.resolve()
    if args.out.exists():
        p.error('out must be a fresh nonexistent directory')
    manifest = json.loads((HERE/'manifest.json').read_text())
    selected = manifest['cases']
    if args.only:
        if set(args.only)-{c['id'] for c in selected}:
            p.error('unknown fixture')
        selected = [c for c in selected if c['id'] in args.only]
    if args.shots and (min(args.shots) < 1 or set(args.shots)-{1, 64, 1024}):
        p.error('shots must be from the frozen 1/64/1024 set')
    # Preserve a venv's executable symlink; resolving it selects the base
    # interpreter and loses the environment's installed simulator packages.
    args.python = args.python.absolute(); args.symft_python = args.symft_python.absolute()
    args.symft_source = args.symft_source.resolve()
    symft_revision = subprocess.check_output(['git','rev-parse','HEAD'], cwd=args.symft_source, text=True).strip()
    if symft_revision != 'c89b98514a919240b8afa53a271e08d926d3c987':
        p.error('SymFT source must be the frozen c89b985 revision')
    if subprocess.check_output(['git','diff','--name-only','HEAD'], cwd=args.symft_source, text=True):
        p.error('SymFT source has modified tracked files')
    os.environ.update(OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1', VECLIB_MAXIMUM_THREADS='1',
        MKL_NUM_THREADS='1', NUMEXPR_NUM_THREADS='1')
    sources = source_inventory()
    harness = harness_inventory()
    subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(HERE/'Cargo.toml')], check=True)
    if sources != source_inventory() or harness != harness_inventory():
        raise ValueError('source or harness changed while building')
    binary = HERE/'target/release/near-clifford-sota'
    args.out.mkdir(parents=True)
    package_command = "import importlib.metadata,pathlib,json,hashlib; names=['symft','numpy']; print(json.dumps({n:{'version':importlib.metadata.version(n),'files':{str(f):hashlib.sha256(pathlib.Path(importlib.metadata.distribution(n).locate_file(f)).read_bytes()).hexdigest() for f in importlib.metadata.distribution(n).files if str(f).endswith(('.so','.py','.json'))}} for n in names}))"
    packages = invoke([str(args.symft_python), '-c', package_command])
    package_command = package_command.replace("['symft','numpy']", "['clifft','numpy']")
    packages['clifft_environment'] = invoke([str(args.python), '-c', package_command])
    result = {'schema': 'rstim.near-clifford-sota-results.v1', 'started': datetime.now(timezone.utc).isoformat(),
        'host': platform.platform(), 'affinity': 'unavailable on macOS; no pinned-core claim',
        'python': sys.version, 'rustc': subprocess.check_output(['rustc','--version'],text=True).strip(),
        'source_revision': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        'source_files': sources, 'harness': harness, 'binary_sha256': sha(binary), 'packages': packages,
        'manifest_sha256': sha(HERE/'manifest.json'), 'symft_source_revision': symft_revision,
        'symft_source_files': {str(path.relative_to(args.symft_source)):sha(path)
            for name in subprocess.check_output(['git','ls-files'],cwd=args.symft_source,text=True).splitlines()
            if (path:=args.symft_source/name).is_file()},
        'pairs': args.pairs, 'repetitions': args.repetitions,
        'validation_shots': args.validation_shots, 'subset': bool(args.only or args.shots),
        'validation': [], 'tuning': [], 'cases': []}
    def save():
        (args.out/'results.json').write_text(json.dumps(result, indent=2)+'\n')
        (args.out/'report.md').write_text(report(result))
    def worker(backend, path, shots, batch='auto', mode='bench', repetitions=None, dump_total=None):
        if backend == 'rstim':
            command=[str(binary), str(path), str(shots), str(repetitions or args.repetitions), '3',
                'dump' if mode == 'dump' else 'bench']
            if dump_total is not None: command.append(str(dump_total))
            return invoke(command)
        python = args.symft_python if backend == 'symft' else args.python
        command=[str(python), str(HERE/'worker.py'), backend, str(path), str(shots),
            '--mode', mode, '--batch', str(batch), '--repetitions', str(repetitions or args.repetitions)]
        if dump_total is not None: command += ['--dump-total',str(dump_total)]
        return invoke(command)
    inputs = {}
    # Validate all cases and freeze all tuning before collecting final timings.
    for case in selected:
        text = (HERE/case['file']).read_text()
        if sha(HERE/case['file']) != case['sha256']:
            raise ValueError('frozen fixture changed')
        native = args.out/(case['id']+'-native.stim')
        adapted = args.out/(case['id']+'-rstim.stim')
        native.write_text(records_only(text))
        width=physical_width(text)
        inspected=worker('clifft',native,1,mode='inspect')
        if inspected['version'] != manifest['baseline_versions']['clifft'] or width != inspected['physical_width']:
            raise ValueError('independent physical width counter mismatch')
        adapted.write_text(lower(records_only(text), width))
        inputs[case['id']] = native, adapted
        validation = {'id':case['id'], 'native_sha256':sha(native), 'adapted_sha256':sha(adapted)}
        try:
            payloads = {backend: worker(backend, adapted if backend=='rstim' else native,
                args.validation_shots, mode='dump') for backend in BACKENDS}
            widths = {payload['width'] for payload in payloads.values()}
            if len(widths) != 1:
                raise ValueError('output widths disagree')
            selected_masks = masks(text, widths.pop())
            counts = {backend:histogram(payload, selected_masks) for backend,payload in payloads.items()}
            # Union bound over every pair and mask. Two-sample Hoeffding: each
            # empirical mean within eps of its true mean => difference <=2eps.
            # Six default pairs plus six pairs for each of three shot sizes.
            comparisons = 24*len(selected_masks)*len(selected)
            threshold = 2*math.sqrt(math.log(4*comparisons/0.001)/(2*args.validation_shots))
            max_difference = max(abs(counts[a][i]-counts[b][i])/args.validation_shots
                for i in range(len(selected_masks)) for a in BACKENDS for b in BACKENDS)
            if max_difference > threshold:
                raise ValueError(f'parity distribution mismatch {max_difference} > {threshold}')
            validation.update(width=payloads['rstim']['width'], physical_width=width,
                transcripts={b:compact(p) for b,p in payloads.items()}, masks=selected_masks, parity_counts=counts, threshold=threshold,
                max_difference=max_difference, passed=True)
            for backend,payload in payloads.items():
                artifact=args.out/(case['id']+'-'+backend+'-validation.json')
                artifact.write_text(json.dumps(payload)+'\n')
                validation.setdefault('payload_sha256',{})[artifact.name]=sha(artifact)
        except (subprocess.SubprocessError, ValueError) as error:
            validation.update(passed=False,error=str(error))
        result['validation'].append(validation); save()
        if not validation['passed']:
            for shots in args.shots or case['shots']:
                result['cases'].append({'id':case['id'],'shots':shots,'error':'correctness/compatibility failed'})
            continue
        for shots in args.shots or case['shots']:
            tuning = {'id':case['id'],'shots':shots,'backends':{}}
            chosen_rst=worker('rstim',adapted,shots,mode='dump',dump_total=args.validation_shots)
            reference=histogram(chosen_rst,selected_masks)
            tuning['rstim_parity_counts']=reference
            chosen_payloads={'rstim':chosen_rst}
            for backend in BACKENDS[1:]:
                trials=[]
                for batch in BATCHES:
                    try:
                        output=worker(backend,native,shots,batch,'tune',3)
                        trials.append({'batch':batch,'median_ns':statistics.median(output['warm_ns']),
                            'raw':output})
                    except subprocess.SubprocessError as error:
                        trials.append({'batch':batch,'error':str(error)})
                valid=[t for t in trials if 'median_ns' in t]
                if not valid:
                    raise ValueError('no valid competitor tuning configuration')
                winner=min(valid,key=lambda t:t['median_ns'])
                # Packed batching can change seeded streams. Validate the
                # selected configuration, not just each backend's default.
                chosen=worker(backend,native,shots,winner['batch'],'dump',dump_total=args.validation_shots)
                if chosen['width'] != validation['width']:
                    raise ValueError('selected output width mismatch')
                chosen_payloads[backend]=chosen
                chosen_counts=histogram(chosen,selected_masks)
                difference=max(abs(a-b)/args.validation_shots
                    for a,b in zip(chosen_counts,reference))
                if difference>threshold:
                    raise ValueError('tuned competitor failed distribution check')
                tuning['backends'][backend]={'selected_batch':winner['batch'],'trials':trials,
                    'selected_parity_counts':chosen_counts,'selected_max_difference':difference}
            all_counts=[reference]+[tuning['backends'][b]['selected_parity_counts'] for b in BACKENDS[1:]]
            if max(abs(a[i]-b[i])/args.validation_shots for a in all_counts for b in all_counts
                    for i in range(len(selected_masks))) > threshold:
                raise ValueError('timed-path parity distribution mismatch')
            for backend,payload in chosen_payloads.items():
                artifact=args.out/f"{case['id']}-{shots}-{backend}-selected.json"
                artifact.write_text(json.dumps(payload)+'\n')
                tuning.setdefault('selected_payload_sha256',{})[artifact.name]=sha(artifact)
            tuning['transcripts']={b:compact(p) for b,p in chosen_payloads.items()}
            result['tuning'].append(tuning); save()
    (args.out/'frozen-tuning.json').write_text(json.dumps(result['tuning'],indent=2)+'\n')
    result['frozen_tuning_sha256']=sha(args.out/'frozen-tuning.json')
    for index,tuning in enumerate(result['tuning']):
        native,adapted=inputs[tuning['id']]
        case={'id':tuning['id'],'shots':tuning['shots'],'runs':[]}
        for pair in range(args.pairs):
            order=BACKENDS[(index+pair)%4:]+BACKENDS[:(index+pair)%4]
            if pair%2:order=list(reversed(order))
            run={'order':order}
            for backend in order:
                batch='auto' if backend=='rstim' else tuning['backends'][backend]['selected_batch']
                run[backend]=worker(backend,adapted if backend=='rstim' else native,tuning['shots'],batch)
            case['runs'].append(run)
        result['cases'].append(case);save()
        print(tuning['id'],tuning['shots'],'complete',flush=True)
    if source_inventory()!=result['source_files']:
        raise ValueError('production source changed during the campaign')
    if harness_inventory()!=result['harness']:
        raise ValueError('harness changed during the campaign')
    if sha(binary)!=result['binary_sha256']:
        raise ValueError('binary changed during the campaign')
    result['finished']=datetime.now(timezone.utc).isoformat();save()


if __name__ == '__main__':
    main()
