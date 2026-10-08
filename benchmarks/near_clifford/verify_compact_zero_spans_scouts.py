"""Replay closed master-to-compact-replay source effects; never collect timings."""
import json
from functools import lru_cache
from pathlib import Path
import subprocess
import sys
from rust_pair_scout import NAMES, PROBE_INPUTS, ENV_KEYS, verify_closed_campaign
from verify_coefficient_intern_scouts import ROOT, digest, load, require, verify_campaign

BASELINE = 'e1453a7b634275d3055ef64b2f4a7b33d3995870'
CANDIDATE = 'd989531d837fe874a77ac05745f5d77ea5639d52'
PEER_X86 = '2a54d756b04b814fa1db129e27168615fb407b58'
COLD_DRIVER_SHA = '123d2520b3d4b6a1c408e500dad958630999810fb07d11e4ba227ae4fdae1142'
CAMPAIGNS = ['warm', 'confirmation', 'same-binary-warm', 'same-binary-confirmation']
FROZEN = 'benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-rust-ablation-2026-10-08/probe/'
ENVIRONMENT = {key: '-C target-cpu=native' if key == 'RUSTFLAGS' else '1' for key in ENV_KEYS}


@lru_cache(maxsize=512)
def git_bytes(ref, path):
    return subprocess.check_output(['git', 'show', ref + ':' + path], cwd=ROOT)


def verify_sources(directory, header, *, cold=False):
    null = header['schema'] == 'exploratory.same-binary-warm-control.v1'
    refs = {'baseline': CANDIDATE if null else BASELINE, 'candidate': CANDIDATE}
    prefix = 'drafts/compact-zero-spans-cold-probe/' if cold else 'drafts/rust-pair-scout-probe/'
    probe_name = 'cold-candidate' if cold else 'warm-candidate'
    mapping = {}
    for role, ref in refs.items():
        identity = header['identities'][role]
        require(identity['head'] == ref and identity['dirty'] == '', 'exact clean measured producer')
        require(identity['environment'] == ENVIRONMENT, 'recorded native compiler/thread environment')
        paths = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', ref, 'rstim/src'], cwd=ROOT, text=True).splitlines()
        paths = [p for p in paths if p.endswith('.rs')]
        paths += ['Cargo.toml', 'Cargo.lock', 'rstim/Cargo.toml']
        paths += ['benchmarks/near_clifford/application_counts/fixtures/' + n + '.stim' for n in NAMES]
        expected = {p: digest(git_bytes(ref, p)) for p in paths}
        expected.update({prefix + p: digest(git_bytes(CANDIDATE, FROZEN + probe_name + '/' + p)) for p in PROBE_INPUTS})
        require(identity['sources'] == expected, 'complete original source/probe inventory')
        require(len(identity['binary']) == 64 and all(x in '0123456789abcdef' for x in identity['binary']), 'measured binary digest')
        mapping[role] = {'source_equivalent_commit':ref,
                         'untracked_input_mapping':{prefix + p:str(Path(directory.name) / 'probe' / role / p) for p in PROBE_INPUTS}}
    return {'routes':mapping}


def verify_cold(archive, bindings):
    directory = archive / 'first-cold'
    header = load(directory / 'header.json')
    binding = verify_sources(directory, header, cold=True)
    require(header['schema'] == 'exploratory.strict-coefficient-pairs-cold.v1', 'original cold schema')
    require(set(header['retained']) == {'baseline','candidate'}, 'cold retained roles')
    for role, entries in header['retained'].items():
        require(set(entries) == set(PROBE_INPUTS) | {'build.log'}, 'cold seven inputs and original build log')
        for name, entry in entries.items():
            require(entry['path'] == str(Path('probe') / role / name), 'cold bounded snapshot path')
            require(digest((directory / entry['path']).read_bytes()) == entry['sha256'], 'cold retained bytes')
    require(bindings['cold_driver_sha256'] == COLD_DRIVER_SHA and digest((directory / 'original-driver.py').read_bytes()) == COLD_DRIVER_SHA, 'frozen cold driver digest')
    require(header['minimum_cold_observation_ns'] is None, 'cold observations have no warm duration threshold')
    require('Darwin' in header['host'] and 'release: 1.93.1\n' in header['rustc'], 'original M4 cold host/toolchain')
    events = [json.loads(line) for line in (directory / 'events.jsonl').read_text().splitlines()]
    for event in events:
        if event['action'] == 'cold':
            for observation in event['result']['observations']:
                require(all(type(observation[k]) is int and observation[k] > 0 for k in ['compile_ns','prepare_ns','first_ns']), 'positive fresh cold phase metadata')
    verify_campaign(archive, 'first-cold', binding, schema_prefix='strict-coefficient-pairs', cold_validation_prefix=True)


