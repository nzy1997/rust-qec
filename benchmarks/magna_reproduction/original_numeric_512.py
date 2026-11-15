#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Capture the original 512 scalar pairs without changing any RNG consumption."""
from pathlib import Path
import argparse
import subprocess
import tempfile
import json
import hashlib
import shutil
import math
import struct
from fractions import Fraction
import reference
from witness_paths import admit_destinations

def capture(upstream):
    u = Path(upstream).resolve()
    manifest = reference.load_manifest()
    before = reference.verify_source(u, manifest)
    name = 'tests/native/rational_random.cpp'
    original = reference.relative_file(u, name).read_text()
    source = original
    oldn = '        Scalar n(int64_t((rng() >> 11) | 1), int64_t(i) - 1000);'
    oldd = '        Scalar d(int64_t((rng() >> 11) | 1), 1000 - int64_t(i));'
    if not source.count(oldn) == source.count(oldd) == 1:
        raise ValueError('original numeric witness validation failed')
    source = source.replace(oldn, '        const auto observed_n=(rng() >> 11) | 1;\n        Scalar n(int64_t(observed_n), int64_t(i)-1000);').replace(oldd, '        const auto observed_d=(rng() >> 11) | 1;\n        Scalar d(int64_t(observed_d),1000-int64_t(i));')
    marker = '        assert(scaled_real_ratio(n, d) == numeric_detail::exact_ratio(n, d));'
    if not source.count(marker) == 1:
        raise ValueError('original numeric witness validation failed')
    source = source.replace(marker, marker + '\n        auto observed=scaled_real_ratio(n,d);uint64_t observed_bits;std::memcpy(&observed_bits,&observed.first,sizeof(observed_bits));\n            std::cout<<i<<","<<observed_n<<","<<observed_d<<","<<std::hex<<observed_bits<<std::dec<<","<<observed.second<<"\\n";')
    source = '#include <iostream>\n#include <cstring>\n' + source
    compiler = shutil.which('clang++') or shutil.which('g++')
    if not compiler:
        raise ValueError('original numeric witness validation failed')
    with tempfile.TemporaryDirectory(prefix='rmagna-numeric-512-') as directory:
        directory = Path(directory)
        program = directory / 'observe.cpp'
        program.write_text(source)
        binary = directory / 'observe'
        subprocess.run([compiler, '-std=c++17', '-O2', '-Wall', '-Wextra', '-I' + str(u / 'core/magna/native'), str(program), '-o', str(binary)], check=True, capture_output=True, text=True, timeout=30)
        result = subprocess.run([str(binary)], check=True, capture_output=True, text=True, timeout=10)
        binary_hash = reference.digest(binary)
        rows = []
        for line in result.stdout.splitlines():
            i, n, d, bits, exponent = line.split(',')
            i, n, d, exponent = map(int, [i, n, d, exponent])
            if not i == len(rows):
                raise ValueError('original numeric witness validation failed')
            mantissa, expected = math.frexp(float(Fraction(n, d)))
            expected += 2 * i - 2000
            expected_bits = struct.unpack('>Q', struct.pack('>d', mantissa))[0]
            if not (int(bits, 16), exponent) == (expected_bits, expected):
                raise ValueError((i, n, d, bits, exponent, expected_bits, expected))
            rows.append(dict(index=i, numerator=str(n), denominator=str(d), numerator_power=i - 1000, denominator_power=1000 - i, mantissa_bits=f'{expected_bits:016x}', exponent=exponent))
    if not len(rows) == 512:
        raise ValueError('original numeric witness validation failed')
    if not reference.verify_source(u, manifest) == before:
        raise ValueError('original numeric witness validation failed')
    out = dict(schema='rmagna.original-finite-scalar-512-probe.v1', upstream_commit=manifest['upstream']['commit'], reference_sha256=reference.digest(reference.MANIFEST), source=name, source_sha256=manifest['upstream']['files'][name], producer_sha256=reference.digest(Path(__file__)), instrumented_cpp_sha256=hashlib.sha256(source.encode()).hexdigest(), binary_sha256=binary_hash, compiler_version=subprocess.check_output([compiler, '--version'], text=True).splitlines()[0], original_assertions_retained=True, standalone_exit_code=result.returncode, cpp_checks_include_fe_upward=True, oracle='Python Fraction.__float__ nearest rounding + frexp; exact separate power offset', cases=rows)
    return out

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream, protected_roots=[reference.HERE])
        result = capture(args.upstream)
        with destination.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, separators=(',', ':'), allow_nan=False)
            stream.write('\n')
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print('512 Native pairs match independent Fraction bits; all original C++ assertions including FE_UPWARD passed')
if __name__ == '__main__':
    main()
