# Bounded signed Clifford encoder

`qec_code::signed_encoder::synthesize_signed_encoder` constructs a literal
unitary from a `SignedStabilizerGroup` and caller-supplied ordered logical
X/Z representatives. It revalidates that frame against the supplied group.
Input wires `0..k` carry arbitrary logical inputs; wires `k..n` start in
physically prepared zero states. Preparation is not part of the returned
unitary and is not a free resource.

The output uses chronological `EncoderGate::{H,S,Sdg,Cx}` instructions with
explicit wire indices. There are no implicit permutations, measurements,
resets, T gates, decoders, error correction, or postselection. The result
includes the exact signed image of every physical input X/Z generator and
one membership witness for each ancillary input Z image in the original
stabilizer generator list. `inverse_gates` returns the literal adjoint.

The first k X/Z images equal the supplied operators exactly, including
their signs; they are not merely equivalent modulo the stabilizer. The
remaining X images provide an explicit completion of the Clifford frame.
The construction fixes the encoding isometry up to one overall phase,
preserving arbitrary logical superpositions and external references.

## Construction and correctness

Construct a decoder while conjugating every supplied operator through its
literal gates. Previously processed logical pairs are always exactly the
positive single-wire X/Z pair on their assigned wire. All unprocessed
operators commute with these pairs, so they have identity on those wires.
No subsequent gate addresses that prefix.

For logical pair j, first reduce its Z representative to positive Z_j.
Move a nonidentity pivot onto j using three actual CX gates if needed.
On each remaining nonidentity factor, H maps X to Z and Sdg followed by H
maps Y to Z. Once the operator is a Z string, CX(q,j) clears each other
Z_q factor. A negative remaining Z_j is corrected by literal H,S,S,H on j.
Every gate is applied to all tracked operators with its exact scalar phase.

The companion X representative anticommutes with Z_j, so its j factor is
X or Y. On other wires, H maps Z to X and Sdg maps Y to X. CX(j,q) then
clears each other X factor. Sdg on j removes a remaining Y factor; S,S
corrects a negative X_j. These operations preserve the now-canonical Z_j.
Rotations on other wires need not be undone: all other representatives
are tracked through them, and subsequent pairs are reduced in that frame.

After all logical pairs are reduced, every stabilizer has support only on
the n-k ancillary wires. Select a nonidentity residual row, reduce it to
the next positive Z using the same procedure, and multiply it into later
rows that contain that Z factor. These are products of commuting signed
stabilizers, not changes to any logical representative. They preserve the
signed generated group, including redundant original rows. Group rank
ensures an independent residual pivot exists until all n-k wires are used.
All remaining rows are consistent identities, since the validated group
does not contain -I.

Consequently the decoder sends the entire supplied logical frame to the
standard k-wire frame and the stabilizer group to the positive ancillary
Z group. Reversing its gates, with S and Sdg interchanged, is an encoder
with the stated full logical action. Final replay checks exact logical
images and obtains witnesses from the original signed group for every
ancillary Z image. Those self-checks are evidence for callers to recompute;
they do not substitute for independent gate-semantic validation.

Phases are `i^phase X^x Z^z`, as in `PhasedPauli`. In particular Y=iXZ:
a positive Hermitian word containing Y need not have scalar phase +1 in
this ordered representation. The algorithm never discards this phase.

## Limits and independent tests

Hard limits are n<=16, at most 256 original generators (including redundant
ones), and at most 8192 emitted gates. `EncoderLimits::max_gates` can tighten
the last bound, including zero. Frame counts and row widths are checked before copying.
The implementation uses bounded dense binary support arrays and no dense
Hilbert-space matrix. It is a small-code construction, not a claim of
large-code scaling, minimum gate count, distance, fault tolerance, noisy
source quality, or resource advantage.

`cargo test -p qec-code --test signed_encoder` uses an independent literal
complex state-vector oracle limited to n<=4. It checks all exported X/Z
images on every physical computational-basis vector, the literal inverse,
and every logical input column of the encoding subspace. It covers signed
mixed-Y codes, all signed single-wire logical orientations, three arbitrary
logical ports, redundant generators, explicit three-CX swaps, zero-logical
and zero-wire cases, invalid frames, and resource limits. Deliberate S/Sdg,
CX-direction, and missing-gate mutations fail the independent image check.
These fixtures do not execute any application-specific nine-wire encoder.

`signed_encoder_allocations` separately checks that every borrowed logical-row
position rejects an oversized width before any in-call allocation. Its probe
uses constant thread-local state so other test-harness threads cannot affect it.

An application should independently validate the exported literal against
its separate signed code/frame declaration, then include all real input
preparations, encoder gates, faults, source checks, and consumer operations
in its own resource/noise model. A unitary algebra certificate alone does
not qualify that physical protocol.
