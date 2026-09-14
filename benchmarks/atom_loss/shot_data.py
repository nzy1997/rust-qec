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


def mask_stream(seed, domain):
    """Independent ChaCha12 word stream: rand 0.8 / rand_chacha 0.3, 64-bit host.

    This reproduces the pinned exporter RNG, not a portable promise about StdRng
    in future rand releases. Counter and stream start at zero; words are LE.
    """
    import struct
    key=hashlib.sha256(b'rstim-decoder-dataset-v1\n'+domain+b'\n'+seed.to_bytes(8,'little')).digest()
    initial=list(struct.unpack('<4I',b'expand 32-byte k'))+list(struct.unpack('<8I',key))
    counter=0; mask=2**32-1
    def rotate(v,n): return ((v<<n)|(v>>(32-n)))&mask
    while True:
        state=initial+[counter&mask,counter>>32,0,0];x=state.copy()
        def quarter(a,b,c,d):
            x[a]=(x[a]+x[b])&mask;x[d]=rotate(x[d]^x[a],16)
            x[c]=(x[c]+x[d])&mask;x[b]=rotate(x[b]^x[c],12)
            x[a]=(x[a]+x[b])&mask;x[d]=rotate(x[d]^x[a],8)
            x[c]=(x[c]+x[d])&mask;x[b]=rotate(x[b]^x[c],7)
        for _ in range(6):
            for q in [(0,4,8,12),(1,5,9,13),(2,6,10,14),(3,7,11,15),
                      (0,5,10,15),(1,6,11,12),(2,7,8,13),(3,4,9,14)]: quarter(*q)
        yield from ((a+b)&mask for a,b in zip(x,state))
        counter+=1


def generated_masks(seed, shots, batch_shots):
    """Reconstruct private labels including per-batch unbiased row shuffling."""
    labels=mask_stream(seed,b'logical-mask');permutation=mask_stream(seed,b'row-permutation')
    result=bytearray();maximum=2**64-1
    for offset in range(0,shots,batch_shots):
        batch=[next(labels)>>31 for _ in range(min(batch_shots,shots-offset))]
        for index in range(len(batch)-1,0,-1):
            width=index+1;zone=(width<<(64-width.bit_length()))-1
            while True:
                value=next(permutation)|(next(permutation)<<32)
                product=value*width
                if (product&maximum)<=zone: break
            replacement=product>>64
            batch[index],batch[replacement]=batch[replacement],batch[index]
        result.extend(batch)
    return bytes(result)


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
    require(masks==generated_masks(generation['seed'],shots,generation['batch_shots']),
            'mask differs from seeded exporter generation')
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


def original_plan(cases):
    """The download has a fixed experiment inventory, independent of its index."""
    settings = {f'd{d}-p{p}': (d, d, p, 20260911)
                for d in (3, 5, 7) for p in (.0001, .0003, .001, .003, .01)}
    settings['tradeoff'] = (3, 2, .003, 20260912)
    require(len(cases) == 16 and {label for label, _ in cases} == set(settings),
            'original experiment requires all 16 fixed settings exactly once')
    for label, case in cases:
        d, rounds, loss, seed = settings[label]
        for key, expected in [('distance', d), ('rounds', rounds), ('seed', seed), ('shots', 5000)]:
            require(type(case.get(key)) is int and case[key] == expected,
                    'original fixed workload '+key)
        for key, expected in [('loss_probability', loss), ('pauli_probability', .001)]:
            require(type(case.get(key)) in (int, float) and case[key] == expected,
                    'Declared Pauli probability differs from fixed workload' if key == 'pauli_probability'
                    else 'original fixed workload '+key)
        backends = {'envelope-matching', 'envelope-matching-offline',
                    'pymatching-fixed', 'pymatching-envelope'}
        if label == 'tradeoff':
            backends |= {'envelope-mle', 'pymatching-fixed-loop'}
        require(type(case.get('decoders')) is dict and set(case['decoders']) == backends,
                'original comparator inventory')
        for name, result in case['decoders'].items():
            require(type(result) is dict and result.get('status') == 'ok',
                    'original comparator did not complete')
            runs = result.get('runs')
            require(type(runs) is list and len(runs) == 3
                    and all(type(run) is dict and bool(run) for run in runs),
                    'original comparator requires three complete run records')
            if name.startswith('pymatching'):
                require(all(type(run.get('export_repetition')) is int and run['export_repetition'] == rep
                            for rep, run in enumerate(runs)), 'original Python repetition identity')
            else:
                require(all(run.get('status') == 'ok' and type(run.get('exit_code')) is int
                            and run['exit_code'] == 0 for run in runs),
                        'original native repetition did not complete')
                for run in runs:
                    stats = run.get('stats')
                    require(type(stats) is dict and all(type(stats.get(key)) is int and stats[key] == value
                            for key, value in [('attempted_shot_count', 5000), ('timeout_count', 0),
                                               ('infeasible_shot_count', 0)]),
                            'original native repetition has incomplete shot outcomes')


