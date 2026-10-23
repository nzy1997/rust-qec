# Near-Clifford simulation: Rust usage guide

The experimental `rstim::near_clifford` module simulates supported circuits with
Clifford gates, coherent `T`/`T_DAG` and axis rotations, measurements, Pauli noise,
classical feedback, and detector/observable annotations. The backend is already
implemented and opt-in. It does not replace the pure-Clifford `Executor` or the
ordinary stabilizer sampling APIs. The usual `rstim sample` / `rstim detect` CLI
commands do not select it automatically when they encounter a non-Clifford gate.
Use the Rust APIs below explicitly.

The active-state representation retains coherent amplitudes, not a classical
mixture. Its cost depends on active rank, not just physical qubit count or T count.
This is a CPU backend with resource limits; support for a gate is not a promise
that an arbitrary circuit containing that gate will fit in memory.

## Choose an execution API

| API | Use it for | Outputs |
| --- | --- | --- |
| `NearCliffordExecutor` | Interpreted/adaptive execution, one-shot `run`, reusable sampling | Structured shots or flat measurement bytes |
| `CompiledNearCliffordExecutor` | An offline Clifford-frame plan reused across batches; compiled-only gate support and native counts | Structured shots, flat measurement bytes, or postselected counts |
| `CompiledNearCliffordEagerSampler` | Explicitly opt in to early raw-detector rejection, with a different error/cache contract | Postselected counts; full records through the original executor |
| `ActiveState` | Low-level state/frame inspection and controlled experiments | Explicit state operations; not a circuit-sampling replacement |

Compile once, then retain the mutable sampler for repeated calls. Compilation,
sampler preparation, the first sampling call and later calls have different
costs. `plan.sample(...)` is convenient for a single batch but prepares a sampler
on each invocation. Each concurrent worker should have its own sampler and RNG.
The compiled sampler borrows its plan, so keep the plan alive while using it.

This guide describes the source-checkout API. Experimental APIs in a previously
published crates.io version may differ. For a small consumer project next to a
checkout named `rust-qec`, use:

```toml
[dependencies]
rstim = { path = "../rust-qec/rstim", default-features = false }
rand = "0.8"
```

Adjust the path to your checkout. No optional `rstim` feature is required for this
module. Use the Rust toolchain required by that checkout's workspace `Cargo.toml`.
Each Rust block below is a standalone program; copy one into `src/main.rs` and run
`cargo run --release`. All examples use `StdRng` from rand 0.8.

## Start with the compiled executor

`compile_text` defaults to Strict arithmetic. Structured shots contain raw
measurement bits, raw detector parities, and observable include events. Flat
output stores only the measurement bits, in shot-major order as bytes 0 or 1.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};

