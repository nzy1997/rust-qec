"""Verify retained native builds, source snapshots, wheels, and actual imports."""
import hashlib
import io
from pathlib import Path
import re
import subprocess
import tarfile

from peer_evidence import inventory, read, require, sha, verify_preparation
from wheel_bindings import bind_wheel
from commands import setup_commands


def git_inventory(repo, revision):
    archive = subprocess.check_output(['git', 'archive', revision], cwd=repo)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        return {member.name: hashlib.sha256(tar.extractfile(member).read()).hexdigest()
                for member in tar.getmembers() if member.isfile()
                and (member.name.endswith('.rs') or Path(member.name).name in {'Cargo.toml', 'Cargo.lock'})}


def verify_prepared(prep, root, *, git_sources=False):
    prep_sha = verify_preparation(prep)
    meta = read(prep/'preparation.json')
    manifest = read(prep/'manifest.json')
    original = Path(meta['preparation_directory'])
    repository = Path(meta['repository_directory'])
    protocol = Path(meta['protocol_directory'])
    require(original.is_absolute() and repository.is_absolute() and protocol == repository/'benchmarks/near_clifford/current_cpu_peer', 'original protocol paths differ')
    for key in ['protocol_revision', 'rust_source_head', 'symft_revision']:
        require(re.fullmatch('[0-9a-f]{40}', meta[key]) is not None and manifest[key] == meta[key], 'exact source refs differ')
    golden = read(Path(__file__).resolve().parent/'manifest.json')
    require(manifest == dict(golden, protocol_revision=meta['protocol_revision'], rust_source_head=meta['rust_source_head'], symft_revision=meta['symft_revision']), 'fixed native matrix differs')
    setup = []
    for path in sorted(prep.glob('*.receipt.json')):
        receipt = read(path)
        require(receipt['child_waited'] is True and type(receipt['exit_code']) is int and receipt['exit_code'] == 0
                and receipt['cancellation'] is None and receipt['started'] <= receipt['closed'], 'setup not successfully closed')
        stem = path.name.removesuffix('.receipt.json')
        require(sha(prep/(stem+'.stdout')) == receipt['stdout_sha256'] and sha(prep/(stem+'.stderr')) == receipt['stderr_sha256'], 'setup stream binding differs')
        require(all(receipt['environment'][k] == '1' for k in ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'RAYON_NUM_THREADS', 'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS']), 'preparation threads differ')
        setup.append(stem)
    required = {'gcc-version','g++-version','peer-official-refs','peer-clone','peer-checkout','peer-git-archive','clifft-venv','symft-venv','pinned-wheel-download','clifft-pinned-install','symft-pinned-install','native-symft-wheel','native-symft-install','clifft-package-inspection','symft-package-inspection','clifft-import-inspection','symft-import-inspection'}
    require(set(setup) == required, 'complete native setup receipt set differs')
    require(read(prep/'peer-checkout.receipt.json')['command'] == ['git','checkout','--detach',meta['symft_revision']], 'peer checkout command differs')
    require(read(prep/'peer-clone.receipt.json')['command'] == ['git','clone','--no-checkout','https://github.com/haoliri0/SOFT.git',str(original/'source')], 'official peer clone command differs')
    require(meta['symft_revision']+'\trefs/heads/symft-26-10-08' in (prep/'peer-official-refs.stdout').read_text(), 'available performance branch pin differs')
    candidates = list((prep/'wheels').glob('symft-*.whl'))
    require(len(candidates) == 1, 'unique native SymFT wheel required')
    interpreter = read(prep/'clifft-venv.receipt.json')['command'][0]
    require(Path(interpreter).is_absolute(), 'preparation interpreter must be absolute')
    commands = setup_commands(original, repository, interpreter, meta['symft_revision'], candidates[0].name)
    for label, command in commands.items():
        receipt = read(prep/(label+'.receipt.json'))
        expected_cwd = original/'source' if label in {'peer-checkout','peer-git-archive'} else repository
        require(receipt['command'] == command and receipt['cwd'] == str(expected_cwd), 'actual setup command/cwd differs '+label)
    native = read(prep/'native-symft-wheel.receipt.json')
    require(native['environment']['SYMFT_PY_NATIVE'] == '1' and native['environment']['SYMFT_PY_ENABLE_CUDA'] == '0'
            and native['environment']['CC'] == 'gcc' and native['environment']['CXX'] == 'g++', 'actual CPU native build environment differs')
    logs = (prep/'native-symft-wheel.stdout').read_bytes()+(prep/'native-symft-wheel.stderr').read_bytes()
    require(b'-march=native' in logs and b'SYMFT_CPP_NATIVE_BUILD=1' in logs, 'actual native compiler command flags missing')
    require(native['command'] == [str(original/'symft/bin/python'),'-I','-m','pip','wheel','--verbose','--no-cache-dir','--no-build-isolation','--no-deps','--wheel-dir',str(original/'wheels'),str(original/'source/python')], 'native wheel build command differs')
    peer = read(prep/'checkout-receipt.json')
    require(peer['head'] == meta['symft_revision'], 'peer snapshot ref differs')
    require(inventory(prep/'peer-sources') == peer['source_inventory'], 'full peer source snapshot differs')
    archive = (prep/'peer-git-archive.stdout').read_bytes()
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        entries = {m.name: dict(bytes=m.size,sha256=hashlib.sha256(tar.extractfile(m).read()).hexdigest()) for m in tar.getmembers() if m.isfile()}
    require(entries == peer['source_inventory'], 'Git archive and peer snapshots differ')
    packages = read(prep/'expected-environment.json')
    wheel_bindings = read(prep/'wheel-installed-bindings.json')
    for backend in ['clifft','symft']:
        installed = read(prep/(backend+'-package-inspection.stdout'))
        expected = packages['packages'] if backend == 'symft' else packages['packages']['clifft_environment']
        require(installed == {dist:expected[dist] for dist in [backend,'numpy']}, 'actual package inspection differs')
        identity = read(prep/(backend+'-import-inspection.stdout'))
        require(identity['isolated'] is True and identity['loaded_files'] == packages['identities'][backend], 'actual isolated import differs')
        modules = {backend,backend+('._native' if backend=='symft' else '._clifft_core')}
        require(set(identity['loaded_files']) == modules, 'actual import set differs')
        for dist in [backend,'numpy']:
            data = installed[dist]
            require(data['version'] == manifest['versions'][dist], 'pinned package version differs')
            first = next(iter(data['files']))
            require('/site-packages/' in first, 'installed root absent')
            site_root = Path(first.split('/site-packages/',1)[0]+'/site-packages')
            require(site_root.is_relative_to(original/backend), 'installed package outside owned environment')
            candidates = list((prep/'wheels').glob(dist+'-*.whl'))
            require(len(candidates) == 1, 'ambiguous retained wheel')
            binding = bind_wheel(candidates[0],data,original_site_packages=str(site_root),installed_root=prep/'installed-source'/backend/dist)
            require(binding == wheel_bindings[backend+'/'+dist], 'complete wheel binding differs')
        for module, identity in packages['identities'][backend].items():
            require(installed[backend]['files'].get(identity['path']) == identity['sha256'], 'import outside bound distribution')
            import_root = Path(identity['path'].split('/site-packages/',1)[0]+'/site-packages')
            relative = Path(identity['path']).relative_to(import_root)
            if module != backend:
                data = (prep/'installed-source'/backend/backend/relative).read_bytes()
                require(data[:5] == b'\x7fELF\x02' and len(data)>64 and int.from_bytes(data[18:20],'little') == 62, 'peer extension is not actual x86_64 ELF')
    rust = prep/'rust'
    verify_preparation(rust)
    rust_meta = read(rust/'preparation.json')
    require(rust_meta['protocol_revision'] == meta['protocol_revision'] and rust_meta['compiler'].startswith('rustc 1.93.1 ('), 'native Rust compiler/protocol differs')
    require(rust_meta['heads'] == {role:meta['rust_source_head'] for role in ['baseline','candidate','control']}, 'Rust source roles differ')
    require(rust_meta['preparation_directory'] == str(original/'rust') and rust_meta['protocol_directory'] == str(repository/'benchmarks/near_clifford/cdf_source_pair')
            and rust_meta['roots'] == {role:str(original/'rust/source'/('baseline' if role == 'control' else role)) for role in ['baseline','candidate','control']}, 'native Rust directories differ')
    source_hashes = None
    for role in ['baseline','candidate']:
        actual_sources = {name:entry['sha256'] for name,entry in inventory(rust/'production-sources'/role).items()}
        if git_sources:
            require(actual_sources == git_inventory(root,meta['rust_source_head']), 'Git/Rust source snapshot differs')
        source_hashes = actual_sources
        for kind, binary in [('counts','near-clifford-application-counts'),('structural','near-clifford-diagnostics')]:
            receipt = read(rust/role/('native-build-'+kind+'.receipt.json'))
            original_probe = original/'rust'/role/'native-probes'/kind
            require(receipt['command'] == ['rustup','run','1.93.1','cargo','build','--release','--locked','--manifest-path',str(original_probe/'Cargo.toml')]
                    and receipt['head'] == meta['rust_source_head'] and receipt['sources'] == receipt['sources_after'] == actual_sources
                    and receipt['child_waited'] is True and receipt['exit_code'] == 0 and receipt['timed_out'] is False and receipt['cancellation'] is None, 'native Rust build/source binding differs')
            require(receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and receipt['environment']['CARGO_ENCODED_RUSTFLAGS'] is None and receipt['environment']['CARGO_PROFILE_RELEASE_OPT_LEVEL'] is None
                    and receipt['environment']['CARGO_TARGET_DIR'] == str(original/'rust'/role/'native-target')
                    and receipt['binary']['path'] == str(original/'rust'/role/(binary+'.bin')), 'native Rust build flags differ')
            retained = rust/role/(binary+'.bin')
            data = retained.read_bytes()
            require(data[:5] == b'\x7fELF\x02' and len(data)>64 and int.from_bytes(data[18:20],'little') == 62
                    and len(data) == receipt['binary']['bytes'] and sha(retained) == receipt['binary']['sha256'] and receipt['binary']['mode'] == '0o755', 'actual native Rust ELF differs')
            require(sha(rust/role/('native-build-'+kind+'.log')) == receipt['log_sha256'], 'native Rust build log differs')
            for name,digest in receipt['probe'].items():
                require(sha(rust/role/'native-probes'/kind/name) == digest, 'native probe input differs')
            package = 'application_counts' if kind == 'counts' else 'diagnostics'
            public = rust/'protocol/benchmarks/near_clifford'/package
            probe = rust/role/'native-probes'/kind
            require(all((probe/name).read_bytes() == (public/name).read_bytes() for name in ['main.rs','Cargo.lock']), 'canonical public probe differs')
            original_manifest = (public/'Cargo.toml').read_text()
            needle = 'path = "../../../rstim"'
            require(original_manifest.count(needle) == 1 and (probe/'Cargo.toml').read_text() == original_manifest.replace(needle,'path = '+__import__('json').dumps(str(original/'rust/source'/role/'rstim'))), 'probe dependency path differs')
    for index in [0,1]:
        receipt = read(rust/('native-check-'+str(index)+'.receipt.json'))
        log = (rust/('native-check-'+str(index)+'.log')).read_text()
        require(receipt['child_waited'] is True and receipt['exit_code'] == 0 and receipt['cancellation'] is None
                and receipt['environment']['RUSTFLAGS'] == '-C target-cpu=native' and sha(rust/('native-check-'+str(index)+'.log')) == receipt['log_sha256'], 'native arithmetic/RNG check receipt differs')
        selection = (['--lib','phase_specialized_cdf_tests'] if index == 0 else ['--test','near_clifford_compiled','compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails','--','--exact'])
        require(receipt['command'] == ['rustup','run','1.93.1','cargo','test','--release','--locked','-p','rstim','--no-default-features',*selection], 'native check command/filter differs')
        require(sum(map(int,re.findall(r'test result: ok\. ([0-9]+) passed',log))) > 0, 'native check ran zero tests')
    if git_sources:
        for subtree in [prep,rust]:
            for name,entry in read(subtree/'seal.json')['files'].items():
                if name.startswith('protocol/'):
                    relative = str(Path(name).relative_to('protocol'))
                    payload = subprocess.check_output(['git','show',meta['protocol_revision']+':'+relative],cwd=root)
                    require(hashlib.sha256(payload).hexdigest() == entry['sha256'], 'Git/protocol snapshot differs')
    return dict(preparation_sha256=prep_sha,meta=meta,manifest=manifest,packages=packages,rust_sources=source_hashes,peer_sources=entries)