def check_score(result, predicted, answers):
    """Recompute every portable accuracy field; timing is outside this check."""
    import math
    require(len(predicted) == len(answers) and set(predicted) <= {0, 1}, 'Prediction rescore mismatch: invalid rows')
    errors = sum(p != a for p, a in zip(predicted, answers))
    shots = len(answers)
    require(type(result.get('errors')) is int and result['errors'] == errors
            and type(result.get('shots')) is int and result['shots'] == shots,
            'Prediction rescore mismatch: integer counts')
    require(result.get('prediction_sha256') == hashlib.sha256(predicted).hexdigest(),
            'Prediction rescore mismatch: hash')
    rate = errors/shots
    require(type(result.get('logical_error_rate')) in (int, float)
            and result['logical_error_rate'] == rate, 'Prediction rescore mismatch: failure rate')
    z = 1.959963984540054
    center = (rate+z*z/(2*shots))/(1+z*z/shots)
    half = z*math.sqrt(rate*(1-rate)/shots+z*z/(4*shots*shots))/(1+z*z/shots)
    interval = result.get('wilson_95')
    require(type(interval) is list and len(interval) == 2
            and all(type(v) in (int, float) and math.isfinite(v) for v in interval)
            and interval == [max(0., center-half), min(1., center+half)],
            'Prediction rescore mismatch: Wilson interval')


def check_pairs(result, native, predicted, answers):
    actual = [sum(n != p for n, p in zip(native, predicted)),
              sum(n != a and p == a for n, p, a in zip(native, predicted, answers)),
              sum(n == a and p != a for n, p, a in zip(native, predicted, answers))]
    keys = ('disagreements_with_native', 'paired_native_only_wrong', 'paired_python_only_wrong')
    require(all(type(result.get(key)) is int for key in keys)
            and [result[key] for key in keys] == actual, 'paired errors differ from published results')


def required_members(cases):
    original_plan(cases)
    names = {'decoding.json', 'tradeoff.json', 'rescore.py'}
    for label, case in cases:
        names.update(f'{label}/{name}' for name in [
            'circuit.stim', 'public/circuit.stim', 'public/manifest.json',
            'public/shots.b8', 'private/manifest.json', 'private/answers.b8', 'private/masks.b8'])
        for decoder in case['decoders']:
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
        if type(index['schema']) is not int or index['schema'] != 1:
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
            require(case.get('pauli_probability') == .001, 'Declared Pauli probability differs from fixed workload')
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
                    check_score(result, predicted, answers)
                    if decoder.startswith('pymatching'):
                        check_pairs(result, native, predicted, answers)
                    count += 1
    require(count == 198, 'original experiment requires all 198 prediction files')
    return f'PASS: {len(cases)} corpora; {count} prediction files rescored'


SEEDS = [2026091401,2026091402,2026091403]


def paired_interval(a,b,n):
    """Conservative pointwise 95% CI for P(native-only wrong)-P(other-only wrong).

    Each discordant count has a binomial marginal. Bound both at 97.5% using
    Clopper-Pearson and subtract opposite ends; union bound gives >=95% joint
    coverage without treating the two decoder outcomes as independent.
    """
    import math
    def cdf(k,p):
        if p==0:return 1.
        if p==1:return float(k==n)
        logs=[n*math.log1p(-p)]
        for j in range(k):logs.append(logs[-1]+math.log(n-j)-math.log(j+1)+math.log(p)-math.log1p(-p))
        top=max(logs)
        return min(1.,math.exp(top)*math.fsum(math.exp(v-top) for v in logs))
    def bound(k,target):
        lo,hi=0.,1.
        for _ in range(55):
            mid=(lo+hi)/2
            if cdf(k,mid)>target:lo=mid
            else:hi=mid
        return (lo+hi)/2
    def cp(k):
        if k>n//2:
            lo,hi=cp(n-k);return 1-hi,1-lo
        return (0. if k==0 else bound(k-1,.9875),1. if k==n else bound(k,.0125))
    al,ah=cp(a);bl,bh=cp(b)
    return [al-bh,ah-bl]


def seed_summaries(cases):
    grouped={}
    for case in cases:grouped.setdefault(case['setting'],[]).append(case)
    summaries=[]
    for label,group in sorted(grouped.items()):
        for backend in sorted(group[0]['paired']):
            n=sum(c['shots'] for c in group)
            a=sum(c['paired'][backend]['native_only_wrong'] for c in group)
            b=sum(c['paired'][backend]['other_only_wrong'] for c in group)
            summaries.append({'setting':label,'comparator':backend,'shots':n,
                'native_only_wrong':a,'other_only_wrong':b,'difference':(a-b)/n,
                'paired_95':paired_interval(a,b,n)})
    return summaries