def verify_binary_receipts(archive, host):
    receipt = load(archive / 'warm-binary-receipt.json')
    confirmation = load(archive / 'confirmation/header.json')
    require(receipt['identities'] == confirmation['identities'], 'original warm binary identity receipt')
    for name in CAMPAIGNS:
        header = load(archive / name / 'header.json')
        require(header['identities']['candidate'] == receipt['identities']['candidate'], 'same candidate binary/source in every campaign')
        role = 'candidate' if name.startswith('same-binary') else 'baseline'
        require(header['identities']['baseline'] == receipt['identities'][role], 'measured A/B or A/A binary/source receipt')
    if host == 'linux-vm-x86':
        for kind in ['header', 'closure']:
            require(digest((archive / 'confirmation' / (kind + '.json')).read_bytes()) == receipt['measured_' + kind + '_sha256'], 'closed extraction receipt binding')
        require(set(receipt['retained']) == {'baseline','candidate'}, 'original extraction role inventory')
        for role, entry in receipt['retained'].items():
            require(entry['path'] == role + '.bin' and entry['sha256'] == receipt['identities'][role]['binary'], 'original measured ELF digest receipt')
        for name in ['direct-bits','highest-gather','gather-cdf','frozen-bits','both-policy-bits']:
            require('test result: ok. 1 passed;' in (archive / ('x86-scout-' + name + '.log')).read_text(), 'actual native coefficient/CDF gate ' + name)
        for name, count in [('compact-zero-spans',3),('public-counts',10)]:
            require(f'test result: ok. {count} passed;' in (archive / ('x86-scout-' + name + '.log')).read_text(), 'actual native counts/RNG gate ' + name)
        require({'avx2','fma'} <= set((archive / 'x86-scout-features.txt').read_text().split()), 'actual native CPU features')
    else:
        cold = load(archive / 'first-cold/header.json')
        require(load(archive / 'first-cold/production-binaries/receipt.json')['identities'] == cold['identities'], 'original cold binary identity receipt')
    # Executables and disassemblies stay in original ignored evidence/workflow
    # artifacts. Publication replay binds receipts; it does not hash absent binaries.


def verify_archive(archive):
    bindings = load(archive / 'bindings.json')
    host = bindings['host']
    require(host in ['apple-m4', 'linux-vm-x86'], 'supported measured host family')
    require(bindings['baseline'] == BASELINE and bindings['candidate'] == CANDIDATE, 'master/candidate source binding')
    require(bindings['campaigns'] == CAMPAIGNS + (['first-cold'] if host == 'apple-m4' else []), 'complete campaign inventory')
    for name in CAMPAIGNS:
        directory = archive / name
        header, closure = verify_closed_campaign(directory)
        expected_schema = 'exploratory.same-binary-warm-control.v1' if name.startswith('same-binary') else 'exploratory.avx2-pair-ablation.v1'
        require(header['schema'] == expected_schema, 'campaign label/schema binding')
        verify_sources(directory, header)
        require(('macOS' if host == 'apple-m4' else 'Linux') in header['host'], 'recorded host family')
        require(header['affinity'] == (None if host == 'apple-m4' else [0]), 'original collector affinity')
        require('release: 1.93.1\n' in header['rustc'], 'pinned measured Rust')
        expected_arch = 'aarch64-apple-darwin' if host == 'apple-m4' else 'x86_64-unknown-linux-gnu'
        require('host: ' + expected_arch + '\n' in header['rustc'], 'measured host architecture')
        require((directory / 'original-driver.py').read_bytes() == git_bytes(CANDIDATE, 'benchmarks/near_clifford/rust_pair_scout.py'), 'original measured warm driver Git bytes')
        print('PASS', host, name, 'complete288/source/probe/closure/semantics/statistics', flush=True)
    verify_binary_receipts(archive, host)
    if host == 'apple-m4':
        verify_cold(archive, bindings)
        print('PASS', host, 'first-cold', 'complete288/7680fresh-observations/source/probe/closure/semantics/statistics', flush=True)


