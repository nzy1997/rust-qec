"""Distribution and known-answer controls for independent persistent-loss sampling."""
import argparse
import itertools
import json
import math
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from . import reference

CASES = {
    'lost_control_skips_cx': 'R 0 1\nX 0\nLOSS(1) 0\nCX 0 1\nML 0 1',
    'lost_target_skips_cx': 'R 0 1\nH 0\nLOSS(1) 1\nCX 0 1\nH 0\nML 0 1',
    'reset_restores_wire': 'R 0 1\nLOSS(1) 0\nR 0\nX 0\nCX 0 1\nML 0 1',
    'readout_reset_restores_wire': 'R 0\nLOSS(1) 0\nMRL 0\nML 0',
    'bell_partner_marginal': 'R 0 1\nH 0\nCX 0 1\nLOSS(1) 0\nML 0 1',
    'loss_at_two_times': 'R 0 1\nH 0\nLOSS(0.2) 0\nCX 0 1\nLOSS(0.3) 1\nML 0 1',
    'two_losses': 'R 0 1\nLOSS(0.3) 0 1\nCX 0 1\nML 0 1',
    'pauli_and_loss': 'R 0 1\nH 0\nCX 0 1\nDEPOLARIZE2(0.17) 0 1\nLOSS(0.2) 0\nX_ERROR(0.11) 1\nML 0 1',
    'persistent_loss_multiple_gates': 'R 0 1 2\nX 0\nLOSS(0.4) 0\nCX 0 1\nCX 0 2\nML 0 1 2',
    'repeat_delayed_readout': 'R 0 1\nREPEAT 3 {\nH 0\nCX 0 1\nLOSS(0.1) 0\n}\nML 0 1',
    'ordinary_lost_measurement': 'R 0 1\nX 0\nLOSS(0.4) 0\nCX 0 1\nM 0 1',
    'no_loss_bell': 'R 0 1\nH 0\nCX 0 1\nLOSS(0) 0\nM 0 1',
}
KNOWN = {
    'lost_control_skips_cx': [1, 1, 0, 0],
    'lost_target_skips_cx': [0, 0, 1, 1],
    'reset_restores_wire': [0, 1, 0, 1],
    'readout_reset_restores_wire': [1, 1, 0, 0],
}


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
    tolerance = 2 * math.sqrt(math.log(4 * bins / 1e-6) / (2 * shots))
    with tempfile.TemporaryDirectory(prefix='loss-correctness-') as tmp:
        for name, text in CASES.items():
            observed = rust_rows(binary, text, shots, 173, Path(tmp))
            independent = reference.sample(text, shots, 827)
            delta = float(np.max(np.abs(histogram(observed) - histogram(independent))))
            known_ok = name not in KNOWN or (np.all(observed == KNOWN[name]) and np.all(independent == KNOWN[name]))
            records.append({'case': name, 'shots_per_sampler': shots, 'max_bin_difference': delta,
                            'tolerance': tolerance, 'known_answer_pass': bool(known_ok),
                            'status': 'PASS' if delta <= tolerance and known_ok else 'FAIL'})
        text = CASES['lost_control_skips_cx']
        bad = reference.sample(text, 32, skip_lost_gates=False)
        negative = not np.all(bad == KNOWN['lost_control_skips_cx'])
        # Unsupported operations must fail explicitly, rather than silently disappear.
        try:
            reference.sample('R 0\nS 0\nM 0', 4)
            rejected = False
        except ValueError:
            rejected = True
    return {'status': 'PASS' if all(r['status'] == 'PASS' for r in records) and negative and rejected else 'FAIL',
            'method': 'independent Stim circuit lowering; joint output distributions and hand-computed controls',
            'familywise_alpha_bound': 1e-6, 'negative_skipped_gate_mutation_rejected': bool(negative),
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
