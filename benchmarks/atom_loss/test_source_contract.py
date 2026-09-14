"""Adversarial source-binding and actual decoder replay regression tests."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile
from .source_contract import clean_source, verify_source, ROOT


class SourceContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git('init', '-q')
        self.git('config', 'user.email', 'test@example.invalid')
        self.git('config', 'user.name', 'Source test')
        self.write('Cargo.toml', '[workspace]\nmembers = ["decoder"]\n')
        self.write('decoder/src/lib.rs', '// measured decoder\n')
        self.write('decoder/build.rs', '// measured build\n')
        self.write('benchmarks/atom_loss/run.py', '# measured harness\n')
        self.write('benchmarks/atom_loss/requirements.txt',
                   (ROOT/'benchmarks/atom_loss/requirements.txt').read_text())
        self.commit()
        self.record = clean_source(self.repo)

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.STDOUT)

    def write(self, name, text):
        path = self.repo/name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(text)

    def commit(self):
        self.git('add', '.'); self.git('commit', '-qm', 'test source')

    def test_later_artifact_commit_valid_but_changed_decoder_fails(self):
        self.write('site/static/data/result.json', '{}'); self.commit()
        self.assertEqual(verify_source(self.record, self.repo), self.record['source_commit'])
        self.write('decoder/src/lib.rs', '// different decoder\n'); self.commit()
        with self.assertRaisesRegex(ValueError, 'Current source/build inputs differ'):
            verify_source(self.record, self.repo)

    def test_missing_inventory_and_new_build_input_rejected(self):
        record = copy.deepcopy(self.record); del record['inputs']['decoder/build.rs']
        with self.assertRaisesRegex(ValueError, 'source inventory'):
            verify_source(record, self.repo)
        self.write('.cargo/config.toml', '[build]\nrustflags = ["-C", "opt-level=0"]\n'); self.commit()
        with self.assertRaisesRegex(ValueError, 'Current source/build inputs differ'):
            verify_source(self.record, self.repo)

    def test_dirty_generation_and_dirty_verification_rejected(self):
        self.write('benchmarks/atom_loss/run.py', '# changed harness\n')
        with self.assertRaisesRegex(ValueError, 'clean checkout'):
            clean_source(self.repo)
        with self.assertRaisesRegex(ValueError, 'Dirty source input'):
            verify_source(self.record, self.repo)

    def test_new_untracked_build_input_is_rejected(self):
        self.write('.cargo/config.toml', '[build]\nrustflags = []\n')
        with self.assertRaisesRegex(ValueError, 'Uncommitted source input'):
            verify_source(self.record, self.repo)

    def test_ignored_local_cargo_config_is_rejected(self):
        from .source_contract import build_environment
        self.write('.git/info/exclude', '.cargo/\n')
        self.write('.cargo/config.toml', '[profile.release]\nopt-level=0\n')
        for check in [lambda:clean_source(self.repo), lambda:verify_source(self.record,self.repo),
                      lambda:build_environment(self.repo,self.repo.parent/'unused-cargo-home')]:
            with self.assertRaisesRegex(ValueError, 'Cargo configuration must be tracked'):
                check()

    def test_optimized_python_rejects_old_results_after_source_change(self):
        record_path = self.repo/'record.json'; record_path.write_text(json.dumps(self.record))
        self.write('decoder/src/lib.rs', '// changed\n'); self.commit()
        program = ('from pathlib import Path; import json; '
                   'from benchmarks.atom_loss.source_contract import verify_source; '
                   'verify_source(json.loads(Path(__import__("sys").argv[1]).read_text()), Path(__import__("sys").argv[2]))')
        for flags in [[], ['-O']]:
            result = subprocess.run([sys.executable, *flags, '-c', program, str(record_path), str(self.repo)], capture_output=True, text=True, cwd=ROOT)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Current source/build inputs differ', result.stderr)

    def runtime_bundle(self):
        """A committed source fixture with internally consistent historical metadata."""
        from .source_contract import BINARIES, CAPTURE_DEPENDENCIES, THREAD_ENVIRONMENT
        out = self.repo/'evidence'; out.mkdir()
        binding = copy.deepcopy(self.record)
        binding.update(binaries={name:'a'*64 for name in BINARIES},
                       rustc='recorded historical rustc, not the verifier compiler',
                       build_environment={'PATH':'/recorded/bin', 'HOME':'/recorded/home',
                           'CARGO_HOME':'/recorded/cargo', 'LANG':'C', 'LC_ALL':'C', **THREAD_ENVIRONMENT})
        requirements = dict(line.split('==') for line in
                            (self.repo/'benchmarks/atom_loss/requirements.txt').read_text().splitlines()
                            if line and not line.startswith('#'))
        sources = {name:(self.repo/name).read_text() for name in binding['inputs']
                   if name.startswith('benchmarks/atom_loss/')}
        stage = {'source_commit':binding['source_commit'], 'working_tree_dirty':False,
                 'input_digest':binding['input_digest'], 'binaries':binding['binaries'],
                 'rustc':binding['rustc'], 'dependencies':{p:requirements[p] for p in CAPTURE_DEPENDENCIES},
                 'environment':dict(THREAD_ENVIRONMENT), 'cpu':'Recorded historical CPU',
                 'os':'Recorded historical OS', 'python':'Recorded historical Python',
                 'sources':{name:hashlib.sha256(text.encode()).hexdigest() for name,text in sources.items()}}
        (out/'source-manifest.json').write_text(json.dumps(binding))
        for suffix in ['all','seeds']:
            (out/f'provenance-{suffix}.json').write_text(json.dumps(stage))
            snapshot = 'source-snapshot.json' if suffix=='all' else 'source-snapshot-seeds.json'
            (out/snapshot).write_text(json.dumps({'base_commit':binding['source_commit'], 'files':sources}))
        return out, binding, stage

    def test_runtime_provenance_contract_in_normal_and_optimized_python(self):
        out, binding, stage = self.runtime_bundle()
        program = ('from pathlib import Path; import sys; '
                   'from benchmarks.atom_loss.source_contract import verify_bundle_source; '
                   'print(verify_bundle_source(Path(sys.argv[1]), Path(sys.argv[2])))')
        def verify(flags):
            return subprocess.run([sys.executable,*flags,'-c',program,str(out),str(self.repo)],
                                  capture_output=True,text=True,cwd=ROOT)
        defects = {
            'dependency_version': (lambda r:r['dependencies'].update(pymatching='999.0.0'), 'dependencies'),
            'missing_dependency': (lambda r:r['dependencies'].pop('stim'), 'dependencies'),
            'extra_dependency': (lambda r:r['dependencies'].update(unrecorded='1.0'), 'dependencies'),
            'compiler': (lambda r:r.update(rustc='contradicts manifest'), 'compiler'),
            'missing_compiler': (lambda r:r.pop('rustc'), 'compiler'),
            'cpu': (lambda r:r.update(cpu='different CPU'), 'runtime'),
            'os': (lambda r:r.update(os='different OS'), 'runtime'),
            'python': (lambda r:r.update(python='different Python'), 'runtime'),
            'missing_cpu': (lambda r:r.pop('cpu'), 'runtime'),
            'non_string_os': (lambda r:r.update(os=123), 'runtime'),
            'empty_python': (lambda r:r.update(python='  '), 'runtime'),
            'thread_count': (lambda r:r['environment'].update(OMP_NUM_THREADS='999'), 'threading'),
            'numeric_thread_count': (lambda r:r['environment'].update(OPENBLAS_NUM_THREADS=1), 'threading'),
            'missing_thread_count': (lambda r:r['environment'].pop('RAYON_NUM_THREADS'), 'threading'),
            'extra_thread_setting': (lambda r:r['environment'].update(UNKNOWN_THREADS='1'), 'threading'),
        }
        for flags in [[],['-O']]:
            # A different but internally consistent historical host is accepted.
            result = verify(flags)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(result.stdout.strip(),binding['source_commit'])
            for name,(mutate,error) in defects.items():
                with self.subTest(flags=flags,defect=name):
                    changed=copy.deepcopy(stage);mutate(changed)
                    (out/'provenance-seeds.json').write_text(json.dumps(changed))
                    result=verify(flags)
                    self.assertNotEqual(result.returncode,0)
                    self.assertIn('ValueError',result.stderr)
                    self.assertIn(error,result.stderr)
            (out/'provenance-seeds.json').write_text(json.dumps(stage))
            # Agreement between stages cannot override requirements/build/thread pins.
            for name in ['dependency_version','compiler','thread_count']:
                with self.subTest(flags=flags,all_stages=name):
                    changed=copy.deepcopy(stage);mutate,error=defects[name];mutate(changed)
                    for suffix in ['all','seeds']:
                        (out/f'provenance-{suffix}.json').write_text(json.dumps(changed))
                    result=verify(flags)
                    self.assertNotEqual(result.returncode,0)
                    self.assertIn(error,result.stderr)
            for suffix in ['all','seeds']:
                (out/f'provenance-{suffix}.json').write_text(json.dumps(stage))

    def test_runtime_provenance_requires_a_recorded_build_compiler(self):
        from .source_contract import verify_bundle_source
        out, binding, _ = self.runtime_bundle()
        for value in [None, '', '  ', 123]:
            with self.subTest(value=value):
                binding['rustc']=value
                (out/'source-manifest.json').write_text(json.dumps(binding))
                with self.assertRaisesRegex(ValueError,'recorded build compiler'):
                    verify_bundle_source(out,self.repo)


class DecoderReplayTests(unittest.TestCase):
    def test_actual_wrong_decoder_with_unchanged_archive_is_rejected(self):
        from .decoder_replay import replay_case
        from .shot_data import cases_from
        binary = ROOT/'target/release/rustqec'
        exporter = ROOT/'target/release/examples/export_matching_benchmark'
        with tempfile.TemporaryDirectory() as tmp, zipfile.ZipFile(ROOT/'site/static/data/atom-loss/shot-data-v1.zip') as z:
            path = Path(tmp); label, case = cases_from(z.read)[0]
            # Positive control exercises real current compiler + all four backends.
            self.assertEqual(replay_case(z, label, case, path/'good', binary, exporter, False), 4)
            wrapper = path/'wrong-decoder'
            wrapper.write_text('#!'+sys.executable+'\nimport pathlib, subprocess, sys\n'
                +'subprocess.run(['+repr(str(binary))+', *sys.argv[1:]], check=True)\n'
                +'p=pathlib.Path(sys.argv[sys.argv.index("--out")+1])\n'
                +'b=bytearray(p.read_bytes()); b[0]^=1; p.write_bytes(b)\n')
            wrapper.chmod(0o755)
            with self.assertRaisesRegex(ValueError, 'Current decoder differs from archive'):
                replay_case(z, label, case, path/'bad', wrapper, exporter, False)


class BuildEnvironmentTests(unittest.TestCase):
    def test_real_entrypoint_ignores_inherited_profile_override(self):
        """A real release build must stay optimized despite ambient profile flags."""
        import os
        from unittest.mock import patch
        from . import evidence_run, source_contract
        real_run = subprocess.run
        class BuildComplete(Exception):
            pass
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp); repo = base/'repo'; repo.mkdir()
            files = {
                'Cargo.toml': '[workspace]\nmembers=["rustqec-cli"]\nresolver="2"\n',
                'rustqec-cli/Cargo.toml': '[package]\nname="rustqec-cli"\nversion="0.1.0"\nedition="2021"\n[features]\nbenchmark-tools=[]\nilp=[]\n[[bin]]\nname="rustqec"\npath="src/main.rs"\n',
                'rustqec-cli/src/main.rs': 'fn main(){println!("debug_assertions={}",cfg!(debug_assertions));}\n',
                'benchmarks/atom_loss/requirements.txt': '',
            }
            for name in ['export_matching_benchmark','export_decoder_oracle','offline_matching_benchmark']:
                files[f'rustqec-cli/examples/{name}.rs'] = 'fn main(){}\n'
            for name, data in files.items():
                path = repo/name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(data)
            real_run(['cargo','generate-lockfile'], cwd=repo, check=True, capture_output=True)
            for args in [['init','-q'],['config','user.email','test@example.invalid'],['config','user.name','Test'],['add','.'],['commit','-qm','fixture']]:
                real_run(['git',*args], cwd=repo, check=True, capture_output=True)
            (repo/'.git/info/exclude').write_text('target/\n')
            def build_then_stop(args, **kwargs):
                if list(args) == source_contract.BUILD_COMMANDS[0]:
                    result = real_run(args, **kwargs, capture_output=True, text=True)
                    self.assertIn('[optimized]', result.stderr)
                    self.assertEqual(subprocess.check_output([repo/'target/release/rustqec'], text=True).strip(), 'debug_assertions=false')
                    self.assertNotIn('CARGO_PROFILE_RELEASE_OPT_LEVEL', kwargs['env'])
                    self.assertNotIn('CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS', kwargs['env'])
                    raise BuildComplete
                return real_run(args, **kwargs)
            overrides = {'CARGO_PROFILE_RELEASE_OPT_LEVEL':'0', 'CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS':'true'}
            with patch.object(evidence_run,'ROOT',repo), patch.object(evidence_run,'clean_source',lambda:source_contract.clean_source(repo)), patch.object(sys,'argv',['evidence_run','--out',str(base/'out'),'--work',str(base/'work')]), patch.dict(os.environ,overrides), patch.object(subprocess,'run',build_then_stop):
                with self.assertRaises(BuildComplete):
                    evidence_run.main()

    def test_external_ancestor_config_is_rejected_before_build(self):
        from .source_contract import build_environment
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp); repo = base/'repo'; repo.mkdir()
            (base/'.cargo').mkdir()
            (base/'.cargo/config.toml').write_text('[profile.release]\nopt-level=0\n')
            with self.assertRaisesRegex(ValueError, 'External ancestor Cargo configuration'):
                build_environment(repo, base/'fresh-cargo-home')


class WorkloadMetadataTests(unittest.TestCase):
    def test_resealed_false_pauli_metadata_rejected_in_every_report(self):
        import hashlib
        import shutil
        from .verify import verify
        from .shot_data import rescore, rescore_seeds
        for report_name, archive_name in [('decoding.json','shot-data-v1.zip'), ('tradeoff.json','shot-data-v1.zip'), ('accuracy-seeds.json','accuracy-seeds.zip')]:
            with self.subTest(report=report_name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)/'bundle'; shutil.copytree(ROOT/'site/static/data/atom-loss',root)
                report = json.loads((root/report_name).read_text())
                cases = report if isinstance(report,list) else report['cases'] if report_name=='accuracy-seeds.json' else [report]
                cases[0]['pauli_probability'] = .5
                (root/report_name).write_text(json.dumps(report))
                with zipfile.ZipFile(root/archive_name) as z:
                    payload = {name:z.read(name) for name in z.namelist()}
                payload[report_name] = (root/report_name).read_bytes()
                index = json.loads(payload['index.json'])
                index['sha256'][report_name] = hashlib.sha256(payload[report_name]).hexdigest()
                payload['index.json'] = json.dumps(index).encode()
                with zipfile.ZipFile(root/archive_name,'w',compression=zipfile.ZIP_DEFLATED) as z:
                    for name,data in payload.items(): z.writestr(name,data)
                bundle = json.loads((root/'bundle.json').read_text())
                for name in [report_name,archive_name]:
                    bundle['sha256'][name] = hashlib.sha256((root/name).read_bytes()).hexdigest()
                (root/'bundle.json').write_text(json.dumps(bundle))
                with self.assertRaisesRegex(ValueError,'Declared Pauli probability'):
                    verify(root)
                with self.assertRaisesRegex(ValueError,'Declared Pauli probability'):
                    (rescore_seeds if archive_name=='accuracy-seeds.zip' else rescore)(root/archive_name)

    def test_generator_uses_declared_pauli_probability(self):
        from .run import generate
        with tempfile.TemporaryDirectory() as tmp:
            low, high = Path(tmp)/'low.stim', Path(tmp)/'high.stim'
            generate(ROOT/'target/release/rustqec',low,3,2,.003,.001)
            generate(ROOT/'target/release/rustqec',high,3,2,.003,.5)
            self.assertNotEqual(low.read_bytes(),high.read_bytes())
            self.assertIn('DEPOLARIZE2(0.5)', high.read_text())


if __name__ == '__main__':
    unittest.main()
