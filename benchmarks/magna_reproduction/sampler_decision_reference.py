#!/usr/bin/env python3
"""Regenerate tiny binary64 sampler-decision witnesses from pinned native Magna."""
import argparse
import hashlib
import importlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys


def bits(value):
    return f"{struct.unpack('>Q', struct.pack('>d', value))[0]:016x}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--magna-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.magna_root.resolve()
    revision = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != '56847478c840395735146ef1e1ccdc3d8aedc10f':
        parser.error('the decision oracle requires the pinned reference revision')
    sys.path.insert(0, str(root / 'core'))
    native = importlib.import_module('magna._native')
    maximum = sys.float_info.max
    tiny = math.ulp(0.0)
    cases = [
        ('tie', 1.0, 1.0, 1.0, 2),
        ('affine_faster', 2.0, 0.25, 1.0, 8),
        ('exact_faster', 1.5, 4.0, 3.0, 8),
        ('zero_kappa', 0.0, 9.0, 1.0, 16),
        ('negative_zero_kappa', -0.0, 9.0, 1.0, 16),
        ('zero_times_infinity', 0.0, maximum, tiny, 2),
        ('overflow_gamma', maximum, 2.0, 1.0, 2),
        ('overflow_ratio', tiny, maximum, tiny, 2),
        ('underflow_gamma', tiny, tiny, maximum, 2),
        ('underflow_per_draw', 1.0, tiny, tiny, 64),
        ('large_count', 1.0, 1.0, 1.0, (1 << 63) - 1),
        ('single_count_binding', 1.0, 2.0, 1.0, 1),
        ('zero_count', 1.0, 1.0, 1.0, 0),
    ]
    for name, value in [('nan', math.nan), ('infinity', math.inf), ('negative', -1.0)]:
        for position in range(3):
            fields = [1.0, 1.0, 1.0]
            fields[position] = value
            cases.append((f'{name}_field_{position}', *fields, 2))
    cases.extend([('zero_affine_time', 1.0, 0.0, 1.0, 2), ('zero_exact_time', 1.0, 1.0, 0.0, 2)])
    witnesses = []
    for name, kappa, affine, exact, count in cases:
        row = dict(name=name, kappa_bits=bits(kappa), affine_bits=bits(affine), exact_bits=bits(exact), count=count)
        try:
            value = native.decide_file_sampler(kappa, affine, exact, count)
        except ValueError as error:
            row['error'] = str(error)
        else:
            row['expected'] = {key: bits(number) if isinstance(number, float) else number for key, number in value.items()}
        witnesses.append(row)
    module = Path(native.__file__)
    result = dict(reference_revision=revision, native_sha256=hashlib.sha256(module.read_bytes()).hexdigest(),
                  oracle='magna._native.decide_file_sampler', cases=witnesses)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
    print(f'Wrote {len(witnesses)} native decision witnesses to {args.output}')


if __name__ == '__main__':
    main()
