"""Independent persistent-loss lowering to Stim, with no RustQEC imports.

Lost wires retain their unobserved quantum state; gates touching them are
omitted until reset. This is equivalent to tracing out those wires for all
surviving-wire observables. LOSS histories remain private to this sampler.
"""
from collections import defaultdict
from dataclasses import dataclass
import re
import numpy as np
import stim


@dataclass(frozen=True)
class Op:
    name: str
    args: tuple
    targets: tuple


def parse(text):
    lines = iter(text.splitlines())
    def block(nested=False):
        result = []
        for raw in lines:
            line = raw.split('#', 1)[0].strip()
            if not line:
                continue
            if line == '}':
                if not nested:
                    raise ValueError('Unexpected closing brace')
                return result
            if line.startswith('REPEAT '):
                match = re.fullmatch(r'REPEAT (\d+)\s*\{', line)
                if not match:
                    raise ValueError(line)
                body = block(True)
                result.extend(body * int(match[1]))
                continue
            match = re.fullmatch(r'([A-Z_0-9]+)(?:\[[^\]]*\])?(?:\(([^)]*)\))?(?:\s+(.*))?', line)
            if not match:
                raise ValueError(line)
            name, args, targets = match.groups()
            result.append(Op(name, tuple(float(x) for x in args.split(',')) if args else (), tuple((targets or '').split())))
        if nested:
            raise ValueError('Unclosed repeat')
        return result
    return block()


def lower(ops, events, *, skip_lost_gates=True):
    """One fixed onset history -> an ordinary Stim circuit, preserving row order."""
    # Assemble text before crossing the Python/C++ boundary. Appending each
    # instruction through Stim separately dominates this reference's runtime.
    instructions = []
    def emit(name, targets, args=()):
        parameters = '(' + ','.join(map(str, args)) + ')' if args else ''
        instructions.append(name + parameters + ' ' + ' '.join(map(str, targets)))
    lost = set()
    loss_index = 0
    for op in ops:
        name, args, targets = op.name, op.args, op.targets
        if name in {'DETECTOR', 'OBSERVABLE_INCLUDE', 'QUBIT_COORDS', 'SHIFT_COORDS', 'TICK'}:
            # Sampling measurement rows does not require detector annotations.
            continue
        if name == 'LOSS':
            for target in targets:
                if events[loss_index]:
                    lost.add(int(target))
                loss_index += 1
            continue
        if name in {'R', 'RZ'}:
            for target in targets:
                q = int(target)
                emit('R', [q])
                lost.discard(q)
            continue
        if name in {'M', 'MZ', 'MR', 'MRZ', 'ML', 'MZL', 'MRL', 'MRZL'}:
            if args:
                raise ValueError('Inline measurement noise is outside the reference subset')
            for target in targets:
                inverted = target.startswith('!')
                q = int(target.lstrip('!'))
                absent = q in lost
                if name.endswith('L'):
                    emit('MPAD', [int(absent)])
                if absent:
                    emit('MPAD', [int(not inverted)])
                    if name.startswith('MR'):
                        emit('R', [q])
                else:
                    emit('MR' if name.startswith('MR') else 'M', ['!' + str(q) if inverted else q])
                if name.startswith('MR'):
                    lost.discard(q)
            continue
        if name in {'CX', 'CNOT', 'ZCX', 'CZ', 'DEPOLARIZE2'}:
            if len(targets) % 2:
                raise ValueError('Unpaired gate')
            for a, b in zip(targets[::2], targets[1::2]):
                pair = [int(a), int(b)]
                if not skip_lost_gates or not lost.intersection(pair):
                    emit('CX' if name in {'CNOT', 'ZCX'} else name, pair, args)
            continue
        if name in {'H', 'X', 'Y', 'Z', 'X_ERROR', 'Y_ERROR', 'Z_ERROR', 'DEPOLARIZE1'}:
            for target in targets:
                q = int(target)
                if q not in lost:
                    emit(name, [q], args)
            continue
        raise ValueError(f'Unsupported reference operation: {name}')
    if loss_index != len(events):
        raise ValueError('Loss history width mismatch')
    return stim.Circuit('\n'.join(instructions))


def sample(text, shots, seed=7, *, skip_lost_gates=True):
    ops = parse(text)
    probabilities = [op.args[0] for op in ops if op.name == 'LOSS' for _ in op.targets]
    rng = np.random.default_rng(seed)
    histories = rng.random((shots, len(probabilities))) < probabilities
    # Grouping only reuses identical physical histories; it never conditions
    # a decoder on their private onset times.
    groups = defaultdict(list)
    for row, history in enumerate(histories):
        groups[np.packbits(history, bitorder='little').tobytes()].append(row)
    output = None
    for packed, indices in groups.items():
        history = np.unpackbits(np.frombuffer(packed, dtype=np.uint8), bitorder='little')[:len(probabilities)]
        circuit = lower(ops, history, skip_lost_gates=skip_lost_gates)
        batch = circuit.compile_sampler(seed=int(rng.integers(0, 2**63))).sample(len(indices))
        if output is None:
            output = np.empty((shots, circuit.num_measurements), dtype=np.bool_)
        output[indices] = batch
    if output is None:
        raise ValueError('Positive shots required')
    return output


def signatures(text, rows):
    """Shared externally observable marginals and small joint observables."""
    ops = parse(text)
    cursor = 0
    flags, detectors, observables = [], [], []
    for op in ops:
        if op.name in {'M', 'MZ', 'MR', 'MRZ'}:
            cursor += len(op.targets)
        elif op.name in {'ML', 'MZL', 'MRL', 'MRZL'}:
            for _ in op.targets:
                flags.append(cursor)
                cursor += 2
        elif op.name in {'DETECTOR', 'OBSERVABLE_INCLUDE'}:
            indices = [cursor + int(re.fullmatch(r'rec\[(-\d+)\]', t)[1]) for t in op.targets]
            parity = np.logical_xor.reduce(rows[:, indices], axis=1)
            (detectors if op.name == 'DETECTOR' else observables).append(parity)
    columns = [rows[:, i] for i in flags] + detectors + observables
    columns += [np.logical_and(a, b) for a, b in zip(columns, columns[1:])]
    if not columns:
        columns = list(rows.T)
    return np.stack(columns, axis=1).mean(axis=0)
