"""Build and regenerate the entire publication from a clean source commit.

Run in a fresh detached worktree. Output/work directories must be outside it.
A later artifact commit may change site outputs, but must retain identical
source/build inputs. This avoids an impossible artifact-to-own-commit hash cycle.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from .source_contract import ROOT, BUILD_COMMANDS, BINARIES, clean_source, capture, verify_bundle_source, build_environment, NETWORK_ENV_KEYS
from .run import save, digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--work', type=Path, required=True)
    a = p.parse_args()
    a.out = a.out.resolve(); a.work = a.work.resolve()
    for path in [a.out, a.work]:
        if path.is_relative_to(ROOT) or path.exists():
            raise ValueError('Use fresh output/work directories outside the source checkout')
    if (ROOT/'target').exists():
        raise ValueError('Use a fresh source checkout without target/ to prevent stale builds')
    env = build_environment(ROOT, a.work/'cargo-home')
    for key in ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'RAYON_NUM_THREADS']:
        os.environ[key] = '1'
    import importlib.metadata
    for line in (ROOT/'benchmarks/atom_loss/requirements.txt').read_text().splitlines():
        if line.strip() and not line.startswith('#'):
            package, version = line.strip().split('==')
            if importlib.metadata.version(package) != version:
                raise ValueError('Install the pinned requirements before measuring: '+package)
    binding = clean_source()
    a.out.mkdir(parents=True); a.work.mkdir(parents=True)
    for command in BUILD_COMMANDS:
        subprocess.run(command, cwd=ROOT, env=env, check=True)
    if clean_source() != binding:
        raise ValueError('Source changed during build')
    binding.update(binaries={name: digest(ROOT/name) for name in BINARIES},
                   build_environment={key: value for key, value in env.items() if key not in NETWORK_ENV_KEYS},
                   rustc=subprocess.check_output(['rustc', '-Vv'], text=True, env=env),
                   cargo=subprocess.check_output(['cargo', '-V'], text=True, env=env))
    save(a.out/'source-manifest.json', binding)
    subprocess.run([sys.executable, '-m', 'benchmarks.atom_loss.run', '--work', str(a.work/'timing'),
                    '--out', str(a.out)], cwd=ROOT, env=env, check=True)
    from .shot_data import pack
    print(pack(a.work/'timing', a.out), flush=True)
    subprocess.run([sys.executable, '-m', 'benchmarks.atom_loss.accuracy_seeds', '--work', str(a.work/'seeds'),
                    '--out', str(a.out)], cwd=ROOT, env=env, check=True)
    # These stage links now refer to the same complete clean run, not old data.
    for stage in ['timing', 'correctness']:
        capture(a.out, stage, {'run': 'Complete clean regeneration; see provenance-all.json for start and timing boundaries.'})
    verify_bundle_source(a.out)
    if {name: digest(ROOT/name) for name in BINARIES} != binding['binaries']:
        raise ValueError('Binaries changed during evidence generation')
    clean_source()
    from .publish import publish
    publish(a.out)
    from .verify import verify
    print(verify(a.out), flush=True)


if __name__ == '__main__':
    main()
