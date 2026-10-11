"""Independent raw parity, finite bounds and observation validators."""
import base64, hashlib, math, re, zlib
from timing import TIMING_CONTRACT, RUST_ROLES

def require(value,message):
    if not value: raise ValueError(message)

def counts(payload):
    return {key:sum(o[key] for o in payload["observations"]) for key in ["attempted","accepted","discarded","logical_errors"]}

def masks(text):
    # Independent iterative interpreter for bounded repeat blocks. Native
    # execution receives the original text, not this annotation-analysis walk.
    lines=text.splitlines()
    content=[line.split('#',1)[0].strip() for line in lines]
    matching={};opening=[];repeat_counts={}
    for index,line in enumerate(content):
        repeat=re.fullmatch(r'REPEAT\s+([0-9]+)\s*\{',line)
        if repeat:
            count=int(repeat[1]);require(1<=count<=1024,'repeat count outside bounded corpus')
            opening.append(index);require(len(opening)<=32,'repeat nesting too deep');repeat_counts[index]=count
        elif line=='}':
            require(bool(opening),'unexpected repeat terminator')
            start=opening.pop();matching[start]=index
        else:
            require('{' not in line and '}' not in line,'unsupported inline repeat syntax')
    require(not opening,'unclosed repeat')
    width=0;detectors=[];observable=set();found=False;cursor=0;frames=[];steps=0
    while cursor<len(content):
        position=cursor;line=content[cursor];cursor+=1;steps+=1
        require(steps<=100000,'expanded corpus too large')
        if position in repeat_counts:
            frames.append([cursor,matching[position],repeat_counts[position]])
            continue
        if line=='}':
            require(frames and frames[-1][1]==position,'repeat structure differs')
            frames[-1][2]-=1
            if frames[-1][2]:cursor=frames[-1][0]
            else:frames.pop()
            continue
        if not line:continue
        match=re.fullmatch(r'([A-Za-z0-9_]+)(?:\(([^)]*)\))?(?:\s+(.*))?',line)
        require(match is not None,'unsupported annotation-analysis syntax')
        gate,argument,targets=match.groups();words=(targets or '').split()
        if gate in {'M','MZ','MX','MY','MR','MRZ','MRX','MRY','MPP'}:width+=len(words)
        if gate not in {'DETECTOR','OBSERVABLE_INCLUDE'}:continue
        parity=set()
        for token in words:
            record=re.fullmatch(r'rec\[(-[0-9]+)\]',token);require(record is not None,'annotation token')
            index=width+int(record[1]);require(0<=index<width,'annotation bounds')
            if index in parity:parity.remove(index)
            else:parity.add(index)
        if gate=='DETECTOR':detectors.append(sorted(parity))
        else:
            require(argument is not None and argument.isdigit(),'observable index required')
            if int(argument)==0:observable.symmetric_difference_update(parity);found=True
    require(found and width>0,'missing observable zero or measurement records')
    # Keep repeat structure and original physical instructions in the raw
    # projection. Only output annotations are removed, including inside blocks.
    projection=[]
    for line,code in zip(lines,content):
        words=code.split()
        if not words or words[0].split('(',1)[0] not in {'DETECTOR','OBSERVABLE_INCLUDE'}:projection.append(line)
    return dict(width=width,detectors=detectors,observable=sorted(observable)), '\n'.join(projection)+'\n'
def replay(p,m):
    width=p['width'];n=p['shots'];bits=p['measurements']
    require(type(width) is int and type(n) is int and n>=8192 and width==m['width'] and len(bits)==width*n,'raw dimensions')
    require(all(type(x) is int and x in [0,1] for x in bits),'raw bit values')
    accepted=logical=0
    # Index-list XOR replay deliberately avoids the author's packed integer masks.
    for i in range(n):
        start=i*width
        if any(sum(bits[start+j] for j in d)%2 for d in m['detectors']):continue
        accepted+=1;logical+=sum(bits[start+j] for j in m['observable'])%2
    return dict(attempted=n,accepted=accepted,discarded=n-accepted,logical_errors=logical)
def compact_equal(raw,event):
    original=dict(raw);encoded=event['measurements'];total=raw['shots']*raw['width']
    require(encoded['codec']=='zlib-packbits-big-v1','compact codec')
    dec=zlib.decompressobj();packed=dec.decompress(base64.b64decode(encoded['data'],validate=True),(total+7)//8+1)
    require(dec.eof and not dec.unused_data and not dec.unconsumed_tail and len(packed)==(total+7)//8,'compact length/stream')
    require(not total%8 or not packed[-1]&((1<<(8-total%8))-1),'compact padding')
    bits=[(packed[i//8]>>(7-i%8))&1 for i in range(total)]
    require(bits==raw['measurements'],'compact raw bit mismatch')
    original['measurements']=encoded;require(original==event,'compact metadata mismatch')
def compare(a,b,alpha):
    n,m=a['attempted'],b['attempted'];bound=(math.log(2/alpha)/(2*n))**.5+(math.log(2/alpha)/(2*m))**.5
    delta=max(abs(a[k]/n-b[k]/m) for k in ['accepted','logical_errors'])
    return dict(bound=bound,max_delta=delta,passed=delta<=bound)
def validate_observations(p,e):
    validate=e['kind']=='counts-validation';shots=e['shots'];rust=e['backend'] in RUST_ROLES
    require(p['status']=='ok' and p['backend']==('rstim' if rust else e['backend']),'status/backend')
    require(p.get('timing_contract') == TIMING_CONTRACT, 'matched timing contract differs')
    require(p['output_contract']=='all-zero raw detector postselection; raw observable 0 counts; no reference normalization','output semantics')
    require(len(p['observations'])==(1 if validate else 7),'observations')
    require(p['shots']==shots if not (rust and validate) else p['call_shots']==shots and p['shots']==sum(o['attempted'] for o in p['observations']),'shots')
    require(type(p['compile_ns']) is int and p['compile_ns']>0 and type(p['prepare_ns']) is int and p['prepare_ns']>=0,'cold phases')
    require(type(p['first_ns']) is int and (p['first_ns']==0 if rust and validate else p['first_ns']>0),'first phase')
    require(type(p['peak_rss_bytes']) is int and p['peak_rss_bytes']>0,'RSS')
    require(type(p['peak_active_rank' if rust else 'peak_active_width']) is int,'activity')
    if rust:
        require(p['arithmetic']==e['policy'] and 0<=p['cache_reserved_bytes']<=64*1024**2,'Rust policy/cache')
        if validate:require(p['exact_native_counts_rng'] is True,'executable selfcheck')
    else:require(p['batch']==e['selection']['batch'] and p['isolated'] is True,'batch/isolation')
    for o in p['observations']:
        require(all(type(o[k]) is int for k in ['calls','elapsed_ns','attempted','accepted','discarded','logical_errors']),'observation integer type')
        require(o['calls']>0 and o['elapsed_ns']>0 and o['attempted']==shots*o['calls'],'attempted/call/time')
        require(o['accepted']+o['discarded']==o['attempted'] and 0<=o['logical_errors']<=o['accepted']<=o['attempted'],'count arithmetic')
        require(math.isfinite(o['ns_per_call']) and o['ns_per_call']==o['elapsed_ns']/o['calls'],'ns arithmetic')
        require(o['attempted']>=8192 if validate else o['elapsed_ns']>=50_000_000,'population/warm bound')
