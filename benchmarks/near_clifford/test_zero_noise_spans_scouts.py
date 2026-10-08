"""Resealed negative controls for retained zero-noise-span scout evidence."""
import copy,hashlib,json,tempfile
from pathlib import Path
from verify_zero_noise_spans_scouts import DEFAULT
from verify_coefficient_intern_scouts import load,verify_campaign

def run():
    bindings=load(DEFAULT/'bindings.json')['campaigns']
    controls=['missing-event','wrong-order','short-warm-observation','wrong-summary','wrong-driver','cold-counts','cold-rng','cache-cap','wrong-schema']
    for control in controls:
        label='first-cold' if control in ['cold-counts','cold-rng','cache-cap'] else 'first-warm'
        with tempfile.TemporaryDirectory() as directory:
            archive=Path(directory);path=archive/label;path.mkdir();(archive/'probe').symlink_to(DEFAULT/'probe',target_is_directory=True)
            for file in (DEFAULT/label).iterdir():(path/file.name).write_bytes(file.read_bytes())
            events=[json.loads(line) for line in (path/'events.jsonl').read_text().splitlines()]
            if control=='missing-event':events.pop()
            elif control=='wrong-order':
                events[48],events[49]=events[49],events[48]
                for index,event in enumerate(events):event['index']=index
            elif control=='short-warm-observation':
                obs=events[48]['result']['observations'][0];obs['elapsed_ns']=1;obs['ns_per_call']=1/obs['calls']
            elif control=='wrong-summary':
                summary=load(path/'summary.json');summary[0]['speedup']+=1;(path/'summary.json').write_text(json.dumps(summary))
            elif control=='wrong-driver':
                with (path/'original-driver.py').open('a') as stream:stream.write('\n# resealed fixture mutation\n')
            elif control=='cold-counts':events[0]['result']['observations'][0]['accepted']+=1
            elif control=='cold-rng':events[0]['result']['observations'][0]['continuation'][0]^=1
            elif control=='cache-cap':events[0]['result']['observations'][0]['cache_reserved_bytes']=64*1024*1024+1
            elif control=='wrong-schema':
                header=load(path/'header.json');header['schema']='exploratory.coefficient-intern-ablation.v1';(path/'header.json').write_text(json.dumps(header))
            data=''.join(json.dumps(event)+'\n' for event in events).encode();(path/'events.jsonl').write_bytes(data);closure=load(path/'closure.json');closure['events']=len(events);closure['events_sha256']=hashlib.sha256(data).hexdigest();(path/'closure.json').write_text(json.dumps(closure))
            try:verify_campaign(archive,label,copy.deepcopy(bindings[label]),schema_prefix='zero-noise-spans')
            except ValueError:print('PASS rejected',control,flush=True)
            else:raise ValueError('accepted resealed control: '+control)

if __name__=='__main__':run()
