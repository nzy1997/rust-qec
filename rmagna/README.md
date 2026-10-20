# rmagna

Independent Rust foundations for global-network near-Clifford simulation.
The current package provides exact scalars, packed binary linear algebra,
quadratic phase sums, phase-preserving stabilizer contraction and third-level
phase reduction, circuit compilation, exact physical sampling, and task evaluation.
Prepared query reuse and the full experiment runner remain subsequent deliverables.

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
`LinearFactorization` solves repeated right-hand sides, detects contradictory
dependent equations and supplies a nullspace basis. Ordinary systems retain
dense row provenance. Tall systems (over 64 equations and over twice as many
equations as variables) instead factor an independent subset of original rows
and verify the solution against every original equation. This avoids a square
provenance matrix in the number of boundary legs, while preserving the same
zero-free-variable solution and nullspace. It adds no probability approximation
or alternate quantum algorithm. There is no fixed physical-qubit or path-bit
cap in these types. Stored matrices and arithmetic temporaries still require
memory; this representation choice is not a process-memory cap.

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

For the original `ClosedPhaseSum::new` and `PreparedPhaseSum`, the term limit
applies to `2^quotient_rank` and is checked before enumeration. The optional
adaptive constructor below can choose a smaller certified Clifford-fiber
expansion. Imported T-state catalog decompositions remain a separate deliverable. The limit is not a
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

`project_z` pulls back sets of at least 64 parity rows through a shared packed
transpose of the support matrix. Sparse trace equalities then combine selected
rows by XOR rather than scanning the entire boundary for each column. Smaller
event sets use direct dot products. The transpose adds temporary matrix storage;
projection still contracts dependent paths and preserves unnormalized mass.
Unitary scalar queries retain constrained paths internally until the final
physical-label sum. That closure contracts hidden paths once before phase
reduction and resource budgeting; public `project_z` still returns a contracted
ket. No intermediate projection renormalizes the state.
`ParityPhase::restrict` likewise shares a packed basis transpose for at least 64
merged parity terms. It retains exact affine-shift constants and reversed
coefficients; smaller term sets use direct dot products. This adds temporary
matrix storage and does not cache phase decompositions across queries.
An empty basis evaluates the shifted scalar directly, without a coordinate-sized
transpose or mapped parity terms.
For Clifford extraction above 64 coordinates, the signature check uses an
injective pivot-coordinate restriction of the odd-axis span. Packed two-plane
addition then extracts pair coefficients modulo four. The returned quadratic
form retains all original coordinates; resource bases and budgets are unchanged.

