"""Compare full post-measurement instruments to independent dense projectors."""
import re
import unittest
from collections import defaultdict
import numpy as np
from lowering import lower

I = np.eye(2, dtype=complex)
X = np.array([[0, 1], [1, 0]], dtype=complex)
Y = np.array([[0, -1j], [1j, 0]], dtype=complex)
Z = np.diag([1, -1]).astype(complex)
H = (X + Z) / np.sqrt(2)
S = np.diag([1, 1j])


def local(matrix, q, n):
    result = np.array([[1]], dtype=complex)
    for i in range(n):
        result = np.kron(result, matrix if i == q else I)
    return result


def cx(a, b, n):
    matrix = np.zeros((2**n, 2**n), dtype=complex)
    for col in range(2**n):
        row = col ^ (1 << (n-1-b)) if col & (1 << (n-1-a)) else col
        matrix[row, col] = 1
    return matrix


def apply_instrument(text, state, n):
    branches = [((), state)]
    for line in text.splitlines():
        words = line.split()
        gate = words[0].split('(')[0]
        probability = float(re.search(r'\((.*?)\)', words[0])[1]) if '(' in words[0] else 0
        if gate in ['M', 'MX', 'MY', 'MR', 'MRX', 'MRY', 'MPP']:
            for target in words[1:]:
                invert = target.count('!') % 2
                operator = np.eye(2**n, dtype=complex)
                for factor in target.replace('!', '').split('*'):
                    basis = factor[0] if gate == 'MPP' else 'X' if gate.endswith('X') else 'Y' if gate.endswith('Y') else 'Z'
                    q = int(factor[1:] if gate == 'MPP' else factor)
                    operator = operator @ local({'X': X, 'Y': Y, 'Z': Z}[basis], q, n)
                updated = []
                for record, vector in branches:
                    for outcome in [0, 1]:
                        projected = ((np.eye(2**n) + (-1)**outcome * operator) / 2) @ vector
                        if gate.startswith('MR') and outcome:
                            projected = local(Z if basis == 'X' else X, q, n) @ projected
                        for flipped, weight in [(0, 1-probability), (1, probability)]:
                            if weight:
                                updated.append((record + (outcome ^ invert ^ flipped,), projected * np.sqrt(weight)))
                branches = updated
        elif gate == 'R':
            for target in words[1:]:
                q = int(target)
                updated = []
                for record, vector in branches:
                    for bit in [0, 1]:
                        projected = ((np.eye(2**n) + (-1)**bit * local(Z, q, n)) / 2) @ vector
                        if bit:
                            projected = local(X, q, n) @ projected
                        updated.append((record, projected))
                branches = updated
        elif gate in ['RX', 'RY']:
            # Reset channel written as independent spectral projectors.
            q = int(words[1]); basis = X if gate == 'RX' else Y
            updated = []
            for record, vector in branches:
                for bit in [0, 1]:
                    projected = ((np.eye(2**n) + (-1)**bit * local(basis, q, n)) / 2) @ vector
                    if bit:
                        projected = local(Z if gate == 'RX' else X, q, n) @ projected
                    updated.append((record, projected))
            branches = updated
        elif gate == 'X_ERROR':
            q = int(words[1])
            branches = [(record, (local(X, q, n) @ vector if flip else vector) * np.sqrt(weight))
                for record, vector in branches for flip, weight in [(0, 1-probability), (1, probability)] if weight]
        else:
            if gate == 'CX':
                if words[1].startswith('rec['):
                    offset = int(words[1][4:-1]); q = int(words[2])
                    branches = [(record, local(X, q, n) @ vector if record[offset] else vector)
                        for record, vector in branches]
                    continue
                operator = cx(int(words[1]), int(words[2]), n)
            else:
                operator = local({'H': H, 'S': S, 'S_DAG': S.conj().T,
                    'T': np.diag([1, np.exp(1j*np.pi/4)]), 'X': X, 'Z': Z}[gate], int(words[1]), n)
            branches = [(record, operator @ vector) for record, vector in branches]
    result = defaultdict(lambda: np.zeros((2**n, 2**n), dtype=complex))
    for record, vector in branches:
        result[record] += np.outer(vector, vector.conj())
    return result


class LoweringTest(unittest.TestCase):
    def check_channel(self, text):
        rng = np.random.default_rng(739)
        for _ in range(3):
            state = rng.normal(size=8) + 1j*rng.normal(size=8)
            state /= np.linalg.norm(state)
            expected = apply_instrument(text, state, 3)
            encoded = np.kron(state, np.array([1, 0]))
            actual = apply_instrument(lower(text, 3), encoded, 4)
            self.assertEqual(set(expected), set(actual))
            for record, rho in actual.items():
                reduced = np.trace(rho.reshape(8, 2, 8, 2), axis1=1, axis2=3)
                np.testing.assert_allclose(reduced, expected[record], atol=2e-14,
                    err_msg=f'{text!r}, record={record}')
                np.testing.assert_allclose(rho, np.kron(expected[record], np.diag([1, 0])),
                    atol=2e-14, err_msg='spectator must finish in |0>, uncorrelated with data')
            self.assertAlmostEqual(sum(np.trace(rho).real for rho in actual.values()), 1)

    def test_signed_y_products_preserve_full_conditional_state(self):
        for product in ['X0*Y1*Z2', '!Y0*Y1*X2', '!X0*!Y1*Y2', 'Y2']:
            self.check_channel(f'MPP {product}\nT 1\nH 2\nM 0 1 2\n')

    def test_overlapping_projectors_repetition_and_feedback(self):
        self.check_channel('MPP X0*X1 Z0*Z1\nMPP X0*X1\nCX rec[-1] 2\nT 0\nMY 1\n')
        self.check_channel('MPP X0*Y1 Z1*X2\nMPP Y0*Z2\nCX rec[-1] 1\nT 2\nM 0 1 2\n')

    def test_noisy_readout_preserves_data_projector_and_reset(self):
        for gate in ['M', 'MX', 'MY', 'MR', 'MRX', 'MRY']:
            for probability in [0, 0.37, 1]:
                self.check_channel(f'{gate}({probability}) !0\nCX rec[-1] 1\nT 2\nM 0 1 2\n')

    def test_rejects_unsupported_products(self):
        for text in ['MPP(0.1) X0*Y1', 'MPP', 'MPP X0*Z0', 'MPP X0**Y1', 'MPP Q1',
                     'MPP X3', 'M(0.1) 3', 'M(nan) 0', 'M(-0.1) 0', 'M(0.1)']:
            with self.assertRaises(ValueError):
                lower(text, 3)


if __name__ == '__main__':
    unittest.main()
