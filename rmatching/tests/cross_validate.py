#!/usr/bin/env python3
"""Cross-validate rmatching against PyMatching on random syndromes."""
import subprocess, sys, random, tempfile, os, math

def make_rep_code_dem(d, p):
    lines = []
    for i in range(d - 1):
        lines.append(f"error({p}) D{i} D{i+1} L0")
    lines.append(f"error({p}) D0")
    lines.append(f"error({p}) D{d-2}")
    return "\n".join(lines) + "\n"

def decode_with_rmatching(dem_text, syndromes):
    with tempfile.NamedTemporaryFile(mode='w', suffix='.dem', delete=False) as f:
        f.write(dem_text); dem_path = f.name
    try:
        stdin_data = "\n".join(" ".join(str(b) for b in s) for s in syndromes) + "\n"
        result = subprocess.run(
            ["cargo", "run", "--bin", "rmatching_cli", "--quiet", "--", dem_path],
            input=stdin_data, capture_output=True, text=True, check=True)
        return [[int(x) for x in line.strip().split()] for line in result.stdout.strip().split("\n") if line.strip()]
    finally:
        os.unlink(dem_path)

def decode_with_pymatching(dem_text, syndromes):
    import pymatching, numpy as np
    m = pymatching.Matching()
    for line in dem_text.strip().split("\n"):
        line = line.strip()
        if not line or not line.startswith("error"): continue
        p = float(line.split("(")[1].split(")")[0])
        tokens = line.split()[1:]
        dets = [int(x[1:]) for x in tokens if x.startswith("D")]
        obs = [int(x[1:]) for x in tokens if x.startswith("L")]
        w = math.log((1-p)/p) if 0 < p < 1 else 0
        if len(dets) == 2:
            m.add_edge(dets[0], dets[1], fault_ids=obs, weight=w, error_probability=p)
        elif len(dets) == 1:
            m.add_boundary_edge(dets[0], fault_ids=obs, weight=w, error_probability=p)
    return [list(m.decode(np.array(s, dtype=np.uint8)).astype(int)) for s in syndromes]

def main():
    random.seed(42)
    N, D, P = 1000, 5, 0.1
    dem = make_rep_code_dem(D, P)
    syns = [[random.randint(0,1) for _ in range(D-1)] for _ in range(N)]
    print(f"Cross-validating: rep code d={D}, p={P}, {N} syndromes")
    rm = decode_with_rmatching(dem, syns)
    pm = decode_with_pymatching(dem, syns)
    mis = [(i,syns[i],rm[i],pm[i]) for i in range(N) if rm[i] != pm[i]]
    if not mis:
        print(f"PASS: all {N} syndromes match!"); sys.exit(0)
    else:
        print(f"FAIL: {len(mis)}/{N} mismatches")
        i,s,r,p = mis[0]
        print(f"  First at {i}: syn={s} rm={r} pm={p}")
        sys.exit(1)

if __name__ == "__main__":
    main()
