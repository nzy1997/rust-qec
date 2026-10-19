# OpenQASM 2.0 conversion

Enable the optional `qasm` feature for semantic OpenQASM 2.0 import. The frontend
parses a syntax tree, checks scopes and types, resolves gate definitions, and
constructs ordered operations before choosing an execution target. It does not
replace gate names in source strings.

```toml
[dependencies]
rstim = { path = "../rstim/rstim", default-features = false, features = ["qasm"] }
rand = "0.8"
```

## Import and execute

```rust
use rand::{SeedableRng, rngs::StdRng};
use rstim::qasm::{Backend, QasmLimits, from_qasm};

let program = from_qasm(r#"
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
measure q -> c;
"#)?;
let plan = program.lower(Backend::Stabilizer, &QasmLimits::default())?;
let mut rng = StdRng::seed_from_u64(7);
let shots = rstim::sampler::sample_batch(&plan.instructions, 16, &mut rng)?;
for shot in 0..16 {
    let record = (0..plan.measurement_events.len())
        .map(|i| shots.measurements.get(i, shot)).collect::<Vec<_>>();
    let classical = plan.project_classical(&record)?;
    assert_eq!(classical[0], classical[1]);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Classical registers start at zero. A measurement executes at its original
position and overwrites its destination bit. The native measurement record
retains every measurement, including overwritten results; `project_classical`
selects the latest assigned value for each declared bit and supplies unwritten
zeros. Output bits use register declaration order, then increasing bit index.
For integer comparisons, bit 0 is the least significant bit. These are separate
ordering rules; do not reverse output bits to evaluate a condition.

The conversion document preserves declared register widths, including idle
qubits. A plain `.stim` execution projection does not retain register names or
classical destination assignments. Keep the conversion document when these are
needed for subsequent export or result interpretation.

## Supported profile

| Construct | Behavior |
| --- | --- |
| QASM 2 declarations and multiple registers | Checked names, types, sizes, offsets and indices |
| Built-in `U`/`CX`, original `qelib1.inc` | Checked primitive or named-gate lowering |
| Custom unitary definitions | Parameter binding and bounded expansion; bodies validated even if unused |
| Register operands | Checked width expansion and quantum operand aliasing |
| Measurement/reset | Ordered quantum operations; measurement destinations retained |
| Barriers | Scoped in the document; a conservative global `TICK` in the execution projection |
| One-bit register conditions on X/Y/Z | Native Pauli feedback; equality to zero and one supported |
| General multi-bit or non-Pauli conditions | Explicitly rejected as outside the supported profile |
| Executed opaque gates | Explicitly rejected; unused declarations can be retained during checking |
| QASM 3, noise and pulse syntax | Not supported by this profile |

The stabilizer target accepts operations whose lowering consists of Clifford
gates. Exact symbolic multiples of `pi/2` can be recognized without a numerical
threshold. Nearby decimal angles are not snapped to Cliffords. Gate sequences
that happen to cancel to a Clifford are not generally optimized.

T gates and general rotations require an explicit `Backend::NearClifford`
selection. Lowered rotations use native half-turns, not radians. `R_Z` is a
rotation; `RZ` without an underscore is a reset. Execute the lowered plan with
`CompiledNearCliffordExecutor::compile_with_limit(plan.instructions.clone(), 16)`
and its sampler; ordinary stabilizer sampling never switches backends. Active
rank and memory limits still apply. `program.analyze(&limits)` reports each
backend's support independently of execution cost.

## Header and phase conventions

The bundled header is the original
[`qelib1.inc`](https://github.com/openqasm/openqasm/blob/e8abf2ccd7876ec92108035591a5176553d24c99/examples/qelib1.inc).
Gate bodies are authoritative. A user-defined gate named `x` does not acquire
Pauli semantics solely from its name. Modern names such as `swap` and `sx` must
have definitions; they are not implicit extensions to the original header.

Primitive `U(theta,phi,lambda)` uses the usual matrix with top-left entry
`cos(theta/2)`, top-right `-exp(i*lambda)*sin(theta/2)`, bottom-left
`exp(i*phi)*sin(theta/2)` and bottom-right
`exp(i*(phi+lambda))*cos(theta/2)`. One common global phase of a complete
unitary is immaterial. Relative phases across control branches are preserved.

The original header aliases `rz` to `u1`. Its original `cu3` definition also
differs from a conventionally controlled U by a control-dependent phase. Import
uses the literal original body rather than silently substituting a differently
defined tool-specific gate. This matters when comparing with other importers;
see [Qiskit's phase convention notes](https://quantum.cloud.ibm.com/docs/en/api/qiskit/qasm2#openqasm-2-phase-conventions).

## Includes, diagnostics and limits

`from_qasm` supplies the bundled standard header and otherwise has no implicit
filesystem or network access. `parse_and_check` accepts an explicit
`IncludeResolver`. For files, `from_qasm_file(path, &FileResolver::default(),
&limits)` searches the including file's directory and configured include paths.
The reserved bundled standard header wins over unrelated working-directory files.

`QasmLimits` bounds source bytes, tokens, include files/depth, gate and expression
depth, expanded operations, qubits, classical bits and output size. File reads
are bounded separately by `FileResolver::max_file_bytes`. Expressions use checked
bounded rational arithmetic where possible, otherwise finite FP64 values.
Invalid domains, non-finite results and excessive expansion fail explicitly.

`QasmError` includes a stable category, message, source location and expansion
context. Syntax/type errors, valid unsupported constructs and backend limitations
are distinct. Conversion failures never remove unsupported operations and
continue with a different circuit.
