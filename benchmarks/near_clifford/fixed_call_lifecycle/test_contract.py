"""Synthetic offline contract tests; never a native timing/accuracy witness."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))

from collections import Counter
from pathlib import Path
import copy, json, tempfile
from schedule import INPUTS, CELLS, HORIZONS, ORDERS, schedule
from verify import verify

def payload(event):
    name, shots, policy, factor = event["cell"]
    validate = event["action"] == "validate"
    maximum = 8 if validate else 256
    rows = []
    for index in range(maximum):
        row = dict(call_index=index, seed=1739+index, elapsed_ns=100+index,
            attempted=shots, accepted=0, discarded=shots, logical_errors=0, cache_reserved_after=0)
        if validate: row.update(rng_tail=list(range(16)), measurements=[[] for _ in range(shots)])
        rows.append(row)
    prefixes = [dict(calls=n, sampling_ns=sum(r["elapsed_ns"] for r in rows[:n]),
        compile_prepare_sampling_ns=30+sum(r["elapsed_ns"] for r in rows[:n]))
        for n in (HORIZONS[:4] if validate else HORIZONS)]
    return dict(schema="rstim.fixed-call-lifecycle.draft.v1", timing_contract="fixed-seeded-counts-prefix-v1",
        action=event["action"], input_sha256=INPUTS[name], arithmetic=policy, cache_factor=factor, shots=shots,
        compile_ns=10, prepare_ns=20, teardown_ns=30, final_compile_prepare_sampling_teardown_ns=60+sum(r["elapsed_ns"] for r in rows),
        cache_reserved_final=0, peak_active_rank=5, calls=rows, prefixes=prefixes)

def main():
    finite, timed = schedule("validate"), schedule("bench")
    if len(finite)!=72 or len(timed)!=864 or len({e["id"] for e in finite+timed})!=936:raise ValueError("coverage")
    for cell in CELLS:
        groups = [[e for e in timed if e["round"]==r and e["cell"]==list(cell)] for r in range(12)]
        if Counter(tuple(e["role"] for e in group) for group in groups)!=Counter({order:2 for order in ORDERS}):raise ValueError("unbalanced role order")
    with tempfile.TemporaryDirectory(prefix="synthetic-fixed-call-") as temp:
        root=Path(temp);events=finite+timed
        manifest=dict(schema="rstim.fixed-call-streams.draft.v1",events=events)
        def write(path,value): path.write_text(json.dumps(value,separators=(",",":"))+"\n")
        write(root/"streams.json",manifest)
        for e in events:write(root/(e["id"]+".stdout"),payload(e))
        result=verify(root)
        if len(result["comparisons"])!=216:raise ValueError("comparison coverage")
        def rejected():
            try:verify(root)
            except (ValueError,KeyError,TypeError):return
            raise ValueError("negative fixture incorrectly accepted")
        for action in ("missing", "duplicate", "reorder"):
            bad=copy.deepcopy(manifest)
            if action=="missing":bad["events"].pop()
            if action=="duplicate":bad["events"][-1]=bad["events"][0]
            if action=="reorder":bad["events"][:2]=reversed(bad["events"][:2])
            write(root/"streams.json",bad);rejected()
        write(root/"streams.json",manifest)
        event=timed[0]; path=root/(event["id"]+".stdout");original=payload(event)
        for action in ("identity", "seed", "prefix", "counts-parity"):
            bad=copy.deepcopy(original)
            if action=="identity":bad["input_sha256"]="0"*64
            if action=="seed":bad["calls"][0]["seed"]+=1
            if action=="prefix":bad["prefixes"][0]["sampling_ns"]+=1
            if action=="counts-parity":bad["calls"][0].update(accepted=1,discarded=bad["shots"]-1)
            write(path,bad);rejected()
        write(path,original)
        path=root/(finite[0]["id"]+".stdout");original=payload(finite[0]);bad=copy.deepcopy(original)
        bad["calls"][0]["rng_tail"][0]=42;write(path,bad);rejected();write(path,original)
        extra=root/"unexpected.stdout";write(extra,{});rejected();extra.unlink()
        verify(root)
    print("PASS synthetic offline only: complete 72/864 schedule, all six role permutations twice per cell, 216 comparisons, 9 negative bundles rejected")
if __name__=="__main__":main()
