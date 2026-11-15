"""Exact experiment-side proposal laws for the v1 paper campaigns.

The historical sampling drivers were deposited by hash, not as source. These
laws reconstruct the stated distributions; no historical RNG identity is
claimed. They produce only physical fault patterns and their probabilities.
Controllers remain owned by the simulator and receive reported records there.
"""

from dataclasses import dataclass
from fractions import Fraction
from itertools import product
from math import lcm, prod


def require(condition, message):
    if not condition:
        raise ValueError(message)


def rational(value):
    require(isinstance(value, (str, int, Fraction)) and not isinstance(value, bool),
            "use an exact rational string or integer")
    return Fraction(value)


def choose(probabilities, random):
    """Exact categorical draw from caller-owned randrange; no floating CDF."""
    require(probabilities and all(p >= 0 for p in probabilities)
            and sum(probabilities) == 1, "categorical probabilities must sum to one")
    if 1 in probabilities:
        return probabilities.index(1)
    denominator = lcm(*(p.denominator for p in probabilities))
    value = random.randrange(denominator)
    require(type(value) is int and 0 <= value < denominator, "out-of-range source response")
    for index, probability in enumerate(probabilities):
        value -= probability.numerator * (denominator // probability.denominator)
        if value < 0:
            return index
    raise ValueError("categorical source did not select a channel")


def xor(values):
    value = 0
    for item in values:
        value ^= item
    return value


class CultivationProposal:
    """Raise occurrence to p+3/N, then condition the last occupied channels.

    Occurrences are chosen first. For the last one/two occupied locations,
    alpha=19/20 mixes the ordinary channel law with that same law conditioned
    on the S-proxy detector parity. An impossible conditional branch uses the
    ordinary law. The empty occurrence set is unchanged. Thus every physically
    supported pattern keeps positive proposal probability, even when the proxy
    is imperfect. Detector signatures are experiment input, not true outcomes
    or a decoder interface. Signature generation is a separate responsibility.
    """

    def __init__(self, signatures, physical_rate, *, channel_weights=None,
                 closing_sites=1, mixture="19/20", extra_faults=3, offset=0):
        self.signatures = tuple(tuple(row) for row in signatures)
        require(4 <= len(self.signatures) <= 65536, "admit 4..65536 noise locations")
        require(all(row and len(row) <= 15 and all(type(v) is int and v >= 0
                    for v in row) for row in self.signatures), "invalid detector signatures")
        require(type(closing_sites) is int and closing_sites in (1, 2),
                "one or two closing sites are supported")
        require(type(offset) is int and offset >= 0, "invalid detector offset")
        self.p = rational(physical_rate)
        self.extra = rational(extra_faults)
        self.alpha = rational(mixture)
        self.q = self.p + self.extra / len(self.signatures)
        require(0 < self.p < 1 and 0 < self.q < 1 and 0 <= self.alpha < 1
                and self.extra >= 0, "invalid physical/proposal/mixing probabilities")
        self.closing_sites, self.offset = closing_sites, offset
        if channel_weights is None:
            self.weights = tuple(tuple(Fraction(1, len(row)) for _ in row)
                                 for row in self.signatures)
        else:
            self.weights = tuple(tuple(map(rational, row)) for row in channel_weights)
        require(len(self.weights) == len(self.signatures) and
                all(len(w) == len(s) and all(v > 0 for v in w) and sum(w) == 1
                    for w, s in zip(self.weights, self.signatures)), "invalid conditional channel law")

    def validate_pattern(self, pattern):
        require(len(pattern) == len(self.signatures) and
                all(type(v) is int and 0 <= v <= len(s)
                    for v, s in zip(pattern, self.signatures)), "invalid physical pattern")

    def physical_mass(self, pattern, rate=None):
        self.validate_pattern(pattern)
        p = self.p if rate is None else rational(rate)
        require(0 <= p <= 1, "invalid reweighting rate")
        return prod((1-p) if channel == 0 else p*self.weights[j][channel-1]
                    for j, channel in enumerate(pattern))

    def closing_law(self, pattern):
        active = [j for j, c in enumerate(pattern) if c]
        tail = active[-self.closing_sites:]
        prefix = active[:-len(tail)] if tail else active
        parity = self.offset ^ xor(self.signatures[j][pattern[j]-1] for j in prefix)
        candidates = list(product(*(range(1, len(self.weights[j])+1) for j in tail)))
        base = [prod(self.weights[j][c-1] for j, c in zip(tail, channels))
                for channels in candidates]
        passing = [xor(self.signatures[j][c-1] for j, c in zip(tail, channels)) == parity
                   for channels in candidates]
        normalizer = sum(p for p, passes in zip(base, passing) if passes)
        return tail, candidates, base, passing, normalizer

    def mass(self, pattern):
        self.validate_pattern(pattern)
        base = prod((1-self.q) if c == 0 else self.q*self.weights[j][c-1]
                    for j, c in enumerate(pattern))
        _, _, _, _, normalizer = self.closing_law(pattern)
        if not normalizer:
            return base
        passes = xor(self.signatures[j][c-1] for j, c in enumerate(pattern) if c) == self.offset
        return base * (1-self.alpha + (self.alpha/normalizer if passes else 0))

    def draw(self, random):
        # Ordinary channels are generated only for the prefix, so there is no
        # unreported resampling of a discarded final channel.
        active = [j for j in range(len(self.signatures))
                  if choose([1-self.q, self.q], random)]
        tail = active[-self.closing_sites:]
        prefix = active[:-len(tail)] if tail else active
        pattern = [0]*len(self.signatures)
        for j in prefix:
            pattern[j] = 1+choose(self.weights[j], random)
        for j in tail:
            pattern[j] = 1  # Membership only; these values are not prefix inputs.
        indices, candidates, base, passing, normalizer = self.closing_law(pattern)
        conditioned = normalizer and choose([1-self.alpha, self.alpha], random)
        probabilities = [p/normalizer if passes else Fraction(0)
                         for p, passes in zip(base, passing)] if conditioned else base
        selected = candidates[choose(probabilities, random)]
        for j, c in zip(indices, selected):
            pattern[j] = c
        pattern = tuple(pattern)
        return FaultDraw(pattern, self.mass(pattern), self.physical_mass(pattern))


@dataclass(frozen=True)
class FaultDraw:
    pattern: tuple
    proposal_probability: Fraction
    physical_probability: Fraction

    @property
    def likelihood(self):
        return self.physical_probability / self.proposal_probability


class ResidualClassProposal:
    """Exact conditional physical patterns, with nonidentity class mass 1/5.

    The finite classifier contains the recovery rule applied to each physical
    X/Z syndrome. It is used offline to partition physical Pauli patterns and
    is never passed to an experiment's runtime controller. Pattern words use
    little-endian two-bit IXZY order. The proposal is fixed at p0=1/100, while
    each returned physical pattern can be reweighted to multiple target rates.
    """

    def __init__(self, x_residual, z_residual, width=9,
                 proposal_rate="1/100", nonidentity="1/5"):
        require(type(width) is int and 1 <= width <= 9, "finite classifier width must be 1..9")
        self.width = width
        self.x = tuple(x_residual)
        self.z = tuple(z_residual)
        require(len(self.x) == len(self.z) == 1 << width and
                all(type(v) is int and v in (0, 1) for v in self.x+self.z),
                "invalid residual-class lookup")
        self.p0, self.nonidentity = rational(proposal_rate), rational(nonidentity)
        require(0 < self.p0 < 1 and 0 < self.nonidentity < 1, "full-support proposal required")
        buckets = [[[] for _ in range(width+1)] for _ in range(4)]
        for x in range(1 << width):
            for z in range(1 << width):
                word = sum((((x >> j) & 1) | (((z >> j) & 1) << 1)) << (2*j)
                           for j in range(width))
                buckets[self.x[x]+2*self.z[z]][(x | z).bit_count()].append(word)
        self.buckets = tuple(tuple(tuple(b) for b in row) for row in buckets)
        self.counts = tuple(tuple(map(len, row)) for row in self.buckets)
        self.class_masses = self.classes_at(self.p0)
        require(all(p > 0 for p in self.class_masses), "all four residual classes must be represented")
        self.class_proposal = (1-self.nonidentity, *(
            self.nonidentity*p/(1-self.class_masses[0]) for p in self.class_masses[1:]))
        self.by_weight = tuple(tuple(self.atom_mass(w, self.p0) for w in range(width+1))
                               for _ in range(4))

    def atom_mass(self, weight, rate):
        return (rate/3)**weight * (1-rate)**(self.width-weight)

    def classes_at(self, rate):
        rate = rational(rate)
        require(0 <= rate <= 1, "invalid physical rate")
        return tuple(sum(n*self.atom_mass(w, rate) for w, n in enumerate(row))
                     for row in self.counts)

    def classify(self, word):
        require(type(word) is int and 0 <= word < 1 << (2*self.width), "invalid Pauli word")
        x = sum(((word >> (2*j)) & 1) << j for j in range(self.width))
        z = sum(((word >> (2*j+1)) & 1) << j for j in range(self.width))
        return self.x[x]+2*self.z[z], (x | z).bit_count()

    def mass(self, word):
        label, weight = self.classify(word)
        return self.class_proposal[label]*self.by_weight[label][weight]/self.class_masses[label]

    def draw(self, random):
        label = choose(self.class_proposal, random)
        weights = [n*self.by_weight[label][w]/self.class_masses[label]
                   for w, n in enumerate(self.counts[label])]
        weight = choose(weights, random)
        bucket = self.buckets[label][weight]
        index = 0 if len(bucket) == 1 else random.randrange(len(bucket))
        require(type(index) is int and 0 <= index < len(bucket), "invalid conditional pattern index")
        word = bucket[index]
        return word, self.mass(word)

    def reweight(self, words, rate):
        rate = rational(rate)
        require(0 <= rate <= 1 and isinstance(words, (tuple, list)) and
                1 <= len(words) <= 1024, "invalid reweighting batch")
        return prod(self.atom_mass(self.classify(word)[1], rate)/self.mass(word) for word in words)


def enumerate_cultivation_branches(model, max_patterns=4096):
    """Independent branch enumeration: collect probabilities of emitted patterns.

    This does not call mass/closing_law/draw. Mixture branch masses are added
    when both branches emit the same pattern, which matters for likelihoods.
    """
    size = prod(len(s)+1 for s in model.signatures)
    require(size <= max_patterns, "finite proposal oracle exceeds max_patterns")
    result = {}
    for occurrences in product((False, True), repeat=len(model.signatures)):
        locations = [i for i, active in enumerate(occurrences) if active]
        cut = max(0, len(locations)-model.closing_sites)
        head, last = locations[:cut], locations[cut:]
        occurrence_mass = model.q**len(locations)*(1-model.q)**(len(occurrences)-len(locations))
        for prefix in product(*(range(1, len(model.weights[i])+1) for i in head)):
            prefix_mass = prod(model.weights[i][c-1] for i, c in zip(head, prefix))
            target = model.offset ^ xor(model.signatures[i][c-1] for i, c in zip(head, prefix))
            endings = list(product(*(range(1, len(model.weights[i])+1) for i in last)))
            valid = [channels for channels in endings if
                     xor(model.signatures[i][c-1] for i, c in zip(last, channels)) == target]
            total_valid = sum(prod(model.weights[i][c-1] for i, c in zip(last, channels))
                              for channels in valid)
            # Enumerate each branch separately, rather than evaluating the
            # production density's mixture multiplier.
            branches = [(Fraction(1), endings)] if not total_valid else [
                (1-model.alpha, endings), (model.alpha, valid)]
            for branch_probability, channels in branches:
                normalizer = sum(prod(model.weights[i][c-1] for i, c in zip(last, row))
                                 for row in channels)
                for ending in channels:
                    pattern = [0]*len(occurrences)
                    for i, c in zip(head+last, prefix+ending):
                        pattern[i] = c
                    channel_probability = prod(model.weights[i][c-1] for i, c in zip(last, ending))
                    value = occurrence_mass*prefix_mass*branch_probability*channel_probability/normalizer
                    pattern = tuple(pattern)
                    result[pattern] = result.get(pattern, Fraction(0))+value
    require(len(result) == size, "proposal oracle lost a physical pattern")
    return result


def check_cultivation_model(model):
    from random import Random
    from paper_proposal_draws import cultivation_draws
    distribution = enumerate_cultivation_branches(model)
    require(sum(distribution.values()) == 1, "proposal branches are not normalized")
    require(all(q > 0 for q in distribution.values()), "proposal lost physical support")
    rates = (Fraction(1, 1000), Fraction(3, 1000), Fraction(1, 100))
    for pattern, q in distribution.items():
        require(model.mass(pattern) == q, "proposal mixture/conditional normalizer mismatch")
    # Check joint contributions against an independently enumerated physical
    # law, including a detector event and an event correlated with that event.
    for rate in rates:
        direct = [Fraction(0), Fraction(0), Fraction(0)]
        weighted = [Fraction(0), Fraction(0), Fraction(0)]
        for pattern, q in distribution.items():
            p = prod((1-rate) if c == 0 else rate*model.weights[j][c-1]
                     for j, c in enumerate(pattern))
            passing = xor(model.signatures[j][c-1] for j, c in enumerate(pattern) if c) == model.offset
            scores = (1, int(passing), int(passing and sum(pattern) % 3 == 1))
            physical = model.physical_mass(pattern, rate)
            for j, score in enumerate(scores):
                direct[j] += p*score
                weighted[j] += q*(physical/model.mass(pattern))*score
        require(direct[0] == 1 and direct == weighted, "physical/proposal reweighting mismatch")
    random = Random(20261116)
    for _ in range(32):
        draw = model.draw(random)
        require(draw.proposal_probability == distribution[draw.pattern]
                and draw.physical_probability == model.physical_mass(draw.pattern),
                "sampled pattern probability mismatch")
    protocol_draws = cultivation_draws(model)
    return dict(patterns=len(distribution), rates=len(rates), draws=32, protocol_draws=protocol_draws)


def check_residual_model(model, expected_counts):
    from random import Random
    from paper_proposal_draws import residual_draws
    require(model.counts == tuple(map(tuple, expected_counts)), "residual-class partition changed")
    require(sum(map(sum, model.counts)) == 4**model.width, "residual class support is incomplete")
    require(sum(model.class_masses) == sum(model.class_proposal) == 1, "residual class normalization changed")
    probabilities = [[model.class_proposal[c]*model.atom_mass(w, model.p0)/model.class_masses[c]
                      for w in range(model.width+1)] for c in range(4)]
    # All 4^9 patterns were classified, not just the 40 aggregate coefficients.
    for label, row in enumerate(model.buckets):
        for weight, bucket in enumerate(row):
            require(len(bucket) == model.counts[label][weight], "a supported physical pattern was removed")
            require(all(model.classify(word) == (label, weight) and
                        model.mass(word) == probabilities[label][weight] for word in bucket),
                    "conditional bucket contains a wrong physical pattern")
            require(probabilities[label][weight] > 0, "a supported physical pattern has zero proposal mass")
    require(sum(n*probabilities[c][w] for c, row in enumerate(model.counts)
                for w, n in enumerate(row)) == 1, "conditional physical-pattern normalizer changed")
    for rate in (Fraction(1,1000), Fraction(3,1000), Fraction(1,100)):
        # Exact per-class weighted masses prove every event determined by a
        # residual class, before any finite-shot precision campaign is run.
        for label in range(4):
            weighted = sum(n*probabilities[label][w]*(model.atom_mass(w,rate)/probabilities[label][w])
                           for w,n in enumerate(model.counts[label]))
            require(weighted == model.classes_at(rate)[label], "multi-rate class reweighting mismatch")
    random = Random(20261116)
    for _ in range(8):
        # Reuse one batch of physical patterns at all three rates; no resampling.
        draws = [model.draw(random) for _ in range(15)]
        words, proposal = zip(*draws)
        total_proposal = prod(proposal)
        for rate in (Fraction(1,1000), Fraction(3,1000), Fraction(1,100)):
            physical = prod(model.atom_mass(model.classify(word)[1],rate) for word in words)
            require(model.reweight(words,rate) == physical/total_proposal,
                    "joint patch likelihood does not reweight the same batch")
    protocol_draws = residual_draws(model)
    return dict(patterns=4**model.width, classes=4, rates=3, shared_batch_draws=8,
                patches_per_draw=15, protocol_draws=protocol_draws)
