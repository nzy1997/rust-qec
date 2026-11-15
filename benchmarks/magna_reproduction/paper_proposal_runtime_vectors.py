#!/usr/bin/env python3
"""Generate finite P/Q inputs for the Rust supplied-action integration probe."""
import argparse
from itertools import product
import json
from pathlib import Path
import reference
from paper_proposals import CultivationProposal, check_cultivation_model, require
from witness_paths import admit_destinations


def capture():
    cases = []
    for closing in (1, 2):
        model = CultivationProposal([[0,1,2],[1],[0,1,3],[2]], "1/1000", closing_sites=closing)
        check_cultivation_model(model)
        rows = []
        for pattern in product(range(4),range(2),range(4),range(2)):
            proposal = model.mass(pattern)
            rates = {rate: str(model.physical_mass(pattern,rate))
                     for rate in ("1/1000","3/1000","1/100")}
            rows.append(dict(pattern=pattern, proposal=str(proposal), physical=rates))
        cases.append(dict(closing_sites=closing, rows=rows))
    return dict(schema="rmagna.paper-proposal-runtime-v1", cases=cases,
        generator_sha256=reference.digest(Path(__file__)),
        model_sha256=reference.digest(reference.HERE/"paper_proposals.py"),
        source="independent finite proposal branch enumeration; physical supplied-action replay",
        rates=["1/1000","3/1000","1/100"])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out",type=Path,required=True)
    args=parser.parse_args()
    try:
        destination,=admit_destinations([args.out],reference.HERE,protected_roots=[reference.HERE])
        result=capture()
        with destination.open("x",encoding="utf-8") as stream:
            json.dump(result,stream,separators=(",",":"),allow_nan=False)
            stream.write("\n")
    except (ValueError,OSError) as error:
        parser.error(str(error))
    print("generated128 patterns x3 physical rates for one/two-site mixture integration")


if __name__=="__main__":main()
