# rmagna

Independent Rust foundations for global-network near-Clifford simulation.
The current package provides exact scalars, packed binary linear algebra,
quadratic phase sums, phase-preserving stabilizer contraction and third-level
phase reduction, Clifford evolution and unitary near-Clifford circuit
compilation. Instruments, noise and sampling are subsequent deliverables.

`ExactScalar` represents
`(a + b*omega + c*omega^2 + d*omega^3) * 2^power`, where
`omega = exp(i*pi/4)`. Coefficients are arbitrary-width signed integers, and
binary powers are signed 64-bit integers. Common powers of two are removed,
so equivalent values compare and hash equally. Arithmetic uses no floats.

```rust
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

let arithmetic = ScalarArithmetic::default();
let t = arithmetic.omega(1, 0)?;
let h = arithmetic.omega(0, -1)?; // 1 / sqrt(2)
assert_eq!(arithmetic.norm_squared(&t)?, ExactScalar::one());
assert_eq!(arithmetic.multiply(&h, &h)?, arithmetic.omega(0, -2)?);
# Ok::<(), rmagna::scalar::ScalarError>(())
```

`ScalarArithmetic::new(bits)` configures the maximum bit length of canonical
coefficients and aligned addition operands (default 1,048,576 bits). This
protects against a tiny operand with a huge exponent gap allocating an enormous
shift. It is an arithmetic resource limit, not a process-memory cap: products
and real-sign comparisons use larger integer temporaries. Failed operations
return a `ScalarError`; they never truncate coefficients or approximate the
result. A cancellation may fail the alignment budget even if its final result
would fit. Exponent limits apply to the normalized result.

`real_sign` compares an exactly real value with zero by integer arithmetic,
including values too small or cancellation-sensitive for floating-point
comparison. It rejects complex values. `conjugate` preserves global phases;
`norm_squared` is exact. Rational noise weights will be represented separately
because a probability such as 1/3 is outside this scalar ring.

Run `cargo test -p rmagna`. Tests cover mixed complex coefficients, phase and
scale identities, destructive interference, tiny probability differences,
large Pell convergents, canonical equality, resource failures and exponent
boundaries. This is independently authored Rust code; the pinned Magna
implementation used for later comparisons is recorded in
`benchmarks/magna_reproduction/reference.json` at the workspace root.

The committed scalar witness file contains numerical outputs from that pinned
reference, including its source-manifest and native-extension hashes. Regenerate
it with the reference environment, leaving the existing file untouched:

```sh
/absolute/path/to/reference-env/bin/python -I \
  benchmarks/magna_reproduction/scalar_vectors.py \
  --upstream /absolute/path/to/magna --out /absolute/path/to/new-witnesses.json
```

The generator refuses to overwrite an existing destination and verifies the
pinned source before and after evaluating the fixed 300-bit operand set.

## Binary and stabilizer algebra

`gf2::BitVector` has an explicit width and packed 64-bit words. It rejects
out-of-range indices, inconsistent widths and nonzero padding. A
`LinearFactorization` preserves row provenance, solves repeated right-hand
sides, detects contradictory dependent equations and supplies a nullspace
basis. There is no fixed physical-qubit or path-bit cap in these types.

`quadratic::QuadraticForm` represents phases modulo four with strictly upper
triangular cross terms. Affine restriction allows dependent basis vectors.
Its exact Gauss sum eliminates variables instead of enumerating assignments.

`stabilizer::AffineKet` represents an unnormalized affine quadratic sum. Its
constructor accepts dependent path columns; `contract` eliminates hidden paths
and produces independent support columns, returning `None` for a zero ket.
Projection preserves the original scalar. `pauli(x, z)` applies `X^x Z^z`,
including its global phase, and tensor ordering puts the first ket on low
boundary indices. `prepare` retains the matrix factorization and kernel for
repeated exact amplitude queries. Overlap and norm include all interference.

