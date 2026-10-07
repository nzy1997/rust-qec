# Compiled annotation-index original-application counts, Apple M4

Measured clean source: `24b7fdb5d7c076de9372e5074e7fd1175b47797d`, retained by
`benchmark-source/counts-annotations-2026-10-08`. This follow-up keeps scalar
rejection and indexes final annotation operation positions in the immutable plan.
Preflight observable lookup and packed count reduction skip unrelated operations.
The index is charged against the existing static plan reservation.

All four supported original circuits × 1/64/1024 attempted shots × Strict/Fused
have complete same-run comparisons with Clifft0.11.0, scheduled Clifft, and
source-bound SymFT0.1.1. Retain all 658 closed events, 24 finite-valid cells,
480 timing processes, 3,360 warm observations and 14 unsupported capability
receipts. Inputs, annotations, import/source/binary identities and random stream
witnesses remain independently verified. Compressed events decode losslessly;
closure hashes cover the original decoded bytes.

In this run, Fused MSCd3 leads the fastest peer by 1.594×/1.297× at 1/64 shots.
Surface d7 leads by 1.208× at 64 shots. At 1024 shots, MSCd3/MSCd5/surface d7/d9
remain 1.141×/3.546×/2.509×/2.758× slower than the fastest same-run peer. These
results identify further batch/compiler work; they do not establish universal
SOTA. No speedup is computed by dividing timings from different campaigns.

Separate paired exploratory ablations against the preceding scalar-rejection
implementation found one-shot gains around 8% (MSCd3), 24–27% (MSCd5), and
29–36% (surface d7/d9). A bounded surface bulk repeat found gains of about17–18%
at64 shots and6–8% at1024 shots, with all paired ranges above1.0. Those ablations
are scoped source-digest-bound local diagnostics, separate from this formal
peer comparison; MSC bulk improvements are not claimed.

The host is shared and unpinned. Ratios are process-median comparisons and paired
ranges are not confidence intervals. Finite accepted/error witnesses do not
certify rare conditional logical-error accuracy. This counts publication has no
lifecycle measurements. See [analysis.md](analysis.md), [comparisons.csv](comparisons.csv)
and [warm.csv](warm.csv) for all policies, backends, phases and acceptance rates.
