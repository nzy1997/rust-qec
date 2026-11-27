# RP2 d5-T rare-event benchmark investigation

The v1 author archive recorded **3,692 positive accepted-error contributions**
in 67,108,864 draws at physical rate $p=0.001$. The sealed Rust v1 capture recorded
**4** in the same number of draws. The runs used separate fault streams and the
counts describe how often each proposal produced a positive contribution.
The strongest subsequent result is an exact three-fault contribution of
$6.992425499562523\times 10^{-11}$ on the pinned public circuit. It accounts for
about 80% of the gap between the previous stratified estimate and the author
archive. The exact search and matched-fault replays use the complete T target.

This investigation followed [issue #1345](https://github.com/nzy1997/rust-qec/issues/1345)
and [PR #1346](https://github.com/nzy1997/rust-qec/pull/1346).
The [author v1 deposit](https://doi.org/10.5281/zenodo.23386778) contains the
author summary at
`raw/cultivation/rp2-d5-T/failure-p0.001/summary.json`. The corresponding
Rust row is the `1/1000` entry under `precision.rp2-d5-T` in
`raw/cultivation/report.json` of the
[sealed v1 evidence release](https://github.com/nzy1997/rust-qec/releases/tag/magna-paper-2610.22315-v1).
The author joint estimate is $(1.481\pm0.195)\times10^{-10}$.
The sealed Rust joint estimate is $(4.27\pm3.80)\times10^{-11}$.
Uncertainties in this report are one standard error unless otherwise specified.

## What the investigation established

- **Fault scoring.** For 833 identical fault configurations, the pinned public
  Magna build and rmagna returned exactly the same unweighted full-T acceptance
  and error masses. This included all four Rust v1 positive configurations.
  Replaying two original Rust blocks with detector screening and score-cache
  reuse disabled reproduced both full histograms exactly (2,097,152 draws).
  These checks cover the public scorer and the Rust paths exercised here.
- **Closing rule.** Rust v1 draws the occupied locations first. It changes only
  the Pauli alternatives at the last two occupied locations in circuit order.
  An experimental proposal instead randomly designates the prefix and lets the
  two closing faults use any two distinct remaining locations. In a paired
  comparison over 589,824 draws per arm this increased positive hits from 0 to
  27 using the same 102 constraints. The movable proposal produced 45.8
  positive hits per million draws, compared with 55.0 in the author archive.
  The paired baseline here uses the same randomly designated prefix and fixes
  its two sampled locations.
- **Weights and frequency.** The 27-hit run initially estimated the joint
  error at $4.25\times10^{-11}$. Summing the proposal probability over all
  possible prefix designations on the same configurations raised it to
  $4.69\times10^{-11}$. Relative to the initial 27-hit run, the archive's
  positive frequency was 1.20 times higher and its mean contribution per hit
  was 2.90 times higher. The three largest hits contributed 59% of the initial
  weighted sum. Matching the hit frequency alone gives weak evidence about
  weighted precision.
- **Historical scope.** The v1 deposit names
  `scripts/cultivation_two_estimator.py` at worktree `d9aba287c` and records the
  script and per-draw stream digests. The public repository and the v1/v2
  deposits supplied counts and metadata. The original driver, per-draw faults,
  exact detector IDs, and rule for replacing a closing location remain
  unverified. The historical kernel `853fa2369817e1be65a710b42ff80606e219e464`
  is distinct from the public commit used for the matched replay. The
  experimental movable proposal therefore has its own identity.
  See [Figure 4](https://arxiv.org/html/2610.22315v1#S3.SS2) and the
  [v2 deposit](https://doi.org/10.5281/zenodo.23907966).

Rust's retained detector IDs are zero-based in circuit declaration order.
The exact search uses
`0–17, 19–25, 27, 31–33, 39–79, 81–92, 94–95, 97, 99, 101, 103, 105, 119–131`
with zero signed offset. They are deterministic constraints on full-T
acceptance. The full-T scorer evaluates the remaining task conditions and the
target. The archive reports a retained count of 102.

## Three-fault exact result

The pinned input has 2,405 independent original noise categories with equal
Pauli probabilities within each category. At three occupied categories, the
search traversed 4,652,759 closing channel-class triples and scored
**1,609,576** distinct full-T score classes. A class uses the XOR-composable
score signature of its three faults. Distinct site IDs are required for every
representative. The 102 detector parities certify zero acceptance for excluded
faults. Each admissible score class is evaluated on the complete T task.

Three classes had positive error. Expanding their members gave **1,840**
distinct physical configurations. Independent replay in public Magna agreed
exactly on acceptance and error for all 1,840.

| Positive configurations | Unweighted error per configuration | Joint contribution at $p=0.001$ |
| ---: | ---: | ---: |
| 500 | $1/32$ | $1.8504239335704064\times10^{-11}$ |
| 500 | $1/32$ | $1.8504239335704064\times10^{-11}$ |
| 840 | $1/16$ | $3.291577632421711\times10^{-11}$ |

For a configuration whose three locations have $m_1,m_2,m_3$ alternatives,
its physical probability is $p^3(1-p)^{2402}/(m_1m_2m_3)$. The sum of these
probabilities times the exact error masses is
$J_3=\frac{10439}{13500}p^3(1-p)^{2402}$. There is no sampling uncertainty
in this term. Under the previous fixed-three-fault proposal, the exact list
gives a positive hit probability of about $3.3284\times10^{-6}$. In 65,536
draws that is 0.22 expected hits and about an 80% probability of zero hits.

Combining the measured contributions at fault counts 3 through 8 gives
$(1.306\pm0.158)\times10^{-10}$. The difference from the all-fault-count
author archive is $1.75\times10^{-11}$, or 0.69 combined standard errors
using the reported uncertainties. Counts 0 through 2 were checked exactly
and gave zero. Counts 9 through 19 gave zero hits in the small screening
runs. Their upper bounds still apply. The simultaneous 95% upper bound for
counts 13 and above is $1.16\times10^{-11}$ under the tested circuit and noise
model. The current result combines measured layers and leaves the other
layers unresolved within their bounds. The 0.69 figure is a descriptive
quadrature comparison. Counts 4 and 5 shared seed IDs in their first batches,
so the quoted combined standard error does not account for their covariance.

### Experimental provenance and stratum estimates

The follow-up pilots are separate from the two archived captures above. The
matched-fault check used the sealed Rust seed `2026101522` and five
1,048,576-draw ranges beginning at `0`, `26214400`, `35651584`, `56623104`,
and `62914560`. It retained the first 128 draws of each range, all positive
draws, and detector-passing draws whose global index was a multiple of 1021.
For each retained pattern the replay resolves the original site and Pauli
alternative, supplies those actions to both engines and compares the exact
full-T acceptance and error coefficients. The two shortcut-off replays
covered the ranges beginning at `0` and `35651584`.

The fixed-site and movable-site arms shared the occupied sets and randomly
designated Pauli prefixes. Their independent tail draws used the 19/20
mixture. Both arms sampled the same nominal occurrence law at
$q=p+3/2405$. For each arm, the first round used seeds `2026112701` through
`2026112703` at 65,536 draws per seed. The confirmation used
`2026112711` through `2026112713` at 131,072 per seed. The movable rule
conditions the baseline channel probabilities over all passing pairs of
distinct sites outside the prefix. It falls back to the ordinary branch
when no passing pair exists.

The table uses the marginal weights over the possible prefix designations.
The exact three-fault term above replaces that stratum's zero-hit pilot.

| Fault count | Conditional draws | Positive hits | Joint contribution and SE |
| ---: | ---: | ---: | ---: |
| 4 | 393,216 | 24 | $(1.90\pm0.68)	imes10^{-11}$ |
| 5 | 1,441,792 | 239 | $(4.10\pm1.42)	imes10^{-11}$ |
| 6 | 98,304 | 9 | $(7.49\pm3.68)	imes10^{-13}$ |
| 7 | 98,304 | 1 | $(5.69\pm5.69)	imes10^{-16}$ |
| 8 | 98,304 | 1 | $(1.15\pm1.15)	imes10^{-17}$ |

Counts 4 and 5 used seeds `2026112741` through `2026112743` at 131,072
draws each. Count 5 also used seeds `2026112751` through `2026112754` at
262,144 each. The screening runs used seeds `2026112761 + 100*k` and
`2026112762 + 100*k` for count $k$. Counts 6–8 used 49,152 draws per
seed, counts 9–12 used 16,384 and counts 13–19 used 8,192.

For a zero-hit layer with $n$ draws, the one-sided hit-probability
bound at risk $\gamma$ is $1-\gamma^{1/n}$. The joint-error bound multiplies
this by the physical count mass and the maximum conditional weight for a
closing pattern, using error mass at most 1. For $m=2405-k+2$ eligible sites,
a uniform upper bound on the baseline closing probability $Z$ is
$\min(1,\sum_s u_s^2/[m(m-1)])$, where $u_s$ sums the baseline conditional
Pauli masses across all sites for detector signature $s$. This follows by
bounding the signature autocorrelation and dropping the distinct-site
restriction in its numerator. For closure mixture $\alpha=19/20$, the
conditional weight for a closing pattern is at most
$Z_{max}/(\alpha+(1-\alpha)Z_{max})$ under every prefix and after
marginalizing the prefix. The joint bound for counts 13 and above uses
risk $0.05/7$ for each of counts 13–19 and adds the full physical
binomial mass above count 19.

### Reproduce the exact check

Use the public Magna checkout at `56847478c840395735146ef1e1ccdc3d8aedc10f`.
Run this from the rmagna workspace root. Use any positive worker count suited
to the host. The example checks the SHA-256 of the circuit, task and target
before scoring. It writes the full positive fault list, exact coefficient and
class counts as JSON. Cargo and the public input checkout suffice for the
enumeration.

```sh
upstream=$(mktemp -d)
git clone https://github.com/itpyi/magna.git "$upstream"
git -C "$upstream" checkout --detach 56847478c840395735146ef1e1ccdc3d8aedc10f
cargo run --locked --release -p rmagna --example paper_cultivation_exact3 -- \
  "$upstream/examples/rp2-d5" 7 > rp2-d5-exact3.json
python3 - <<'PY'
import json
from pathlib import Path
r = json.loads(Path("rp2-d5-exact3.json").read_text())
assert r["status"] == "complete"
assert r["eligible_class_triples"] == 4652759
assert r["full_t_score_classes"] == 1609576
assert r["positive_score_classes"] == 3
assert r["positive_configurations"] == 1840
assert r["joint_error_multiplier"] == ["10439/13500", "0", "0", "0"]
assert r["sampling_se"] == 0
print(r["joint_error"])
PY
```

The output `positive_faults` gives zero-based internal site and channel
alternative indices and exact unweighted masses in the cyclotomic basis.
An independent scorer can inject these same Pauli actions into the pinned
public circuit and compare the acceptance and error coefficients before
weighting. No historical RNG stream is required for this check.

## Protocol for future rmagna rare-event benchmarks

1. **Establish matched scoring first.** Freeze the circuit, signed
   postselection, noise categories, complete target and evaluator builds.
   Replay identical physical assignments on both engines, including positive,
   zero-error and rejected controls. Compare unweighted joint acceptance
   and joint error before comparing proposals. Use the S proxy to construct
   proposal constraints and score the complete T task.
2. **Separate exact and sampled strata.** Enumerate tractable low-fault
   strata first, with score equivalence or another checked shortcut.
   Report each exact $J_k$ and use it as a validation reference for the
   sampler. For other counts use disjoint seeds across strata, draw batches
   conditional on the occupied count, and restore its probability. For the RP2 pilot,
   the original occurrence law uses $N=2405$ and $q=p+3/N$, so the
   natural proposal mass of count $k$ is $Q(K=k)$. If the reported
   weight includes $P(K=k)/Q(K=k)$, multiply the stratum mean by
   $Q(K=k)$. With a purely conditional weight, multiply by $P(K=k)$.
3. **Name and normalize the proposal.** Record the detector IDs, eligible
   closing locations, channel mass and mixture probability. Keep the
   ordinary branch for full support. For a randomly designated prefix,
   use $P(x)/Q(x)$ with $Q(x)$ summed over all ways to generate the same
   fault configuration. For a paired proposal comparison, share the
   prefix draws where practical and evaluate each arm with its own
   likelihood. Retain per-hit unweighted error, $P/Q$ and their product.
4. **Measure precision and throughput separately.** Report the weighted
   joint estimate and standard error per stratum, independent seed
   counts, positive hits, concentration of the largest contributions, and
   error-contribution effective sample size. Report zero-hit strata as
   unresolved with a stated bound. For conditional logical error use
   $J/A$, where $A$ is estimated in a separate physical-noise acceptance
   batch, and propagate uncertainty from both estimates. Report preparation,
   warmup, attempted-draw throughput and elapsed time to the stated
   precision as separate quantities.
5. **Scale by uncertainty.** Define the precision target and fixed pilot
   budgets before sampling. Add draws where they can materially reduce
   uncertainty or where a remaining upper bound matters to the stated
   comparison. Keep timing runs, pilot outcomes, and confirmatory precision
   runs distinguishable in the results. For the current archive comparison,
   defer fault counts 9 and 10. The residual difference is already smaller
   than one combined standard error, and their current zero-hit screens
   do not certify those strata individually.
