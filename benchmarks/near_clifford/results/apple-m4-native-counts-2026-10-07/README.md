# Native postselected counts on Apple M4

The [verified comparison](analysis.md) measures the public compiled Rust native
counts API against native Clifft and SymFT counts on the same shared, unpinned
Apple M4. Measured source is `bb11a636b0c855ed7deb547d5dbdd2baff96b987`.
All 24 selected cells have finite correctness witnesses and complete timing;
14 capability failures retain seven unsupported original circuits under both
arithmetic policies. Original source text and licenses are retained unchanged.

The contract counts attempted shots, survivors of all-zero raw detectors, and
raw observable-0 parity-one survivors. Rust reduces packed annotation masks
without constructing batch output records. It preserves random draws and does
not reject shots early. Independent structured witnesses require exact counts
and a 16-word RNG continuation; their collection is outside native timing.

At 1024 shots, Fused Rust takes 228.062µs on MSCd3, 22033.333µs on MSCd5,
469.604µs on surface d7 and 933.010µs on surface d9. It is respectively
1.147×, 3.570×, 2.855× and 2.844× slower than the fastest peer in this run.
MSCd3 at 1 and 64 shots is 1.442× and 1.301× faster. Surface-d7/Fused/64 has
a paired range crossing parity, so its median does not establish a consistent
winner across process rounds. These results support continued optimization;
they do not establish universal SOTA performance.

Five independent rotated/reversed process rounds each retain seven warm
observations of at least 50ms. Paired ranges are process-median ranges, not
confidence intervals. Strict and Fused remain separate. Native peer build flags
are not fully attested, and sparse logical-error counts do not certify rare
conditional-error accuracy. Earlier structured-counts publications came from
different process runs; dividing their latencies by these values is not a
controlled improvement measurement.

Next measure early rejection and streaming annotation reduction on these four
originals, and schedule/state reduction on MSCd5 and the failing rank profiles.
The existing benchmark matrix supplies concrete directions before further broad
expansion. See [warm.csv](warm.csv) for cold phases, acceptance and throughput,
and [capability.csv](capability.csv) for retained unsupported inputs.