fn main() -> Result<(), String> {
    let text = "H 0\nT 0\nMX 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text)?;
    assert_eq!(
        plan.rotation_arithmetic(),
        CompiledRotationArithmetic::Strict
    );
    println!("compiled peak active rank: {}", plan.peak_active_rank());

    let mut sampler = plan.prepare_sampler()?;
    let mut rng = StdRng::seed_from_u64(42);
    let shots = sampler.sample(8, &mut rng)?;
    for shot in &shots {
        assert_eq!(shot.measurements.len(), 1);
        assert_eq!(shot.detectors, shot.measurements);
        assert_eq!(shot.observables, vec![(0, shot.measurements[0])]);
    }

    // A later call continues the RNG stream; it is a new batch, not a conversion
    // of the structured shots above.
    let batch = sampler.sample_measurements_u8(65, &mut rng)?;
    assert_eq!(batch.shots, 65);
    assert_eq!(batch.measurements_per_shot, 1);
    assert_eq!(batch.measurements.len(), 65);
    assert!(batch.measurements.iter().all(|&bit| bit <= 1));
    println!("first flat record: {}", batch.measurements[0]);
    Ok(())
}
```

For width `w = batch.measurements_per_shot`, shot `s` occupies
`measurements[s*w..(s+1)*w]`. A circuit with no measurement records has width 0
and an empty byte vector even when `batch.shots` is nonzero; do not call
`chunks(0)`. Flat output omits detectors and observables, rather than returning
empty per-shot annotation objects. Use structured output when those records are
needed, or native counts when only the aggregate below is needed.

## Choose Strict or Fused arithmetic

The policy is selected at compilation and fixed for that plan and every sampler
prepared from it. It applies to coherent rotations in the deterministic prefix,
scalar execution and packet execution. It is not a gate-set, precision, cache or
postselection mode: both policies use FP64.

| Policy | Rotation arithmetic | Reproducibility consequence |
| --- | --- | --- |
| `CompiledRotationArithmetic::Strict` (default) | Separately rounded multiply/add operations | Retains the strict rotation evaluation order |
| `CompiledRotationArithmetic::Fused` (opt-in) | Explicit `mul_add` for the partner product and sum | Coefficient/CDF bits and sampled records can differ from Strict |

Fused does not switch probability-reduction or projection arithmetic to a new
policy. It is not a guaranteed speedup. Do not assert bitwise equality of
Strict/Fused results for the same seed; compare the appropriate probabilities
and application observables when changing arithmetic policies.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};

fn main() -> Result<(), String> {
    let text = "H 0\nT 0\nR_Z(0.125) 0\nMX 0\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)?;
        assert_eq!(plan.rotation_arithmetic(), arithmetic);
        let mut sampler = plan.prepare_sampler()?;
        let mut rng = StdRng::seed_from_u64(42);
        let batch = sampler.sample_measurements_u8(1024, &mut rng)?;
        let ones = batch.measurements.iter().filter(|&&bit| bit == 1).count();
        println!("{arithmetic:?}: {ones}/{} raw ones", batch.shots);
    }
    Ok(())
}
```

## Count postselected shots without retaining output records

The compiled sampler's `sample_postselected_counts(shots, observable_index, rng)`
returns `NearCliffordPostselectedCounts`:

| Field | Meaning |
| --- | --- |
| `attempted` | The requested number of attempts, including rejected shots |
| `accepted` | Attempts for which **every raw detector parity is zero** |
| `logical_errors` | Accepted attempts whose chosen raw observable parity is one |

All `OBSERVABLE_INCLUDE(index)` events matching the chosen index are XOR-folded.
Other observable indices do not enter that parity. Detector record targets are
also XOR-folded, so duplicate targets cancel. There is **no noiseless/reference
sample normalization and no decoder correction**. If the circuit has no detectors,
every attempt is accepted. The observable index must occur in the circuit, even
for zero attempts; an empty `OBSERVABLE_INCLUDE(7)` is a valid zero parity.

The method makes exactly the requested number of attempts, not enough attempts
to reach a target accepted count. It returns aggregate counts, not the accepted
measurement records. Some execution paths reject rows/lanes early while still
consuming their remaining typed random events. Successful calls preserve the
compiled structured sampler's RNG stream. See the error caveat below.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::CompiledNearCliffordExecutor;

