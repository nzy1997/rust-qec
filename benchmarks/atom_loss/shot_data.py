"""Package and independently rescore the synthetic, blinded benchmark corpus.

The rescore subcommand needs only the Python standard library and reads ZIP
members directly; it never extracts paths or invokes a decoder.
"""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

ARCHIVE = 'shot-data-v1.zip'


def case_label(case, tradeoff=False):
    return 'tradeoff' if tradeoff else f"d{case['distance']}-p{case['loss_probability']}"


def cases_from(read):
    return [(case_label(c), c) for c in json.loads(read('decoding.json'))] + [
        ('tradeoff', json.loads(read('tradeoff.json')))]


def required_members(cases):
    names = {'decoding.json', 'tradeoff.json', 'rescore.py'}
    for label, case in cases:
        names.update(f'{label}/{name}' for name in [
            'circuit.stim', 'public/circuit.stim', 'public/manifest.json',
            'public/shots.b8', 'private/manifest.json', 'private/answers.b8', 'private/masks.b8'])
        for decoder, result in case['decoders'].items():
            if result['status'] != 'ok' or len(result['runs']) != 3:
                raise ValueError('Only complete three-run evidence can be archived')
            names.update(f'{label}/{decoder}-{rep}.b8' for rep in range(3))
    return names


def pack(work, out):
    results = {name: (out/name).read_bytes() for name in ['decoding.json', 'tradeoff.json']}
    cases = cases_from(results.__getitem__)
    payload = {**results, 'rescore.py': Path(__file__).read_bytes()}
    for name in required_members(cases) - payload.keys():
        payload[name] = (work/name).read_bytes()
    index = {'schema': 1, 'description': 'Synthetic benchmark inputs, scoring keys and all timing-repetition predictions. Decoders receive public/ only.',
             'sha256': {name: hashlib.sha256(data).hexdigest() for name, data in sorted(payload.items())}}
    payload['index.json'] = (json.dumps(index, indent=2) + '\n').encode()
    with zipfile.ZipFile(out/ARCHIVE, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in sorted(payload.items()):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data)
    return rescore(out/ARCHIVE, out)


def rescore(path, results_root=None):
    count = 0
    with zipfile.ZipFile(path) as archive:
        read = archive.read
        index = json.loads(read('index.json'))
        if index['schema'] != 1:
            raise ValueError('Unknown shot archive schema')
        cases = cases_from(read)
        expected = required_members(cases)
        if set(index['sha256']) != expected or sorted(archive.namelist()) != sorted(expected | {'index.json'}):
            raise ValueError('Incomplete or duplicate shot archive members/checksums')
        for name, checksum in index['sha256'].items():
            if hashlib.sha256(read(name)).hexdigest() != checksum:
                raise ValueError(f'Shot archive checksum mismatch: {name}')
        if results_root is not None:
            for name in ['decoding.json', 'tradeoff.json']:
                if read(name) != (results_root/name).read_bytes():
                    raise ValueError('Archive results differ from published results')
        if len(cases) != 16 or len({label for label, _ in cases}) != 16:
            raise ValueError('Expected all 16 benchmark corpora')
        for label, case in cases:
            for name, key in [('circuit.stim', 'circuit_sha256'), ('public/shots.b8', 'public_rows_sha256'),
                              ('private/answers.b8', 'answers_sha256')]:
                if hashlib.sha256(read(f'{label}/{name}')).hexdigest() != case[key]:
                    raise ValueError(f'Corpus differs from published hash: {label}/{name}')
            if read(f'{label}/circuit.stim') != read(f'{label}/public/circuit.stim'):
                raise ValueError('Public circuit differs from benchmark circuit')
            for kind in ['public', 'private']:
                manifest = json.loads(read(f'{label}/{kind}/manifest.json'))
                if manifest['dataset_id'] != case['dataset_id']:
                    raise ValueError('Mismatched dataset identity')
            answers = read(f'{label}/private/answers.b8')
            if len(answers) != case['shots'] or not set(answers) <= {0, 1}:
                raise ValueError('Invalid scoring key')
            native = read(f'{label}/envelope-matching-0.b8')
            for decoder, result in case['decoders'].items():
                for rep in range(3):
                    predicted = read(f'{label}/{decoder}-{rep}.b8')
                    if len(predicted) != len(answers) or not set(predicted) <= {0, 1}:
                        raise ValueError('Invalid prediction rows')
                    errors = sum(p != a for p, a in zip(predicted, answers))
                    if (hashlib.sha256(predicted).hexdigest() != result['prediction_sha256']
                            or errors != result['errors'] or result['shots'] != len(answers)
                            or result['logical_error_rate'] != errors/len(answers)):
                        raise ValueError(f'Prediction rescore mismatch: {label}/{decoder}/{rep}')
                    if 'disagreements_with_native' in result:
                        actual = [sum(n != p for n, p in zip(native, predicted)),
                                  sum(n != a and p == a for n, p, a in zip(native, predicted, answers)),
                                  sum(n == a and p != a for n, p, a in zip(native, predicted, answers))]
                        expected_pairs = [result[k] for k in ['disagreements_with_native',
                            'paired_native_only_wrong', 'paired_python_only_wrong']]
                        if actual != expected_pairs:
                            raise ValueError('Paired errors differ from published results')
                    count += 1
    return f'PASS: {len(cases)} corpora; {count} prediction files rescored'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    packing = subs.add_parser('pack')
    packing.add_argument('--work', type=Path, required=True)
    packing.add_argument('--out', type=Path, required=True)
    scoring = subs.add_parser('rescore')
    scoring.add_argument('archive', type=Path)
    args = parser.parse_args()
    print(pack(args.work, args.out) if args.command == 'pack' else rescore(args.archive))
