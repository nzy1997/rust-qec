"""Independent exact transcripts for the actual experiment proposal samplers.

Each positive oracle branch is replayed at both ends of every categorical
interval. No production choose, closing_law, mass, classify or draw helper is
used to construct a transcript or its expected result. Integer interval widths
prove the branch probabilities; exhaustion detects skipped/extra random draws.
"""
from fractions import Fraction
from itertools import product
from math import lcm, prod


def insist(condition, message):
    if not condition:
        raise ValueError("draw protocol: " + message)


class Transcript:
    def __init__(self, requests):
        self.requests = list(requests)
        self.index = 0

    def randrange(self, bound):
        insist(self.index < len(self.requests), "unexpected random request")
        expected, value = self.requests[self.index]
        self.index += 1
        insist(bound == expected, "random range differs from independent law")
        return value

    def exhausted(self):
        insist(self.index == len(self.requests), "sampler ignored a random request")


def interval(probabilities, selected):
    """Closed integer endpoints for an independently specified CDF outcome."""
    probabilities = tuple(map(Fraction, probabilities))
    insist(all(p >= 0 for p in probabilities) and sum(probabilities) == 1,
           "oracle law is not normalized")
    insist(probabilities[selected] > 0, "oracle selected zero mass")
    if probabilities[selected] == 1:
        return []
    denominator = lcm(*(p.denominator for p in probabilities))
    start = sum(probabilities[:selected])*denominator
    end = start+probabilities[selected]*denominator-1
    insist(start.denominator == end.denominator == 1, "nonintegral oracle interval")
    return [(denominator, int(start), int(end))]


def transcripts(intervals):
    # Replay all lower endpoints, then change each nontrivial coordinate to its
    # upper endpoint while keeping the other decisions fixed on the same path.
    base = [(bound, low) for bound, low, _ in intervals]
    yield Transcript(base)
    for index, (bound, low, high) in enumerate(intervals):
        if low != high:
            yield Transcript(base[:index]+[(bound, high)]+base[index+1:])


def cultivation_draws(model):
    count, branch_mass = 0, Fraction(0)
    for occupied in product((False, True), repeat=len(model.signatures)):
        active = [i for i, yes in enumerate(occupied) if yes]
        split = max(0, len(active)-model.closing_sites)
        head, tail = active[:split], active[split:]
        occurrence = model.q**len(active)*(1-model.q)**(len(occupied)-len(active))
        requests = sum((interval((1-model.q, model.q), int(yes)) for yes in occupied), [])
        for prefix in product(*(range(len(model.weights[i])) for i in head)):
            prefix_probability = prod(model.weights[i][c] for i, c in zip(head, prefix))
            before_tail = list(requests)
            parity = model.offset
            for i, c in zip(head, prefix):
                before_tail += interval(model.weights[i], c)
                parity ^= model.signatures[i][c]
            endings = list(product(*(range(len(model.weights[i])) for i in tail)))
            base = [prod((model.weights[i][c] for i, c in zip(tail, row)), start=Fraction(1)) for row in endings]
            passes = []
            for row in endings:
                value = parity
                for i, c in zip(tail, row):
                    value ^= model.signatures[i][c]
                passes.append(value == 0)
            normalizer = sum(p for p, yes in zip(base, passes) if yes)
            mixture = [(Fraction(1), False)] if not normalizer else [
                (1-model.alpha, False), (model.alpha, True)]
            for mixture_probability, conditioned in mixture:
                if not mixture_probability:
                    continue
                decisions = before_tail + (interval((1-model.alpha, model.alpha), int(conditioned))
                                          if normalizer else [])
                law = [p/normalizer if yes else Fraction(0) for p, yes in zip(base, passes)] if conditioned else base
                for selected, channel_probability in enumerate(law):
                    if not channel_probability:
                        continue
                    expected = [0]*len(occupied)
                    for i, c in zip(head+tail, prefix+endings[selected]):
                        expected[i] = c+1
                    expected = tuple(expected)
                    # Sum both mixture branches independently for reported Q.
                    selected_base = base[selected]
                    q = occurrence*prefix_probability*selected_base
                    if normalizer:
                        q *= 1-model.alpha+(model.alpha/normalizer if passes[selected] else 0)
                    p = prod((1-model.p) if c == 0 else model.p*model.weights[i][c-1]
                             for i, c in enumerate(expected))
                    branch_mass += occurrence*prefix_probability*mixture_probability*channel_probability
                    for source in transcripts(decisions+interval(law, selected)):
                        draw = model.draw(source)
                        insist(draw.pattern == expected, "cultivation emitted the wrong physical pattern")
                        insist(draw.proposal_probability == q and draw.physical_probability == p,
                               "cultivation returned wrong P/Q")
                        source.exhausted()
                        count += 1
    insist(branch_mass == 1, "cultivation draw branches lost probability")
    return count


def residual_draws(model):
    # Reconstruct the complete ordered buckets from raw X/Z recovery tables.
    # This also detects duplicate/missing words which aggregate counts conceal.
    buckets = [[[] for _ in range(model.width+1)] for _ in range(4)]
    spread = [sum(((v >> j) & 1) << (2*j) for j in range(model.width))
              for v in range(1 << model.width)]
    for x, even in enumerate(spread):
        for z, odd in enumerate(spread):
            buckets[model.x[x]+2*model.z[z]][(x | z).bit_count()].append(even | (odd << 1))
    insist(tuple(tuple(tuple(b) for b in row) for row in buckets) == model.buckets,
           "residual bucket order or support changed")
    atom = [(model.p0/3)**w*(1-model.p0)**(model.width-w) for w in range(model.width+1)]
    class_mass = [sum(len(b)*p for b, p in zip(row, atom)) for row in buckets]
    pi = [1-model.nonidentity]+[model.nonidentity*p/(1-class_mass[0]) for p in class_mass[1:]]
    count = 0
    for label, row in enumerate(buckets):
        weights = [len(b)*p/class_mass[label] for b, p in zip(row, atom)]
        for weight, bucket in enumerate(row):
            if not bucket:
                continue
            decisions = interval(pi, label)+interval(weights, weight)
            # Exhaust every physical index on small admitted models. The full
            # nine-wire law checks first, interior and last indices per bucket,
            # plus every class/weight CDF endpoint and all 262144 bucket words.
            indices = range(len(bucket)) if model.width <= 4 else sorted({0,len(bucket)//2,len(bucket)-1})
            for index in indices:
                word = bucket[index]
                requests = decisions+([(len(bucket), index, index)] if len(bucket) > 1 else [])
                for source in transcripts(requests):
                    actual, mass = model.draw(source)
                    insist(actual == word, "residual emitted the wrong physical pattern")
                    insist(mass == pi[label]*atom[weight]/class_mass[label], "residual returned wrong Q")
                    source.exhausted()
                    count += 1
    return count