fn main() -> Result<(), String> {
    let text = "R 0 1\nH 0\nT 0\nMX 0\nX_ERROR(0.1) 1\nM 1\n\
                DETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text)?;
    let mut sampler = plan.prepare_sampler()?;
    let mut rng = StdRng::seed_from_u64(1739);
    let counts = sampler.sample_postselected_counts(1024, 7, &mut rng)?;
    assert_eq!(counts.attempted, 1024);
    assert!(counts.logical_errors <= counts.accepted);
    assert!(counts.accepted <= counts.attempted);

    let acceptance = counts.accepted as f64 / counts.attempted as f64;
    let conditional_error =
        (counts.accepted != 0).then(|| counts.logical_errors as f64 / counts.accepted as f64);
    println!(
        "attempted={}, accepted={}, raw logical errors={}",
        counts.attempted, counts.accepted, counts.logical_errors
    );
    println!("acceptance={acceptance}, conditional error={conditional_error:?}");

    let empty = sampler.sample_postselected_counts(0, 7, &mut rng)?;
    assert_eq!(empty.attempted, 0);
    assert_eq!(empty.accepted, 0);
    assert_eq!(empty.logical_errors, 0);
    assert!(sampler.sample_postselected_counts(0, 99, &mut rng).is_err());
    Ok(())
}
```

The conditional error rate is undefined when `accepted == 0`; acceptance is also
undefined for zero attempts. Report the denominators and counts. Do not silently
convert undefined rates to zero.

A deterministic detector value of one is rejected even if a reference-normalized
detector sampler would call it zero. This example also checks repeated observable
include events: raw one XOR raw one is zero.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, NearCliffordPostselectedCounts};

fn main() -> Result<(), String> {
    let mut rng = StdRng::seed_from_u64(42);
    let rejected = CompiledNearCliffordExecutor::compile_text(
        "X 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )?;
    assert_eq!(
        rejected
            .prepare_sampler()?
            .sample_postselected_counts(65, 0, &mut rng)?,
        NearCliffordPostselectedCounts {
            attempted: 65,
            accepted: 0,
            logical_errors: 0
        }
    );

    let folded = CompiledNearCliffordExecutor::compile_text(
        "X 0\nM 0\nOBSERVABLE_INCLUDE(7) rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-1]\n",
    )?;
    assert_eq!(
        folded
            .prepare_sampler()?
            .sample_postselected_counts(65, 7, &mut rng)?,
        NearCliffordPostselectedCounts {
            attempted: 65,
            accepted: 65,
            logical_errors: 0
        }
    );
    Ok(())
}
```

The first nonempty counts call may lazily construct an affine detector/observable
model for a rotation-free plan within its remaining plan budget. A declined model
keeps the existing sampling path; structured/flat calls do not use that counts
model. Include this first-call work when measuring lifecycle costs.

### Opt in to eager postselection

`plan.prepare_eager_postselected_sampler()` creates a separate sampler that can
use detector conditions determined by recorded random events to reject lanes
before executing their earlier quantum work. It uses the plan's Strict or Fused
arithmetic and the same raw all-zero-detector/observable-XOR count definition.
The existing `prepare_sampler()` remains the default.

```rust
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};

fn main() -> Result<(), String> {
    let text = "R 0 1\nH 0\nT 0\nMX 0\nX_ERROR(0.1) 1\nM 1\n\
                DETECTOR rec[-1]\nOBSERVABLE_INCLUDE(7) rec[-2]\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic)?;
        let mut ordinary = plan.prepare_sampler()?;
        let mut eager = plan.prepare_eager_postselected_sampler()?;
        let mut ordinary_rng = StdRng::seed_from_u64(1739);
        let mut eager_rng = ordinary_rng.clone();

        let expected = ordinary.sample_postselected_counts(1024, 7, &mut ordinary_rng)?;
        let actual = eager.sample_postselected_counts(1024, 7, &mut eager_rng)?;
        assert_eq!(actual, expected);
        let ordinary_next: [u64; 16] = std::array::from_fn(|_| ordinary_rng.next_u64());
        let eager_next: [u64; 16] = std::array::from_fn(|_| eager_rng.next_u64());
        assert_eq!(eager_next, ordinary_next);
        println!("{arithmetic:?}: {actual:?}");
    }
    Ok(())
}
```

Successful eager counts calls preserve the corresponding counts and complete
typed caller random stream, including random events for rejected lanes. The
example checks a finite continuation on this input; it is not an equivalence
proof for every circuit. Start comparisons with fresh equivalent sampler states
and cloned RNGs, as above.

**The error and cache contract is different.** Eager rejection can omit earlier
quantum allocation or numerical errors and coefficient-cache publication on
rejected lanes. Failed-call RNG prefixes and identical cache contents are not
promised. If an application relies on those error paths or cache population,
retain `prepare_sampler()`.

The first suitable counts call lazily constructs a bounded detector predicate;
include that work in first-call cost when benchmarking. Small batches and models
whose predicate is unavailable use the original sampling path. Performance
depends on circuit and batch size, so measure the intended workload before
enabling eager sampling. The eager sampler's `sample_measurements_u8` and
`sample_measurements_u8_with_sweep` methods always use the original full-record
path. Counts calls also have `sample_postselected_counts_with_sweep`.