These primitives build exact Clifford boundaries. The compilers below add
unitary near-Clifford circuit lowering and affine instruments with reset,
reported records and prescribed noise. Tests
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
with no terms. The default `ParityCopies` route reuses the Clifford ket and
constructs a phase resource for each query. The explicit
[shared commuting-layer strategy](#explicit-shared-commuting-layer-compilation)
selects shared observations; cross-fault resource reuse remains separate work.
The runtime below adds a separate causal controller route.
The instrument layer below adds resets, measurements and prescribed noise. It does not claim the paper's reduced representation ranks or
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

## Magna circuit frontend

`circuit::parse_circuit(text, &ParseOptions)` parses the `magna-0.1` text format,
including gate aliases, repeats, Pauli-product measurements, exact noise
arguments, raw detectors, control registers, external decisions, postselection,
observables and coordinate annotations. `Circuit::to_text` retains the original
source. Expanded instructions and individual operations have distinct counts:
one multi-target line may produce several operations. Locations retain the
source line, column and each enclosing repeat iteration.

```rust
use rmagna::circuit::{parse_circuit, ParseOptions};

let circuit = parse_circuit("H 0\nM(1/30) 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\n",
    &ParseOptions::default())?;
assert_eq!(circuit.num_records(), 1);
assert_eq!(circuit.detectors().len(), 1);
assert_eq!(circuit.to_text().lines().count(), 4);
# Ok::<(), rmagna::circuit::ParseError>(())
```

Classical expressions are canonical XORs over reported records and external
decision bits. Every register read is resolved before any output of that
instruction is installed, and writes carry monotonically increasing versions.
Overwriting a register cannot change an earlier selection, gate or observable.
Detectors retain raw parity rather than subtracting a noiseless reference.
Report-flip probability is stored separately from the quantum measurement.

Declare external function names in `ParseOptions::controllers`. Parsing captures
their ordered inputs and output slots and never invokes a function. External
functions can read detectors and earlier registers, while built-in XOR/NOT and
conditional gates can also read earlier records. The runtime below binds
versioned callbacks and executes them over sampled physical inputs. Guarded noise requires a
history route even for a constant guard; contiguous correlated alternatives
retain their conditional probabilities and common guard.

Default budgets permit 65,536 qubits, 1,000,000 expanded instructions,
1,000,000 operations, repeat depth 128 and 64 MiB of source. All are configurable.
Text probabilities retain the parsing limits described above. Coordinates are
parsed as rational literals and converted to finite `f64` annotation values;
noise probabilities remain exact rationals.

Inspect a file without simulation or decoder execution:

```sh
cargo run --locked -p rmagna --example validate_circuit -- circuit.magna decoder_name
```

After building that example, compare the 31 packaged reference tasks' circuit
files with the pinned parser using `benchmarks/magna_reproduction/frontend_compare.py`:

```sh
/absolute/path/to/reference-env/bin/python -I \
  benchmarks/magna_reproduction/frontend_compare.py \
  --upstream /absolute/path/to/magna \
  --rust-example /absolute/path/to/target/debug/examples/validate_circuit \
  --out /absolute/path/to/new-parser-comparison.json
```

The comparison checks format, qubit/record/instruction/detector counts, external
call count and history routing. Its receipt hashes each input and the Rust
binary and verifies the pinned source/native binary before and after. It is a
format/static-semantics check, not an end-to-end sampling benchmark. The
instrument layer below executes affine circuits and pure targets. The task schema
section documents models and module validation; [exact task evaluation](#exact-task-evaluation)
executes loaded tasks with terminal recovery, normalized targets and output moments.
The runtime section describes causal record sampling used by that task layer.

## Affine instruments and conditional target masses

`instrument::compile_affine` compiles parsed reset, measurement, affine feedback,
and Pauli-noise operations into one coherent reference instrument. Resets move
the live physical output to a fresh wire and retain the old wire as an environment.
Measurements copy the declared Pauli parity into a fresh record wire. Report
errors flip that record after the true projection; feedback and postselection
read reported values. A reusable work wire computes and uncomputes affine controls.
Discarded outputs and old reset wires are traced when querying probabilities.

`compile_unitary` compiles a measurement-free pure target using the same standard
gate matrices. `CompiledUnitary::target_weight` closes a chosen, ordered subset of
physical outputs against that target and traces the remaining environment.
Repeated output indices and incompatible target dimensions are errors.

```rust
use rmagna::circuit::{parse_circuit, ParseOptions};
use rmagna::instrument::{compile_affine, compile_unitary, CompileOptions, FaultSelection};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

let arithmetic = ScalarArithmetic::default();
let parse = ParseOptions::default();
let options = CompileOptions::default();
let source = parse_circuit("H 0\nM(1/3) 0\nCX rec[-1] 0", &parse)?;
let instrument = compile_affine(&source, &options, &arithmetic)?;
let target = compile_unitary(&parse_circuit("I 0", &parse)?, &options, &arithmetic)?;
let clean = instrument.masses(&[0], &target, &[], 1, &arithmetic)?;
assert_eq!(clean.target_success.value, ExactScalar::one());
let faults = [FaultSelection { site: 0, alternative: 0 }];
assert_eq!(instrument.fault_probability(&faults)?.to_string(), "1/3");
assert_eq!(instrument.masses(&[0], &target, &faults, 1, &arithmetic)?.error,
    ExactScalar::one());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`masses` returns acceptance, accepted target success, and their exact difference.
These are conditional on the supplied noise choices; multiply by
`fault_probability` when summing over the physical noise distribution. Omitted
sites mean identity, including report-flip sites of probability zero. Alternatives
at a site are mutually exclusive. Correlated `E` / `ELSE_CORRELATED_ERROR` chains
form one category, with each later probability multiplied by the probability of
all earlier alternatives failing. Two-qubit Pauli channels use the order
IX, IY, IZ, XI, XX, XY, XZ, YI, YX, YY, YZ, ZI, ZX, ZY, ZZ.
`noise_sites` retains source operation IDs and exact rational weights.

`reported_probability` queries simultaneous affine parities of reported records
under the physical instrument, without applying postselection. This separation
is necessary for later causal controller sampling. External decisions and guarded
noise return `NeedsHistory`; this interface does not execute callbacks, sample
noise, choose nonlinear histories, or implement task schemas and recovery yet.

`CompileOptions` bounds coherent instrument wires before diagonal phase-copy
ancillas (65,536), lowered unitary gates (1,000,000), and noise sites (1,000,000).
The underlying unitary compiler and exact arithmetic retain their own dimension
and coefficient checks. These limits do not bound total process memory. The
Clifford reference is shared across supplied faults, but final phase resources
are currently prepared for each query; no paper throughput claim follows.

Tests check every supported gate against literal matrix entries, including global
phase; partial-output target contractions against dense density sums; resets,
Pauli products, feedback, and exact channel categories; and 28 prescribed native
mass witnesses including the unmodified pinned BH2-T input. The BH2 witnesses
include no fault, rejected faults, report errors, and accepted failing triples.
Both its acceptance and fidelity contractions fit a one-term budget without
protocol-specific compiler code. The imported BH2 input files retain their own
MPL-2.0 license in `tests/data/magna-bh2/`.

Regenerate prescribed instrument witnesses with the isolated, verified reference:

```sh
/tmp/rstim-magna-reference-env/bin/python -I \
  benchmarks/magna_reproduction/instrument_vectors.py \
  --upstream /tmp/rstim-magna-261022315 \
  --out /tmp/instrument-reference.json
cmp /tmp/instrument-reference.json rmagna/tests/data/instrument-reference.json
```

`cargo run -p rmagna --example affine_mass -- PHYSICAL TARGET OUTPUT_CSV [MAX_TERMS]`
prints a no-fault exact-mass pilot and cold timing diagnostics. It is not a formal
benchmark or a complete task-file runner.

## Exact categorical noise draws

`noise::NoiseModel::physical` captures an affine instrument's independent noise
categories, including correlated-chain categories and report flips.
`NoiseModel::new` accepts exact probability rows with identity first; every row
must sum to one. `NoiseSampler::physical` samples the physical distribution.
`NoiseSampler::importance` accepts a separate independent categorical proposal,
checks every physical choice has proposal support, and returns exact physical
and proposal probabilities and their likelihood ratio. A proposal-only event
has weight zero. This interface does not implement correlated mixture proposals.

```rust
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
let options = NoiseOptions::default();
let physical = NoiseModel::new(vec![vec!["9/10".parse()?, "1/10".parse()?]], &options)?;
let proposal = NoiseModel::new(vec![vec!["1/2".parse()?, "1/2".parse()?]], &options)?;
let sampler = NoiseSampler::importance(physical, proposal, options)?;
let mut random = SplitMix64::new(20261024);
let draw = sampler.draw(&mut random)?;
assert_eq!(&draw.proposal_probability * &draw.likelihood_ratio, draw.physical_probability);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Sampling scales each category to a common integer denominator, draws a uniform
integer below it using rejection, and locates its exact cumulative interval.
There is no fixed-width probability cutoff: positive probabilities such as
`1e-400` remain reachable. Zero-width intervals cannot be selected. Deterministic
categories consume no entropy. Defaults bound categorical denominators to
1,048,576 bits and each complete draw to 1,048,576 random words; exhausting a
budget is an error, never a silently dropped draw. Sampler limits also apply
when models were prepared under more permissive options: importance construction
rejects excessive denominators, and physical draws check them before entropy.
A failed draw may have consumed
random words, so callers must retain failures and stream state when retrying.

`RandomSource` permits caller-owned entropy. `SplitMix64` defines a stable seeded
Monte Carlo stream; its `state()` can resume through `SplitMix64::new(state)`.
Sequential draws have identical results across caller chunk boundaries. This is
an independent Rust stream contract and does not match the upstream RNG stream.
Exact distribution claims assume unbiased independent input words; a finite
pseudorandom generator supplies a reproducible approximation to that entropy.
Quantum masses remain exact conditional queries on the returned fault assignment.
Tests enumerate exact categorical intervals and all two-site importance draws,
verify weighted acceptance, exercise rare events and rejection limits, and check
published SplitMix64 words plus chunk/resume identity.

## Physical reported-record sampling

`records::sample_reported` draws ordered affine reported parities from exact
physical prefix marginals under supplied faults. Correlated and duplicate queries
remain correlated. `sample_reported_conditioned` accepts a `RecordQuery` containing
previously sampled parity values and new inputs; it retains the earlier values
and rejects a zero-mass condition before reading random words. A returned
`ReportedDraw` includes the original condition mass, the final joint mass, and
entropy usage. It does not apply the circuit's postselection predicates.

```rust
use rmagna::circuit::{parse_circuit, ParseOptions, BitAtom, BitExpr};
use rmagna::instrument::{compile_affine, CompileOptions};
use rmagna::noise::SplitMix64;
use rmagna::records::{sample_reported, SamplingOptions};
use rmagna::scalar::ScalarArithmetic;
let arithmetic = ScalarArithmetic::default();
let source = parse_circuit("H 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]", &ParseOptions::default())?;
let instrument = compile_affine(&source, &CompileOptions::default(), &arithmetic)?;
let draw = sample_reported(&instrument, &[BitExpr::atom(BitAtom::Record(0))], &[],
    &mut SplitMix64::new(7), &SamplingOptions::default(), &arithmetic)?;
assert_eq!(draw.mass, arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`bernoulli_ratio` samples a ratio of nonnegative exact masses without dividing
or converting to floating point. It refines a uniform binary interval until exact
real comparisons place the entire interval above or below the threshold. Rational,
irrational and very small nonzero probabilities use the same route. Deterministic
endpoints need no entropy. The default complete-query entropy budget is 1,048,576
words and the per-contraction term budget is 256. Budget exhaustion fails explicitly;
a failed query can have consumed entropy. Scalar coefficient limits also apply.
As with categorical sampling, exact probability statements assume independent
unbiased entropy words.

This is the cold exact-marginal route for affine instruments. The runtime below
adds external callbacks, guarded noise and affine-support histories. Runtime
integration of cached query preparation remains subsequent work. Tests check
exact binary boundaries and refinement,
`2^-1200`, Bell-record and report-flip correlations, conditioned histories,
stream/resume identity, and a physically possible record that postselection rejects.

## Causal controllers and guarded physical noise

`runtime::ControllerRegistry` binds a name and nonempty version to a deterministic,
stateless Rust function over declared reported observations. A controller returns
one `bool` per declared output; errors and wrong output lengths fail the draw.
Bindings are captured when constructing `CausalProgram`, so replacing a registry
later cannot change the prepared program. `controller_versions` exposes the
captured identity contract. Caller-supplied version strings must identify the
actual decoder and configuration; the runtime cannot infer function identity.

```rust
use rmagna::circuit::{parse_circuit, ParseOptions};
use rmagna::instrument::{compile_unitary, CompileOptions};
use rmagna::noise::SplitMix64;
use rmagna::runtime::{ControllerRegistry, CausalProgram, RuntimeOptions};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
let arithmetic = ScalarArithmetic::default();
let mut registry = ControllerRegistry::default();
registry.register("parity", "1", |bits: &[bool]| Ok(vec![bits.iter().fold(false, |p, b| p ^ b)]))?;
let source = parse_circuit("H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[parity] det[0] -> ctrl[0]\nCX ctrl[0] 0",
    &ParseOptions { controllers: registry.names(), ..Default::default() })?;
let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &arithmetic)?;
let target = compile_unitary(&parse_circuit("I 0", &ParseOptions::default())?,
    &CompileOptions::default(), &arithmetic)?;
let draw = program.sample(&[0], &target, &mut SplitMix64::new(11), &arithmetic)?;
assert_eq!(draw.masses.error, ExactScalar::zero());
assert_eq!(draw.masses.target_success.value, draw.history_mass);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Before each controller call or guarded category, the exact-marginal route queries
only the already executed physical prefix, conditioned on earlier sampled inputs.
Postselection never enters those queries. Built-in affine feedback remains coherent.
Controller outputs are installed atomically in their parsed decision slots.
A correlated chain evaluates its common guard once. Inactive channels have no
action and contribute no factor to `noise_probability`. The runtime draws all
potential independent category choices up front and discards inactive ones;
this consumes entropy for some inactive categories without changing the physical
path distribution.

`sample_prescribed` accepts potential-category fault selections for differential
checks; `potential_site(original_operation_id)` supplies the stable mapping.
The returned exact masses are joint with the sampled history, and `history_mass`
is the corresponding physical history mass. An exact-marginal Monte Carlo
contribution divides the joint accepted/error mass by this history mass.
`noise_probability` describes the active noise choices and is useful for complete
enumeration; multiplying it into a physical Monte Carlo contribution would count
the sampled noise distribution twice.

Defaults additionally cap sampled history inputs at 1,000,000 and history queries
at 100,000 per draw, with separate noise and history entropy budgets. The explicit
`CausalStrategy::ColdPrefix` baseline also caps prefix compiles at 100,000. The
default fixed-template route below reuses its Clifford circuit across histories;
final scalar query closures are still prepared per query. Affine-support histories
are described below; causal importance proposals and complete paper-performance validation remain
subsequent work. Terminal recovery and task evaluation are described below.

Tests enumerate majority histories, preserve repeated observations and atomic
register updates, distinguish active and inactive guarded paths, verify correlated
chain weights, and reject missing bindings, callback failures, wrong output shapes
and resource limits. Eighteen prescribed histories match the pinned public native
backend's exact history, accepted, target-success and error masses, including
irrational branch probabilities. Regenerate those witnesses with
`benchmarks/magna_reproduction/history_vectors.py --upstream PINNED_SOURCE --out NEW_JSON`
using the same isolated reference Python environment.

## Exact normalized and weighted field values

Network contractions remain dyadic `ExactScalar` values. Normalizing by an
irrational history mass or multiplying by a rational importance weight can leave
that ring. `field::ExactField` represents the resulting element of `Q(omega)` with
four canonical arbitrary-precision rational coefficients. `FieldArithmetic`
provides checked construction, scalar conversion, addition, subtraction,
multiplication, division, rational scaling, and exact real comparison.

```rust
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::scalar::ScalarArithmetic;
let scalars = ScalarArithmetic::default();
let fields = FieldArithmetic::default();
let history = scalars.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)?;
let weight = BigRational::new(7.into(), 13.into());
assert_eq!(fields.contribution(&history, &history, &weight)?,
    fields.from_rational(weight)?);
assert_eq!(fields.divide(&ExactField::one(), &ExactField::one())?, ExactField::one());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`contribution` divides a joint exact mass by its supplied history denominator
(physical or proposal mass) and applies the supplied rational likelihood ratio. Zero denominators and division by zero
are errors. Raw rational inputs are normalized and validated before arithmetic.
Division uses the other three field embeddings to form a rational norm; it does
not approximate an inverse numerically.

Defaults limit each canonical rational numerator and denominator to 1,048,576
bits, including intermediate field multiplication results used for inversion.
`FieldArithmetic::new(bits)` changes that budget. Conversion rejects impossible
binary exponents before allocating a huge power of two. Arithmetic temporaries
and exact real-sign comparisons can exceed the stored coefficient budget; this
is not a total memory limit. A small final quotient can fail if an intermediate
field norm exceeds the configured budget. Tests compare random rational complex
operations against independent `Q(sqrt(2)) + i Q(sqrt(2))` arithmetic, retain
`2^-1200` weighted errors, check normalized irrational histories, and exercise
rational validity, exponent/resource bounds and Pell cancellation.

## Projected and mixed target density networks

`CompiledUnitary::density_overlap` takes a `DensityBoundary` for each preparation:
ordered selected outputs, physical Z-parity event rows, and their required bits.
It returns the unnormalized Hilbert-Schmidt overlap of the two reduced density
operators. Each preparation traces its own discarded environment, and events act
on both ket copies before contraction. Output order may differ between the two
preparations. `target_weight` uses the same implementation for a pure target with
all target wires kept and no target-side events.

This primitive supports target auxiliary wires, postselection and mixed reductions
without a dense state or density matrix in production. The target normalization
is a separate `target.probability_z(...)` query; normalization belongs to the field
arithmetic layer above. A zero projected target has zero overlap at this level;
`TaskEvaluator::new` rejects it before using it as an ideal state.
Tests independently construct literal complex reduced density matrices, trace
both environments, and compare all sixteen selected-output/event combinations,
including empty and permuted outputs. Existing pure-target and partial-target
checks remain unchanged. Task JSON loading is described in the following section.

## Task schema and module loading

`task::parse_task(text, base_directory, &TaskOptions)` reads task schema 1 and
resolves target/recovery paths relative to the supplied directory. Read-only
`TaskSpec` accessors preserve optional output mappings, the choice of target or
classical error observables, sampled records and exact integer Pauli masks.
`validate(physical)` checks indices, unique ordered outputs, declared logical
observables, and Pauli widths. Masks use arbitrary-precision JSON integers;
no floating conversion truncates masks above 64 bits. Y phases follow Hermitian
Pauli operators in output order.

```rust
use rmagna::circuit::{parse_circuit, ParseOptions};
use rmagna::task::{parse_task, TaskOptions};
let circuit = parse_circuit("M 0\nOBSERVABLE_INCLUDE(4) rec[-1]", &ParseOptions::default())?;
let spec = parse_task(r#"{"schema_version":1,"error_observables":[4],"sample_records":[0]}"#,
    std::path::Path::new("."), &TaskOptions::default())?;
let task = spec.validate(&circuit)?;
assert!(task.outputs().is_empty());
assert_eq!(task.error_observables(), Some([4].as_slice()));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`load_modules` also parses the target and recovery files with the caller's circuit
and controller-name options. Pure path targets use only the selected local output
wires; an empty preparation means zero on those wires. Object targets require
an explicit ordered outputs list within the preparation's declared width, and can
specify auxiliary wires and disjoint projected auxiliaries. Only path targets
receive implicit zero-wire padding. Target modules
must be unitary and noiseless. Target modules may retain detector and observable
declarations without classical targets; declarations with references remain
forbidden even when their expressions cancel to zero. Recovery modules require a
target, can have their
own local records and controllers, and forbid noise channels, nonzero report
errors and postselection. Their outcomes are averaged during task evaluation.
Loading modules never executes controllers or contracts a target normalization;
zero-norm target rejection belongs to evaluation preparation.

Defaults bound task JSON to 16 MiB, each list/object collection to 1,000,000 items,
and Pauli masks to 65,536 bits. Circuit files are read through the circuit source
byte budget, with file path and task-field context retained on errors. Semantic
operation restrictions also identify the offending source line and column. Unknown
fields and invalid numeric types are rejected. `ideal_state: null` and
`recovery: null` mean omitted modules; list-valued fields require lists.

All task fields retain their raw JSON shapes until typed validation. Numeric
objects using serde_json's private number/raw-value tags cannot become indices,
schema versions or projection bits when another dependency enables
`arbitrary_precision`. Wide masks remain exact without enabling that global
feature. `make test-magna-task-json` runs the same eleven adversarial task tests
under default, arbitrary-precision and float-roundtrip feature configurations;
CI runs this target alongside the ordinary workspace tests.

Build `cargo build -p rmagna --example validate_task`, then run
`validate_task PHYSICAL TASK [CONTROLLER...]` for static validation.
`benchmarks/magna_reproduction/task_compare.py --upstream PINNED_SOURCE --rust-example BINARY --out NEW_JSON`
compares all 31 packaged task/module mappings against the pinned public native
API, with controller callbacks that fail if invoked. It checks output order,
target preparation width/mapping/projections and recovery width/records/route,
records input and binary hashes, and works with Python `-O`. This is separate from
complete task execution and statistical benchmark reproduction.

## Exact task evaluation

`task::TaskEvaluator::new` prepares loaded schema-1 modules, validates selected
outputs and records, and rejects a zero-norm projected target before entropy is
consumed or any controller runs. `sample` draws physical categorical faults;
`sample_prescribed` fixes potential fault choices for exact differential checks.
Both return accepted/error estimator contributions as `ExactField` values.
A physical Monte Carlo draw already samples the noise distribution: its
`noise_probability` is enumeration metadata, not an additional estimator factor.

Terminal recovery maps its local data wires onto the task's ordered physical
outputs and allocates fresh auxiliary wires. Its record and external-call indices
occupy distinct namespaces from the physical module. All recovery outcomes are
averaged, including resets and affine feedback; external recovery controllers use
the same causal prefix and sampler rules as physical-module controllers.

Targets may be pure, reduced mixed states, or projected auxiliary preparations.
The density overlap is divided by the exact target normalization. For a quantum
target, the native-compatible `raw_error` is
`raw_acceptance * target_normalization - raw_target_success`; normalized `error`
also uses the selected history denominator. For classical error definitions, success means
all selected logical observables are zero, and `raw_error` is accepted mass minus
that joint success mass. The optional `target_success` fields refer only to a
quantum target. Requested Pauli moments are accepted, unnormalized moments after
recovery and before any ideal-target projection; they use the history denominator
without target normalization. Raw scalar numerators remain available for audits.

All reported contributions divide by physical history mass for exact-marginal
draws and prescribed-history diagnostics, or by proposal mass for actual
affine-support draws. Physically zero affine proposals remain zero contributions;
see [affine-proposed causal histories](#affine-proposed-causal-histories).

Requested records follow the selected history sampler without postselection.
Exact-marginal draws are physical observations; affine-support draws can include
physically zero histories. Records remain correlated with controller inputs and
each other. `ResolvedHistory`
separates causal history resolution from final metrics and provides read-only
branch instruments, faults, conditions, resolved expressions, and sampled records. The
runtime's existing pure-target sampling methods remain convenience wrappers.

`acceptance_resources` and `target_resources` expose the actual final quotient
ranks and enumerated term counts for the corresponding draw. These describe cold
contractions, not prepared-query throughput. Compilation and target normalization
have separate resource checks; per-query term, scalar, field, and history budgets
remain explicit errors.

Build `cargo build --release -p rmagna --example evaluate_task`, then run
`evaluate_task PHYSICAL TASK [SEED]` for one prescribed no-fault draw. The optional
external bindings are described in the process-bridge section below. Use
`benchmarks/magna_reproduction/affine_task_compare.py --upstream PINNED_SOURCE --rust-example BINARY --out NEW_JSON`
to compare exact no-fault native numerators on all controller-free packaged tasks.
The pilot matches 26 tasks and records the five tasks requiring external decoders
as unsupported by this controller-free comparison script. It retains input and
binary provenance and query resource counts.
It is neither a statistical experiment nor evidence for paper throughput.

Tests cover mixed and irrationally normalized projected targets, zero-norm
rejection, local recovery records and controller namespaces, exact full physical
noise enumeration for classical errors, reported samples rejected by selection,
and Pauli moments before target projection. Target coordinate declarations and
empty detector/logical declarations are ignored during target compilation;
syntactic classical references remain forbidden even when they cancel.
All sixteen prescribed BH2 fault
witnesses also pass through task evaluation with a one-term budget. A separate
literal matrix oracle checks 768 projected Pauli-moment queries. Ten general-task
native witnesses cover target normalization, recovery, moments, and classical
error conventions. Regenerate them in the verified isolated Python environment:

```sh
/tmp/rstim-magna-reference-env/bin/python -I \
  benchmarks/magna_reproduction/task_vectors.py \
  --upstream /tmp/rstim-magna-261022315 --out /tmp/task-reference.json
cmp /tmp/task-reference.json rmagna/tests/data/task-reference.json
```

The evaluator uses the explicitly selected history sampler and independent
physical noise. Python decoder bindings are available through the bridge below.
Causal importance proposal selection, runtime integration of prepared query
families, aggregate statistics and the
public run/result interface are separate deliverables.

## External decoder process bridge

`runtime::process::ControllerProcess::spawn(command, options)` launches an
explicitly supplied executable without a shell. It owns a persistent JSON-line
session and registers its versioned bindings atomically into `ControllerRegistry`.
Captured bindings share the process; the last owner kills and reaps the worker.
Only declared input bits, the binding name and a request ID enter a callback
request. The child does not receive fault assignments, ideal outcomes or network
state. This is an execution interface for trusted decoders, not a sandbox.

The protocol is `rmagna.controller.v1`. A worker first sends
`{"protocol":"rmagna.controller.v1","bindings":{"name":"version"},"manifest":{}}`.
Requests contain `id`, `name` and `bits`; responses contain the matching `id`
and `outputs`, or an `error`. Output length is checked by the causal runtime
against the circuit declaration. Bits must be booleans or integer zero/one.
The default whole-exchange timeout is 60 seconds, including blocked writes;
frames are limited to 16 MiB and bit lists/binding counts to 1,000,000.
Malformed frames, wrong IDs, callback exceptions, EOF and timeout poison the
session and kill the worker. An ambiguous callback is never retried. Stderr
inherits the parent's diagnostics. Workers must not spawn descendants retaining
protocol pipes; process-tree management belongs to the caller's execution host.

Wire frames retain raw JSON token types before typed validation. Duplicate
greeting/response fields and binding names are rejected, including escaped
duplicate names. IDs must be actual unsigned integer tokens; bits must be actual
booleans or integer zero/one tokens. Objects containing serde_json's internal
number marker cannot masquerade as numeric protocol fields under feature
unification. Arbitrary manifest objects preserve their actual container shapes,
reject duplicate keys and have a nesting limit of 128. They retain the
worker-supplied, unverified provenance status.

`rmagna/python/decoder_worker.py` implements an observation-only Python host for
schema-1 `controllers.json` bindings with `decode(bits, config)` functions.
Imports are explicit; callbacks run with a fresh copy of the declared config.
Python prints become stderr diagnostics. A verified-source loader bypasses local
bytecode caches and rejects changed lazy-import sources. The source manifest
covers at most 4,096 Python files under the explicit module root, each limited to
16 MiB. Installed dependency identity remains the execution environment's
responsibility. Config and module hashes are returned as worker-supplied
provenance; benchmark callers must independently verify them. The host blocks
imports of the Magna quantum package. Compilation, noise, quantum history
probabilities and weighting continue to execute in Rust.

Source files and their path components under the resolved module root must not
be symbolic links. Such paths reject during inventory or import, including
links introduced before a lazy import and package-directory links. This policy
keeps the advertised source inventory and verified loading consistent. Installed
dependencies outside that root retain the separate environment-identity scope.

Use the diagnostic task example with explicit bindings:

```sh
cargo build --release -p rmagna --example evaluate_task
target/release/examples/evaluate_task PHYSICAL TASK 20261026 \
  --controllers CONTROLLERS_JSON --python PYTHON --module-root DECODER_ROOT
```

The example returns controller transcripts and decoder provenance alongside exact
masses. `--constant-word WORD` is a prescribed entropy diagnostic, not a Monte
Carlo stream and not a contract for matching the native backend's history basis.
The two implementations can sample different parities even with constant entropy.
Common-history differential checks must explicitly fix observations.

Integration tests run the Python host with native backend imports blocked, check
majority histories against exact probabilities, preserve config isolation, and
cover noisy decoder stdout, malformed protocols, exceptions, non-bit and wrong
shaped outputs, process crashes, startup/exchange timeouts, oversized frames and
bit lists, atomic registration, poisoned bytecode and changed lazy imports.
Symlink/cache controls and malformed wire/metadata regressions also run through
`make test-magna-controller-json` under default, arbitrary-precision,
float-roundtrip and combined serde features; CI runs the same matrix.
This bridge supplies external bindings; full packaged decoder campaigns and
prepared history reuse remain separate validation and performance work.

## Explicit physical-history queries

`CausalProgram::resolve_prescribed_history(requested, faults, groups, arithmetic)`
fixes observations without reading entropy. Groups follow circuit order: one
input list per external callback (including empty lists), one singleton for the
first guarded category in a correlated chain, and one final requested-record
list when nonempty. Group counts and lengths are checked before callbacks run.
Every group must have positive joint physical prefix mass given all earlier
observations; impossible or contradictory groups fail before their callback.
Final postselection and target fidelity never condition this validation.
The returned history retains its exact physical mass and active noise weight.

`TaskEvaluator::evaluate_prescribed_history(faults, groups)` evaluates the same
normalized conditional metrics as a physical draw on that history. This is a
conditional query, not an unbiased sampling proposal by itself. Complete physical
enumeration multiplies a conditional metric by its history mass and physical
noise probability. Linear tasks require an empty group list. The diagnostic
example accepts `--history JSON_FILE` containing boolean group lists and
`--max-terms COUNT` to apply the task's explicit contraction budget.

`benchmarks/magna_reproduction/controller_task_compare.py` takes the usual
`--upstream`, `--rust-example` and `--out` arguments, with optional `--tasks` and
`--timeout`. Each native task runs in a separate process to avoid decoder module
name collisions. Two native no-fault histories are explicitly replayed in Rust;
raw exact masses, norms, and complete callback inputs/outputs must agree.
Decoder and binary provenance, timeouts, unsupported paths and mismatches are
retained. Malformed successful Rust output is a failed history with its raw
stdout/stderr, error and native reference preserved; earlier and later histories
and task provenance remain available. Failed cases keep the receipt partial and
the command exits nonzero. The receipt distinguishes a selected subset from complete five-task
coverage. These finite histories establish neither Monte Carlo precision nor
paper performance.

Runtime tests replay all eighteen pinned native histories directly, verify shape
errors and impossible histories before callbacks, preserve guarded-chain weights,
and reject inconsistent duplicate observations. Requested records remain physical
even when selection rejects them. Task tests explicitly integrate conditional
contributions against history mass to recover the physical accepted probability.


## Certified Clifford-fiber decompositions

`fibers::PreparedCliffordFibers` partitions the residual phase domain into affine
fibers on each of which the phase is Clifford. It first computes the original
signature quotient, then searches for a subspace on which the restricted
trilinear signature vanishes. This condition differs from the radical condition:
all three arguments belong to the chosen subspace. For example, fixing one bit
of a CCZ phase leaves a quadratic phase on the other two bits, giving two
Clifford terms rather than eight computational terms. A pair of T phases also
has two Clifford fibers, distinguished by even or odd input parity.

The search uses deterministic greedy orders and linear signature slices. Every
candidate is independently certified with exact Clifford extraction. The associated
bilinear signature bounds the possible subspace dimension and stops the search
when that bound is reached. There is no universal optimal-rank claim. `FiberOptions` defaults to 256 orders for signature ranks
at most 16; larger ranks or zero attempts retain computational fibers. This
bounds the search policy, not total memory or time. Construction checks the
selected term count before allocating fibers. Each reference fiber caches its
affine shift, quadratic phase and exact eighth-root constant.

`evaluate(updated, boundary, coefficient, arithmetic)` requires the update to
differ by a Clifford phase and reuses that affine partition. It adds the exact
update and boundary quadratics and evaluates one Gauss sum per fiber. All
constants, complex coefficients and powers of square root of two are preserved.
`quotient_rank` reports the original signature rank; `num_terms` reports actual
Clifford terms and `enumerated_coordinates` reports their binary label count.
These quantities must not be treated as interchangeable ranks.

`ClosedPhaseSum::new_adaptive` applies this decomposition after recursively
absorbing radical directions and their affine constraints. Unitary network
queries use that adaptive route by default, so a four-term verified decomposition
can fit a four-term budget even when its signature quotient has sixteen points.
The original closed constructor retains its computational expansion contract.
This change does not prepare whole circuit queries across faults or replace the
cold causal prefix compiler.

Tests exhaust all third-level signature classes through three variables with
Clifford updates, compare dense four- through eight-variable phases against raw
integer eighth-root sums, and check paired T, CCZ, two independent cubic blocks,
wide 129-bit radicals, incompatible updates and budgets. All 28 pinned native
phase witnesses also pass through both prepared fibers and adaptive closed sums.
A literal four-qubit circuit matrix verifies every amplitude with a four-term
budget that rejects three terms.


A bounded TQD3 prescribed-history pilot matches the native raw masses and full
real-decoder transcripts at two histories. Its acceptance/fidelity networks use
2/4 Clifford terms at signature ranks 5/10, replacing the computational 32/1,024
terms. This is finite correctness and resource evidence; it does not establish
arbitrary-history coverage, complete TQD campaigns, or paper throughput.


When a canonical ket has a fixed computational value on a Hadamard's wire, the
new Fourier column is provably independent of the old support columns. Clifford
evolution keeps that exact ket directly instead of factoring an empty hidden-path
problem. Gates with a nonconstant input row still perform full contraction.
Initial arbitrary affine inputs are contracted before this invariant is used.
A wide exact Fourier-sign test checks complex coefficients, nonzero fixed bits,
existing entangled support, four independent Hadamards and their adjoints.


## Fixed coherent causal templates

`CausalProgram` defaults to `CausalStrategy::FixedTemplate`. It prepares one
coherent instrument with a separate initial-zero virtual wire for every external
decision slot. Fixed coherently controlled Clifford gates read these wires.
A known true decision becomes a Pauli X insertion on its virtual input; unknown
future decisions remain zero. Controller functions still execute only when their
reported observations have been sampled in causal order. Preparation invokes no
controller and draws no entropy.

History queries close the whole fixed instrument while selecting only already
available reported parities. All later quantum operations are trace preserving,
so they cannot change these physical prefix probabilities. Postselection is
excluded from reported-record queries. Inactive or unresolved guarded channels
contribute no insertion; final branch metadata contains only active categories,
so its exact noise probability retains the conditional guard semantics. Correlated
chains resolve their common guard once. Final history conditions become additional
event rows on a shared immutable instrument view.

Instrument clones share the Clifford circuit, live/record mappings, selection
rows, decision layout and noise alternatives. A view replaces its complete decision
assignment and active-site list, preventing state from leaking between draws.
`compile_affine` keeps its strict rejection of unresolved history operations;
the causal compiler is internal to the runtime.

Select `CausalStrategy::ColdPrefix` explicitly to retain the previous prefix
compiler as a correctness oracle. There is no automatic fallback. The diagnostic
example accepts `--causal-strategy fixed|cold`. `max_prefix_compiles` limits actual
prefix constructions only on that cold route; `max_history_queries` limits the
observation groups on both routes. `ResolvedHistory`, pure draws and task draws
report actual post-preparation Clifford `network_compilations` and history query
counts. These counts exclude final scalar-network closure and phase preparation,
which are still cold; zero circuit recompiles do not establish full prepared-query
throughput.

All eighteen pinned native histories also compare the fixed and cold routes,
including exact complex masses, active noise weights and callback transcripts.
Analytical irrational-probability tests cover future controlled-H/S operations,
guarded faults, rejected selections and requested records. Seeded physical draws
retain identical entropy states and histories across routes. Fixed histories use
one initial reference construction and zero subsequent Clifford constructions;
these tests also check the cold route's actual constructions.

## Prepared compatible Z projections

`stabilizer::PreparedZProjection::new(reference, rows)` factors the pulled-back
constraints and prepares the reference quadratic restriction once. `project`
accepts a ket with the same support columns, quadratic cross terms and odd linear
coefficients; offsets, global coefficients, phase constants and even linear
coefficients may vary. Incompatible structure and dimensions are explicit errors.

Queries solve only the new right-hand side and apply exact affine and even-linear
phase corrections. They return unnormalized constrained paths, which can still
be dependent or have zero coefficient; call `contract` for a canonical ket.
An inconsistent constraint system returns `None`. The plan owns immutable matrix
storage and prepares no non-Clifford decomposition. It is an explicit library
primitive; task-level query-plan reuse is a subsequent layer.

```rust
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::scalar::ScalarArithmetic;
use rmagna::stabilizer::PreparedZProjection;
let arithmetic = ScalarArithmetic::default();
let input = CliffordCircuit::computational(BitVector::zeros(2)?)?;
let reference = CliffordCircuit::new(2, [CliffordGate::H(0)])?
    .evolve(&input, &arithmetic)?.unwrap();
let rows = [BitVector::from_indices(2, [0])?];
let plan = PreparedZProjection::new(&reference, &rows)?;
let projected = plan.project(&reference, &BitVector::zeros(1)?)?.unwrap();
assert_eq!(projected.norm_squared(&arithmetic)?, arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Prepared physical probability constraints

`CompiledUnitary::prepare_probability(rows, arithmetic)` prepares physical trace
and fixed event constraints for the original compiled reference. The returned
`PreparedProbability::evaluate(state, outcomes, max_terms, arithmetic)` accepts
any insertion view of that same program. Preparing a perturbed view still uses
the original reference; a separately compiled program is rejected even if its
gates and dimensions happen to match.

The plan shares binary elimination, mapped support columns and the reference
quadratic restriction across queries. It owns immutable preparation storage;
callers can retain it explicitly. Ordinary `probability_z` remains available.
Queries still construct their joint ket and perform final phase closure and
non-Clifford resource preparation, so this is partial preparation and makes no
warm-throughput or complete task-network reuse claim.

```rust
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::{UnitaryCircuit, UnitaryGate};
let arithmetic = ScalarArithmetic::default();
let circuit = UnitaryCircuit::new(2, [UnitaryGate::Clifford(CliffordGate::H(0))])?
    .compile(&BitVector::zeros(2)?, &arithmetic)?;
let rows = [BitVector::from_indices(2, [0])?];
let plan = circuit.prepare_probability(&rows, &arithmetic)?;
assert_eq!(plan.evaluate(&circuit, &BitVector::zeros(1)?, 1, &arithmetic)?.value,
    arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Conservative affine record proposals

`gf2::AffineSupport` represents a nonempty affine binary set with independent
columns. `linear_image`, `condition` and `translated` preserve the exact set;
conditioning returns `None` for an empty intersection. It carries no quantum
phase or physical probability.

`CompiledUnitary::structural_z_support` projects the coherent Clifford boundary's
computational support. Deferred phases can remove labels through interference,
but cannot create labels outside this family. `CompiledInstrument::reported_support`
includes prescribed faults, report flips, duplicate parities and inversions, and
excludes postselection.

`records::propose_affine_reported` conditions that family on the given reported
values and proposes the requested group uniformly. It returns the proposal's
exact dyadic mass and entropy count, without querying physical probabilities.
A physical interference zero is a valid proposal; its eventual weighted
contribution is zero. The family can be larger than the physical support or a
more tightly reduced proposal, so its variance must be measured separately.
The causal runtime selects this proposal explicitly as described below.

Each random word supplies up to 64 independent directions, low bit first.
Deterministic groups use no entropy. Invalid records, faults, structural
conditions and entropy budgets fail before drawing. `max_terms` does not apply
to this algebraic proposal; the caller's later physical contractions still use
their explicit phase-resource budgets.

```rust
use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, compile_affine};
use rmagna::noise::SplitMix64;
use rmagna::records::{RecordQuery, SamplingOptions, propose_affine_reported};
use rmagna::scalar::ScalarArithmetic;
let arithmetic = ScalarArithmetic::default();
let circuit = parse_circuit("H 0\nT 0\nH 0\nM 0", &ParseOptions::default())?;
let instrument = compile_affine(&circuit, &CompileOptions::default(), &arithmetic)?;
let parities = [BitExpr::atom(BitAtom::Record(0))];
let draw = propose_affine_reported(&instrument,
    &RecordQuery { given: &[], parities: &parities }, &[],
    &mut SplitMix64::new(7), &SamplingOptions::default(), &arithmetic)?;
assert_eq!(draw.proposal_mass, arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```


## Affine-proposed causal histories

Set `RuntimeOptions::history_sampler` to `HistorySampler::AffineSupport` to
propose each controller/guard/requested-record group uniformly from its
conditioned conservative support. `ExactMarginal` remains the current Rust
default. Both choices retain causal reported inputs and atomic controller
decisions; neither includes selection predicates or silently changes methods.

The runtime multiplies the exact group proposal masses, then evaluates the
physical history mass at the end. `history_proposal_mass` is present only for
actual affine proposals. Interference-zero histories are retained and may invoke
controllers on structurally valid inputs; their task contributions and history
weights are zero. Controllers must accept every correctly shaped bit input.
Inactive guarded channels still contribute no noise action or probability.

`TaskEvaluation` reports `history_mass`, optional `history_proposal_mass` and
`history_weight = physical_mass / proposal_mass`. Acceptance, error, target
success and moments already include history reweighting: their joint masses are
divided by proposal mass, and target quantities also use the target norm. They
can exceed one on a single draw. Do not multiply the history weight again.
Exact-marginal and explicitly prescribed diagnostics retain their physical
conditional normalization and report no affine proposal mass. Zero proposed
physical mass never leads to division by zero.

`evaluate_task --history-sampler affine_support` selects this path;
`--history-sampler exact_marginal` selects the existing path. `--history`
continues to request positive-physical-mass prescribed diagnostics regardless of
the sampler option. The example reports the selected method and both masses.

Tests enumerate proposed irrational and interference-zero histories and a
three-qubit circuit with sequential copy/XOR controllers, conditional Hadamard,
guarded noise and postselection. A separate four-qubit majority truth table checks
nonlinear callbacks, repeated observations, one-word entropy and selection independence. Proposal/noise-weighted acceptance, error and
target overlap agree with analytical physical enumeration for both fixed-template
and cold-prefix strategies. The family can contain additional zero histories;
these tests do not establish the same proposal variance as another compiler or
complete prepared-decomposition reuse.

The unitary compiler stores its frozen observations as an immutable general
`ParityPhase`, including its constant. Scalar closures compose each family's
linear parity observations in leg order and conjugate both constants and
coefficients for bra copies. Existing compilation still creates individual
copies; the representation can now express shared observations without changing
amplitude, probability, overlap, density or Pauli-moment semantics. The explicit layer
compiler below supplies layer selection and fault-dependent phase corrections.


## Explicit shared commuting-layer compilation

`UnitaryCircuit::compile_with_strategy(..., CompilerStrategy::SharedCommutingLayer)`
transports rotation axes through Clifford prefixes with their exact Pauli signs,
partitions consecutive mutually commuting slots, and keeps the largest group
(the earliest wins a tie). T contributes one slot, controlled-S three, and CCZ
seven; Clifford special cases are lowered directly. The selected group copies
an independent commuting Pauli basis once, while other slots use individual
parity-copy observations. General Pauli observations are copied coherently.

Original gate boundaries and same-position fault ordering remain available.
Faults after the shared snapshot also update the later selected rotation signs
and constant phases. Each insertion assignment starts from immutable reference
metadata, including when replacing a perturbed view. Prepared probability plans
use the queried view's current phase rather than freezing its original signs.

`CompileOptions::compiler_strategy` selects this compiler for instruments and
targets; the Rust default remains `ParityCopies` during validation.
`evaluate_task --compiler shared` selects it explicitly. `compilation()` reports
slot groups, the selected group, outside-group `phase_ancillas`, shared coordinate
storage, actual coherent width, Clifford gate count and path variables. Shared
coordinates use real frozen wires in this representation, so the reported width
includes them. After the reference Clifford evolution, fixed-input affine relations reduce
the shared observations to independent support coordinates. Fault-dependent
offsets restrict both parity observations and their constant phases. The original
full-width reference remains available for Pauli transport;
`reference_coherent_width` and `shared_observations_before_reduction` report its
storage separately from the compact query width. Dropping determined labels is
injective on the support and preserves every path amplitude. Prepared probability
plans use the compact immutable reference, including when created from a perturbed
view. Outside-group observations remain unchanged.

`CausalProgram::compilation()` and `TaskEvaluator::compilation()`
return initialization metadata. Diagnostic JSON labels its `compilation.scope`
as `linear_instrument`, `initial_fixed_template` or `initial_potential`. In
`ColdPrefix`, the initial potential network substitutes false for unresolved
controller decisions; later resolved instruments can have different slot groups,
widths and path counts. Those initial counts do not describe final draw geometry.
It still prepares final scalar closures per query and claims no timing parity.

The differential task drivers accept `--compiler shared` or `--compiler parity_copies`,
verify the selected strategy and term budget in the Rust response, and retain the
choice in their receipt. Controller tasks use explicit native histories. The
affine driver leaves those tasks to the controller driver rather than treating
missing history execution as an affine success.

Tests compare random noncommuting circuits and every single-Pauli mask at every
original gate boundary with direct literal matrices, plus multi-fault replacement,
negative/dependent axes, complex phases, prepared queries and a CCZ split across
two groups. Both compilers match all 28 prescribed native instrument witnesses,
and both causal strategies retain unbiased affine-history estimates.

Fixed-input tests cover rank-zero CCZ observations on every three-qubit input,
all Pauli masks before and after the shared snapshot, and 129 correlated shared
observations with a single independent coordinate across packed-word boundaries.
Exact amplitudes, fault-dependent global phases, prepared queries and restoration
are checked against literal matrices or analytical GHZ amplitudes.

A bounded TQD3 prescribed-history comparison reduces 504 full-space shared
observations to 191 fixed-input coordinates, matching the pinned native count.
Both histories preserve exact masses and controller transcripts, with two
acceptance terms and four target terms. This is a correctness diagnostic, not a
Monte Carlo precision or performance result.

Quadratic affine restrictions accumulate packed symmetric polar rows and use
sparse coordinate products when shorter than a packed dot product. Arbitrary
shifts and dependent directions retain exact phases. Independent literal
polynomial tests cover dense and sparse maps through 129 variables; existing
prepared projections continue to reuse the same polar images.
