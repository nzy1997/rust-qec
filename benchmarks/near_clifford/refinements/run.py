"""Paired pristine timings and separate diagnostics for near-Clifford refinements."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
ROOT = HERE.parents[2]

def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def matrix():
    scale = load(BASE / 'scale/run.py', 'refinement_scale_inputs')
    ent = load(BASE / 'entangled/run.py', 'refinement_entangled_inputs')
    extra = [(name, shots) for name in ['rank_11', 'rank_12', 'rank_16',
             'brick_11_11_3', 'brick_12_12_3', 'brick_16_16_3'] for shots in [1, 16, 256]]
    return list(dict.fromkeys(scale.MATRIX + ent.MATRIX + extra))

def driver():
    raw = (BASE / 'scale/main.rs').read_text()
    anchor = 'fn circuit(name: &str) -> Result<String, String> {'
    if raw.count(anchor) != 1: raise ValueError('nonunique driver anchor')
    raw = raw.replace(anchor, 'mod fixtures;\nmod oracle;\n' + anchor + '''
    if ["brick", "parity", "rounds"].contains(&name.split('_').next().unwrap()) {
        return fixtures::circuit(name);
    }
''', 1)
    # Extend only the untimed verification command, including individual runs.
    begin = raw.index('    if args.get(1).map(String::as_str) == Some("verify") {')
    end = raw.index('    #[cfg(diagnostics)]', begin)
    verification = raw[begin:end].replace('(0..16)', '(0..256)')
    verification = verification.replace('verify_fixture(&executor)?;', '''verify_fixture(&executor)?;
        let mut prepared = executor.prepare_sampler()?;
        let mut flat = executor.prepare_sampler()?;
        let mut a = StdRng::seed_from_u64(739);
        let mut b = StdRng::seed_from_u64(739);
        let prepared_shots = prepared.sample(256, &mut a)?;
        let flat_shots = flat.sample_measurements_u8(256, &mut b)?;
        let physics = if ["brick", "parity", "rounds"].contains(&name.split('_').next().unwrap()) {
            fixtures::validate(name)?
        } else { json!(null) };''')
    verification = verification.replace('        println!(', '\n'.join([
        '        let expected = shots.iter().flat_map(|s| s.measurements.iter().map(|&bit| u8::from(bit))).collect::<Vec<_>>();',
        '        if shots != prepared_shots || expected != flat_shots.measurements {',
        '            return Err("extended prepared/flat semantics differ".into());',
        '        }',
        '        let continuation = rng.next_u64();',
        '        if continuation != a.next_u64() || continuation != b.next_u64() {',
        '            return Err("extended RNG continuation differs".into());',
        '        }',
        '        println!(',
    ]), 1)
    verification = verification.replace('"continuation":rng.next_u64()', '"continuation":continuation,"physics":physics')
    raw = raw[:begin] + verification + raw[end:]
    anchor = '"counters":rstim::near_clifford::benchmark_counters()'
    if raw.count(anchor) != 1: raise ValueError('nonunique driver anchor')
    return raw.replace(anchor, anchor + ',"cache_reservation":rstim::near_clifford::benchmark_cache_reservation()', 1)

def instrument(raw):
    old = load(BASE / 'scale/run.py', 'refinement_overlay_inputs')
    observed = old.instrument(raw)
    # The historic counter labels are unchanged and retained for comparison.
    # Cache-policy conclusions use this explicit ledger, not those labels.
    body = '''self.terminal.as_ref().map(|t| [t.cached.reserved_cache_bytes,
        MAX_CACHE_BYTES, usize::from(t.cached.root.measurement.is_some()),
        t.cached.root.children.iter().filter(|c| c.is_some()).count()])'''
    if 'reserved_cache_bytes: usize' not in raw:
        body = 'None'
    return observed + '''
impl NearCliffordSampler<'_> {
    pub fn benchmark_cache_reservation(&self) -> Option<[usize; 4]> { ''' + body + ''' }
}
'''

def report(result):
    lines = ['# Near-Clifford refinement campaign', '',
        f"Baseline `{result['sources']['baseline']['revision']}`; candidate `{result['sources']['candidate']['revision']}`.",
        'Three alternating process pairs × three repetitions. Paired ranges are not confidence intervals.',
        'All six timing modes are retained; RSS is whole-process high-water memory.', '',
        '| Fixture | Shots | Baseline ms | Candidate ms | Speedup | Paired range | Candidate RSS MiB |',
        '| --- | ---: | ---: | ---: | ---: | --- | ---: |']
    for c in result['cases']:
        ns = lambda p, label: p[label]['measurements'][0]['warm_prepared_flat']['median_ns']
        a = statistics.median(ns(p, 'baseline') for p in c['runs']) / 1e6
        b = statistics.median(ns(p, 'candidate') for p in c['runs']) / 1e6
        ratios = [ns(p, 'baseline') / ns(p, 'candidate') for p in c['runs'] if ns(p, 'baseline') and ns(p, 'candidate')]
        gain = f'{a/b:.3f}×' if a and b else 'n/a'
        spread = f'{min(ratios):.3f}–{max(ratios):.3f}×' if ratios else 'n/a'
        rss = statistics.median(p['candidate']['peak_rss_bytes'] for p in c['runs']) / 1048576
        lines.append(f"| {c['fixture']} | {c['shots']} | {a:.4f} | {b:.4f} | {gain} | {spread} | {rss:.1f} |")
    return '\n'.join(lines) + '\n'

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline', required=True)
    p.add_argument('--candidate', default='HEAD')
    p.add_argument('--scratch', type=Path, required=True)
    p.add_argument('--only', nargs='+')
    args = p.parse_args()
    args.scratch = args.scratch.resolve()
    if args.scratch.exists(): p.error('scratch must be a fresh nonexistent directory')
    cases = matrix()
    if args.only:
        if set(args.only) - {n for n, _ in cases}: p.error('unknown fixture')
        cases = [(n, shots) for n, shots in cases if n in args.only]
    revisions = {label: subprocess.check_output(['git', 'rev-parse', ref + '^{commit}'], cwd=ROOT, text=True).strip()
        for label, ref in [('baseline', args.baseline), ('candidate', args.candidate)]}
    if len(set(revisions.values())) != 2: p.error('source revisions must differ')
    args.scratch.mkdir(parents=True)
    generated = args.scratch / 'driver.rs'; generated.write_text(driver())
    campaign = load(BASE / 'scale/run.py', 'refinement_campaign')
    ent = load(BASE / 'entangled/run.py', 'refinement_oracle_adapter')
    class Inputs:
        def __truediv__(self, name):
            return generated if name == 'main.rs' else BASE / 'scale' / ('Cargo.unified.lock' if name == 'Cargo.lock' else name)
    campaign.HARNESS = Inputs()
    campaign.MATRIX = cases
    campaign.REVISIONS = revisions
    campaign.instrument = instrument
    campaign.report = report
    build = campaign.build
    def bound_build(scratch, label, revision, diagnostic=False):
        directory = scratch / (label + ('-diagnostic' if diagnostic else '')) / 'harness'
        directory.mkdir(parents=True)
        (directory / 'fixtures.rs').write_bytes((BASE / 'entangled/fixtures.rs').read_bytes())
        oracle = subprocess.check_output(['git', 'show', revision + ':rstim/tests/support/near_clifford_oracle.rs'], cwd=ROOT)
        (directory / 'oracle.rs').write_bytes(ent.adapt_oracle(oracle))
        binary, metadata = build(scratch, label, revision, diagnostic)
        metadata.update(tableau_source_sha256=sha(directory.parent / 'source/rstim/src/sim/tableau.rs'),
            oracle_source_sha256=hashlib.sha256(oracle).hexdigest(), oracle_sha256=sha(directory / 'oracle.rs'),
            has_cache_ledger='reserved_cache_bytes: usize' in (directory.parent / 'source/rstim/src/near_clifford.rs').read_text())
        if not diagnostic:
            env = {k: v for k, v in os.environ.items() if k not in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']}
            env.update(CARGO_TARGET_DIR=str(scratch / 'target'), RUSTFLAGS='')
            subprocess.run(['cargo', 'test', '--release', '--locked'], cwd=directory, env=env, check=True)
        return binary, metadata
    campaign.build = bound_build
    payloads = {}
    output = campaign.output
    def capture(cmd, **kwargs):
        value = output(cmd, **kwargs)
        if len(cmd) == 3 and cmd[-1] == 'verify':
            payloads.setdefault(Path(cmd[0]).parent.name, {})[cmd[1]] = json.loads(value)
        return value
    campaign.output = capture
    save = campaign.atomic_save
    def retain(path, result):
        result['schema'] = 'near-clifford.refinements.v1'
        result['entry_sha256'] = sha(Path(__file__))
        result['verification_results'] = payloads
        result['selected_matrix'] = cases
        result['input_hashes'] = {str(path.relative_to(ROOT)): sha(path) for path in [
            BASE / 'scale/run.py', BASE / 'scale/main.rs', BASE / 'scale/Cargo.unified.lock',
            BASE / 'entangled/run.py', BASE / 'entangled/fixtures.rs']}
        save(path, result)
    campaign.atomic_save = retain
    sys.argv = [sys.argv[0], '--scratch', str(args.scratch), '--output', str(args.scratch / 'results.json')]
    campaign.main()

if __name__ == '__main__': main()