`prepare_eager_postselected_sampler_with_cache_budget(bytes)` selects the same
0..=64 MiB coefficient-cache budget. The predicate has separate bounded
reservations; this budget is not a process-memory ceiling. Report compilation,
sampler preparation, first and later counts calls separately.

## Sweeps and classical feedback

`rec[-k]` refers to a previously produced measurement record; offset zero, positive
or out-of-range offsets are errors. The first control of `CX`/`CY`/`CZ` and their
accepted aliases can be a record or `sweep[k]`; the second target must be a qubit.
There is no classical-control form of `SWAP`.

Methods without `_with_sweep` use all-zero sweep input, and missing sweep bits
are zero. One supplied slice applies to every shot in that call. The compiled
counts argument order is `(shots, observable_index, sweep, rng)`; structured/flat
sampling uses `(shots, sweep, rng)`.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::CompiledNearCliffordExecutor;

fn main() -> Result<(), String> {
    let plan = CompiledNearCliffordExecutor::compile_text(
        "R 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )?;
    let mut sampler = plan.prepare_sampler()?;
    let mut rng = StdRng::seed_from_u64(42);
    let rows = sampler.sample_with_sweep(8, &[true], &mut rng)?;
    assert!(rows.iter().all(|row| row.measurements == vec![true]));
    let rejected = sampler.sample_postselected_counts_with_sweep(8, 0, &[true], &mut rng)?;
    assert_eq!(rejected.accepted, 0);
    let accepted = sampler.sample_postselected_counts(8, 0, &mut rng)?;
    assert_eq!(accepted.accepted, 8);
    assert_eq!(accepted.logical_errors, 0);
    Ok(())
}
```

`NearCliffordShot::observables` contains `(u32, bool)` include events in circuit
order; repeated indices are not already folded. `detectors` contains one raw XOR
per detector instruction. Neither output is reference-normalized.

## Use the interpreter when its interface fits

`NearCliffordExecutor` validates the instruction tree, then executes with adaptive
active coordinates. `run` returns one `NearCliffordShot`, not just a bit vector.
`sample` returns a vector of these shots. A prepared interpreter sampler reuses
the deterministic prefix and bounded terminal measurement cache.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;

fn main() -> Result<(), String> {
    let circuit = NearCliffordExecutor::compile_text("H 0\nT 0\nMX 0\n")?;
    let mut rng = StdRng::seed_from_u64(42);
    let shot = circuit.run(&mut rng)?;
    assert_eq!(shot.measurements.len(), 1);
    let mut sampler = circuit.prepare_sampler()?;
    let records = sampler.sample(100, &mut rng)?;
    assert_eq!(records.len(), 100);
    let flat = sampler.sample_measurements_u8(100, &mut rng)?;
    assert_eq!(flat.measurements_per_shot, 1);
    Ok(())
}
```

The interpreter has `run_with_sweep`, `sample_with_sweep` and prepared
`sample_measurements_u8_with_sweep`. It does not offer native postselected counts
or a Strict/Fused selector. The compiled executor has a different RNG policy:
same circuit/RNG type/seed does not imply interpreter/compiled bitwise identity.
Compare physical distributions rather than seeded rows across these APIs.

## Supported instructions and conventions

Unsupported operations, argument counts and target shapes return errors. The
parser accepting a name is not proof that this backend supports it. The supported
subset is narrower than the full Stim instruction set.

| Family | Interpreter and compiled | Compiled-only additions |
| --- | --- | --- |
| Single-qubit Cliffords | `I`, `H`, `S`/`SQRT_Z`, `S_DAG`/`SQRT_Z_DAG`, `X`, `Y`, `Z`, `SQRT_X`, `SQRT_X_DAG`, `SQRT_Y`, `SQRT_Y_DAG` | — |
| Two-qubit Cliffords | `CX`/`CNOT`/`ZCX`, `CY`/`ZCY`, `CZ`/`ZCZ`, `SWAP` | — |
| Coherent gates | `T`, `T_DAG`, `R_X(alpha)`, `R_Y(alpha)`, `R_Z(alpha)` | — |
| Measurement/reset | `M`/`MZ`, `MX`, `MY`, `MR`/`MRZ`, `MRX`, `MRY`, `R`/`RZ`, `RX`, `RY`; measurement target inversion `!q` | Pauli-product `MPP`; optional readout probability on M/MR variants and MPP |
| Pauli noise | `X_ERROR(p)`, `Y_ERROR(p)`, `Z_ERROR(p)`, `DEPOLARIZE1(p)`, `DEPOLARIZE2(p)` | — |
| Control/annotations | Positive `REPEAT` blocks, record/sweep feedback above, `DETECTOR`, `OBSERVABLE_INCLUDE` | — |
| Metadata | `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, with no effect on sampled bits | — |

`T = diag(1, exp(i*pi/4))`; `T_DAG = T†`. Axis-rotation arguments are finite
**half-turns**: `R_P(alpha) = exp(-i alpha*pi*P/2)`. Thus `R_X(0.5)` rotates by
pi/2. Negative angles reverse the rotation. Exact multiples of 0.5 use Clifford
evolution. The underscore is essential: `R_Z(0.25)` rotates, whereas `RZ 0`
**resets** qubit0; `RZ` does not take an angle. Generic `ROT_Z`, `U3`, arbitrary
unitary matrices, and non-Pauli noise channels are not supported by these APIs.

Noise probabilities must be finite and in `[0,1]`. Compiled `M(0.02)` and
`MPP(0.02)` flip reported records with the supplied probability; physical
projection and reset still follow the ideal branch. Later record feedback and
annotations see the reported bits. The interpreter rejects these probability
arguments. Inverted measurement targets flip reported parity rather than the
physical state. Each MPP product must have distinct qubit factors; for example,
`X0*X0` is rejected instead of silently simplified. Factor inversions are XORed
into the reported bit.

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, NearCliffordExecutor};

fn main() -> Result<(), String> {
    let text = "H 0\nCX 0 1\nMPP(0.02) X0*X1 Z0*Z1\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text)?;
    let mut rng = StdRng::seed_from_u64(42);
    let rows = plan.sample(32, &mut rng)?;
    assert!(rows.iter().all(|row| row.measurements.len() == 2));
    assert!(NearCliffordExecutor::compile_text(text).is_err());
    Ok(())
}
```

## Rank limits, cache budgets and failures

Both entry points default to an active-rank limit of 16 and reject more than
4096 physical qubits. Compiled `peak_active_rank()` is a structural planning
metric; the interpreter evolves adaptive coordinates at runtime. Neither equals
T count or another simulator's reported active width. Dense coefficients have
`2^r` entries at rank `r`; raising the rank limit does not remove memory limits.
The interpreter checks rank and platform allocation sizes, but exposes no
whole-state byte-budget setting. Its terminal-cache budget is not a bound on
the active state; a raised rank limit can still require very large allocations.

For a custom limit, parse the circuit to `Vec<StimInstr>` and use the relevant
`compile_with_limit` method. The compiled variant also has
`compile_with_limit_and_arithmetic`:

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};
use rstim::parser::parse_lines;

