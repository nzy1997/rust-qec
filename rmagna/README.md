# rmagna

Independent Rust foundations for global-network near-Clifford simulation.
The current package provides exact scalars, packed binary linear algebra,
quadratic phase sums, phase-preserving stabilizer contraction and third-level
phase reduction, circuit compilation, exact physical sampling, and task evaluation.
Bounded [probability-query preparation](#bounded-instrument-probability-preparation)
supports opt-in trace-plan and fixed-resource reuse. The full experiment runner
remains a separate deliverable.

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
Clifford reference is shared across supplied faults. In the default `Cold`
probability mode, phase resources are prepared per query. `Trace` retains trace
plans with adaptive phase closure; `FixedFibers` also retains fixed phase resources,
as described in [bounded probability preparation](#bounded-instrument-probability-preparation).
Target-density and Pauli-moment closures retain their separate paths. No paper
throughput claim follows.

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

Cold exact marginals remain the default route for affine instruments. The runtime
below adds external callbacks, guarded noise and affine-support histories; the
[bounded instrument probability preparation](#bounded-instrument-probability-preparation)
options also apply to these probability queries. Tests check exact binary boundaries and refinement,
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
are described below. [Task and causal importance sampling](#task-and-causal-importance-sampling)
is available through `new_with_sampling`. File/run/result integration and complete
paper-performance validation remain subsequent work. Terminal recovery and task
evaluation are described below.

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

`acceptance_resources` and `target_resources` expose quotient ranks and term
counts for the corresponding draw. Acceptance counts follow the selected
[probability-preparation mode](#bounded-instrument-probability-preparation): cold
and trace modes report adaptive closure, while fixed mode reports its immutable
resource before adaptive radical absorption. Fixed counts can exceed adaptive
counts. Target counts retain their separate target-density closure. These are
resource metrics, not prepared-query throughput. Compilation and target
normalization have separate resource checks; per-query term, scalar, field, and
history budgets remain explicit errors.

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
Bounded prepared probability families are available through the opt-in modes
[documented below](#bounded-instrument-probability-preparation). Standalone
aggregation is available through
[`statistics::ExactStatistics`](#exact-streaming-paired-statistics).
[Task and causal importance proposals](#task-and-causal-importance-sampling)
are available through `new_with_sampling`; automatic public run/result
integration remains a separate deliverable.

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
problem. For a nonconstant input row, packed row reduction identifies zero or
one hidden direction, and the exact contraction kernel uses that proved basis
without general equation provenance. Initial arbitrary affine inputs are
contracted before this invariant is used.
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
counts. These counts exclude final scalar-network closure and phase preparation.
The default `Cold` probability mode prepares per query; the opt-in
[bounded probability modes](#bounded-instrument-probability-preparation) reuse trace
plans and optionally fixed phase resources. Gaussian evaluation remains
query-specific, and zero circuit recompiles alone do not establish prepared-query
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

The plan shares binary elimination, mapped support columns, the reference
quadratic restriction and retained-boundary Gaussian contraction across queries.
Its structural reference does not select an outcome: an inconsistent zero event
cannot disable a later nonzero event or a fault-flipped event. It owns immutable
preparation storage;
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
Coordinate lists are cached only for those sparse products; dense directions
stay packed. A thread-local allocation counter checks sparse and dense pullbacks
through 1,025 variables against a budget scaled to packed matrix storage.

Hadamard evolution uses the independent-support invariant to find zero or one
hidden direction after clearing its boundary row. A streaming row basis avoids
general equation provenance. The known independent-column/nullspace basis feeds
the same exact quadratic-elimination kernel used by general contraction; global
phases and unnormalized scalars remain unchanged. Dependent input is still
canonicalized once at circuit entry. This is a general linear-algebra optimization,
with no circuit-family recognition or measured throughput claim.


## Prepared quadratic sums

`PreparedQuadraticSum` records a Gaussian elimination program once and executes
only linear-phase and scalar updates for compatible queries. Constants and even
linear coefficients may change; cross terms and linear parity must match.
Incompatible forms return an explicit error before execution. A zero reference
still records every structural step, so later nonzero updates remain valid.
The cold and prepared reducers share the same numeric update rules.

```rust
use rmagna::gf2::BitVector;
use rmagna::quadratic::{PreparedQuadraticSum, QuadraticForm};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
let row = BitVector::zeros(1)?;
let zero = QuadraticForm::new(0, vec![2], vec![row.clone()])?;
let plan = PreparedQuadraticSum::new(&zero)?;
let updated = QuadraticForm::new(0, vec![0], vec![row])?;
let arithmetic = ScalarArithmetic::default();
assert_eq!(plan.evaluate(&zero, &arithmetic)?, ExactScalar::zero());
assert_eq!(plan.evaluate(&updated, &arithmetic)?, arithmetic.integer(2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

This primitive prepares only a quadratic sum. Query-family Gaussian contraction,
non-Clifford phase resources and runtime integration require separate plans.

The evolving Clifford ket is owned inside the gate loop. Label permutations
update its support without copying the unchanged quadratic matrix. Z, S and CZ
update exact affine parity/product phases in place with packed upper-triangle
operations. The caller's input stays immutable. Wide reverse-label/diagonal-phase
oracles cover dense support coordinates through 129 bits, in addition to literal
small-circuit tests. These changes do not establish a formal speedup.

## Prepared affine contraction

`PreparedAffineContraction::new(&reference)` prepares the support factorization,
path basis, hidden Gaussian actions and output offset substitutions. Its
`contract(&updated, &arithmetic)` accepts changed offsets, coefficients, quadratic
constants and even linear phases. It rejects changed width, support columns,
cross terms or linear parity before returning a zero result. Output is canonical
and unnormalized; exact global phase is retained in its coefficient.

Preparation records the full structure even for a zero reference coefficient or
zero interference sum. Cold and prepared contraction share Gaussian actions and
numeric updates. Literal path enumeration tests cover compatible updates,
empty boundaries and dependent columns; analytical 129-bit examples cover wide
hidden Gaussian sums. This prepares affine contraction only. Non-Clifford
resource plans and complete prepared circuit queries remain separate work.

Signature quotients use coordinate restrictions spanning the odd-axis space for
wide phases. Bilinearity makes the corresponding pair slices span the complete
ambient signature row space; each slice remains full width. Even terms are skipped
and the odd-span rank is a certified stopping bound. The full radical and quotient
basis are unchanged. Wide dense embeddings are checked against every ambient
coordinate-pair equation, including zero-signature cancellation with odd terms.

Prepared probability tests include deterministic impossible outcomes made
possible by a fault, immutable preparation from a perturbed view, both compiler
strategies and direct literal event masses. Final non-Clifford phase closure
remains query-specific.


## Fixed phase resources for probability families

`prepare_probability_with_fibers(rows, max_terms, options, arithmetic)` adds a
certified immutable Clifford-fiber resource to the prepared probability plan.
`fixed_phase_resource()` exposes its quotient rank, enumerated coordinates and
term count. Preparation uses the original program's structural output even if
the zero reference event has no support; faults and outcomes update exact
Clifford corrections and Gaussian sums while reusing that resource.

This explicit mode freezes the resource before adaptive radical absorption.
Its term count and rank may exceed ordinary adaptive closure and are reported as
such; a smaller evaluation budget rejects the fixed resource. Structural zero
queries still return zero without enumeration. The default `prepare_probability`
retains adaptive query-specific closure. Final Gaussian elimination remains
query-specific in the fixed-resource mode; whole-runtime caching is separate.

## Bounded instrument probability preparation

`CompileOptions::probability_preparation` selects `Cold` (the default), `Trace`
(shared trace/contraction plans), or `FixedFibers(options)` (also shared certified
phase resources). Reported and accepted probability interfaces use the selected
mode. Conditioned and decision branches share immutable plans keyed by ordered
physical event rows; outcomes, expression constants and faults may change.
Preparation from a perturbed branch uses its original program. Target-density
and Pauli-moment closures retain their documented preparation paths.

`max_prepared_probabilities` bounds retained query families (default 64). Exceeding
that bound or a term budget returns an error; there is no strategy fallback.
Fixed-resource preparation that fails its term budget remains retryable. One lock
serializes preparation across threads, and evaluation releases the lock first.
Plans own their programs; programs do not own the cache. `prepared_probability_statistics()`
reports retained query and fixed-resource counts. A task evaluator reports `None`
when cold-prefix execution has no persistent instrument cache.

```rust
use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{CompileOptions, ProbabilityPreparation, compile_affine};
use rmagna::scalar::ScalarArithmetic;
let arithmetic = ScalarArithmetic::default();
let source = parse_circuit("H 0\nM 0", &ParseOptions::default())?;
let options = CompileOptions {
    probability_preparation: ProbabilityPreparation::Trace,
    max_prepared_probabilities: 1,
    ..Default::default()
};
let program = compile_affine(&source, &options, &arithmetic)?;
let row = BitExpr::atom(BitAtom::Record(0));
for outcome in [BitVector::zeros(1)?, BitVector::from_indices(1, [0])?] {
    assert_eq!(program.reported_probability(&[row.clone()], &outcome, &[], 1, &arithmetic)?.value,
        arithmetic.omega(0, -2)?);
}
assert_eq!(program.prepared_probability_statistics()?.cached_queries, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`evaluate_task --probability-preparation cold|trace|fixed` records the actual mode,
cache limit and retained counts. Fixed mode accepts `--fiber-search-rank N`;
`--max-prepared-probabilities N` adjusts the family bound. Both differential task
drivers accept `--probability-preparation` and `--fiber-search-rank`, verify the
Rust configuration, and retain these settings in their receipts. Per-bit exact
marginals can create many distinct families; grouped affine-history proposals
usually use fewer. These options make no whole-runtime or formal throughput claim.

Bounded fixed-resource diagnostics with search rank 64 match all 26 affine
packaged tasks and both prescribed TQD3 histories against the pinned native
implementation. TQD3 retains 20 query/resource families, with two acceptance
terms and four target terms. These are exact no-fault correctness witnesses,
not the 31-task draw matrix or paper precision/performance results.

## Bounded warm replay diagnostics

`evaluate_task --warm-replays N` (zero through 16, default zero) repeats the same
assignment on one evaluator and decoder process. Entropy is reset to the declared
seed/constant word, or the prescribed history is reused. Every exact evaluation
field and controller transcript must match the first call; retained query/resource
counts must remain unchanged. A mismatch returns an error. Replay calls still
execute controllers and Gaussian queries; only preparation already owned by the
evaluator can be reused.

`query_ms` measures the first evaluation after history-file loading; `warm_replays`
contains separate call times. Exact comparisons and cache checks occur outside
each timer. Both differential drivers accept `--warm-replays` and verify its count,
checks and finite nonnegative times in the Rust output. The receipt marks these
as same-assignment diagnostics. They are separate from independent Monte Carlo
draws and the formal multi-process performance protocol.

Two warm replays match every exact field and retain the same cache counts on
all 26 affine tasks and both prescribed TQD3 histories. A deliberately stateful
decoder negative control changes its output on replay and is rejected.

`cargo test --locked -p rmagna --test warm_replay_cli` runs the diagnostic's
actual replay implementation with real Python decoder processes. It checks
process identity and call counts, exact irrational masses, seed/history reuse,
zero/maximum/invalid counts, both causal strategies and history samplers,
all probability preparation modes, and rejection of a stateful decoder whose
output changes on replay. The normal crate test command includes this test.

## Prepared reduced-density overlap

`DensityLayout` describes ordered outputs and fixed Z-event rows, with outcome
values supplied separately. `prepare_density_overlap` prepares projection and
retained-boundary Gaussian contraction for both original programs.
`prepare_density_overlap_with_fibers` also freezes a certified phase resource;
`fixed_phase_resource()` reports its actual rank and terms.

`PreparedDensityOverlap::evaluate(left, right, left_outcomes, right_outcomes,
max_terms, arithmetic)` accepts independent insertion views and event outcomes
on both sides. Preparing perturbed views still uses original references. An
unrelated program, changed layout shape or exhausted term budget is rejected.
Both environments are traced independently, output order is preserved, and
masses remain unnormalized. Zero reference events do not disable later nonzero
updates. The fixed resource precedes adaptive radical absorption and may be
larger than the adaptive result; remaining Gaussian sums are query-specific.

```rust
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::{DensityLayout, UnitaryCircuit, UnitaryGate};
let arithmetic = ScalarArithmetic::default();
let state = UnitaryCircuit::new(2, [
    UnitaryGate::Clifford(CliffordGate::H(0)),
    UnitaryGate::Clifford(CliffordGate::Cx { control: 0, target: 1 }),
])?.compile(&BitVector::zeros(2)?, &arithmetic)?;
let layout = DensityLayout { outputs: &[0], event_rows: &[] };
let plan = state.prepare_density_overlap(layout, &state, layout, &arithmetic)?;
let empty = BitVector::zeros(0)?;
assert_eq!(plan.evaluate(&state, &state, &empty, &empty, 1, &arithmetic)?.value,
    arithmetic.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Probability and density families share one internal scalar-closure implementation.
Literal reduced-density-matrix tests cover mixed and projected states, different
widths, output permutations, empty outputs, both compiler strategies, independent
faults, preparation from perturbed views and impossible reference events. The
cold `density_overlap` interface remains available. Instrument/task target-cache
integration is separate from this primitive.

## Bounded target-density preparation

`CompileOptions::density_preparation` independently selects `QueryPreparation::Cold`
(the default), `Trace`, or `FixedFibers(options)`. Target-density queries and the
target portion of `masses` share retained plans across compatible instrument
branches, target insertion views and event outcomes. Keys include both original
program identities, ordered output layouts and event rows. Different target
programs remain distinct even when their gates and dimensions agree.

`max_prepared_densities` bounds retained density families (default 16), and
`prepared_density_statistics()` reports their query/resource counts. Task/runtime
statistics return `None` for cold-prefix execution without a persistent reference.
`QueryPreparation` and `PreparedQueryStatistics` are shared names; the existing
`ProbabilityPreparation` and `PreparedProbabilityStatistics` remain aliases.

```rust
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::fibers::FiberOptions;
use rmagna::instrument::{CompileOptions, QueryPreparation, compile_affine, compile_unitary};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
let arithmetic = ScalarArithmetic::default();
let source = parse_circuit("H 0\nT 0", &ParseOptions::default())?;
let options = CompileOptions {
    density_preparation: QueryPreparation::FixedFibers(FiberOptions::default()),
    max_prepared_densities: 1,
    ..Default::default()
};
let program = compile_affine(&source, &options, &arithmetic)?;
let ideal = compile_unitary(&source, &options, &arithmetic)?;
for _ in 0..2 {
    assert_eq!(program.masses(&[0], &ideal, &[], 16, &arithmetic)?.target_success.value,
        ExactScalar::one());
}
assert_eq!(program.prepared_density_statistics()?.cached_queries, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`evaluate_task --density-preparation cold|trace|fixed` and
`--max-prepared-densities N` expose the separate density policy and bound. The
shared fiber-search option applies whenever either preparation mode is fixed.
Both differential drivers verify and retain the requested density setting. Warm
replays compare retained density counts as well as probability counts and every
exact evaluation field. Native instrument and weighted nonlinear-history tests
cover all combinations of probability and density preparation; target identity,
layout, fault/outcome reuse and explicit capacity errors have separate tests.

Bounded fixed probability+density pilots match all 26 affine tasks and both
prescribed TQD3 histories, including two same-evaluator warm replays. TQD3
retains 20 probability families and one density family, with two acceptance
terms and four target terms. These are correctness/reuse diagnostics, separate
from the complete draw matrix and formal performance protocol.

Exact categorical models prepare the product of positive identity probabilities
and each alternative's replacement ratio once. Prescribed sparse assignments
multiply only their replacement factors. Zero-identity categories are tracked as
forced choices and use their absolute alternative probabilities, so no division
by zero occurs. All supplied faults are validated before a zero mass is returned.

Noise sampling preserves its categorical choices, random-word order and stream
state, then obtains physical/proposal masses from the prepared products. Causal
resolution reuses the already validated physical draw mass when every potential
category remains active; general active subsets and public instrument fault
products use exact balanced multiplication. Identical physical and proposal
distributions reuse the exact mass and have unit likelihood without a second
product or rational division. No probability is rounded.
Exhaustive assignment products, large powers, tiny nonzero values and forced/
zero guarded-category endpoints are checked against independent exact formulas.


## Exact streaming paired statistics

`statistics::ExactStatistics` accepts a vector of already weighted real
cyclotomic contributions per draw, for example accepted mass, joint error mass
and signed joint Pauli moments. The separate diagnostic weight is the combined
noise and history likelihood; it is used for weight mean and ESS and is never
applied to the contributions again. Zero-weight draws remain in the count.

```rust
use rmagna::field::ExactField;
use rmagna::statistics::{ExactStatistics, StatisticsOptions};

let mut stats = ExactStatistics::new(2, StatisticsOptions::default())?;
stats.push(&[ExactField::one(), ExactField::zero()], &ExactField::one())?;
stats.push(&[ExactField::one(), ExactField::one()], &ExactField::one())?;
let summary = stats.summarize()?.expect("two draws");
assert_eq!(summary.count, 2);
let conditional = stats.paired_ratio(1, 0)?.expect("positive accepted sum");
assert!(conditional.variance.is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Summaries contain exact means, covariance of the mean, mean weight and
`effective_sample_size = (sum w)^2 / sum(w^2)`; ESS is zero when all weights are
zero. ESS describes weights and does not itself guarantee the precision of an
error estimate. `paired_ratio` divides aggregate means, rather than averaging
individual ratios, and uses their paired covariance for delta-method variance.
It supports any ordered numerator/denominator indices, including signed moments.
Interpreting covariance and ratio variance as uncertainty assumes independent
draws from the same intended sampling law with finite second moments. Dependence
and replicated rows are not checked, and no autocorrelation correction is
applied. In particular, same-assignment warm replays are not fresh independent
Monte Carlo observations. Exact arithmetic and weight ESS do not establish these
sampling assumptions.
No draws produce no summary; no draws or a zero aggregate denominator produce no
ratio. With one draw, covariance and ratio variance are absent. Standard errors
require a separately rounded square root of exact variances; no floating-point
conversion, clipping or square-root approximation occurs in this module.

`StatisticsOptions` bounds the quantity count (64 by default) and exact
coefficient arithmetic. Accumulators store a triangular second-moment matrix,
not individual draws: their number is quadratic in quantities and independent of
draw count, while integer sizes remain subject to the arithmetic budget.
`merge` combines nonoverlapping partitions of the same intended sampling law
and applies the receiver's budget; it checks neither overlap nor independence.
Updates and merges are atomic on invalid shapes, complex contributions, negative
weights, arithmetic limits and count overflow. Fully enumerated rational toys,
real cyclotomic examples, `1e-400` errors, signed moments, partition merges and
failure controls verify these contracts. Run/result serialization, kappa and
sampler selection remain separate interfaces.

## Exact common-scale noise proposals

`NoiseModel::scaled_proposal` multiplies every exclusive nonidentity alternative
by a common positive rational scale when the category's total error probability
lies strictly between zero and one. It preserves channel proportions, including
already expanded exclusive alternatives of correlated chains. It leaves total
error endpoints and explicitly excluded sites unchanged. A total error of one
can still have stochastic alternatives; those proportions also remain unchanged.
Every scaled total must stay strictly below one, preserving physical identity
support as well as nonidentity support.

```rust
use std::collections::BTreeSet;
use num_rational::BigRational;
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler};

let options = NoiseOptions::default();
let physical = NoiseModel::new(
    vec![vec!["9/10".parse()?, "1/15".parse()?, "1/30".parse()?]],
    &options,
)?;
let proposal = physical.scaled_proposal(
    &BigRational::from_integer(2.into()), &BTreeSet::new(), &options,
)?;
assert_eq!(proposal.summary().scaled_categories, 1);
let sampler = NoiseSampler::importance(physical, proposal.into_model(), options)?;
# let _ = sampler;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ScaledNoiseProposal` exposes the immutable model and exact configuration/count
summary. Counts of endpoint and excluded categories can overlap; scaled and
unchanged counts partition all categories. A scale of one keeps every category
unchanged and preserves the physical sampler's seeded draws and random-word
consumption. Malformed/raw nonfinite scales, nonpositive scales, invalid excluded
indices, support-destroying scaled totals and denominator limits fail during
preparation, before any entropy is drawn. Tiny nonzero alternatives stay exact.

This low-level constructor takes explicit exclusions; callers representing
causal guarded noise must exclude those categories. Automatic guard exclusion
and file/task sampling configuration are separate higher-level interfaces.
Arbitrary proposal models remain supported by `NoiseSampler::importance`.
The common-scale proposal does not implement experiment-specific mixtures or
residual-class distributions.

## Task and causal importance sampling

`TaskEvaluator::new_with_sampling` and `CausalProgram::new_with_sampling` accept
`noise::NoiseSampling::Physical` or `NoiseSampling::Importance { proposal_scale }`.
The original `new` constructors retain physical sampling and their seeded stream.
Noise proposals are separate from the copyable runtime options and history
sampler. Causal preparation automatically excludes guarded potential categories
from scaling; endpoint categories also remain unchanged. All scale, support and
denominator checks complete before sampling entropy. `scaled_proposal_summary`
returns the exact common-scale configuration and category counts, including unit
scales, and returns `None` for physical sampling.

```rust
use std::path::Path;
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};

let parse = ParseOptions::default();
let physical = parse_circuit("X_ERROR(1/10) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]", &parse)?;
let task = parse_task(r#"{"error_observables":[0]}"#, Path::new("."), &TaskOptions::default())?;
let modules = task.load_modules(&physical, &parse)?;
let evaluator = TaskEvaluator::new_with_sampling(
    &physical, &modules, &ControllerRegistry::default(), EvaluationOptions::default(),
    &NoiseSampling::Importance { proposal_scale: BigRational::from_integer(2.into()) },
)?;
let draw = evaluator.sample(&mut SplitMix64::new(7))?;
assert_eq!(evaluator.scaled_proposal_summary().unwrap().scaled_categories, 1);
# let _ = draw;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Task accepted/error/target/moment contributions already include the noise
likelihood exactly once and use the configured physical/history-proposal
normalization. The complete diagnostic weight for ESS is
`noise_likelihood_ratio * history_weight`. Weighted acceptance may exceed one on
individual draws; it is not clipped. Inactive guarded choices have no physical
action or active-path probability factor and carry unit noise likelihood because
their proposal is unchanged.

`ResolvedHistory::noise_likelihood_ratio` and `CausalDraw::noise_likelihood_ratio`
expose the noise factor independently of history mass and history proposal mass.
Low-level causal `masses` remain joint quantum masses, rather than already
weighted task contributions. Prescribed fault assignments and explicit prescribed
histories retain their conditional physical diagnostic semantics and unit noise
likelihood even on an importance-configured evaluator.

Exact finite entropy grids verify acceptance, error, target success, signed
moments, guard-dependent path probabilities and biased reported histories under
both causal strategies. Uniform and nonuniform history proposals, interference
zeros, scaled report flips that change guard activation, unit-scale streams and
invalid preparation controls are covered. File/result schemas and specialized
paper proposals are separate interfaces.

## Exact physical noise-sector enumeration

`enumeration::enumerate_noise` returns every positive-probability physical
assignment with at most `max_weight` selected nonidentity categories. A
multi-Pauli correlated alternative and a report flip each count as one category.
Potential guarded selections count before the higher-level runtime determines
whether their actions are active. Zero alternatives are omitted; a category
with zero identity mass must select a nonidentity alternative. This is noise
sector enumeration, separate from quantum accepted/error masses and integrated
decoder histories.

```rust
use rmagna::enumeration::{EnumerationOptions, enumerate_noise};
use rmagna::noise::{NoiseModel, NoiseOptions};

let model = NoiseModel::new(
    vec![vec!["9/10".parse()?, "1/10".parse()?]; 2],
    &NoiseOptions::default(),
)?;
let sector = enumerate_noise(&model, &EnumerationOptions::default())?;
assert_eq!(sector.patterns().len(), 3);
assert_eq!(sector.covered_probability().to_string(), "99/100");
assert_eq!(sector.omitted_probability().to_string(), "1/100");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Patterns contain sorted original site/alternative selections and exact physical
probabilities. Results also expose probabilities by weight, total covered mass
and its exact complement. Weight entries extend through
`min(max_weight, number of sites)`, including empty sectors. Ordering is stable:
increasing weight, lexicographic optional-site combinations, then alternative
choices in original site order. An empty model has one empty assignment of mass
one; a cutoff below the number of mandatory sites has no supported assignments.

The default limits are 100,000 patterns, 1,000,000 retained fault entries,
100,000 sites, 10,000,000 site/weight counting pairs and 1,048,576 probability
numerator/denominator bits. `max_weight` defaults to one. A positive pattern
limit is required. Counting work is `sites * (min(max_weight, sites) + 1)`;
pattern and fault-entry counts are checked with bounded dynamic counting before
any patterns are generated. Generation uses mandatory sites, optional-site
combinations and iterative alternative choices, avoiding circuit-depth recursion
and repeated scans over all identity sites for each sparse pattern. Exact
probability limits also apply during accumulation. Limit failures are errors;
no assignments are silently truncated, sampled or rounded.

## Exact conditional reported-group enumeration

`records::enumerate_reported_conditioned` enumerates every positive physical
outcome of an ordered reported-parity group, given earlier reported conditions.
It uses no entropy and excludes final postselection from its marginal queries.
Each branch contains complete label values and their joint physical mass with
the given conditions; the separate `condition_mass` is the sum of branch masses.
Divide by that mass only when a normalized conditional distribution is needed.

```rust
use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, compile_affine};
use rmagna::records::{RecordEnumerationOptions, RecordQuery, enumerate_reported_conditioned};
use rmagna::scalar::ScalarArithmetic;

let a = ScalarArithmetic::default();
let circuit = parse_circuit("H 0\nM 0", &ParseOptions::default())?;
let instrument = compile_affine(&circuit, &CompileOptions::default(), &a)?;
let parities = [BitExpr::atom(BitAtom::Record(0))];
let distribution = enumerate_reported_conditioned(
    &instrument, &RecordQuery { given: &[], parities: &parities }, &[],
    &RecordEnumerationOptions::default(), &a,
)?;
assert_eq!(distribution.branches.len(), 2);
assert_eq!(distribution.branches[0].mass, a.omega(0, -2)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The conditioned structural affine support identifies independent unresolved
labels. Exact joint prefix marginals then prune interference-zero branches;
structural points are never assumed to have positive physical probability.
`max_record_bits` bounds structural rank, rather than the number of repeated or
complemented labels. Defaults are 64 independent bits, 4,096 positive branches,
1,000,000 actual marginal queries and 256 contraction terms. Scalar arithmetic
limits also apply. A rank-zero or empty group needs one conditioning query and
returns its single deterministic group with the conditioning mass. Impossible
physical conditions are errors.

Traversal visits independent labels in pivot order and false before true,
then reconstructs all original labels. Complete results report structural rank
and actual query count. Branch-frontier and query limits fail explicitly, with
no silently incomplete distribution. A one-branch budget can therefore succeed
inside a larger structural support when all other physical branches cancel.
Tests compare an independent biased four-state distribution under all report
flips, both compilers and all probability preparation modes, and cover zero
interference, postselection separation, 65 correlated records/129 labels,
conditioned and empty groups, and exact limit boundaries. This group primitive
does not invoke external decoders; causal history traversal is a separate layer.

## Exact forward-causal history traversal

`CausalProgram::enumerate_prescribed_histories` enumerates every positive
physical history for one prescribed potential fault assignment. It expands
external decoder inputs, guarded-noise observations and optional requested
records in causal order using the exact reported-group primitive above.
Controllers see reported bits, including selected report flips. Their outputs
are applied before later observations. Final postselection never filters which
histories are visited. The result contains resolved instruments, active faults,
controller transcripts and exact joint history masses, with no entropy draws.

```rust
use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::runtime::{CausalProgram, ControllerRegistry, HistoryEnumerationOptions, RuntimeOptions};
use rmagna::scalar::ScalarArithmetic;

let a = ScalarArithmetic::default();
let registry = ControllerRegistry::default();
let circuit = parse_circuit("H 0\nM 0\nX_ERROR(1/3) rec[-1] 1", &ParseOptions::default())?;
let program = CausalProgram::new(&circuit, &registry, RuntimeOptions::default(), &a)?;
let result = program.enumerate_prescribed_histories(
    &[BitExpr::atom(BitAtom::Record(0))], &[],
    HistoryEnumerationOptions::default(), &a,
)?;
assert_eq!(result.histories.len(), 2);
for history in &result.histories {
    assert_eq!(history.history_mass(), &a.omega(0, -2)?);
    assert_eq!(history.entropy_words(), (0, 0));
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Default traversal limits are 4,096 retained complete/pending histories, 64
independent structural bits per unresolved observation group, 1,000,000 actual
physical marginal queries and 1,000,000 replay attempts. Existing per-execution
runtime, contraction-term and arithmetic limits also apply. Limits fail with
errors rather than incomplete results. Deterministic and empty groups each
have one outcome; interference zeros are pruned before positive branches count
against the frontier budget. No-history circuits return one mass-one history.

Traversal replays prefixes, so callbacks may execute repeatedly for identical
inputs. They must obey the existing deterministic, stateless controller
contract; an observed output change for the same call and inputs is an error.
The audit detects observed inconsistencies, rather than proving callback purity.
Returned query, replay and network-compilation counters include interrupted
prefixes. Network counts exclude initial reference preparation; fixed templates
need no additional circuit compilation, while the cold-prefix baseline records
all actual post-preparation constructions.

This low-level API is a conditional diagnostic: fault choices are validated even
when their physical probability is zero, and configured sampling proposals have
no effect. Noise likelihood is one and no history proposal mass is returned.
History masses exclude noise weights and final postselection. For exact sums
over potential noise patterns, multiply quantum response by each pattern's
full physical probability, including inactive guarded categories. The returned
`noise_probability` is an active-path diagnostic and must not replace that
potential-pattern probability. Task-level integration and rigorous sector
bounds are described below.

Tests compare majority feedback and biased non-Clifford/report-flip/guarded
histories against independent exact probabilities under both causal strategies.
They check postselection separation, repeated/complemented inputs, zero/forced
and inactive categories, original source mappings, empty histories, interference
zeros, callback failures and observed nondeterminism, proposal independence and
exact branch/rank/query/replay limit boundaries.

## Exact physical task-sector integration

`TaskEvaluator::enumerate_physical` combines categorical noise-sector enumeration
with every positive causal history and the task's existing quantum queries.
It returns exact unconditional covered acceptance, error, optional normalized
target success and signed output moments, plus totals for each categorical
weight. A correlated multi-Pauli alternative is one selected category. All
potential noise pattern probabilities are applied once, including inactive
guarded categories; conditional history normalization and sampling likelihoods
are absent. Projected or mixed partial targets retain their exact target norm.

```rust
use std::path::Path;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::FieldArithmetic;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task};

let parse = ParseOptions::default();
let physical = parse_circuit(
    "H 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]", &parse,
)?;
let task = parse_task(r#"{"error_observables":[0]}"#, Path::new("."), &TaskOptions::default())?;
let modules = task.load_modules(&physical, &parse)?;
let evaluator = TaskEvaluator::new(
    &physical, &modules, &ControllerRegistry::default(), EvaluationOptions::default(),
)?;
let result = evaluator.enumerate_physical(&TaskEnumerationOptions::default())?;
let f = FieldArithmetic::default();
assert_eq!(result.masses.error, f.from_rational("1/6".parse()?)?);
assert_eq!(result.covered_probability.to_string(), "1");
assert_eq!(result.histories, 4);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`TaskEnumerationOptions::noise` sets the noise cutoff and storage/work budgets;
its defaults are those of `EnumerationOptions`. `histories` sets traversal
limits, with complete-history, marginal-query and replay budgets applied globally
across all patterns. Its structural-bit cap applies to each unresolved group.
Each linear pattern counts as one history with no marginal queries or replays.
Counters include interrupted causal prefixes and exclude initial reference
preparation. Existing per-execution and exact arithmetic limits also apply.
No partial result is returned on failure. Importance-configured evaluators,
including unit-scale proposals, and nonempty task `sample_records` are rejected.

The covered responses are unconditional masses, never divided by covered noise
probability or accepted mass. Let their acceptance and error be `A` and `E`,
and omitted physical noise probability be `u`. The rigorous full-distribution
intervals are `[A, A+u]` and `[E, E+u]`. The tight conditional-error interval is
`[E/(A+u), (E+u)/(A+u)]`; it is undefined when `A+u` is zero. The bounds enforce
`0 <= E <= A <= covered_probability` with exact comparisons. Full coverage gives
point intervals whenever acceptance is positive. Returned signed moments are
covered masses; they do not have a conditional-error interpretation.

Tests compare every cutoff of an independent small categorical/Boolean oracle,
biased non-Clifford decoder histories, correlated alternatives rescued by report
flips, guard-dependent active/inactive/zero/forced choices, signed moments,
recovery and projected or entangled partial targets. They also verify exact
global resource boundaries and omitted-sector extremal/interior responses,
empty sectors, undefined zero-acceptance rates and arithmetic failures. Run-file
and result schemas and specialized paper proposals remain separate deliverables.

## Correctly rounded numeric exports

`numeric::approximate_real` and `approximate_sqrt` export exact real field values
without converting their individual coefficients to floating point first.
The latter rounds a nonnegative square root even when it is outside the exact
field, as needed for uncertainty magnitudes. Negative square roots and nonreal
inputs fail explicitly. These are export operations; exact simulation, sampling
and statistics retain their existing arithmetic.

```rust
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::FieldArithmetic;
use rmagna::numeric::{Binary64, NumericOptions, approximate_real};

let f = FieldArithmetic::default();
let tiny = f.from_rational(BigRational::new(1.into(), BigInt::from(10).pow(400)))?;
let exported = approximate_real(&tiny, NumericOptions::default())?;
assert_eq!(exported.binary64, Binary64::Underflow { negative: false });
assert_eq!(exported.scientific.exponent, -1329);
assert!(exported.scientific.mantissa >= 1.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`RoundedReal::scientific` represents `mantissa * 2^exponent` with a signed,
nearest-even binary64 mantissa whose magnitude is in `[1,2)`. Exact zero has
mantissa zero and exponent zero. `binary64` is rounded independently, so
subnormal output is not subject to double rounding through the scientific
mantissa. `Binary64::Finite` includes exact zero; `Underflow` identifies a
nonzero exact value rounded to signed zero, and `Overflow` identifies rounding
to signed infinity. Both nonfinite classes retain the sign. Undefined
statistical quantities remain optional at their owning layer.

The implementation compares `A+B*sqrt(2)` with exact dyadic candidates, uses
integer norm bounds to locate the exponent even under severe cancellation,
and compares square-root candidates by squaring them. Exact midpoint tests
implement ties to even at normal, subnormal and overflow boundaries. Defaults
allow 16,777,216 bits per intermediate integer, including squares. Checks use
conservative pre-operation bounds; insufficient budgets fail rather than
returning an uncertified approximation. This is a bit budget for individual
integers, not a total-memory or constant-time guarantee.

Tests include literal IEEE witnesses for both signs and halfway transitions,
direct-versus-scientific double-rounding controls, Pell cancellation, tiny and
huge scales, square roots and explicit input/budget failures. Eighty independent
Decimal witnesses agree at 1,200 and 2,400 decimal digits and cover both exported
representations and square roots. Regenerate them with:

```sh
python3 benchmarks/magna_reproduction/numeric_vectors.py --out rmagna/tests/data/numeric-reference.json
```

The finite Decimal witnesses supplement the exact midpoint and boundary tests;
they are reference calculations rather than a proof of all possible inputs.
File/result serialization is a separate interface.

## Exact history-sampler efficiency diagnostics

`statistics::HistoryEfficiency` accumulates the finite-data history efficiency
comparison separately from ordinary standard-error estimation. Supply already
weighted acceptance `A`, error `E`, complete noise/history weight `W`, and
noise-only likelihood `L` on each completed draw. The accumulator never applies
these weights to contributions a second time. A missing error metric is `None`,
not an invented zero error measurement.

```rust
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::statistics::{EfficiencyStatus, HistoryEfficiency};

let mut efficiency = HistoryEfficiency::new(FieldArithmetic::default())?;
for error in [ExactField::zero(), ExactField::one()] {
    efficiency.push(&ExactField::one(), Some(&error), &ExactField::one(), &ExactField::one())?;
}
let summary = efficiency.summarize()?;
assert_eq!(summary.status, EfficiencyStatus::Available);
assert_eq!(summary.kappa, Some(ExactField::one()));
# Ok::<(), Box<dyn std::error::Error>>(())
```

For `r = sum(E)/sum(A)`, the actual residual sum is
`U = sum((E-r*A)^2)`. Its exact-marginal reference is
`B = sum((L/W)*(E-r*A)^2)` over positive-weight draws. The summary exposes
`kappa = U/B`, `variance_coefficient = n*U/sum(A)^2` and
`exact_marginal_variance_coefficient = n*B/sum(A)^2`. These are finite-data
variance diagnostics, without the unbiased sample correction used for standard
errors. The comparison holds the noise proposal and terminal estimator fixed.
For exact-marginal histories `W=L`, so any estimable kappa is exactly one.

Weight ESS is `(sum(W))^2/sum(W^2)` and `rho` is `n/ESS`. All-zero weights give
ESS zero and undefined rho; no draws give undefined mean weight. Zero-weight
draws still increment the completed and zero-weight counters. Statuses distinguish
`no_error_metric`, `insufficient_samples`, `zero_acceptance`,
`zero_residual_variance` and `available`, in that precedence order. Unavailable
comparisons have no kappa or variance coefficients. Unlike an intermediate
floating-point diagnostic, this exact layer does not discard an estimable
comparison because its coefficients have extreme magnitude; numeric export
reports any ordinary binary64 range failure separately.

Inputs must be real with `0 <= E <= A`, nonnegative likelihoods, and positive
complete/noise likelihoods whenever acceptance is positive. Failed updates and
disjoint merges are atomic under the receiver's arithmetic budget. The number
of retained accumulators is independent of draw count; integer sizes can grow
and remain bounded by that budget. Tests use independent centered full-list
oracles with unequal noise and history weights, zero draws, irrational values,
scale changes of 10,000 square-root-two powers, disjoint partitions and explicit
unavailable, invalid, arithmetic and count-overflow cases. Run summaries are described below;
sampler-pilot decisions remain a separate interface.

## Seeded task batches and partial failures

`TaskEvaluator::run_batch` reuses a prepared evaluator and starts a fresh
`SplitMix64` stream at the requested seed. `task::run_task_batch` additionally
owns and measures preparation. Both consume the existing per-draw contributions
once and retain exact streaming estimates, paired conditional-error variance,
weight ESS and noise-corrected efficiency diagnostics.

```rust
use std::path::Path;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, RunStatus, TaskOptions, TaskRunOptions, parse_task, run_task_batch};

let parse = ParseOptions::default();
let physical = parse_circuit("X_ERROR(1/4) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]", &parse)?;
let task = parse_task(r#"{"error_observables":[0]}"#, Path::new("."), &TaskOptions::default())?;
let modules = task.load_modules(&physical, &parse)?;
let result = run_task_batch(
    &physical, &modules, &ControllerRegistry::default(), EvaluationOptions::default(),
    &NoiseSampling::Physical, &TaskRunOptions { shots: 16, seed: 7, ..Default::default() },
)?;
assert_eq!(result.status, RunStatus::Complete);
assert_eq!((result.completed, result.failed, result.not_attempted), (16, 0, 0));
assert_eq!(result.statistics.as_ref().unwrap().completed, 16);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run options default to one shot, seed zero, the existing statistics limits and
1,000,000 retained Boolean record values. Shot counts must be positive. Only
explicit task `sample_records` are retained per draw; their total requested
value count is checked before entropy or callbacks. The retention cap counts
values, not vector overhead or total process bytes. Without requested records,
the number of retained statistical accumulators is independent of shot count;
exact integer sizes can grow under the arithmetic budget. Repeated prepared
calls with the same seed reproduce the same contributions and records. The RNG
contract is the independent Rust stream, rather than upstream's stream.

`statistics` contains complete-run estimates only. Any ordinary preparation,
draw, accumulation or finalization failure makes status `Failed` and leaves
that field absent. Successfully summarized completed draws move to
`partial_statistics`, never masquerading as a complete estimate. A missing
error definition remains absent; zero acceptance has no conditional error rate.
Variances are variances of mean estimators, including the paired delta-method
conditional ratio, and are absent with fewer than two completed draws. Numeric
square roots and file serialization remain separate export operations.

Preparation failures have zero completed/failed draws and every requested draw
not attempted. A sampling or accumulation failure stops at one failed attempt;
its zero-based shot index is recorded, and that row enters neither statistics
nor retained records. Statistics and efficiency update together atomically.
Finalization can also exhaust exact arithmetic while summarizing already
completed rows. Such a failure has no failed-shot index and does not invent an
additional attempted draw; any unavailable provisional aggregate stays absent.
The accounting identity is always `requested = completed + failed + not_attempted`.

The wrapper records measured preparation time; reused preparation has no new
preparation measurement. Sampling time covers its draw/accumulation loop and
total time also covers finalization. `completed_network_compilations` and
`completed_history_groups` sum diagnostics from committed draws only; they do
not count work inside failed attempts, and observation groups are distinct
from individual marginal contractions. Actual per-call costs and loaded-content
fingerprints are described below; file-result schemas and CLI integration follow
separately.

Tests compare fixed-seed batches with independent centered full-list statistics
under physical/importance noise, exact/affine histories and both causal
strategies. They cover signed moments, paired ratios, repeated prepared calls,
owned preparation, bounded record retention, callback/contraction/arithmetic
failures, atomic statistics/efficiency alignment, zero/one completed draw,
undefined metrics and finalization failure accounting.

## Numerical batch-result payloads

`task::export_batch_metrics` turns a typed batch result into JSON-compatible
estimates, standard errors, scientific twins, weight/efficiency diagnostics and
explicit count/failure/timing fields. It exports the numerical payload used by
a file-result layer; it does not claim a complete versioned result document.
It includes actual execution costs. File execution metadata and loaded-content
fingerprints are separate responsibilities. No `schema_version` is attached to
this payload.

```rust
use std::path::Path;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, TaskOptions, TaskRunOptions, export_batch_metrics, parse_task, run_task_batch};

let parse = ParseOptions::default();
let physical = parse_circuit("I 0", &parse)?;
let modules = parse_task("{}", Path::new("."), &TaskOptions::default())?.load_modules(&physical, &parse)?;
let result = run_task_batch(
    &physical, &modules, &ControllerRegistry::default(), EvaluationOptions::default(),
    &NoiseSampling::Physical, &TaskRunOptions::default(),
)?;
let payload = export_batch_metrics(&result, NumericOptions::default())?;
assert_eq!(payload["estimates"]["acceptance"], 1.0);
assert!(payload["estimates"]["error"].is_null()); // No declared error metric.
# Ok::<(), Box<dyn std::error::Error>>(())
```

Numerical names follow native schema-2 conventions: `estimates`,
`standard_errors`, `estimates_scientific`, `standard_errors_scientific`,
`weights`, `efficiency` and `numerics`. Scientific objects are
`{"mantissa": m, "exponent2": e}` with nonzero mantissa magnitude in `[0.5,1)`;
zero has exponent zero. Ordinary values are rounded independently. Standard
errors are correctly rounded square roots of exact mean variances, including
the paired conditional ratio. Undefined fields are null without range flags.
Exact zeros stay zero. A nonzero value rounded to zero or infinity has a null
ordinary field, a retained scientific twin, and its full path listed in
`numerics.underflow_fields` or `overflow_fields`.

An estimable kappa outside binary64 range has
`efficiency.status = "diagnostic_range_exceeded"` and retains
`kappa_scientific`. The underlying exact efficiency summary is unchanged.
Weights always describe complete noise/history likelihoods; they are diagnostic
and are not applied to estimates again. Failed runs have null main estimate
objects and a separate `partial_statistics` payload when summarization was
possible. Range paths then identify the actual provisional location, such as
`partial_statistics.estimates.acceptance`.

The typed batch phases `accumulation` and `finalization` remain explicit in this
payload alongside `preparation` and `sampling`. Counters under
`completed_resources` describe committed rows, rather than claiming all work of
failed attempts. Export validates count/shape/timing consistency and fails with
a field-specific `BatchExportError` when a numeric budget or real square-root
requirement is violated. It returns no partly exported object or NaN/infinity.

Tests check literal binary statistics and standard errors, signed moments,
`1e-400`/`1e400` values and variances, the direct-rounding subnormal control,
undefined-versus-zero distinctions, extreme kappa, failed/provisional placement,
finalization accounting, malformed public state and JSON round trips.

The exporter also rejects unexplained missing completed-prefix summaries,
ragged retained rows, uncertainty availability inconsistent with sample count,
and contradictory efficiency status or duplicated weight summaries. Each missing
accumulated summary requires its own finalization failure; a finalization-only
failure follows all requested draws. Total time must be at least each measured
component (components are compared separately to avoid addition rounding).
The crate enables serde_json's `float_roundtrip` feature so its supported JSON
writer/reader path preserves binary64 bits, including scientific mantissas
immediately below one. This does not prescribe other JSON consumers' parsers.

The zero-weight count must also agree with whether the completed nonnegative
weights have zero mean and zero ESS. All-zero weights have no rho or available
variance-efficiency coefficients, and all accumulated contributions vanish.
Genuine interference-zero affine proposals preserve these zero/undefined
distinctions. Both completed-resource counters must be zero when no draw commits;
preparation and interrupted attempts belong to separate work accounting.

## Loaded-content fingerprints

`task::fingerprint_content` identifies seven independently hashed categories:
physical circuit, validated task, ideal state, recovery, controller declarations,
resolved options and noise sampling. It uses loaded snapshots and never opens
files or executes callbacks. Circuit identities include original text, format
and effective width; this preserves exact probability spelling and distinguishes
padded module views. Comments and whitespace therefore affect circuit identity.
Target identities also retain ordered output mappings and auxiliary projections.
Missing modules hash a domain-specific null, distinct from an empty module.

```rust
use std::{collections::BTreeMap, path::Path};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, FingerprintInputs, FingerprintOptions,
    TaskOptions, TaskRunOptions, fingerprint_content, parse_task};

let physical = parse_circuit("I 0", &ParseOptions::default())?;
let modules = parse_task("{}", Path::new("."), &TaskOptions::default())?
    .load_modules(&physical, &ParseOptions::default())?;
let fingerprints = fingerprint_content(&FingerprintInputs {
    physical: &physical, modules: &modules, controllers: &ControllerRegistry::default(),
    controller_identities: &BTreeMap::new(), evaluation: EvaluationOptions::default(),
    run: TaskRunOptions::default(), sampling: &NoiseSampling::Physical,
    numeric: NumericOptions::default(),
}, FingerprintOptions::default())?;
assert_eq!(fingerprints.to_json()["algorithm"], "sha256");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Encoding `rmagna-provenance-v1` hashes its UTF-8 name, a zero byte, the category
name, a zero byte and canonical UTF-8 JSON. Keys sort lexicographically at every
level, arrays retain semantic order, strings use JSON escapes with lowercase
hex digits and non-ASCII UTF-8, and numbers use serde_json's Display form.
This local encoding does not mathematically normalize JSON numbers or implement
RFC 8785. `canonical_fingerprint` exposes the same encoding in the `json` domain.
The manifest records both algorithm and encoding; digests use lowercase hex.

Options include all evaluation/compiler/history/cache/arithmetic limits, batch
shots and seed, statistics/record-retention limits and numerical export budget.
Parsing budgets are excluded because inputs are already loaded and validated.
Noise sampling has a separate identity: physical and unit-scale importance
remain distinct. Exact proposal numerator/denominator strings are retained
without rounding; this records the supplied configuration and does not validate
its executability. RNG/build/software metadata belongs to the result layer.

Controller names and versions, plus optional per-binding JSON identity material,
are explicitly `caller_declared_unverified`. Unknown binding names fail. Equal
declarations can describe different Rust closures: closure source cannot be
introspected. External worker manifests supplied here are also declarations,
and their contents are not independently attested by hashing them.

`FingerprintOptions` defaults to 16 MiB of encoded JSON per category and depth
64 (root depth zero). Domain prefixes are excluded from the byte budget. Hash
encoding is streamed; object-key sorting and loaded/constructed JSON still use
memory, so these are encoding limits rather than process-memory caps. Any
category exceeding either limit fails without returning a partial set. Paths
and source-location metadata are excluded, and modifying files after module
loading cannot alter those snapshots' identities.

Tests retain independent Python hashlib witnesses, UTF-8/control escapes,
arbitrary-width numbers and masks, nested key and semantic array order, exact
byte/depth boundaries, loaded/reloaded module snapshots, path independence,
projected targets, every current resolved option, caller declarations and tiny
exact proposal scales.

The Value entry fingerprints numbers as supplied in serde_json's Display form.
For numeric text that must retain its exact spelling, use
canonical_raw_fingerprint with a checked serde_json RawValue. It sorts object
keys, keeps the last duplicate key, normalizes string escapes and retains number
tokens verbatim, including arbitrary-width integers and large exponents.
Numbers are not converted to binary64. This entry uses the same json domain and
encoding; converting a Value with serde_json to_raw_value gives the same digest.
Already-parsed Value numbers cannot recover precision lost before the call.
The crate retains raw_value and float_roundtrip without enabling a global
arbitrary_precision dependency feature. Byte/depth limits bound encoded output;
raw JSON decoding, object sorting and caller-owned values still allocate memory.

## Actual execution costs

`costs::ExecutionMeter` collects attempted, successful and failed operations and
their wall times. `TaskEvaluator::new_with_costs` records preparation;
`sample_with_costs` records one draw. Existing constructors and `sample` retain
their semantics without collecting costs. Each `run_batch` owns a fresh collector;
`run_task_batch` uses one collector across its owned preparation and draw loop.
Snapshots are stored in `TaskRunResult::execution_costs` and exported as
`execution_costs` with `scope = "this_call_including_failed_attempts"`.

```rust
use std::path::Path;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};

let physical = parse_circuit("I 0", &ParseOptions::default())?;
let modules = parse_task("{}", Path::new("."), &TaskOptions::default())?
    .load_modules(&physical, &ParseOptions::default())?;
let meter = ExecutionMeter::default();
let evaluator = TaskEvaluator::new_with_costs(&physical, &modules,
    &ControllerRegistry::default(), EvaluationOptions::default(),
    &NoiseSampling::Physical, &meter)?;
evaluator.sample_with_costs(&mut SplitMix64::new(7), &meter)?;
assert_eq!(meter.snapshot().compilation.succeeded, 1);
assert_eq!(meter.snapshot().query.succeeded, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The four categories have explicit logical boundaries:

- `compilation` counts circuit program constructions: target preparation,
  protocol reference and cold causal prefixes/final instruments. It does not
  count every prepared query-family plan or stabilizer decomposition.
- `query` counts physical marginal, acceptance, target-density, classical
  accepted-success and Pauli-moment invocations, plus target normalization when
  preparation is owned. Empty and deterministic logical queries still count.
  Cache hits count because they still evaluate the requested network; cold
  family planning inside a query is included in its elapsed time.
- `decoder` counts registry evaluations, including empty-input calls, callback
  errors and incorrect output shapes.
- `support` counts conditioned structural affine-support constructions used
  by the affine history proposal. It excludes entropy generation and proposal
  arithmetic after support construction.

Timings are inclusive wall seconds. They include called code within each
boundary; nested manually measured operations are permitted, so categories are
not a partition to sum into total runtime. Sampling overhead, noise draws,
exact accumulation and finalization are outside these four categories and
remain in their existing batch timing scopes. First-query family preparation
is not retroactively attributed to constructor preparation time.

Counts survive returned errors and unwinding panics; an exhausted u64 attempt
counter fails before invoking the operation. A query returning exact zero is a
successful query, even if a later validation or accumulator rejects the draw.
Snapshots taken during an active operation can have unfinished attempts;
finished batch snapshots balance attempted = succeeded + failed, which JSON
export validates along with finite nonnegative times.

Collectors are passed explicitly and retain no references in compiled objects.
There is no global or thread-local state and no subtraction of shared lifetime
counters. Concurrent and repeated runs over one prepared evaluator have their
own counts, and reused batches exclude earlier preparation. Existing
`completed_resources` still counts committed rows only; actual costs also retain
the work of a failed draw or preparation.

Tests independently count both causal strategies and history samplers, duplicate
record groups, empty-input callbacks and their failures, cached/concurrent
target/moment queries, target normalization, compilation/contraction failures,
noise failures before a query and accumulator failures after successful work.
Identical seeded draws and final RNG states verify observational equivalence.

## Per-run query and time budgets

`TaskRunOptions::execution_limits` configures `costs::ExecutionLimits`: maximum
logical query attempts (`u64::MAX` by default) and an optional nonnegative
`Duration` (`None` by default). The same limits can be supplied directly to
`ExecutionMeter::new` for individually profiled operations. They apply to this
collector's lifetime; each batch creates a fresh collector.

```rust
use std::path::Path;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, RunStatus, TaskEvaluator, TaskOptions,
    TaskRunOptions, parse_task};

let physical = parse_circuit("I 0", &ParseOptions::default())?;
let modules = parse_task("{}", Path::new("."), &TaskOptions::default())?
    .load_modules(&physical, &ParseOptions::default())?;
let evaluator = TaskEvaluator::new(&physical, &modules,
    &ControllerRegistry::default(), EvaluationOptions::default())?;
let result = evaluator.run_batch(&TaskRunOptions {
    shots: 3, execution_limits: ExecutionLimits { max_queries: 2, ..Default::default() },
    ..Default::default()
})?;
assert_eq!(result.status, RunStatus::Failed);
assert_eq!(result.completed, 2);
assert_eq!(result.execution_costs.query.attempted, 2);
assert_eq!(result.execution_costs.query.failed, 0); // Third query never began.
# Ok::<(), Box<dyn std::error::Error>>(())
```

The query quota includes every logical query described above: target
normalization during owned preparation, conditional history marginals,
acceptance, classical success, target overlap and moments. A query rejected
before starting consumes no attempt and creates no fictitious kernel failure.
Its enclosing run still fails with an explicit `max_queries` reason, preserves
the completed prefix and leaves main estimates null. A failed actual query
consumes one query attempt. Owned preparation and sampling share one quota;
reused-evaluator batches exclude previous preparation and start fresh.

Deadlines are cooperative: check elapsed wall time before each measured
operation and, for batches, before each draw's entropy and callbacks. At or past
the exact duration boundary, the next operation is rejected. Zero duration is
an immediate exhaustion control. An in-progress operation, unmetered setup,
accumulation or finalization is not interrupted and can exceed the duration;
the limit is not a hard process timeout. Individually profiled samples check
measured operation boundaries; their caller owns any pre-draw check through
`ExecutionMeter::check_time`.

`TaskRunResult::execution_limits` and exported `execution_limits` record the
applied quota and exact `max_duration` as `{"seconds": u64, "nanoseconds": u32}`
or null. Fingerprints include both values without converting duration through
binary64, so even nanosecond differences at large second counts remain distinct.
Export rejects counters exceeding the declared quota. Concurrent and repeated
batches share prepared query caches while retaining separate quotas/deadlines.

Source compatibility: full struct literals for `TaskRunOptions` and
`TaskRunResult` must supply `execution_limits`; callers using
`..Default::default()` for run options keep the unbounded defaults. `CostError`
is now an enum: construct or match `CounterOverflow { operation }`,
`QueryLimit { limit }` and `TimeLimit { limit }` instead of the previous
`CostError { operation }` struct.

Tests check exact N and N-1 query boundaries, partial prefixes, target
normalization, both causal strategies and history samplers, profiled seeded
equivalence, fresh repeated/concurrent quotas, zero-duration failures before
work, nested quota exhaustion, exact controlled deadline boundaries and
nanosecond-preserving fingerprints. Deadline tests do not sleep.

## Prepared Pauli insertion catalogs

`CompiledUnitary::prepare_insertions` prepares an explicit catalog once against
its immutable original program. `with_prepared_insertions` takes catalog indices
and combines their pretransported Clifford frames without walking the gate
sequence again. Shared-layer observation signs are prepared as binary effects,
and fixed-input observation reduction still applies. Different circuit positions
are processed in time order; entries
at one position retain their supplied order, and repeated indices are allowed.
Every call replaces the assignment relative to the original program, even when
called on a perturbed view. A catalog cannot be used with a separately compiled
program, including an otherwise identical circuit.

```rust
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{InsertionPreparationOptions, PauliInsertion, UnitaryCircuit,
    UnitaryGate};

let arithmetic = ScalarArithmetic::default();
let base = UnitaryCircuit::new(1, [UnitaryGate::Clifford(CliffordGate::H(0))])?
    .compile(&BitVector::zeros(1)?, &arithmetic)?;
let catalog = [PauliInsertion {
    before_gate: 0,
    word: PauliWord::new(BitVector::from_indices(1, [0])?, BitVector::zeros(1)?, 0)?,
}];
let prepared = base.prepare_insertions(&catalog, InsertionPreparationOptions::default())?;
let changed = base.with_prepared_insertions(&prepared, &[0], &arithmetic)?;
assert_eq!(changed.amplitude(&BitVector::from_indices(1, [0])?, 1, &arithmetic)?.value,
    arithmetic.omega(0, -1)?.negated());
let restored = changed.with_prepared_insertions(&prepared, &[], &arithmetic)?;
assert_eq!(restored.overlap(&base, 1, &arithmetic)?.value, ExactScalar::one());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Preparation scans the Clifford gates backward, maintains exact images of input
X/Z generators, and captures each catalog word's suffix image. Phases remain
modulo eight, including non-Hermitian words and same-position anticommutation.
The direct `with_insertions` route remains available. An empty assignment on
either route now shares the immutable no-fault query reference directly while
retaining its coefficient-budget check.

`InsertionPreparationOptions` defaults to one million catalog entries and a
64 MiB conservative preparation estimate. Limits are checked before allocating
the suffix-image table or cloning catalog entries. `estimated_peak_bytes`
includes packed frame/sign masks, suffix-image tables, entry metadata,
sorting/lookup indices and temporary
word allowance; it excludes the existing compiled program, caller-owned catalog
and allocator rounding, and is not a process-memory cap. An empty catalog has
zero table footprint. Applying a selection uses memory proportional to that
selection and is outside the preparation estimate. Exceeded limits, invalid
positions/mask widths, invalid selection indices and incompatible programs are
explicit errors. `UnitaryError` gains `InvalidPreparedInsertion` and
`InsertionResourceLimit`; exhaustive matches must handle these new variants.

Tests compare every
three-qubit Pauli at every position to dense exact amplitudes, compare reverse
suffix images to forward conjugation as phased operators, and retain mixed
T/CS/CCZ, both compiler strategies, repeated/noncommuting insertions, original
assignment replacement, 129-wire masks, exact resource boundaries and the
zero-wire scalar vacuum.

For shared-layer compilation, each catalog entry additionally stores its exact
effect on the selected non-Clifford observation signs. A reverse binary Clifford
pass prepares those masks; assignment combines them by XOR and rebuilds the phase
from fixed observation coordinates, with no scan of the original gate sequence.
Global eighth-root phases remain in the transported Pauli frame. Insertions at
or before the reference snapshot cut have zero later sign effect because their
signs already reside in the copied observations. They still change the boundary
through their full Clifford frame, so that cut rule does not drop their action.

Sign-generator tables, retained masks and scratch are included in the same
preparation-byte preflight. Empty catalogs and programs without selected shared
observations remain valid. Additional tests compare reverse sign effects with
forward physical Pauli propagation across 129 wires and 130 observations, and
compare mixed-phase assignments at every boundary against dense amplitudes and
fixed-resource probability queries. They include late snapshot cuts, shuffled
catalog order, equal lowered gate boundaries and exact shared-table byte limits.

## Instrument insertion preparation

`instrument::CompileOptions::insertion_preparation` defaults to
`InsertionPreparation::Direct`. Selecting
`InsertionPreparation::Prepared(InsertionPreparationOptions::default())` builds
one immutable catalog containing every physical noise alternative and every
controller decision's virtual-input X. It checks the combined entry and byte
limits before constructing the temporary input catalog. The table estimate has
the scope described above; the compiler's temporary input catalog is excluded,
as are existing circuit metadata and allocator overhead.

Fault assignments and resolved decision bits select indices into that catalog.
Filtering inactive guarded categories preserves their original indices, even
when active site numbers change. Fixed-template branches share the original
catalog through `Arc`; they do not rebuild it for each history. Cold-prefix
execution prepares a separate catalog for each compiled prefix and final branch.
Those catalogs are ephemeral, so its top-level
`prepared_insertion_statistics()` returns `None`. Linear and fixed-template
evaluators return the retained reference catalog's entry count and estimated
peak table bytes, including an explicit zero-entry, zero-byte empty catalog.

`evaluate_task` and both task comparison drivers accept
`--insertion-preparation direct|prepared`, `--max-insertion-table-bytes N` and
`--max-insertion-catalog-entries N`. Custom limits require prepared mode. The
example emits the selected mode and effective limits in `configuration`, and
retained catalog diagnostics in `prepared_insertions`. The comparison drivers
validate those fields before accepting a result. All modes retain the same
exact fault/history semantics and seeded random stream; choosing preparation
does not change the noise proposal or history sampler.

Tests compare both propagation modes against native affine and causal mass
witnesses, full seeded task rows and RNG state. They also cover correlated
Pauli/report categories, inactive guards, multiple controllers, projected
targets, recovery, signed moments, importance sampling, concurrent use and exact
catalog resource boundaries. Task-scale timing remains a separate measurement.

Source compatibility: exhaustive `instrument::CompileOptions` literals must
provide `insertion_preparation` or use `..Default::default()`, and exhaustive
`InstrumentError` matches must handle `PreparedInsertionCatalogUnavailable`.
The options fingerprint now records the insertion mode and, for prepared mode,
both limits. Even direct-mode options digests differ from earlier versions
because the resolved configuration now includes this field; circuit and module
content digests retain their existing definitions.

## Fixed-query phase coordinate maps

Fixed-fiber probability and density families prepare the coordinate restriction
of each reference phase leg once. Warm queries combine updated coefficients,
conjugations and offset signs directly on the retained coordinates. They avoid
constructing a full-width composed phase and retransposing the same support
columns. The frozen fiber partition and its resource counts remain unchanged;
the cold and trace-only query routes retain their existing independent closure.

The immutable maps cache reference axes only. A term hidden by reference
cancellation can reappear after a Pauli update; such an axis is mapped exactly
through the prepared coordinate rows without growing a warm cache. Zero images,
dependent columns, scalar legs and global eighth-root constants are retained.
These tables belong to query preparation, outside the instrument insertion-table
footprint; neither footprint is a process-memory cap.

Independent truth tables cover 130 input and retained coordinates, multiple
conjugated legs, shifted offsets, dependent columns, cancelled axes and scalar
vacua. Mixed T/CS/CCZ inverse pairs with faults between the pairs are compared
against dense amplitudes, probability sums and reduced-density matrices. Existing
native masses and history fixtures also exercise the fixed query route.

## Owned file tasks and versioned reports

`file_api::FileTask` owns a parsed circuit, loaded task/target/recovery snapshots,
a captured controller registry, declared controller identities and execution
configuration. `FileTask::load` bounds circuit and task reads with
`FileLoadOptions`, resolves relative module paths once and validates UTF-8.
`from_loaded` accepts existing parsed snapshots. Later source-file changes,
deleted module files and additions to the caller's registry do not alter the
captured task. Cloning a `FileTask` shares that immutable snapshot.

Snapshot admission validates captured content and controller identities without
assuming a future run. Statistics, record-retention and run-dependent fingerprint
budgets are checked by `fingerprints`, `inspect` and `run` against the supplied
`TaskRunOptions`; loading does not impose their hypothetical default values.

```rust
use std::{collections::BTreeMap, path::Path};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{TaskOptions, TaskRunOptions, parse_task};

let parse = ParseOptions::default();
let circuit = parse_circuit("H 0", &parse)?;
let modules = parse_task(r#"{"observables":[[1,0]]}"#, Path::new("."),
    &TaskOptions::default())?.load_modules(&circuit, &parse)?;
let task = FileTask::from_loaded(circuit, modules, &ControllerRegistry::default(),
    NoiseSampling::Physical, FileTaskOptions::default(), BTreeMap::new())?;
let run = TaskRunOptions { shots: 8, seed: 19, ..Default::default() };
assert_eq!(task.inspect(&run)?["route"], "not_compiled");
let compiled = task.compile()?;
let report = compiled.run(&run)?;
let json = report.to_json()?;
assert_eq!(json["schema_version"], 2);
assert_eq!(json["software"]["name"], "rmagna");
assert_eq!(json["estimates"]["moments"], serde_json::json!([1.0]));
assert_eq!(report.raw_result().completed, 8);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`FileTask::inspect(&run)` validates the proposed run and fingerprints without
quantum queries, controller callbacks or entropy. `compile()` constructs a
`CompiledFileTask`; `compile_with_limits` additionally bounds that preparation.
`CompiledFileTask::inspect(&run)` describes its actual route and retained caches
without drawing. Controller callbacks retain the existing deterministic/stateless
contract. Optional identity JSON is a caller declaration, not a code attestation;
unknown binding names and fingerprint byte/depth limits fail explicitly.

`FileTask::run` owns fresh preparation and shares the run's deadline/query quota
with it. Ordinary preparation failures return failed reports with zero attempted
draws. `CompiledFileTask::run` reuses preparation while creating a fresh RNG,
accumulators, deadline and operation collector for each call. Concurrent calls
share caches and preserve separate run state. Invalid run configuration and
fingerprint failures reject before drawing or invoking callbacks.

`FileRunReport::to_json` builds a schema-2 envelope around the existing exact
batch metrics. It records the independent `rmagna` software identity, execution
route, sampling/evaluation meanings, actual configuration and limits, all seven
content fingerprints, compilation diagnostics, cache counts and actual per-call
operation costs. It retains scientific-number twins, conditional ratio and paired
uncertainty semantics, requested records and failure/partial-statistic placement.
No complete-run estimates are emitted for a failed batch. A numerical export
error leaves the typed batch available through `raw_result()` or
`into_raw_result()`; a metadata snapshot error similarly owns the completed batch
in `FileApiError::Snapshot`.

Compilation metadata is also mirrored under `resources.compilation` in a run
report. `preparation_included_in_this_call` distinguishes fresh preparation from
a previously compiled task. Captured preparation costs remain separate from
`resources.queries`/`compilations` and `execution_costs`, which count actual work
of this call, including failed attempts. Run `timings` measure the engine call,
excluding file loading, fingerprinting and JSON export. Preparation is included
only when owned. Query/decoder/support/compiler component times are inclusive and
must not be added together; the report marks this explicitly.

The current `query_preparation: on_demand` declaration is intentional. Cold query
modes execute directly; trace/fixed modes create families as requested. Cache
metadata describes only retained families, with null reference-cache receipts
for the cold-prefix causal strategy. It does not claim exhaustive preparation of
all task query families. Full eager preparation, a command-line interface, Python
bindings and the upstream compatibility ledger remain separate deliverables.

Tests compare whole reports with exact batches and analytic projected-target,
recovery and signed-moment values. They cover both causal strategies/history
samplers, physical/importance noise, guarded categories, record retention,
concurrent reuse, snapshot/path identities, exact input byte limits, malformed
UTF-8/modules, missing bindings, partial decoder failure, zero-duration
preparation and retained typed results after numerical export failure.
