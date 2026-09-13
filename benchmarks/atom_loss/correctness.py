"""Distribution and known-answer controls for independent persistent-loss sampling."""
import argparse
import itertools
import json
import math
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from . import reference, noise_controls, low_probability

from .probe_specs import CASES, KNOWN


def rust_rows(binary, text, shots, seed, work):
    circuit = work / 'input.stim'
    output = work / 'shots.01'
    circuit.write_text(text + '\n')
    subprocess.run([str(binary), 'circuit', 'sample', '--in', str(circuit), '--shots', str(shots),
                    '--seed', str(seed), '--out', str(output), '--out-format', '01'],
                   check=True, capture_output=True, timeout=120)
    rows = output.read_text().splitlines()
    return np.array([[int(x) for x in row] for row in rows], dtype=np.bool_)


def histogram(rows):
    values = (rows.astype(np.uint64) * (1 << np.arange(rows.shape[1], dtype=np.uint64))).sum(axis=1).astype(int)
    return np.bincount(values, minlength=2**rows.shape[1]) / len(rows)


def run(binary, shots=32768):
    records = []
    # Union bound for every bin in every two-sample small-circuit comparison.
    bins = 64 * len(CASES)
    tolerance = 2 * math.sqrt(math.log(4 * bins / 5e-7) / (2 * shots))
    with tempfile.TemporaryDirectory(prefix='loss-correctness-') as tmp:
        for name, text in CASES.items():
            observed = rust_rows(binary, text, shots, 173, Path(tmp))
            independent = reference.sample(text, shots, 827)
            delta = float(np.max(np.abs(histogram(observed) - histogram(independent))))
            known_ok = name not in KNOWN or (np.all(observed == KNOWN[name]) and np.all(independent == KNOWN[name]))
            records.append({'histogram_counts':{key:np.rint(histogram(rows)*shots).astype(int).tolist() for key,rows in [('rust',observed),('reference',independent)]}, 'case': name, 'shots_per_sampler': shots, 'max_bin_difference': delta,
                            'tolerance': tolerance, 'known_answer_pass': bool(known_ok),
                            'status': 'PASS' if delta <= tolerance and known_ok else 'FAIL'})
        text = CASES['lost_control_skips_cx']
        bad = reference.sample(text, 32, skip_lost_gates=False)
        negative = not np.all(bad == KNOWN['lost_control_skips_cx'])
        # Unsupported operations must fail explicitly, rather than silently disappear.
        try:
            reference.sample('R 0\nT 0\nM 0', 4)
            rejected = False
        except ValueError:
            rejected = True
    analytic = noise_controls.run(binary, rust_rows, shots)
    low = low_probability.run(binary, rust_rows, shots)
    return {'low_probability_controls': low, 'status': 'PASS' if all(r['status'] == 'PASS' for r in records) and negative and rejected and analytic['status']=='PASS' and low['status']=='PASS' else 'FAIL',
            'analytic_noise_controls': analytic,
            'method': 'independent Stim circuit lowering; joint output distributions and hand-computed controls',
            'familywise_alpha_bound': 1.2e-6, 'negative_skipped_gate_mutation_rejected': bool(negative),
            'unsupported_reference_operation_rejected': rejected, 'cases': records}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--shots', type=int, default=32768)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    result = run(args.binary.resolve(), args.shots)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(result['status'])
    raise SystemExit(0 if result['status'] == 'PASS' else 1)
