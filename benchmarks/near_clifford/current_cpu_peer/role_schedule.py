"""Prespecified independent role permutations for each case and paired round.

The public seed is sampled once while drafting, frozen before native data,
and included in the reviewed manifest. Hash ordering avoids dependence on a
particular Python PRNG implementation. This does not itself prove timing
exchangeability, independence of measurements, or statistical significance.
"""
import hashlib
import json
import re

from peer_evidence import require


def role_order(manifest, name, shots, policy, pair):
    require(manifest['role_order_contract'] == 'sha256-per-case-round-v1', 'role order contract differs')
    seed = manifest['role_order_seed']
    require(isinstance(seed, str) and re.fullmatch('[0-9a-f]{64}', seed) is not None, 'role order seed differs')
    require(type(pair) is int and 0 <= pair < manifest['pairs'], 'role order round differs')
    require(name in manifest['names'] and type(shots) is int and shots in manifest['shots']
            and policy in manifest['policies'], 'role order case differs')
    roles = manifest['roles']
    require(len(roles) == 6 and len(set(roles)) == 6 and all(isinstance(r, str) for r in roles), 'role set differs')
    ranks = [(hashlib.sha256(json.dumps([seed, name, shots, policy, pair, role],
                       separators=(',', ':'), ensure_ascii=True).encode('ascii')).digest(), role)
             for role in roles]
    require(len({digest for digest, _ in ranks}) == len(roles), 'ambiguous role order digest')
    return [role for _, role in sorted(ranks)]