def verify_peer_receipts(archive, host):
    """Bind portable receipts; actual executables remain in original artifacts."""
    require(host in ['apple-m4', 'linux-vm-x86'], 'supported measured peer host')
    header = load(archive / 'header.json')
    closure = load(archive / 'closure.json')
    receipt = load(archive / 'production-binaries/receipt.json')
    expected_ref = CANDIDATE if host == 'apple-m4' else PEER_X86
    require(header['source_revision'] == receipt['source_revision'] == expected_ref, 'exact measured peer producer')
    require(set(receipt['binaries']) == set(header['binary_sha256']) == {'near-clifford-application-counts', 'near-clifford-diagnostics'}, 'complete original peer binary receipt')
    for name, entry in receipt['binaries'].items():
        require(entry['path'] == name + '.bin' and entry['sha256'] == header['binary_sha256'][name] == closure['binary_sha256'][name], 'original measured peer binary digest receipt')
        require(type(entry['bytes']) is int and entry['bytes'] > 0, 'positive original binary byte length')
    if host == 'linux-vm-x86':
        for kind in ['header', 'closure']:
            require(digest((archive / (kind + '.json')).read_bytes()) == receipt['measured_' + kind + '_sha256'], 'original peer closed extraction binding')
        equivalence = load(archive / 'runtime-source-equivalence.json')
        require(equivalence['head'] == PEER_X86 and equivalence['runtime_source'] == CANDIDATE, 'measured runtime equivalence producers')
        paths = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', CANDIDATE, 'rstim/src'], cwd=ROOT, text=True).splitlines()
        paths = [p for p in paths if p.endswith('.rs')] + ['Cargo.toml', 'Cargo.lock', 'rstim/Cargo.toml']
        expected = {p: digest(git_bytes(CANDIDATE, p)) for p in paths}
        require(equivalence['production_files'] == expected, 'complete runtime equivalence inventory')
        require(all(digest(git_bytes(PEER_X86, p)) == sha for p, sha in expected.items()), 'immutable measured runtime byte equivalence')
        require({'avx2', 'fma'} <= set((archive / 'x86-scout-features.txt').read_text().split()), 'peer actual native CPU features')
        for name, count in [('direct-bits',1), ('highest-gather',1), ('gather-cdf',1), ('frozen-bits',1), ('both-policy-bits',1), ('compact-zero-spans',3), ('public-counts',10)]:
            require(f'test result: ok. {count} passed;' in (archive / ('x86-scout-' + name + '.log')).read_text(), 'peer actual native correctness gate ' + name)
    print('PASS', host, 'peer production receipts and immutable runtime equivalence', flush=True)


if __name__ == '__main__':
    require(len(sys.argv) <= 2, 'usage: verify_compact_zero_spans_scouts.py [ARCHIVE]')
    if len(sys.argv) == 2:
        verify_archive(Path(sys.argv[1]))
    else:
        for host in ['apple-m4','linux-vm-x86']:
            verify_archive(ROOT / 'benchmarks/near_clifford/results' / (host + '-compact-zero-spans-rust-ablation-2026-10-08'))

        for host in ['apple-m4', 'linux-vm-x86']:
            verify_peer_receipts(ROOT / 'benchmarks/near_clifford/results' / (host + '-compact-zero-spans-2026-10-08'), host)
