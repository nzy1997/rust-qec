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


def require(condition, message):
    if not condition:
        raise ValueError('Dataset contract: ' + message)


def circuit_layout(text):
    """Independent record indexing for the benchmark's documented circuit subset.

    No Rust m2d/compiler or private answers participate in this calculation.
    Unknown operations fail closed; REPEAT is expanded, annotations are counted.
    """
    import re
    def parse(lines, nested=False):
        operations=[]
        for raw in lines:
            line=raw.split('#',1)[0].strip()
            if not line: continue
            if line=='}':
                require(nested,'unexpected closing brace')
                return operations
            if line.startswith('REPEAT '):
                match=re.fullmatch(r'REPEAT (\d+)\s*\{',line)
                require(match is not None,'invalid REPEAT')
                operations.extend(parse(lines,True)*int(match[1]));continue
            match=re.fullmatch(r'([A-Z_0-9]+)(?:\[[^\]]*\])?(?:\(([^)]*)\))?(?:\s+(.*))?',line)
            require(match is not None,'invalid instruction')
            operations.append((match[1],match[2],(match[3] or '').split()))
        require(not nested,'unclosed REPEAT')
        return operations
    measurements=detectors=0;observables={}
    inert={'R','RZ','H','S','S_DAG','X','Y','Z','CX','CNOT','ZCX','CZ','X_ERROR','Y_ERROR',
           'Z_ERROR','DEPOLARIZE1','DEPOLARIZE2','LOSS','TICK','QUBIT_COORDS','SHIFT_COORDS'}
    for name,args,targets in parse(iter(text.splitlines())):
        if name in {'M','MR','MZ','MRZ','ML','MRL','MZL','MRZL'}:
            require(args is None,'inline measurement noise outside subset')
            measurements+=len(targets)*(2 if name.endswith('L') else 1)
        elif name in {'DETECTOR','OBSERVABLE_INCLUDE'}:
            indices=[]
            for target in targets:
                match=re.fullmatch(r'rec\[(-\d+)\]',target)
                require(match is not None,'invalid record target')
                index=measurements+int(match[1]);require(0<=index<measurements,'record outside prefix')
                indices.append(index)
            if name=='DETECTOR': detectors+=1
            else:
                require(args is not None and args.isdigit(),'invalid observable index')
                observables.setdefault(int(args),[]).extend(indices)
        else:
            require(name in inert,'unsupported instruction '+name)
    return measurements,detectors,observables


def validate_dataset(read, expected=None):
    """Validate public/private formats and derive every scoring bit independently.

    `read` resolves paths relative to one corpus. Returns validated scoring keys.
    This function stays inside rescore.py so the download has no dependencies.
    """
    sha=lambda data:hashlib.sha256(data).hexdigest()
    public=json.loads(read('public/manifest.json'));private=json.loads(read('private/manifest.json'))
    text=read('public/circuit.stim')
    bits,detectors,observables=circuit_layout(text.decode())
    require(set(observables)=={0},'exactly one observable required')
    shots=public['shots'];require(type(shots) is int and shots>0,'invalid shot count')
    stride=(bits+7)//8;require(bits>0,'empty measurements')
    rows=read('public/shots.b8');answers=read('private/answers.b8');masks=read('private/masks.b8')
    require(len(rows)==shots*stride,'public row length')
    require(len(answers)==len(masks)==shots,'private row length')
    require(set(answers)<={0,1} and set(masks)<={0,1},'private bits outside 0/1')
    if bits%8:
        require(all(rows[i]>>(bits%8)==0 for i in range(stride-1,len(rows),stride)),'nonzero padding')
    identity=(f'format=rstim_decoder_dataset\nschema_version=1\nmode=measurements_blinded\n'
              f'circuit_sha256={sha(text)}\nshots={shots}\nrow_bits={bits}\nshots_b8_sha256={sha(rows)}\n')
    for manifest in [public,private]:
        require(manifest['format']=='rstim_decoder_dataset' and type(manifest['schema_version']) is int
                and manifest['schema_version']==1 and manifest['mode']=='measurements_blinded','schema/mode')
        require(type(manifest['shots']) is int and manifest['shots']==shots,'manifest shots')
        require(manifest['dataset_id']==sha(identity.encode()),'dataset identity')
    require(public['row']==dict(kind='measurements',bits=bits,encoding='b8',bit_order='lsb_first',bytes_per_shot=stride),'row format')
    require(public['circuit']==dict(file='circuit.stim',sha256=sha(text),measurements=bits,detectors=detectors,
                                   observables=1,sweep_bits=0),'circuit metadata')
    for manifest,key,file,data,width,size in [(public,'shots_file','shots.b8',rows,bits,stride),
            (private,'answers_file','answers.b8',answers,1,1),(private,'masks_file','masks.b8',masks,1,1)]:
        require(manifest[key]==dict(file=file,sha256=sha(data),bits=width,bytes_per_shot=size),'file metadata '+key)
    for record,keys in [(public['row'],['bits','bytes_per_shot']),
            (public['circuit'],['measurements','detectors','observables','sweep_bits']),
            (public['shots_file'],['bits','bytes_per_shot']),
            (private['answers_file'],['bits','bytes_per_shot']),(private['masks_file'],['bits','bytes_per_shot'])]:
        require(all(type(record[k]) is int for k in keys),'non-integer row metadata')
    generation=private['generation']
    require(isinstance(generation['rstim_version'],str) and bool(generation['rstim_version']),'generation version')
    require(type(generation['batch_shots']) is int and generation['batch_shots']>0,'generation batch size')
    require(type(generation['seed']) is int and 0<=generation['seed']<2**64,'generation seed')
    record_bits=observables[0]
    derived=bytes((sum((rows[row*stride+bit//8]>>(bit%8))&1 for bit in record_bits)%2)^masks[row]
                  for row in range(shots))
    require(answers==derived,'scoring answer differs from observable XOR mask')
    if expected is not None:
        require(expected['shots']==shots and expected['dataset_id']==public['dataset_id'],'case identity/shots')
        require(expected['seed']==generation['seed'],'case seed')
        for key,data in [('circuit_sha256',text),('public_rows_sha256',rows),('answers_sha256',answers)]:
            require(expected[key]==sha(data),'case digest '+key)
    return derived


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
            if read('rescore.py') != Path(__file__).read_bytes():
                raise ValueError('Archived rescorer differs from the checked implementation')
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
            answers = validate_dataset(lambda name:read(f'{label}/{name}'), case)
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