def rescore_seeds(path, results_root=None):
    with zipfile.ZipFile(path) as z:
        index=json.loads(z.read('index.json'));report=json.loads(z.read('accuracy-seeds.json'))
        if results_root is not None:
            require(z.read('accuracy-seeds.json')==(results_root/'accuracy-seeds.json').read_bytes(),'seed report mismatch')
            require(z.read('rescore.py')==Path(__file__).read_bytes(),'seed rescorer mismatch')
        cases=report['cases'];settings={f'd{d}-p{p}' for d in [3,5,7] for p in [.0001,.0003,.001,.003,.01]}|{'tradeoff'}
        require(len(cases)==48 and {(c['setting'],c['seed']) for c in cases}=={(label,seed) for label in settings for seed in SEEDS},'seed experiment completeness')
        require(type(report['seeds']) is list and all(type(seed) is int for seed in report['seeds'])
                and report['seeds']==SEEDS and type(report['shots_per_seed']) is int
                and report['shots_per_seed']==5000,'seed plan')
        members={'accuracy-seeds.json','rescore.py'}
        for c in cases:
            require(c.get('pauli_probability') == .001, 'Declared Pauli probability differs from fixed workload')
            label=c['setting'];prefix=f"{label}-s{c['seed']}"
            read=lambda name:z.read(f'{prefix}/{name}')
            d,p=(3,.003) if label=='tradeoff' else (int(label[1]),float(label.split('-p')[1]))
            require(all(type(c.get(key)) is int for key in ('distance','rounds','shots','seed'))
                    and c['distance']==d and c['rounds']==(2 if label=='tradeoff' else d)
                    and type(c['loss_probability']) in (int,float) and c['loss_probability']==p
                    and c['shots']==5000,'seed workload')
            answers=validate_dataset(read,c)
            names={'envelope-matching','pymatching-envelope','pymatching-fixed'}|({'envelope-mle'} if label=='tradeoff' else set())
            require(set(c['decoders'])==names and set(c['paired'])==names-{'envelope-matching'},'seed comparators')
            for name in ['public/circuit.stim','public/manifest.json','public/shots.b8','private/manifest.json','private/answers.b8','private/masks.b8']:
                members.add(prefix+'/'+name)
            native=read('envelope-matching.b8')
            for name,r in c['decoders'].items():
                members.add(prefix+'/'+name+'.b8');pred=read(name+'.b8')
                check_score(r,pred,answers)
                if name!='envelope-matching':
                    a=sum(n!=k and q==k for n,q,k in zip(native,pred,answers))
                    b=sum(n==k and q!=k for n,q,k in zip(native,pred,answers))
                    pair=c['paired'][name]
                    require(type(pair) is dict and all(type(v) is int for v in pair.values())
                            and pair=={'native_only_wrong':a,'other_only_wrong':b},'seed paired scores')
        require(set(index['sha256'])==members and sorted(z.namelist())==sorted(members|{'index.json'}),'seed archive completeness')
        for name,digest in index['sha256'].items():require(hashlib.sha256(z.read(name)).hexdigest()==digest,'seed archive checksum')
        expected=seed_summaries(cases)
        import math
        require(len(report['pooled'])==len(expected),'seed pooled completeness')
        for actual,want in zip(report['pooled'],expected):
            require(all(type(actual.get(key)) is int for key in
                        ('shots','native_only_wrong','other_only_wrong'))
                    and type(actual.get('difference')) in (int,float), 'seed pooled score types')
            interval=actual['paired_95']
            require(isinstance(interval,list) and len(interval)==2 and all(type(v) in [float,int] and math.isfinite(v) for v in interval),'seed paired interval format')
            require(all(math.isclose(a,b,rel_tol=1e-10,abs_tol=1e-12) for a,b in zip(interval,want['paired_95'])),'seed pooled paired intervals')
            require({**actual,'paired_95':want['paired_95']}==want,'seed pooled counts/difference')
    return 'PASS: 48 independent-seed corpora and 147 predictions rescored'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    seeds = subs.add_parser('rescore-seeds')
    seeds.add_argument('archive', type=Path)
    packing = subs.add_parser('pack')
    packing.add_argument('--work', type=Path, required=True)
    packing.add_argument('--out', type=Path, required=True)
    scoring = subs.add_parser('rescore')
    scoring.add_argument('archive', type=Path)
    args = parser.parse_args()
    print(pack(args.work, args.out) if args.command == 'pack' else
          rescore_seeds(args.archive) if args.command == 'rescore-seeds' else rescore(args.archive))