```rust
use rmagna::gf2::BitVector;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::ScalarArithmetic;
use rmagna::stabilizer::AffineKet;

let arithmetic = ScalarArithmetic::default();
let plus = AffineKet::new(
    BitVector::zeros(1)?,
    vec![BitVector::from_indices(1, [0])?],
    QuadraticForm::new(0, vec![0], vec![BitVector::zeros(1)?])?,
    arithmetic.omega(0, -1)?,
)?;
let branch = plus.project_z(
    &[BitVector::from_indices(1, [0])?],
    &BitVector::zeros(1)?,
    &arithmetic,
)?.unwrap();
assert_eq!(branch.norm_squared(&arithmetic)?, arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The additional tests exhaust small binary systems and all quadratic forms up
to three variables, compare random contractions against literal complex sums,
and compare 24 prescribed contractions with the pinned C++ reference. Boundary
tests cross word boundaries through 4097 bits. Regenerate the numerical ket
witnesses with `benchmarks/magna_reproduction/stabilizer_vectors.py`, using the
same arguments and reference environment as the scalar generator above.

## Clifford radical and quotient

`phase::ParityPhase` stores a phase modulo eight as a weighted sum of binary
parities. Duplicate axes combine modulo eight. Affine restriction preserves
the constant phase and accepts dependent coordinate maps. T, controlled-S and
CCZ phases can all be expressed by these terms.

`signature(u, v, w)` is the phase's third discrete derivative divided by four.
It includes repeated directions; a single T has a nonzero signature even in
one dimension. `quotient` computes the radical that annihilates every signature
slice. Its full basis consists of complementary quotient representatives,
followed by radical directions. Zero signature is equivalent to a Clifford
phase, which `clifford` separates into a global eighth-root phase and a
quadratic form modulo four. Odd parity terms can cancel into a Clifford phase,
so checking individual coefficients is insufficient.

`PreparedPhaseSum::new(reference, max_terms, arithmetic)` caches the exact
computational expansion on the quotient. Evaluation accepts any updated phase
with the same signature, including affine shifts and Clifford corrections,
and an additional quadratic boundary in the original coordinates. It retains
the reference coordinates and resource coefficients across queries. Gaussian
elimination sums the radical coordinates without physical-state enumeration.

```rust
use rmagna::gf2::BitVector;
use rmagna::phase::{ParityPhase, PreparedPhaseSum};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

