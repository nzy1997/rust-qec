"""Retain independently enumerated B02 CDF transcripts for the Rust paper sampler."""
import argparse
from fractions import Fraction
import hashlib
from math import prod
from pathlib import Path
import json

import paper_proposal_draws as protocol
import paper_proposals as laws

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate():
    models = []
    small = [[1], [2, 3, 1], [1, 4, 5], [4]]
    definitions = [('single', small, 1, 0), ('double', small, 2, 0),
                   ('offset', small, 2, 1),
                   ('all225', [[1], [2], list(range(1, 16)), list(range(1, 16))], 2, 0)]
    for name, signatures, closing, offset in definitions:
        model = laws.CultivationProposal(signatures, '1/32', closing_sites=closing, offset=offset)
        distribution = laws.enumerate_cultivation_branches(model)
        if sum(distribution.values()) != 1 or any(value <= 0 for value in distribution.values()):
            raise ValueError('independent closing branches lost normalization or support')
        records = []
        original = model.draw

        def record(random):
            requests = [list(row) for row in random.requests]
            draw = original(random)
            physical = prod((1-model.p) if choice == 0 else model.p/len(signatures[site])
                            for site, choice in enumerate(draw.pattern))
            proposal = distribution[draw.pattern]
            if draw.physical_probability != physical or draw.proposal_probability != proposal:
                raise ValueError('B02 sampler differs from its independent branch enumeration')
            # The large model exhausts all 225 tail pairs at three prefix
            # parities/weights. Other models retain every physical pattern.
            if name != 'all225' or (draw.pattern[2] and draw.pattern[3]
                                    and draw.pattern[:2] in [(0, 0), (1, 0), (1, 1)]):
                records.append([requests, list(draw.pattern), str(physical/proposal)])
            return draw

        model.draw = record
        total = protocol.cultivation_draws(model)
        models.append(dict(name=name, signatures=signatures, physical_rate='1/32',
                           closing_sites=closing, offset=offset,
                           complete_patterns=len(distribution), validated_transcripts=total,
                           columns=['integer_range_requests', 'original_channel_pattern', 'exact_P_over_Q'],
                           transcripts=records))
    return dict(schema='rmagna.cultivation-range-oracles.v1',
                generator_sha256=digest(Path(__file__)),
                law_sha256=digest(HERE/'paper_proposals.py'),
                protocol_sha256=digest(HERE/'paper_proposal_draws.py'), models=models)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    value = generate()
    text = json.dumps(value, separators=(',', ':'))+'\n'
    if len(text.encode()) > 1 << 20:
        raise ValueError(f'bounded range fixture exceeds one MiB: {len(text.encode())}')
    args.out.write_text(text)
    print(json.dumps(dict(bytes=len(text.encode()), models=[
        dict(name=model['name'], retained=len(model['transcripts']),
             validated=model['validated_transcripts'], patterns=model['complete_patterns'])
        for model in value['models']])))


if __name__ == '__main__':
    main()