fn main() -> Result<(), String> {
    let instructions = parse_lines("H 0\nT 0\nMX 0\n")?;
    let plan = CompiledNearCliffordExecutor::compile_with_limit_and_arithmetic(
        instructions,
        12,
        CompiledRotationArithmetic::Strict,
    )?;
    // Zero disables optional coefficient-transition caching, not all allocations.
    let mut sampler = plan.prepare_sampler_with_cache_budget(0)?;
    assert_eq!(sampler.coefficient_cache_reserved_bytes(), 0);
    let mut rng = StdRng::seed_from_u64(42);
    let batch = sampler.sample_measurements_u8(8, &mut rng)?;
    assert_eq!(batch.shots, 8);
    Ok(())
}
```

Compiled samplers default to a 64 MiB optional coefficient-cache budget; a budget
above 64 MiB is an error. Cache admission/allocation failure falls back to
arithmetic. `coefficient_cache_reserved_bytes()` reports a cache reservation
ledger, not process memory. Separate current implementation limits include
64 MiB reservations for each validation view, transient Pauli tape, emitted plan
and coefficient accounting, a 9 MiB packed compilation frame, and 16 MiB packet
work/output buffers. Batched coherent arithmetic has its own 64 MiB reservation.
These are separate conservative component budgets, **not a single RSS ceiling**.

The interpreter's terminal cache is different: at most 1024 nodes, depth 16 and a
conservative 64 MiB reservation per sampler. Optional caching is disabled if the
mandatory root exceeds that reservation. Above 2048 coefficients, cached children
must reduce the coefficient vector. Cache exhaustion falls back to sampling.

Compile and sampling methods return `Result<_, String>`. Handle unsupported gates,
invalid probabilities/record references and resource errors rather than retrying
indefinitely or silently switching backends. Some resource errors occur during
preparation or sampling, not at parsing. A failed sampling call may already have
consumed RNG events; no identical failed-call RNG prefix is promised across
execution strategies. Counts can skip rejected suffix physics and its fallible
allocations, so successful counts need not reproduce a structured call's failure.

## Reproducibility and performance

For successful calls with a fixed compiled plan/arithmetic, the structured, flat
and counts paths preserve the corresponding records/counts and RNG continuation.
To compare paths, start with equivalent fresh sampler state and cloned RNGs; do
not compare consecutive batches as if they used the same random inputs. The
interpreter likewise preserves successful structured/flat streams within its own
policy. Neither policy promises seeded rows across different rstim versions,
RNG implementations or interpreter/compiled backends.

Record the source revision, compiler, RNG type/seed, arithmetic, output contract,
cache budget, shot count and host configuration when measuring performance.
Report compilation, preparation, first and warm calls separately. Counts and
full-record throughput are different workloads; acceptance and conditional error
rates require their own denominators. Fused, larger caches and compiler
scheduling are not universal performance wins. Process lifetime peak RSS is not
current/scoped simulator memory. For protocols and revision-bound evidence, see
[the benchmark guide](https://github.com/nzy1997/rust-qec/tree/master/benchmarks/near_clifford).
Two application circuits do not establish universal SOTA.

## Representation and correctness notes

The following explains the coherent-state convention behind both execution APIs.
It is implementation background, not an additional setup step for the examples.

### Shared frame, active coordinates, and phase

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
For this example, the interpreter's `ActiveState` stores coefficients in the
virtual computational basis `U|ab>`.
For this circuit `U|11> = -Z1 Z0|phi>`, so its raw fourth coefficient is
`+s²`; canonicalizing to the sign-sector basis above multiplies that entry
by `-1`. The exact acceptance test checks both bases against an independent
dense-state projection.

This example has uniform computational-basis measurement probabilities. It
therefore cannot by itself distinguish a coherent state from a classical
mixture of the four sectors. The additional interference check is
`<X0 X1> = 1/2`, or equivalently X-basis even-parity probability `3/4`.
An incoherent mixture of the same sectors instead gives `<X0 X1> = 0`.

### Resource boundary and reference oracle

For `r` independent active coordinates, the dense coefficient array has
`2^r` entries. Expansion checks the configured active-rank and platform size
limits. The compiled executor additionally enforces the component byte budgets
described above.
Repeated T gates acting on the same active coordinate must not add a new
dimension solely because the T count increased. The implementation does not
guarantee a globally minimal active basis.

After measurement, an axis with one zero-amplitude half is removed. Its fixed
virtual bit is retained in the `origin` coordinate, so later gates see the
correct Pauli sign. When rebasing that origin into the frame, right composition
by the selected virtual X operators flips the corresponding stabilizer signs;
the row bits and destabilizer signs stay unchanged. This avoids a full tableau
snapshot and physical-Pauli sweeps. The global-phase multiplications retain their
previous per-qubit order. Other measurements can leave a non-minimal active basis;
the configured rank limit still applies and reports an error if exceeded.

Small-qubit tests use an independent dense state-vector simulator and an
explicit `T = e^(i*pi/8)(cos(pi/8) I - i sin(pi/8) Z)` weighted-tableau
expansion. Each expanded term has a tableau plus a dense phase witness;
the test checks every stabilizer generator against its witness before terms
are coherently summed. A tableau without an amplitude phase for each weighted
term is insufficient as a coherent oracle: the phase omitted by a single
stabilizer state becomes a relative phase when terms are added.
