# Experimental near-Clifford backend contract

Issue #739 introduces an opt-in state simulator for Clifford circuits containing
`T` and `T_DAG`. This document fixes the mathematical and interface conventions
before the backend is added. The current `Executor` and compiled sampler remain
the pure-Clifford implementations.

## Gates and circuit scope

`T = diag(1, exp(i*pi/4))` and `T_DAG = T†`. The circuit spelling is `T` and
`T_DAG`, each with no arguments and one or more qubit targets. Both are unitary
gates. A future experimental circuit entry point must reject unsupported names,
argument counts, and target shapes with an explicit error; it must not route a
non-Clifford gate through a pure-Clifford sampler.

`RZ` is the existing Stim Z-basis **reset**, with no angle argument. It must
never be interpreted as an arbitrary-angle rotation. A negative test for an
unsupported rotation should use a distinct name such as `ROT_Z(0.4487989505128276)`
(an approximation to pi/7) and assert the unsupported-gate error, not a parser
error on the text `pi/7`.

The delivery sequence is: Clifford and T/T_DAG evolution; terminal X/Y/Z
measurement and sampling; the experimental circuit API; mid-circuit measurement
and reset; then Pauli noise, classical feedback, and detector/observable records.
At each stage the entry point must reject operations that have not been
implemented, without changing the existing Clifford execution path. Arbitrary
angle rotations, ZX compilation, GPU execution, and throughput claims are
outside issue #739.

The experimental circuit entry point is `NearCliffordExecutor`. It takes
`StimInstr` values or parses circuit text, validates the supported subset before
running, and returns one measurement bit vector per shot. For example:

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let circuit = NearCliffordExecutor::compile_text("H 0\nT 0\nMX 0")?;
    let mut rng = StdRng::seed_from_u64(42);
    let shots = circuit.sample(100, &mut rng)?;
    assert_eq!(shots.len(), 100);
    Ok(())
}
```

The circuit API accepts the basic single-qubit Clifford generators,
`CX`/`CNOT`/`ZCX`, `CZ`/`ZCZ`, `SWAP`, `T`, `T_DAG`, `REPEAT`, mid-circuit
`M`/`MZ`/`MX`/`MY`, and `R`/`RZ`/`RX`/`RY` and
`MR`/`MRZ`/`MRX`/`MRY` resets. Additional Clifford aliases,
feedback, noise, and annotations are delivered in later PRs.
The default active-rank limit is 16; `compile_with_limit` lets callers choose a
different limit. Reaching the limit returns an error. The circuit entry point
also rejects more than 4096 physical qubits before allocating a tableau.

## Shared frame, active coordinates, and phase

The exact pure state is a **coherent** sum of orthogonal basis states from one
shared Clifford frame. Symbolic coordinates select active basis vectors and a
dense complex vector holds their amplitudes. Probabilities are computed from
the sum of amplitudes **after** a measurement projector is applied; summing
branch probabilities beforehand would incorrectly discard interference.

The frame includes an ordered choice of basis-vector phases. Stabilizer signs
alone do not specify relative phases between the vectors. A canonicalized frame
must transform coefficients when its basis convention changes. The implementation
may discard one common global phase, but must preserve all relative phases.

For the known-answer circuit `H 0; CX 0 1; T 0; H 0; T 0` starting at `|00>`,
let

```text
|phi> = (|00> + |01> + |10> - |11>)/2
g1 = Z0 X1,  g2 = X0 Z1
|a,b>_g = Z1^a Z0^b |phi>,  a,b in {0,1}
```

Here `a=1` flips the sign of `g1` and `b=1` flips the sign of `g2`.
Set `c=cos(pi/8)` and `s=sin(pi/8)`. In the coordinate order
`(a,b)=(0,0),(0,1),(1,0),(1,1)`, the coefficient vector is
`[c², -ics, -isc, -s²]` after removing the common phase `exp(i*pi/4)`.
With standard `T`, the physical amplitudes in computational order
`|00>,|01>,|10>,|11>` are
`[1/2, exp(i*pi/4)/2, exp(i*pi/4)/2, -i/2]`.

This example has uniform computational-basis measurement probabilities. It
therefore cannot by itself distinguish a coherent state from a classical
mixture of the four sectors. The additional interference check is
`<X0 X1> = 1/2`, or equivalently X-basis even-parity probability `3/4`.
An incoherent mixture of the same sectors instead gives `<X0 X1> = 0`.

## Resource boundary and reference oracle

For `r` independent active coordinates, the dense coefficient array has
`2^r` entries. Before expansion, the implementation must check both the shift
and configured memory limit and return a resource-limit error if exceeded.
Repeated T gates acting on the same active coordinate must not add a new
dimension solely because the T count increased. The first implementation need
not find a globally minimal active basis.

After measurement, an axis with one zero-amplitude half is removed. Its fixed
virtual bit is retained in the `origin` coordinate, so later gates see the
correct Pauli sign. Other measurements can leave a non-minimal active basis;
the configured rank limit still applies and reports an error if exceeded.

The small-qubit test oracle is an independent dense state-vector simulator. It
does not share the production frame or branch update code. A tableau without
an amplitude phase for each weighted term is insufficient as a coherent oracle:
the phase omitted by a single stabilizer state becomes a relative phase when
terms are added.
