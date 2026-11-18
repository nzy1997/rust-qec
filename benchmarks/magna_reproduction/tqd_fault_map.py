"""Map pinned terminal-circuit faults to the author's spacetime edge addresses."""

import argparse
from collections import Counter
from fractions import Fraction
from itertools import product
import json
from pathlib import Path
import re

import reference
from remaining_contracts import read_json, require


COLORS = ('b', 'g', 'r')
BASE_RATE = Fraction(1, 100)
SPATIAL = re.compile(r'([XZ])_ERROR\(([^)]+)\)')
MEASUREMENT = re.compile(r'(M[XZY]?)(?:\(([^)]+)\))?')
JIT_CALL = re.compile(r'CONTROL\[jit_([grb])_(\d+)\]')
MEMORY_RECORD = re.compile(r'memory\.(\d+)\.([AB])\.([grb])\.(\d+)\.(\d+)')


def circuit_faults(text, layout):
    size, depth = layout['size'], layout['depth']
    require((size, depth) in ((3, 5), (4, 6), (5, 8)) and layout['noise_window'] == 'author',
            'unexpected TQD scale or physical fault window')
    require(layout['terminal_recovery']['kind'] == 'noiseless-charge-round'
            and layout['terminal_recovery']['rounds'] == 1
            and layout['terminal_recovery']['memory_depth'] == depth
            and layout['terminal_recovery']['charge_depth'] == depth+1, 'TQD terminal round is not ideal')
    records = layout['records']
    vertices = size**2
    width = 2*vertices
    measurement = 0
    spatial_rounds = Counter()
    next_memory_record = None
    couplings = []
    pending_corrections = {}
    jit_calls = set()
    entries = []
    for line, raw in enumerate(text.splitlines(), start=1):
        fields = raw.partition('#')[0].split()
        if not fields:
            continue
        op, *args = fields
        if noisy := SPATIAL.fullmatch(op):
            basis, probability = noisy.groups()
            require(Fraction(probability) == BASE_RATE, 'unexpected TQD spatial error probability')
            wires = list(map(int, args))
            color_index = wires[0]//width
            require(0 <= color_index < len(COLORS) and wires == list(range(color_index*width, (color_index+1)*width)),
                    'TQD spatial fault declaration is not a full color slice')
            color = COLORS[color_index]
            time = spatial_rounds[basis,color]
            spatial_rounds[basis,color] += 1
            coordinate = (time, color)
            if basis == 'X':
                require(next_memory_record is None, 'TQD memory blocks overlap')
                next_memory_record = coordinate
            else:
                require(next_memory_record == coordinate, 'TQD Pauli channels use different memory blocks')
            for wire in wires:
                node, direction = divmod(wire%width, 2)
                if basis == 'X':
                    # X faults touch B plaquettes on the dual lattice.
                    # A physical x edge borders the plaquette below, while a
                    # physical y edge borders the plaquette to its left.
                    y, x = divmod(node, size)
                    if direction == 0:
                        node = ((y-1) % size)*size+x
                    else:
                        node = y*size+(x-1) % size
                    direction = 1-direction
                edge = 3*(time*vertices+node)+direction
                entries.append(dict(kind=basis, color=color, edge=edge, line=line, wire=wire))
        elif match := MEASUREMENT.fullmatch(op):
            require(measurement < len(records) and records[measurement]['record'] == measurement and len(args) == 1,
                    'TQD measurement record ordering differs from layout')
            label = MEMORY_RECORD.fullmatch(records[measurement]['label'])
            if label is not None:
                _, stabilizer, color, x, y = label.groups()
                x, y = int(x), int(y)
                ancilla = int(args[0])
                start = COLORS.index(color)*width
                wire = lambda sx, sy, direction: start+2*((sy % size)*size+sx % size)+direction
                if stabilizer == 'B':
                    actual = sorted(control for control,target in couplings if target == ancilla)
                    expected = [wire(x,y,0), wire(x,y,1), wire(x,y+1,0), wire(x+1,y,1)]
                else:
                    actual = sorted(target for control,target in couplings if control == ancilla)
                    expected = [wire(x,y,0), wire(x,y,1), wire(x-1,y,0), wire(x,y-1,1)]
                require(actual == sorted(expected), 'TQD primal or dual stabilizer support changed')
            if next_memory_record is not None:
                time, color = next_memory_record
                require(label is not None and label.groups() == (str(time), 'B', color, '0', '0'),
                        'TQD spatial wires disagree with the measured color and time')
                next_memory_record = None
            probability = Fraction(match[2]) if match[2] is not None else Fraction(0)
            if probability:
                require(label is not None and probability == BASE_RATE, 'noisy measurement outside TQD memory window')
                time, stabilizer, color, x, y = label.groups()
                time, x, y = map(int, (time, x, y))
                require(0 <= time < depth-1 and 0 <= x < size and 0 <= y < size
                        and match[1] == ('M' if stabilizer == 'B' else 'MX'),
                        'TQD temporal error does not match the corresponding measurement')
                edge = 3*(time*vertices+y*size+x)+2
                entries.append(dict(kind='X' if stabilizer == 'B' else 'Z', color=color,
                                    edge=edge, line=line, record=measurement))
            measurement += 1
        elif call := JIT_CALL.fullmatch(op):
            color, time = call[1], int(call[2])
            key = color,time
            require(not pending_corrections and key not in jit_calls and 0 <= time < depth and '->' in args,
                    'unexpected TQD JIT correction order')
            jit_calls.add(key)
            outputs = args[args.index('->')+1:][:width]
            require(len(outputs) == len(set(outputs)) == width, 'missing TQD JIT correction bits')
            base = COLORS.index(color)*width
            for offset, bit in enumerate(outputs):
                node, direction = divmod(offset, 2)
                y, x = divmod(node, size)
                # Inverse dual map, checked against the actual controlled X.
                if direction == 0:
                    target = 2*(y*size+(x+1) % size)+1
                else:
                    target = 2*(((y+1) % size)*size+x)
                pending_corrections[bit] = base+target
        elif op == 'CX' and len(args) == 2 and args[0] in pending_corrections:
            require(args[1].isdecimal() and int(args[1]) == pending_corrections.pop(args[0]),
                    'TQD JIT corrected a different physical edge')
        elif op in ('R', 'RX', 'RY', 'RZ'):
            couplings.clear()
        elif op == 'CX' and len(args) == 2 and all(arg.isdecimal() for arg in args):
            couplings.append(tuple(map(int, args)))
        else:
            require('_ERROR' not in op and not op.startswith(('DEPOLARIZE', 'PAULI_CHANNEL')),
                    'unexpected TQD noise operation')
    require(next_memory_record is None, 'TQD final memory block lacks a measured syndrome')
    require(not pending_corrections and jit_calls == set(product(COLORS, range(depth))),
            'TQD circuit omitted a JIT correction')
    require(measurement == len(records), 'TQD layout does not cover all physical observations')
    keys = Counter((entry['kind'], entry['color'], entry['edge']) for entry in entries)
    expected = Counter((basis,color,3*(time*vertices+node)+axis)
                       for basis,color,time,node,axis in product('XZ', COLORS, range(depth), range(vertices), range(3))
                       if time != depth-1 or axis != 2)
    require(keys == expected, 'TQD physical noise and original spacetime edge inventory differ')
    return entries


def inspect(upstream, pinned=None):
    pinned = reference.load_manifest() if pinned is None else pinned
    files = pinned['upstream']['files']
    result = {}
    for size in (3, 4, 5):
        prefix = f'examples/tqd{size}/'
        physical, layout = (prefix + filename for filename in ('physical.magna', 'layout.json'))
        reference.verify_files(upstream, {name: files[name] for name in (physical, layout)})
        source = read_json(upstream/layout)
        entries = circuit_faults((upstream/physical).read_text(), source)
        result[f'tqd{size}'] = dict(size=size, depth=source['depth'],
                                    spatial_sites=sum('wire' in entry for entry in entries),
                                    temporal_sites=sum('record' in entry for entry in entries),
                                    physical_sha256=files[physical],layout_sha256=files[layout],entries=entries)
    return dict(schema='rmagna.paper-tqd-fault-map.v1', upstream_commit=pinned['upstream']['commit'],
                edge_order='time_major_y_x_axis', families=result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('upstream', type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect(args.upstream), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