let arithmetic = ScalarArithmetic::default();
let t = ParityPhase::new(1, 0, [(BitVector::from_indices(1, [0])?, 1)])?;
let prepared = PreparedPhaseSum::new(&t, 2, &arithmetic)?;
let boundary = QuadraticForm::new(0, vec![0], vec![BitVector::zeros(1)?])?;
assert_eq!(prepared.quotient().rank(), 1);
assert_eq!(prepared.evaluate(&t, &boundary, &ExactScalar::one(), &arithmetic)?,
    arithmetic.add(&ExactScalar::one(), &arithmetic.omega(1, 0)?)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ClosedPhaseSum` additionally sums the Clifford radical into its quadratic
boundary before imposing the term budget. The resulting affine constraints
can further reduce the non-Clifford support; it repeats this polynomial-time
step until no radical remains. `reductions` records input dimensions, signature
ranks and surviving support dimensions. No quotient assignments are enumerated
during reduction. An inconsistent boundary returns exact zero with no terms.
This cold closed-network plan has no update API; use `PreparedPhaseSum` for a
fixed quotient with covariant phase updates.

The term limit applies to `2^quotient_rank` and is checked before enumeration.
This version provides the computational expansion only; choosing a smaller
T-state catalog decomposition is a subsequent deliverable. The limit is not a
process-memory cap, and these algebra types do not perform circuit compilation
or choose a global commuting layer.

Tests exhaust every signature up to three coordinates using explicit discrete
differences, compare random prepared updates to literal sums, and retain wide
low-rank examples. The 28 native numerical witnesses additionally compare the
full complex scalar and quotient dimensions before/after absorption through 129 coordinates. Regenerate
them with `benchmarks/magna_reproduction/phase_vectors.py` using the reference
environment and arguments shown above. The generator checks its reconstructed
closed amplitude against the native norm and verifies source and binary hashes
before writing a new destination.

## Clifford circuit boundaries

`clifford::CliffordCircuit` validates a fixed width and a sequence of H, S
(zero through three turns), X, Z, CX, CZ and SWAP gates. Two-qubit gates require
distinct endpoints. `computational` prepares a basis ket; `evolve` accepts an
arbitrary unnormalized affine ket and retains its scalar and global phase.
Each H introduces one path variable and contracts it immediately, keeping the
number of independent paths bounded by the boundary width. `adjoint` reverses
the circuit and inverts every S phase.

```rust
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

let arithmetic = ScalarArithmetic::default();
let input = CliffordCircuit::computational(BitVector::zeros(2)?)?;
let circuit = CliffordCircuit::new(2, [
    CliffordGate::H(0), CliffordGate::Cx { control: 0, target: 1 },
])?;
let bell = circuit.evolve(&input, &arithmetic)?.unwrap();
assert_eq!(bell.norm_squared(&arithmetic)?, ExactScalar::one());
assert_eq!(bell.prepare()?.amplitude(&BitVector::zeros(2)?, &arithmetic)?,
    arithmetic.omega(0, -1)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

For tensor-network construction, `AffineKet::conjugate` conjugates all complex
phases. `sum_boundary(keep, arithmetic)` sums every discarded computational
label against the unnormalized bra `<0| + <1|`, and retains the requested
indices in their given order. Repeated indices copy a label to several legs.
This linear operation is distinct from tracing a density operator; a trace
requires boundary equality between a ket and its conjugate first.

These primitives build exact Clifford boundaries. The unitary compiler below
adds near-Clifford circuit lowering; reset, records and noise are subsequent
deliverables. Tests
compare random six-qubit circuits to explicit complex matrices, retain the
global phase in `(SH)^3 = omega I`, invert the circuits exactly, and verify
boundary sums, repeated indices and wide Bell projections.

## Unitary near-Clifford networks

`unitary::UnitaryCircuit` adds T (zero through seven turns), controlled-S
(zero through three turns), and CCZ to the Clifford gate set. `compile` takes
a computational-basis input and builds a Clifford ket with auxiliary parity
legs. Clifford cases of T and controlled-S are lowered directly. Other diagonal
phases are copied to fresh zero ancillas by CNOTs and evaluated at the final
boundary, preserving the order of intervening noncommuting gates.

`CompiledUnitary::amplitude` computes a complex output amplitude;
`probability_z` computes the exact mass of simultaneous Z-parity conditions;
`overlap(target)` computes `<self|target>`. Probabilities trace output indices
by enforcing ket/conjugate equality before summing them. Overlap closes all
physical outputs against the target. The phase is restricted to the resulting
affine support before contracting its Clifford radical and preparing the final
quotient. Resource constraints apply after this exact reduction.

```rust
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::{UnitaryCircuit, UnitaryGate};

let arithmetic = ScalarArithmetic::default();
let circuit = UnitaryCircuit::new(1, [
    UnitaryGate::Clifford(CliffordGate::H(0)),
    UnitaryGate::T { qubit: 0, turns: 1 },
    UnitaryGate::Clifford(CliffordGate::H(0)),
])?.compile(&BitVector::zeros(1)?, &arithmetic)?;
let amplitude = circuit.amplitude(&BitVector::zeros(1)?, 2, &arithmetic)?;
let probability = circuit.probability_z(
    &[BitVector::from_indices(1, [0])?], &BitVector::zeros(1)?, 4, &arithmetic,
)?;
assert_eq!(probability.value, arithmetic.norm_squared(&amplitude.value)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Each query takes an explicit computational-term budget and returns its scalar,
quotient dimension and term count. Inconsistent or zero boundaries return zero
with no terms. This initial unitary compiler reuses the Clifford ket, but
constructs a phase resource for each query; global commuting-layer selection,
cross-fault resource reuse, instruments, controllers and noise remain later
deliverables. It does not claim the paper's reduced representation ranks or
sampling performance. Tests compare noncommuting circuits, exact events and
target overlaps against literal matrices, including an eight-qubit example
with quotient rank greater than one.

## Prescribed Pauli insertions

`pauli::PauliWord` represents `omega^phase X^x Z^z`, with Z acting first.
A Hermitian single-qubit Y has both masks set and phase two. Ordered products
and Clifford conjugation retain the eighth-root phase, including the minus
sign from conjugating Y by H and the phase from conjugating XX by CZ.

`unitary::PauliInsertion { before_gate, word }` inserts a physical Pauli at
an original gate boundary. Position zero precedes the circuit, and position
equal to the gate count follows it. Different positions are processed in time
order; insertions sharing a position act in their supplied order.
`CompiledUnitary::with_insertions` transports these words through the lowered
Clifford program and updates its boundary. It shares the immutable original
Clifford ket and program rather than evolving the circuit again. Each call
replaces the complete assignment relative to that original, including when
called on an already perturbed view. An empty assignment restores it.

```rust
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{PauliInsertion, UnitaryCircuit};

let arithmetic = ScalarArithmetic::default();
let reference = UnitaryCircuit::new(1, [])?
    .compile(&BitVector::zeros(1)?, &arithmetic)?;
let changed = reference.with_insertions(&[PauliInsertion {
    before_gate: 0,
    word: PauliWord::new(BitVector::from_indices(1, [0])?, BitVector::zeros(1)?, 0)?,
}], &arithmetic)?;
assert_eq!(changed.amplitude(&BitVector::from_indices(1, [0])?, 1, &arithmetic)?.value,
    ExactScalar::one());
# Ok::<(), Box<dyn std::error::Error>>(())
```

This is a deterministic insertion interface; it does not sample noise or
reuse phase-resource preparation across queries yet. Tests exhaust all
three-qubit Pauli masks under the Clifford gates, check ordered products and
complex matrix identities, and compare prescribed multi-fault unitary networks
to direct gate-by-gate insertion.

## Exact rational probabilities

`probability::Probability` stores a normalized arbitrary-precision rational
between zero and one. Decimal, scientific and integer-fraction input is parsed
exactly, without conversion through a float. Signed fraction denominators are
normalized; underscores between digits are accepted. Zero denominators and
out-of-range results are errors. Complement,
checked sums and rational scaling retain the exact value.

```rust
use rmagna::probability::{OccurrenceThreshold, Probability};

let decimal: Probability = "0.125".parse()?;
assert_eq!(decimal, "1/8".parse()?);
assert_eq!(decimal.complement().to_string(), "7/8");
let tiny: Probability = "1e-30".parse()?;
assert_ne!(tiny, Probability::zero());
assert_eq!(tiny.threshold_64(), OccurrenceThreshold::Never);
# Ok::<(), rmagna::probability::ProbabilityError>(())
```

`ProbabilityParser` limits token bytes and absolute decimal powers (both
100,000 by default). Exceeding either limit fails without rounding; these are
parsing resource limits, not process-memory caps or precision settings.
`from_ratio` and `from_rational` accept already allocated integers and validate
the denominator and interval independently of those text limits.

`threshold_64` explicitly implements the upstream file interface's occurrence
rule: floor the probability times `2^64`, with exact zero and one endpoints.
`OccurrenceThreshold::accepts` compares a supplied uniform 64-bit word with
that cutoff. A positive value below `2^-64` rounds to `Never`, while the stored
exact rational remains nonzero. This method does not implement the separate
exact-rational sampling path or generate random words. Tests cover rational
identities, canonical equality and hashing, malformed and unnormalized raw
ratios, resource limits, tiny probabilities and threshold endpoints.
