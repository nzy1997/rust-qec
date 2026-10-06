"""Exact instrument lowering for the frozen ideal-MPP/noisy-readout corpus.

This is a benchmark adapter, not a production gate implementation. Competitors
receive native products/readout noise; rstim receives this disclosed lowering.
"""
import re


def physical_width(text):
    """Count target indices for the frozen linear Stim corpus (no REPEAT)."""
    targets = []
    for line in text.splitlines():
        words=line.split('#',1)[0].split()
        if words and words[0] == 'REPEAT':
            raise ValueError('REPEAT unsupported in the frozen corpus adapter')
        for word in words[1:]:
            for factor in word.split('*'):
                match=re.fullmatch(r'!?[XYZ]?(\d+)',factor)
                if match: targets.append(int(match[1]))
    return max(targets,default=-1)+1


def records_only(text):
    """Project the output contract onto all raw records, without postselection."""
    return '\n'.join(line for line in text.splitlines()
        if not line.strip().startswith(('DETECTOR', 'OBSERVABLE_INCLUDE'))) + '\n'


def lower(text, num_qubits):
    if type(num_qubits) is not int or num_qubits < 1:
        raise ValueError('positive integer physical width required')
    lines = []
    for line in text.splitlines():
        code = line.split('#', 1)[0].strip()
        if not code:
            continue
        words = code.split()
        noisy = re.fullmatch(r'(M|MZ|MX|MY|MR|MRZ|MRX|MRY)\(([^()]*)\)', words[0])
        if noisy:
            name, probability = noisy.groups()
            if len(words) == 1:
                raise ValueError('empty noisy measurement')
            if not 0 <= float(probability) <= 1:
                raise ValueError('invalid readout probability')
            basis = 'X' if name.endswith('X') else 'Y' if name.endswith('Y') else 'Z'
            for target in words[1:]:
                if not re.fullmatch(r'!?\d+', target):
                    raise ValueError('invalid noisy measurement target')
                q = int(target.lstrip('!'))
                if q >= num_qubits:
                    raise ValueError('noisy target outside physical width')
                inverted = target.startswith('!')
                # Encode ideal outcome into a fresh spectator, then flip only
                # its readout label. Undoing CX preserves the data projector;
                # the spectator is reset and emits no additional record.
                lines.append(f'R {num_qubits}')
                if basis == 'Y':
                    lines.append(f'S_DAG {q}')
                if basis in 'XY':
                    lines.append(f'H {q}')
                lines.extend([f'CX {q} {num_qubits}', f'X_ERROR({probability}) {num_qubits}',
                    f'M {"!" if inverted else ""}{num_qubits}', f'CX {q} {num_qubits}'])
                if basis in 'XY':
                    lines.append(f'H {q}')
                if basis == 'Y':
                    lines.append(f'S {q}')
                lines.append(f'R {num_qubits}')
                if name.startswith('MR'):
                    lines.append(f'R{basis if basis != "Z" else ""} {q}')
            continue
        if words[0] == 'MPP':
            if len(words) == 1:
                raise ValueError('empty MPP')
            for product in words[1:]:
                factors = []
                inverted = False
                for factor in product.split('*'):
                    if factor.startswith('!'):
                        inverted = not inverted
                        factor = factor[1:]
                    if not re.fullmatch(r'[XYZ]\d+', factor):
                        raise ValueError('invalid MPP factor')
                    factors.append((factor[0], int(factor[1:])))
                    if factors[-1][1] >= num_qubits:
                        raise ValueError('MPP target outside physical width')
                if len({q for _, q in factors}) != len(factors):
                    raise ValueError('duplicate target in MPP product')
                pivot = factors[-1][1]
                for basis, q in factors:
                    if basis == 'Y':
                        lines.append(f'S_DAG {q}')
                    if basis in 'XY':
                        lines.append(f'H {q}')
                lines.extend(f'CX {q} {pivot}' for _, q in factors[:-1])
                lines.append(f'M {"!" if inverted else ""}{pivot}')
                lines.extend(f'CX {q} {pivot}' for _, q in reversed(factors[:-1]))
                for basis, q in reversed(factors):
                    if basis in 'XY':
                        lines.append(f'H {q}')
                    if basis == 'Y':
                        lines.append(f'S {q}')
            continue
        if words[0].startswith('MPP'):
            raise ValueError('only ideal MPP is supported by this adapter')
        lines.append(code)
    return '\n'.join(lines) + '\n'
