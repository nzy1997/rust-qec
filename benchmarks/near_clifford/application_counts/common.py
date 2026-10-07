"""Frozen original-circuit annotations and independently replayed count semantics."""
import hashlib
import math
import re
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
CONTRACT='all-zero raw detector postselection; raw observable 0 counts; no reference normalization'
NAMES=['msc_d3_inject_cultivate_p1e-3','msc_d5_inject_cultivate_p1e-3',
       'pure_surface_d7_r7_p1e-3','pure_surface_d9_r9_p1e-3']
POLICIES=['strict','fused']
SHOTS=[1,64,1024]


def require(value,message):
    if not value: raise ValueError(message)


def digest(data): return hashlib.sha256(data).hexdigest()


def annotations(text):
    """Expand only bounded REPEATs for analysis; execution receives original bytes."""
    lines=[line.split('#',1)[0].strip() for line in text.splitlines()]
    lines=[line for line in lines if line]
    def block(index,nested=False):
        result=[]
        while index<len(lines):
            line=lines[index];index+=1
            if line=='}':
                require(nested,'unexpected repeat terminator')
                return result,index
            match=re.fullmatch(r'REPEAT\s+(\d+)\s*\{',line)
            if match:
                count=int(match[1]);require(1<=count<=1024,'repeat count outside bounded corpus')
                inner,index=block(index,True)
                result.extend(inner*count)
            else:
                require('{' not in line and '}' not in line,'unsupported inline repeat syntax')
                result.append(line)
            require(len(result)<=100000,'expanded corpus too large')
        require(not nested,'unclosed repeat')
        return result,index
    expanded,_=block(0)
    width=0;detectors=[];observable=set();found=False
    for line in expanded:
        match=re.fullmatch(r'(\w+)(?:\(([^)]*)\))?(?:\s+(.*))?',line)
        require(match is not None,'unsupported annotation-analysis syntax: '+line)
        gate,argument,targets=match.groups();words=(targets or '').split()
        if gate in ['M','MZ','MX','MY','MR','MRZ','MRX','MRY','MPP']:
            width+=len(words)
        if gate not in ['DETECTOR','OBSERVABLE_INCLUDE']: continue
        mask=set()
        for token in words:
            ref=re.fullmatch(r'rec\[(-\d+)\]',token)
            require(ref is not None,'annotation target must be negative record reference')
            index=width+int(ref[1]);require(0<=index<width,'record reference outside available history')
            mask.symmetric_difference_update([index])
        if gate=='DETECTOR': detectors.append(sorted(mask))
        else:
            require(argument is not None and argument.isdigit(),'observable index required')
            if int(argument)==0:
                found=True;observable.symmetric_difference_update(mask)
    require(found and width>0,'observable 0 and measurement records required')
    return dict(width=width,detectors=detectors,observable=sorted(observable))


def raw_counts(payload,masks):
    bits=payload['measurements'];width=payload['width'];shots=payload['shots']
    require(width==masks['width'] and len(bits)==width*shots,'record dimensions disagree with original annotations')
    require(all(type(b) is int and b in (0,1) for b in bits),'invalid record bit')
    accepted=logical=0
    detectors=[sum(1<<q for q in mask) for mask in masks['detectors']]
    observable=sum(1<<q for q in masks['observable'])
    for offset in range(0,len(bits),width):
        row=sum(bit<<q for q,bit in enumerate(bits[offset:offset+width]))
        if all((row&mask).bit_count()%2==0 for mask in detectors):
            accepted+=1;logical+=(row&observable).bit_count()%2
    return dict(attempted=shots,accepted=accepted,discarded=shots-accepted,logical_errors=logical)


def counts(result):
    return {key:sum(obs[key] for obs in result['observations'])
            for key in ['attempted','accepted','discarded','logical_errors']}


def compare(left,right,alpha):
    n,m=left['attempted'],right['attempted']
    require(n>=8192 and m>=8192,'insufficient finite counts evidence')
    bound=math.sqrt(math.log(2/alpha)/(2*n))+math.sqrt(math.log(2/alpha)/(2*m))
    delta=max(abs(left[key]/n-right[key]/m) for key in ['accepted','logical_errors'])
    return dict(bound=bound,max_delta=delta,passed=delta<=bound)


def check_result(result,backend,shots,repetitions,validate=False,policy=None,batch=None):
    require(result['status']=='ok' and result['backend']==backend,'executor/status mismatch')
    require(result['output_contract']==CONTRACT,'output contract differs')
    require(result['shots']==shots or validate and backend=='rstim' and result['call_shots']==shots,
            'call size differs')
    require(len(result['observations'])==repetitions,'observation count differs')
    if backend=='rstim': require(result['arithmetic']==policy,'Rust policy mismatch')
    else: require(str(result['batch'])==str(batch) and result['isolated'] is True,'peer batch/isolation mismatch')
    require(type(result['shots']) is int and result['shots']>0,'invalid shot count type')
    for obs in result['observations']:
        require(all(type(obs[key]) is int for key in
                    ['calls','attempted','accepted','discarded','logical_errors','elapsed_ns']),
                'observation counts/time must be integers')
        require(obs['calls']>0,'non-positive call count')
        require(obs['attempted']==shots*obs['calls'],'attempt count mismatch')
        require(obs['accepted']+obs['discarded']==obs['attempted'] and
                0<=obs['logical_errors']<=obs['accepted']<=obs['attempted'] and obs['discarded']>=0,
                'invalid postselection counts')
        require(obs['elapsed_ns']>0 and type(obs['ns_per_call']) in [int,float] and
                math.isfinite(obs['ns_per_call']) and obs['ns_per_call']>0 and
                obs['ns_per_call']==obs['elapsed_ns']/obs['calls'],'timing arithmetic differs')
        require(obs['attempted']>=8192 if validate else obs['elapsed_ns']>=50_000_000,'incomplete observation')
    require(type(result['compile_ns']) is int and result['compile_ns']>0 and
            type(result['prepare_ns']) is int and result['prepare_ns']>=0 and
            type(result['peak_rss_bytes']) is int and result['peak_rss_bytes']>0,
            'missing measured phases or process RSS')
    require(type(result['first_ns']) is int and
            (result['first_ns']==0 if validate and backend=='rstim' else result['first_ns']>0),
            'invalid measured first-call time')
    metric='peak_active_rank' if backend=='rstim' else 'peak_active_width'
    require(type(result[metric]) is int and result[metric]>=0,'invalid activity metric')
    if backend=='rstim':
        require(type(result['cache_reserved_bytes']) is int and
                0<=result['cache_reserved_bytes']<=64*1024*1024,'invalid coefficient cache ledger')
