"""Full local AB confirmation and identical-binary controls; source fixed throughout."""
import hashlib, json, os, platform, shutil, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'benchmarks/near_clifford'))
import rust_pair_scout as scout
out = ROOT / 'drafts/compact-zero-spans-master-m4-comparison'
if scout.run(['git','rev-parse','HEAD']).strip() != 'd989531d837fe874a77ac05745f5d77ea5639d52':
    raise ValueError('unexpected candidate')
out.mkdir(parents=True,exist_ok=False)
shutil.copyfile(Path(__file__),out/'original-launcher.py')
(out/'host.json').write_text(json.dumps(dict(platform=platform.platform(),processor=scout.run(['sysctl','-n','machdep.cpu.brand_string']).strip(),rustc=scout.run(['rustc','-Vv']),environment={k:os.environ.get(k) for k in scout.ENV_KEYS},scope='same-host local native Rust AB and identical-binary controls'),indent=2)+'\n')
roots = dict(baseline=ROOT / 'drafts/compact-zero-spans-master-baseline', candidate=ROOT)
if scout.run(['git','rev-parse','HEAD'], roots['baseline']).strip() != 'e1453a7b634275d3055ef64b2f4a7b33d3995870':
    raise ValueError('unexpected baseline')
# All preparation has already finished. No builds occur inside this collector.
for name in ['warm','confirmation']:
    scout.collect(roots, out / name)
    scout.verify_closed_campaign(out / name)
# Snapshot the exact measured production binaries before the null controls.
binary_dir = out / 'production-binaries'
binary_dir.mkdir(parents=True, exist_ok=False)
identities = {role: scout.identity(root) for role, root in roots.items()}
confirmation = json.loads((out / 'confirmation/closure.json').read_text())
if identities != confirmation['identities_after']:
    raise ValueError('source or binary changed after measured closure')
for role, root in roots.items():
    target = binary_dir / (role + '.bin')
    shutil.copyfile(scout.probe(root) / 'target/release/counts-path-ablation', target)
    if scout.digest(target) != identities[role]['binary']:
        raise ValueError('binary copy mismatch')
(binary_dir / 'receipt.json').write_text(json.dumps(dict(identities=identities),indent=2)+'\n')
for name in ['same-binary-warm','same-binary-confirmation']:
    scout.collect(dict(baseline=ROOT,candidate=ROOT), out / name, same_binary_control=True)
    scout.verify_closed_campaign(out / name)
print('All four full campaigns and source/binary guards closed',flush=True)
