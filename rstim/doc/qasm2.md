# OpenQASM 2.0 conversion

Enable the optional `qasm` feature for semantic OpenQASM 2.0 import and export. The frontend
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

## Export checked and native circuits

```rust
use rstim::qasm::{from_qasm, to_qasm, export_stim, ExportOptions, QasmLimits};

let limits = QasmLimits::default();
let options = ExportOptions::default();
let program = from_qasm("OPENQASM 2.0; qreg h[1]; creg c[1]; measure h -> c;")?;
let output = to_qasm(&program, &options, &limits)?;
assert!(output.text.contains("qreg h[1]"));

let native = rstim::parser::parse_lines("H 0\nCX 0 1\nMX 0\nMR 1\n")?;
let output = export_stim(&native, &options, &limits)?;
assert_eq!(output.classical_registers.len(), 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Checked-document export preserves declared registers and each measurement's
destination. Native Stim export has no original classical layout; it creates
one-bit `m0`, `m1`, ... registers in record order. This allows record feedback
to use legal QASM 2 register-equality conditions. Basis measurements restore the
post-measurement quantum state; inverted measurements complement their output
without changing that state. Reset does not add a visible measurement result.

The writer emits portable primitive `U`/`CX` operations, without a header or
helper names that might collide with a source register. This also avoids relying
on tool-specific gates such as `sx` or `swap`. Gate names and original source
formatting are not reconstructed.
Exact expression literals use the signed 64-bit integer range, including an
arithmetic spelling for its minimum value. A checked angle can have an exact
unsigned 64-bit denominator produced by several divisions; export rejects a
denominator above `i64::MAX` with `qasm_unrepresentable_export` instead of
silently replacing it with a floating-point parameter.

| Native instruction family | Export behavior |
| --- | --- |
| I/H/Paulis/S and inverse, SQRT_X/Y/Z and inverse, CX/CY/CZ/SWAP aliases | Checked primitive decompositions |
| T/T_DAG and R_X/R_Y/R_Z | Coherent rotations; exact binary half-turn periods retained |
| M/MZ/MX/MY, MR/MRZ/MRX/MRY, inverted measurement targets | Basis-preserving projective instruments |
| R/RZ/RX/RY | Unobserved reset in the corresponding basis |
| Record-controlled X/Y/Z | Singleton-register equality feedback |
| Positive REPEAT | Bounded expansion, including empty-body work accounting |
| TICK | Barrier; empty ticks have an explicit omission report |
| Detector/observable/coordinate annotations and tags | Strict rejection by default |
| Noise, loss, sweep, MPP/MXX/MYY/MZZ, SPP, MPAD and unknown gates | Rejected, including zero-probability noise |

`ExportOptions { omit_annotations: true }` explicitly requests a projection that
omits QEC and layout annotations. Every removed item appears in
`omitted_annotations` with its instruction/repeat path. This option never permits
discarding quantum noise, reset, measurement, feedback, or unknown operations.
Export performs validation without compiling or simulating a near-Clifford plan.

## Includes, diagnostics and limits

`from_qasm` supplies the bundled standard header and otherwise has no implicit
filesystem or network access. `parse_and_check` accepts an explicit
`IncludeResolver`. For files, `from_qasm_file(path, &FileResolver::default(),
&limits)` searches the including file's directory and configured include paths.
The reserved bundled standard header wins over unrelated working-directory files.

`QasmLimits` bounds source bytes, tokens, include files/depth, gate and expression
depth, expanded operations, qubits and classical bits. `max_operations` separately
bounds the number of retained operations and their cumulative expanded operand
work: each gate/condition target or barrier qubit counts once, and each
measurement/reset counts once. Wide barriers therefore share one cumulative
budget instead of allocating an unbounded target list per source statement.
Declared idle register width remains governed by the qubit/classical-bit limits.
Gate expansion also charges broadcast-lane operand visits against
`max_operations * max_gate_depth`, including calls to empty gate bodies.
`max_output_bytes` is
used by QASM export and semantic JSON input/output; QASM parsing/lowering do not
consume that budget. File reads
are bounded separately by `FileResolver::max_file_bytes`. Expressions use checked
bounded rational arithmetic where possible, otherwise finite FP64 values.
Invalid domains, non-finite results and excessive expansion fail explicitly.

`QasmError` includes a stable category, message, source location and expansion
context. Syntax/type errors, valid unsupported constructs and backend limitations
are distinct. Conversion failures never remove unsupported operations and
continue with a different circuit.

## Persistent semantic documents

`QasmCircuit::to_json(&limits)` produces schema `rstim.qasm2-program.v1`.
`QasmCircuit::from_json(text, &limits)` checks the schema, every nested field,
register layout, operation arity, angle domain, provenance, and all resource
limits before returning a usable document. Unknown fields and malformed
operations fail rather than changing meaning. Floating-point angles retain
their FP64 values through serialization; exact rational multiples of pi retain
their integer representation.

The document stores expanded typed operations, declared quantum/classical
registers, and optional source spans. It does not store a second executable plan
or a cached classical-output map that could disagree with those operations.
Lowering recomputes measurement events and final classical bindings. Keeping the
document is the normal workflow for a semantic round trip; `.stim` is an explicit
execution projection with a warning about lost layout information.

## Command-line workflows

Build conversion commands with both optional features:

```sh
cargo build --locked -p rstim --no-default-features --features qasm,unified-cli
RSTIM=target/debug/rstim
$RSTIM circuit import-qasm --in input.qasm --out program.qasm2.json --backend near-clifford
$RSTIM circuit export-qasm --in program.qasm2.json --in-format qasm2-json --out output.qasm
$RSTIM circuit sample --in program.qasm2.json --in-format qasm2-json --backend near-clifford --result-view classical --shots 1024 --seed 7 --out shots.01
$RSTIM circuit export-qasm --in native.stim --out native.qasm
```

For a Clifford-only program, choose `--backend stabilizer` (the default).
Near-Clifford sampling requires an explicit selection and accepts
`--max-active-rank` (default 16). Exceeding that limit fails with an actionable
error; rank limits are never automatically raised. Conversion itself does not
sample or compile the potentially expensive near-Clifford execution state.

Import defaults to `--out-format qasm2-json`; `--out-format stim` requests the
execution projection and reports layout loss. Export defaults to
`--in-format stim`; use `--in-format qasm2-json` to retain the original layout.
`--omit-annotations` reports every omitted native annotation and never removes
noise or unsupported quantum operations.

Omitting `--in`, or passing `--in -`, reads stdin. File imports resolve relative
includes beside the including file. Stdin imports require explicit
`--include-dir PATH` for external includes; nested includes then resolve beside
the included file. The bundled standard header remains available in either
case. Circuit artifacts go to the required `--out` file; JSON/human reports go
to stdout and diagnostics go to stderr. Failed validation or sampling leaves an
existing output file intact.

Sampling defaults to `--result-view measurement-record`. The `classical` view
uses declared register order, preserves unwritten zeros, and selects the latest
measurement assigned to each bit. It requires a semantic document; a plain Stim
file has no classical layout. Supported shot encodings are `01`, `b8`, `r8`,
`hits`, and `ptb64`. Existing Stim-only sampling defaults are unchanged.
QASM documents are not direct input to detection-event, DEM, or decoder commands.
`rstim capabilities --format json` advertises the conversion commands only when
the feature is enabled.

## Verification and reference implementations

Ordinary Rust tests use an independent full-matrix and labelled-instrument
interpreter for small circuits. Comparisons use one common unitary phase, or
outcome-labelled Choi matrices for measurement/reset/feedback. Generated
properties cover source/document/export and native/export/import round trips;
they check quantum state changes as well as final classical results. Focused
CLI tests cover both backends, result views, file/stdin handling, atomic output,
and disabled-feature behavior.

The dedicated [QASM workflow](../../.github/workflows/qasm2.yml) runs an
independent strict Qiskit parser and pinned upstream Stim differential checks.
Run its external check locally with the exact pinned dependencies in that
workflow and `python3 tools/qasm2_conformance.py --binary target/debug/rstim`.
The same workflow smoke-tests the bounded `qasm2_parse` and `qasm2_document`
fuzz targets. The general CI suite also exercises feature combinations and
includes conversion code in coverage.

Stim's [exporter](https://github.com/quantumlib/Stim/blob/6f15adb47f54fbec6c5f68a673f453f3d5c0a9fd/src/stim/util_top/export_qasm.cc)
informed basis/inverted-measurement decompositions and explicit failure behavior.
The [yao-rs converter](https://github.com/GiggleLiu/yao-rs/blob/4a398fe0a641b98792f8d3c28c56c2b79e84cedb/src/qasm.rs)
informed the AST/type-check/expansion architecture. Its measurement treatment is
not an execution oracle. rstim uses its own bounded frontend because the examined
`openqasm` 0.1.2 parser stores real literals as FP32 and lacks scientific notation.
These references guide interoperability; they do not replace semantic tests.
