"""Check matched routes and the independent finite-cutoff oracle."""

from copy import deepcopy
import unittest

from paper_evidence_ablations import (
    CUTOFFS, HALF, HISTORY_DRAWS, JOINT_ERROR, LANE_SEED, LIVE, MASK, MODES, ZERO,
    determining_prefix, splitmix_words, verify_histories, verify_occurrence,
)


class PaperAblationTests(unittest.TestCase):
    def test_finite_occurrence_keeps_exact_threshold_ties_and_boundary_work(self):
        self.assertEqual(determining_prefix(0,0),0)
        self.assertEqual(determining_prefix(1 << 63,1 << 63),1)
        self.assertEqual(determining_prefix(2,1),63)
        self.assertEqual(determining_prefix(MASK-1,MASK),64)

        stream = splitmix_words(LANE_SEED)
        groups = []
        # These maxima come from explicit cutoff and cutoff-minus-one ties.
        for index,(cutoff,planes) in enumerate(zip(CUTOFFS,(0,64,1,64,63,64),strict=True)):
            for live in LIVE:
                words = [next(stream) for _ in range(64)]
                words[:4] = [0,(cutoff-1)&MASK,cutoff,MASK]
                selected = sum(1 << i for i,word in enumerate(words) if (live >> i) & 1 and word < cutoff)
                groups.append(dict(cutoff_index=index, cutoff=f"{cutoff:016x}", live=f"{live:016x}",
                                   words=[f"{word:016x}" for word in words], scalar=f"{selected:016x}",
                                   packed=f"{selected:016x}", scalar_entropy_words=live.bit_count() if cutoff else 0,
                                   packed_entropy_planes=planes))
        observed = verify_occurrence(groups)
        self.assertEqual(observed["active_lanes"],576)
        self.assertEqual(observed["scalar_entropy_words"],480)
        self.assertEqual(observed["packed_entropy_planes"],512)
        groups[2]["packed"] = "0000000000000000"
        with self.assertRaisesRegex(ValueError, "independent oracle"):
            verify_occurrence(groups)

    def test_replaying_matched_histories_requires_switching_one_declared_factor(self):
        routes = []
        for name,(causal,compiler,sampler) in MODES.items():
            rows = []
            for index in range(HISTORY_DRAWS):
                bit = bool(index % 2)
                rows.append(dict(index=index, tape="c000000000000001" if bit else "0000000000000000",
                                 reported=bit, callback=[bit], history=HALF,
                                 proposal=HALF if name == "affine" else None,
                                 accepted=ZERO if bit else HALF, error=ZERO if bit else JOINT_ERROR,
                                 network_compilations=2 if name == "cold" else 0,
                                 history_queries=1, history_words=1, noise_words=0))
            routes.append(dict(name=name, causal_strategy=causal, compiler=compiler, history_sampler=sampler,
                               reference_compilations=1, clifford_gates=12,path_variables=3,
                               phase_ancillas=int(name == "parity_copies"),
                               shared_phase_coordinates=int(name != "parity_copies"),rows=rows))
        observed = verify_histories(routes)
        self.assertEqual(observed["cold"]["network_compilations"],128)
        self.assertEqual(observed["baseline"]["network_compilations"],0)
        changed = deepcopy(routes)
        changed[1]["compiler"] = "ParityCopies"
        with self.assertRaisesRegex(ValueError, "declared factor"):
            verify_histories(changed)
        routes[3]["rows"][0]["proposal"] = None
        with self.assertRaisesRegex(ValueError, "exact branch masses changed"):
            verify_histories(routes)


if __name__ == "__main__":
    unittest.main()
