"""Reject resealed corruptions in actual closed compact-replay evidence, offline."""
import contextlib
import copy
import io
import json
from pathlib import Path
import shutil
import tempfile

from verify_compact_zero_spans_scouts import (
    ROOT, digest, load, verify_archive, verify_cold, verify_peer_receipts,
)

RESULTS = ROOT / 'benchmarks/near_clifford/results'
M4 = RESULTS / 'apple-m4-compact-zero-spans-rust-ablation-2026-10-08'
X86 = RESULTS / 'linux-vm-x86-compact-zero-spans-2026-10-08'


def save(path, value):
    path.write_text(json.dumps(value))


def mutate_events(directory, mutation):
    path = directory / 'events.jsonl'
    events = [json.loads(line) for line in path.read_text().splitlines()]
    mutation(events)
    data = ''.join(json.dumps(event) + '\n' for event in events).encode()
    path.write_bytes(data)
    closure = load(directory / 'closure.json')
    closure.update(events=len(events), events_sha256=digest(data))
    save(directory / 'closure.json', closure)


def mutate_identity(directory, mutation):
    header = load(directory / 'header.json')
    mutation(header['identities']['candidate'])
    save(directory / 'header.json', header)
    closure = load(directory / 'closure.json')
    closure['identities_after'] = copy.deepcopy(header['identities'])
    save(directory / 'closure.json', closure)


def run():
    # Work only on disposable physical copies. No retained probe/binary executes.
    with tempfile.TemporaryDirectory() as temporary:
        scratch = Path(temporary)
        archive = scratch / 'm4'
        peer = scratch / 'x86-peer'
        shutil.copytree(M4, archive)
        shutil.copytree(X86, peer)
        snapshots = {base: {p.relative_to(base): p.read_bytes() for p in base.rglob('*') if p.is_file()}
                     for base in [archive, peer]}

        def restore(base):
            for relative, data in snapshots[base].items():
                (base / relative).write_bytes(data)

        def check(label, mutation, verification, expected, base=archive):
            restore(base)
            mutation()
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    verification()
            except ValueError as error:
                if expected not in str(error):
                    raise ValueError(f'{label}: rejected at unrelated check: {error}') from error
                print('PASS rejected', label, flush=True)
            else:
                raise ValueError('accepted resealed corruption: ' + label)

        warm = archive / 'warm'
        cold = archive / 'first-cold'
        full = lambda: verify_archive(archive)
        cold_check = lambda: verify_cold(archive, load(archive / 'bindings.json'))
        check('missing validation', lambda: mutate_events(warm, lambda e: e.pop(0)), full, 'time/events/digest')
        check('failed validation', lambda: mutate_events(warm, lambda e: e[0]['result'].update(native_counts_rng=False)), full, 'records/counts/RNG failed')
        def reorder(events):
            events[48], events[49] = events[49], events[48]
            for index, event in enumerate(events): event['index'] = index
        check('rotated schedule', lambda: mutate_events(warm, reorder), full, 'schedule')
        check('short warm observation', lambda: mutate_events(warm, lambda e: e[48]['result']['observations'][0].update(elapsed_ns=1, ns_per_call=1/e[48]['result']['observations'][0]['calls'])), full, 'invalid observation')
        def summary():
            value = load(warm / 'summary.json'); value[0]['speedup'] += 1; save(warm / 'summary.json', value)
        check('derived summary', summary, full, 'derived statistics')
        check('source digest with resealed closure', lambda: mutate_identity(warm, lambda i: i['sources'].update({'rstim/src/lib.rs':'0'*64})), full, 'original source/probe inventory')
        check('compiler environment', lambda: mutate_identity(warm, lambda i: i['environment'].update(RUSTFLAGS='')), full, 'environment')
        check('retained probe', lambda: (warm / 'probe/candidate/main.rs').write_bytes(b'corrupted source'), full, 'probe bytes')
        check('retained driver', lambda: (warm / 'original-driver.py').write_bytes(b'corrupted driver'), full, 'driver bytes')
        def relabel():
            restore(archive)
            shutil.copyfile(archive / 'same-binary-warm/header.json', warm / 'header.json')
            shutil.copyfile(archive / 'same-binary-warm/closure.json', warm / 'closure.json')
            shutil.copyfile(archive / 'same-binary-warm/events.jsonl', warm / 'events.jsonl')
            shutil.copyfile(archive / 'same-binary-warm/summary.json', warm / 'summary.json')
            shutil.copytree(archive / 'same-binary-warm/probe', warm / 'probe', dirs_exist_ok=True)
        check('A/A labelled A/B', relabel, full, 'label/schema')
        def wrong_null():
            directory = archive / 'same-binary-warm'
            header = load(directory / 'header.json'); header['identities']['baseline']['binary']='0'*64
            save(directory / 'header.json', header)
            closure=load(directory / 'closure.json'); closure['identities_after']=header['identities']; save(directory / 'closure.json',closure)
        check('distinct binary A/A', wrong_null, full, 'null control source/binary')
        check('cold validation', lambda: mutate_events(cold, lambda e: e[0]['result'].update(native_counts_rng=False)), cold_check, 'validation')
        check('cold seeds', lambda: mutate_events(cold, lambda e: e[48]['result']['observations'][0].update(seed=1)), cold_check, 'fixed seeds')
        check('cold accepted counts', lambda: mutate_events(cold, lambda e: e[48]['result']['observations'][0].update(accepted=e[48]['result']['observations'][0]['attempted']+1)), cold_check, 'cold counts')
        check('cold RNG continuation', lambda: mutate_events(cold, lambda e: e[48]['result']['observations'][0]['continuation'].__setitem__(0,e[48]['result']['observations'][0]['continuation'][0]^1)), cold_check, 'counts/RNG')
        check('cold cache budget', lambda: mutate_events(cold, lambda e: e[48]['result']['observations'][0].update(cache_reserved_bytes=64*1024*1024+1)), cold_check, 'cache cap')
        check('zero cold phase', lambda: mutate_events(cold, lambda e: e[48]['result']['observations'][0].update(compile_ns=0)), cold_check, 'positive fresh cold')
        def driver_and_bindings():
            data=b'changed cold driver'; (cold / 'original-driver.py').write_bytes(data)
            b=load(archive/'bindings.json');b['cold_driver_sha256']=digest(data);save(archive/'bindings.json',b)
            mutate_identity(cold,lambda i:i.update(driver_sha256=digest(data)))
        check('resealed cold driver and binding', driver_and_bindings, cold_check, 'frozen cold driver')
        def binary_receipt():
            p=peer/'production-binaries/receipt.json';r=load(p);r['binaries']['near-clifford-diagnostics']['sha256']='0'*64;save(p,r)
        peer_check=lambda:verify_peer_receipts(peer,'linux-vm-x86')
        check('peer binary receipt', binary_receipt, peer_check, 'binary digest receipt',peer)
        def peer_closure():
            p=peer/'closure.json';c=load(p);c['finished_utc']='changed';save(p,c)
        check('closed peer extraction receipt', peer_closure, peer_check, 'extraction binding',peer)
        def equivalence():
            p=peer/'runtime-source-equivalence.json';e=load(p);e['production_files'].pop('rstim/src/lib.rs');save(p,e)
        check('runtime equivalence inventory', equivalence, peer_check, 'equivalence inventory',peer)
        restore(archive);restore(peer)
        verify_archive(archive)
        verify_peer_receipts(peer,'linux-vm-x86')
        print('PASS all 21 resealed controls and restored full positive archives',flush=True)


if __name__ == '__main__':
    run()
