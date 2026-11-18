"""Evaluate the pinned TQD author approximation on supplied physical faults."""

import argparse
from fractions import Fraction
from hashlib import sha256
from pathlib import Path
import json
import sys

import reference
from remaining_contracts import read_json, require
from tqd_fault_map import COLORS


NESTED_DRAWS = 32


def target_z_infidelity():
    """Derive the Z-error cost from the four-logical-qubit branch state."""
    support = [word for word in range(16) if (word & 3 == 3) == (word & 12 == 12)]
    require(len(support) == 10, 'invalid logical + branch norm')
    overlaps = {Fraction(sum(1 if (word & z).bit_count() % 2 == 0 else -1 for word in support), len(support))**2
                for z in range(1,16)}
    require(len(overlaps) == 1, 'nonuniform logical Z target overlap')
    return 1-overlaps.pop()


class AuthorApproximation:
    def __init__(self, upstream, size, depth):
        import numpy as np
        from pymatching import Matching
        root = upstream / 'examples' / f'tqd{size}'
        sys.path.insert(0, str(root))
        try:
            from decoder import author
            source = Path(author.__file__).resolve()
            require(source == (root/'decoder/author.py').resolve(), 'loaded a different TQD author package')
            self.hashes = author.verify()
            self.author = author.load()
        finally:
            sys.path.remove(str(root))
        self.np = np
        self.size, self.depth = size, depth
        geometry, lattice = self.author.geometry, self.author.lattice
        require(geometry.get_time_depth(size) == depth and self.author.twisted.COLORS == list(COLORS),
                'author TQD memory dimensions or color order changed')
        self.full = lattice.build_incidence_matrix(size,depth)
        self.matching = Matching(self.full)
        self.endpoints = lattice.build_edge_endpoints(self.full)
        self.lookup = lattice.build_neighbor_edge_lookup(size,depth)
        self.prefixes = [lattice.build_incidence_matrix(size,t,open_end_node=t<depth) for t in range(1,depth+1)]
        self.prefix_matchings = list(map(Matching,self.prefixes))
        self.masked = set(map(int,geometry.last_time_step_measurement_edges(size,depth)))
        require(len(self.masked) == size**2, 'unexpected author final-time measurement mask')

    def evaluate(self, site_map, selected, seed, index):
        np = self.np
        size, depth = self.size, self.depth
        require(type(seed) is int and 0 <= seed < 1 << 64 and type(index) is int and 0 <= index < 1 << 32,
                'invalid author nested draw address')
        require(isinstance(selected,list) and all(type(i) is int and 0 <= i < len(site_map) for i in selected)
                and selected == sorted(set(selected)), 'invalid TQD supplied fault list')
        noise = {(kind,color):np.zeros(self.full.shape[1],dtype=np.uint8) for kind in 'XZ' for color in COLORS}
        for site in selected:
            entry = site_map[site]
            require(entry['edge'] not in self.masked, 'fault in an ideal author measurement')
            noise[entry['kind'],entry['color']][entry['edge']] ^= 1
        decoder = self.author.decoder
        loops = {}
        flux = {}
        for color in COLORS:
            x = noise['X',color]
            estimate = decoder.jit_decode_full(size,depth,x,self.full,self.matching,self.prefixes,self.prefix_matchings)
            loops[color] = x ^ (estimate & 1)
            flux[color] = bool(decoder.is_logical_error(loops[color],size,depth,error_type='x',edge_endpoints=self.endpoints))
        if any(flux.values()):
            return dict(index=index, flux_success=False, flux_failures=flux,
                        nested_attempted=0, nested_logical_errors=None, matched_infidelity=None)
        # The author's original function takes randomness from NumPy's legacy
        # global RNG. Keep its original sampler and restore the caller's state.
        old_state = np.random.get_state()
        seed_words = [seed & 0xffffffff, seed >> 32, index, size, depth]
        np.random.seed(seed_words)
        logical_errors = 0
        try:
            for _ in range(NESTED_DRAWS):
                twisted = self.author.twisted.generate_twisted_z_errors(loops,size,depth,self.lookup,is_full=False)
                failed = False
                for color in COLORS:
                    z = noise['Z',color] ^ twisted[color]
                    prediction = self.matching.decode(self.full @ z % 2)
                    failed |= bool(decoder.is_logical_error(z ^ prediction,size,depth,error_type='z'))
                logical_errors += int(failed)
        finally:
            np.random.set_state(old_state)
        return dict(index=index, flux_success=True, flux_failures=flux,
                    nested_attempted=NESTED_DRAWS, nested_logical_errors=logical_errors,
                    matched_infidelity=str(target_z_infidelity()*Fraction(logical_errors,NESTED_DRAWS)))


def fault_identity(selected):
    return sha256(json.dumps(selected,separators=(',', ':')).encode()).hexdigest()


def evaluate_payload(upstream, fault_map, payload):
    size = payload['configuration']['size']
    seed = payload['configuration']['seed']
    require(size in (3,4,5), 'unsupported TQD author scale')
    info = fault_map['families'][f'tqd{size}']
    author = AuthorApproximation(upstream,size,info['depth'])
    selected = [dict(author.evaluate(info['entries'],row['faults'],seed,row['index']),
                     fault_sha256=fault_identity(row['faults'])) for row in payload['rows']]
    return dict(schema='rmagna.paper-tqd-author-approximation.v1', size=size, depth=info['depth'],
                noise_seed=seed, nested_draws=NESTED_DRAWS, fault_map_sha256=payload['fault_map_sha256'],
                author_source_sha256=author.hashes,
                logical_z_infidelity=str(target_z_infidelity()), rows=selected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--fault-map', type=Path, required=True)
    parser.add_argument('--physical-result', type=Path, required=True)
    args = parser.parse_args()
    physical = read_json(args.physical_result)
    require(physical['fault_map_sha256'] == reference.digest(args.fault_map), 'TQD author and physical fault maps differ')
    result = evaluate_payload(args.upstream.resolve(),read_json(args.fault_map),physical)
    print(json.dumps(result,sort_keys=True))


if __name__ == '__main__':
    main()
