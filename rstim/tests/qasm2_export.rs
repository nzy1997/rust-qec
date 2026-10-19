#![cfg(feature = "qasm")]

#[allow(dead_code)]
#[path = "support/qasm2_reference.rs"]
mod reference;

use reference::{Primitive as P, assert_unitary};
use rstim::ir::{StimInstr, StimTarget};
use rstim::qasm::{
    Backend, ErrorCode, ExportOptions, Gate, OperationKind, Pauli, QasmLimits, export_stim,
    from_qasm, lift, to_qasm,
};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI};

fn op(name: &str, targets: Vec<StimTarget>) -> StimInstr {
    StimInstr::new(name, vec![], targets)
}
fn qubit(q: u32) -> StimTarget {
    StimTarget::Qubit(q)
}
fn export(instructions: &[StimInstr]) -> rstim::qasm::QasmExport {
    export_stim(
        instructions,
        &ExportOptions::default(),
        &QasmLimits::default(),
    )
    .unwrap()
}

#[test]
fn native_unitary_recipes_match_independent_full_matrices() {
    for (name, parameters) in [
        ("I", (0., 0., 0.)),
        ("H", (FRAC_PI_2, 0., PI)),
        ("X", (PI, 0., PI)),
        ("Y", (PI, FRAC_PI_2, FRAC_PI_2)),
        ("Z", (0., 0., PI)),
        ("S", (0., 0., FRAC_PI_2)),
        ("SQRT_Z", (0., 0., FRAC_PI_2)),
        ("S_DAG", (0., 0., -FRAC_PI_2)),
        ("SQRT_Z_DAG", (0., 0., -FRAC_PI_2)),
        ("T", (0., 0., FRAC_PI_4)),
        ("T_DAG", (0., 0., -FRAC_PI_4)),
        ("SQRT_X", (FRAC_PI_2, -FRAC_PI_2, FRAC_PI_2)),
        ("SQRT_X_DAG", (-FRAC_PI_2, -FRAC_PI_2, FRAC_PI_2)),
        ("SQRT_Y", (FRAC_PI_2, 0., 0.)),
        ("SQRT_Y_DAG", (-FRAC_PI_2, 0., 0.)),
    ] {
        let output = export(&[op(name, vec![qubit(0)])]);
        let plan = from_qasm(&output.text)
            .unwrap()
            .lower(Backend::NearClifford, &QasmLimits::default())
            .unwrap();
        assert_unitary(&[P::U(0, parameters.0, parameters.1, parameters.2)], &plan);
    }
    for name in ["CX", "CNOT", "ZCX", "CY", "ZCY", "CZ", "ZCZ", "SWAP"] {
        let expected = match name {
            "CY" | "ZCY" => vec![P::ControlledU(0, 1, PI, FRAC_PI_2, FRAC_PI_2)],
            "CZ" | "ZCZ" => vec![P::ControlledU(0, 1, 0., 0., PI)],
            "SWAP" => vec![P::CX(0, 1), P::CX(1, 0), P::CX(0, 1)],
            _ => vec![P::CX(0, 1)],
        };
        let output = export(&[op(name, vec![qubit(0), qubit(1)])]);
        assert!(!output.text.contains("include"));
        assert!(!output.text.contains("swap"));
        let plan = from_qasm(&output.text)
            .unwrap()
            .lower(Backend::NearClifford, &QasmLimits::default())
            .unwrap();
        assert_unitary(&expected, &plan);
    }
}

#[test]
fn enormous_native_half_turns_are_reduced_before_radian_conversion() {
    for value in [1e20, -1e20, f64::MAX, -f64::MAX] {
        for name in ["R_X", "R_Y", "R_Z"] {
            let output = export(&[StimInstr::new(name, vec![value], vec![qubit(0)])]);
            let plan = from_qasm(&output.text)
                .unwrap()
                .lower(Backend::Stabilizer, &QasmLimits::default())
                .unwrap();
            assert_unitary(&[P::U(0, 0., 0., 0.)], &plan);
        }
    }
    for name in ["T", "T_DAG"] {
        assert!(
            export_stim(
                &[op(name, vec![])],
                &ExportOptions::default(),
                &QasmLimits::default()
            )
            .is_err()
        );
    }
    assert!(
        export_stim(
            &[StimInstr::new("R_X", vec![0.3], vec![])],
            &ExportOptions::default(),
            &QasmLimits::default()
        )
        .is_err()
    );
}

#[test]
fn half_turn_rotations_export_exact_radians_and_never_become_resets() {
    for value in [0., 0.25, -0.5, 1., 2., 4., 0.1, -0.231] {
        for name in ["R_X", "R_Y", "R_Z"] {
            let output = export(&[StimInstr::new(name, vec![value], vec![qubit(0)])]);
            assert!(!output.text.contains("reset"));
            let expected = match name {
                "R_X" => P::U(0, value * PI, -FRAC_PI_2, FRAC_PI_2),
                "R_Y" => P::U(0, value * PI, 0., 0.),
                _ => P::U(0, 0., 0., value * PI),
            };
            let parsed = from_qasm(&output.text).unwrap();
            let plan = parsed
                .lower(Backend::NearClifford, &QasmLimits::default())
                .unwrap();
            assert_unitary(&[expected], &plan);
            if value == 0.25 {
                assert!(output.text.contains("(1*pi/4)"));
            }
        }
    }
    let output = export(&[op("RZ", vec![qubit(0)])]);
    assert!(output.text.contains("reset q[0];"));
    assert!(output.classical_registers.is_empty());
}

#[test]
fn semantic_export_preserves_layout_overwrites_barrier_scope_and_condition_equality() {
    let input = "OPENQASM 2.0; include \"qelib1.inc\"; qreg a[1]; qreg b[3]; creg unused[2]; creg c[1]; h a[0]; measure a[0] -> c[0]; if(c==0) y b[1]; measure b[1] -> c[0]; barrier a[0],b[2];";
    let parsed = from_qasm(input).unwrap();
    let output = to_qasm(&parsed, &ExportOptions::default(), &QasmLimits::default()).unwrap();
    let restored = from_qasm(&output.text).unwrap();
    assert_eq!(restored.quantum_registers(), parsed.quantum_registers());
    assert_eq!(restored.classical_registers(), parsed.classical_registers());
    assert!(output.text.contains("if(c==0) U(pi,pi/2,pi/2) b[1];"));
    assert!(output.text.contains("barrier a[0],b[2];"));
    assert_eq!(
        restored
            .operations()
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Measure { bit: 2, .. }))
            .count(),
        2
    );
    assert_eq!(
        restored
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap()
            .classical_outputs,
        parsed
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap()
            .classical_outputs
    );
}

#[test]
fn primitive_only_output_avoids_standard_header_namespace_capture() {
    let source = "OPENQASM 2.0; qreg h[1]; creg x[1]; U(pi/2,0,pi) h[0]; measure h[0] -> x[0]; if(x==1) U(pi,0,pi) h[0];";
    let circuit = from_qasm(source).unwrap();
    let output = to_qasm(&circuit, &ExportOptions::default(), &QasmLimits::default()).unwrap();
    assert!(!output.text.contains("include"));
    from_qasm(&output.text)
        .unwrap()
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
}

#[test]
fn repeat_feedback_resolves_history_across_iterations_and_nested_bodies() {
    let instructions = vec![
        op("M", vec![qubit(0)]),
        StimInstr::Repeat {
            count: 2,
            body: vec![
                op("CX", vec![StimTarget::Rec(-1), qubit(1)]),
                StimInstr::Repeat {
                    count: 2,
                    body: vec![
                        op("M", vec![qubit(1)]),
                        op("CY", vec![StimTarget::Rec(-2), qubit(0)]),
                    ],
                },
            ],
        },
        op("CZ", vec![qubit(1), StimTarget::Rec(-5)]),
    ];
    let output = export(&instructions);
    assert_eq!(output.classical_registers.len(), 5);
    assert!(output.classical_registers.iter().all(|reg| reg.size == 1));
    let text = &output.text;
    assert!(text.contains("if(m0==1) U(pi,0,pi) q[1];"));
    assert!(text.contains("if(m2==1) U(pi,0,pi) q[1];"));
    assert!(text.contains("if(m0==1) U(0,0,pi) q[1];"));
    let restored = from_qasm(text).unwrap();
    assert_eq!(restored.num_classical_bits(), 5);
    restored
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
}

#[test]
fn annotations_require_explicit_omission_and_each_removal_has_a_path_and_source() {
    let instructions = vec![
        op("M", vec![qubit(0)]),
        StimInstr::Repeat {
            count: 2,
            body: vec![
                StimInstr::Op {
                    name: "H".into(),
                    tag: Some("label".into()),
                    args: vec![],
                    targets: vec![qubit(0)],
                },
                StimInstr::new("DETECTOR", vec![0., 1.], vec![StimTarget::Rec(-1)]),
            ],
        },
        StimInstr::new("QUBIT_COORDS", vec![1., 2.], vec![qubit(3)]),
        StimInstr::new("SHIFT_COORDS", vec![1.], vec![]),
        StimInstr::new("OBSERVABLE_INCLUDE", vec![0.], vec![StimTarget::Rec(-1)]),
    ];
    assert_eq!(
        export_stim(
            &instructions,
            &ExportOptions::default(),
            &QasmLimits::default()
        )
        .unwrap_err()
        .code,
        ErrorCode::Unrepresentable
    );
    let output = export_stim(
        &instructions,
        &ExportOptions {
            omit_annotations: true,
        },
        &QasmLimits::default(),
    )
    .unwrap();
    assert_eq!(output.omitted_annotations.len(), 7);
    assert_eq!(output.omitted_annotations[0].name, "TAG");
    assert_eq!(output.omitted_annotations[0].instruction_path, [1, 0, 0]);
    assert_eq!(output.omitted_annotations[1].instruction_path, [1, 0, 1]);
    assert_eq!(
        output.omitted_annotations[1].source,
        "DETECTOR(0,1) rec[-1]\n"
    );
    assert!(output.text.contains("qreg q[4];"));
}

#[test]
fn unsupported_native_semantics_are_never_dropped_even_with_annotation_omission() {
    let options = ExportOptions {
        omit_annotations: true,
    };
    for instruction in [
        StimInstr::new("X_ERROR", vec![0.], vec![qubit(0)]),
        StimInstr::new("LOSS", vec![0.], vec![qubit(0)]),
        StimInstr::new("M", vec![0.], vec![qubit(0)]),
        op("MPP", vec![qubit(0)]),
        op("MXX", vec![qubit(0), qubit(1)]),
        op("MYY", vec![qubit(0), qubit(1)]),
        op("MZZ", vec![qubit(0), qubit(1)]),
        op("SPP", vec![qubit(0)]),
        op("MPAD", vec![qubit(0)]),
        op("UNKNOWN", vec![]),
        op("CX", vec![StimTarget::Sweep(0), qubit(0)]),
    ] {
        assert_eq!(
            export_stim(
                std::slice::from_ref(&instruction),
                &options,
                &QasmLimits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::Unrepresentable,
            "{instruction:?}"
        );
    }
}

#[test]
fn malformed_arguments_targets_and_record_references_fail_checked_lifting() {
    for instruction in [
        StimInstr::new("H", vec![0.], vec![qubit(0)]),
        StimInstr::new("R_X", vec![], vec![qubit(0)]),
        StimInstr::new("R_Z", vec![f64::NAN], vec![qubit(0)]),
        op("H", vec![StimTarget::QubitInv(0)]),
        op("RX", vec![StimTarget::Rec(-1)]),
        op("CX", vec![qubit(0)]),
        op("CX", vec![qubit(0), qubit(0)]),
        op("CX", vec![StimTarget::Rec(0), qubit(0)]),
        op("CZ", vec![StimTarget::Rec(-1), qubit(0)]),
        op("TICK", vec![qubit(0)]),
        StimInstr::new("OBSERVABLE_INCLUDE", vec![-1.], vec![]),
        StimInstr::new("SHIFT_COORDS", vec![0.], vec![qubit(0)]),
        StimInstr::Repeat {
            count: 0,
            body: vec![],
        },
    ] {
        assert!(
            lift(
                std::slice::from_ref(&instruction),
                &ExportOptions {
                    omit_annotations: true
                },
                &QasmLimits::default()
            )
            .is_err(),
            "{instruction:?}"
        );
    }
}

#[test]
fn expansion_width_records_output_and_empty_repeat_work_are_bounded() {
    let options = ExportOptions::default();
    let limits = QasmLimits {
        max_operations: 4,
        ..QasmLimits::default()
    };
    for instructions in [
        vec![StimInstr::Repeat {
            count: u64::MAX,
            body: vec![],
        }],
        vec![StimInstr::Repeat {
            count: 3,
            body: vec![op("H", vec![qubit(0)])],
        }],
        vec![op("MY", vec![StimTarget::QubitInv(0)])],
    ] {
        assert_eq!(
            lift(&instructions, &options, &limits).unwrap_err().code,
            ErrorCode::Limit
        );
    }
    for (limits, instructions) in [
        (
            QasmLimits {
                max_qubits: 1,
                ..QasmLimits::default()
            },
            vec![op("H", vec![qubit(1)])],
        ),
        (
            QasmLimits {
                max_classical_bits: 1,
                ..QasmLimits::default()
            },
            vec![op("M", vec![qubit(0), qubit(0)])],
        ),
        (
            QasmLimits {
                max_output_bytes: 10,
                ..QasmLimits::default()
            },
            vec![op("T", vec![qubit(0)])],
        ),
        (
            QasmLimits {
                max_gate_depth: 0,
                ..QasmLimits::default()
            },
            vec![StimInstr::Repeat {
                count: 1,
                body: vec![],
            }],
        ),
    ] {
        assert_eq!(
            export_stim(&instructions, &options, &limits)
                .unwrap_err()
                .code,
            ErrorCode::Limit
        );
    }
}

#[test]
fn empty_circuits_and_ticks_have_defined_output_without_fake_records() {
    let output = export(&[]);
    assert_eq!(output.text, "OPENQASM 2.0;\n");
    assert!(output.classical_registers.is_empty());
    from_qasm(&output.text).unwrap();
    let output = export(&[op("TICK", vec![])]);
    assert_eq!(output.omitted_annotations.len(), 1);
    assert_eq!(output.omitted_annotations[0].name, "TICK");
    assert!(output.classical_registers.is_empty());
    let output = export(&[op("I", vec![qubit(2)]), op("TICK", vec![])]);
    assert!(output.text.contains("barrier q[0],q[1],q[2];"));
}

// A separate one-qubit instrument oracle compares the exporter directly with
// Pauli projectors and reset Kraus matrices. It never executes rstim lowering or
// reuses the basis-change recipes under test. Operators retain both input columns
// and are grouped by the visible measurement, preserving entangled-reference
// behavior and conditional post-measurement states.
#[derive(Clone, Copy, Debug, Default)]
struct C(f64, f64);
impl std::ops::Add for C {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(self.0 + b.0, self.1 + b.1)
    }
}
impl std::ops::Sub for C {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1)
    }
}
impl std::ops::Mul for C {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self(self.0 * b.0 - self.1 * b.1, self.0 * b.1 + self.1 * b.0)
    }
}
impl C {
    fn conj(self) -> Self {
        Self(self.0, -self.1)
    }
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
    fn phase(t: f64) -> Self {
        Self(t.cos(), t.sin())
    }
}
type M = [C; 4];
const ZERO: C = C(0., 0.);
const ONE: C = C(1., 0.);
const I: C = C(0., 1.);
const ID: M = [ONE, ZERO, ZERO, ONE];
fn mul(a: M, b: M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
    ]
}
fn pauli(p: Pauli) -> M {
    match p {
        Pauli::X => [ZERO, ONE, ONE, ZERO],
        Pauli::Y => [ZERO, C(0., -1.), I, ZERO],
        Pauli::Z => [ONE, ZERO, ZERO, C(-1., 0.)],
    }
}
fn u(t: f64, p: f64, l: f64) -> M {
    let (s, c) = (t / 2.).sin_cos();
    [
        C(c, 0.),
        C::phase(l) * C(-s, 0.),
        C::phase(p) * C(s, 0.),
        C::phase(p + l) * C(c, 0.),
    ]
}
#[derive(Clone)]
struct Branch {
    matrix: M,
    record: Vec<bool>,
    bits: Vec<bool>,
}
fn choi(branches: Vec<Branch>) -> BTreeMap<Vec<bool>, Vec<C>> {
    let mut out = BTreeMap::new();
    for b in branches {
        if b.matrix.iter().all(|v| v.abs() < 1e-14) {
            continue;
        }
        let mut key = b.record;
        key.extend(b.bits);
        let matrix = out.entry(key).or_insert_with(|| vec![ZERO; 16]);
        for row in 0..4 {
            for col in 0..4 {
                matrix[row * 4 + col] =
                    matrix[row * 4 + col] + b.matrix[row] * b.matrix[col].conj();
            }
        }
    }
    out
}
fn exported_instrument(text: &str) -> BTreeMap<Vec<bool>, Vec<C>> {
    let circuit = from_qasm(text).unwrap();
    assert_eq!(circuit.num_qubits(), 1);
    let mut branches = vec![Branch {
        matrix: ID,
        record: vec![],
        bits: vec![false; circuit.num_classical_bits()],
    }];
    for op in circuit.operations() {
        match &op.kind {
            OperationKind::Gate { gate, .. } => {
                let m = match gate {
                    Gate::U { theta, phi, lambda } => u(theta.value(), phi.value(), lambda.value()),
                    _ => panic!("export expected primitive U"),
                };
                for b in &mut branches {
                    b.matrix = mul(m, b.matrix);
                }
            }
            OperationKind::Measure { bit, .. } => {
                branches = branches
                    .into_iter()
                    .flat_map(|b| {
                        [false, true].into_iter().map(move |v| {
                            let mut next = b.clone();
                            let p = if v {
                                [ZERO, ZERO, ZERO, ONE]
                            } else {
                                [ONE, ZERO, ZERO, ZERO]
                            };
                            next.matrix = mul(p, next.matrix);
                            next.record.push(v);
                            next.bits[*bit] = v;
                            next
                        })
                    })
                    .collect();
            }
            OperationKind::Reset { .. } => {
                branches = branches
                    .into_iter()
                    .flat_map(|b| {
                        [[ONE, ZERO, ZERO, ZERO], [ZERO, ONE, ZERO, ZERO]]
                            .into_iter()
                            .map(move |m| {
                                let mut next = b.clone();
                                next.matrix = mul(m, next.matrix);
                                next
                            })
                    })
                    .collect();
            }
            OperationKind::Conditional {
                bit, value, gate, ..
            } => {
                for b in &mut branches {
                    if b.bits[*bit] == *value {
                        b.matrix = mul(pauli(*gate), b.matrix);
                    }
                }
            }
            OperationKind::Barrier { .. } => {}
        }
    }
    choi(branches)
}
fn close_instruments(a: BTreeMap<Vec<bool>, Vec<C>>, b: BTreeMap<Vec<bool>, Vec<C>>) {
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (key, want) in a {
        for (x, y) in want.iter().zip(&b[&key]) {
            assert!(
                (*x - *y).abs() < 1e-12 + 1e-11 * x.abs(),
                "{key:?}: {x:?} vs {y:?}"
            );
        }
    }
}

#[test]
fn all_measurement_bases_and_inversions_preserve_projectors_and_final_quantum_states() {
    for (name, basis, reset) in [
        ("M", Pauli::Z, false),
        ("MZ", Pauli::Z, false),
        ("MX", Pauli::X, false),
        ("MY", Pauli::Y, false),
        ("MR", Pauli::Z, true),
        ("MRZ", Pauli::Z, true),
        ("MRX", Pauli::X, true),
        ("MRY", Pauli::Y, true),
    ] {
        for inverted in [false, true] {
            let target = if inverted {
                StimTarget::QubitInv(0)
            } else {
                qubit(0)
            };
            let output = export(&[op(name, vec![target])]);
            assert_eq!(output.classical_registers.len(), 1);
            let mut expected = Vec::new();
            let plus = match basis {
                Pauli::Z => [ONE, ZERO],
                Pauli::X => [C(FRAC_1_SQRT_2, 0.), C(FRAC_1_SQRT_2, 0.)],
                Pauli::Y => [C(FRAC_1_SQRT_2, 0.), C(0., FRAC_1_SQRT_2)],
            };
            let minus = match basis {
                Pauli::Z => [ZERO, ONE],
                Pauli::X => [C(FRAC_1_SQRT_2, 0.), C(-FRAC_1_SQRT_2, 0.)],
                Pauli::Y => [C(FRAC_1_SQRT_2, 0.), C(0., -FRAC_1_SQRT_2)],
            };
            for value in [false, true] {
                let eigen = if value ^ inverted { minus } else { plus };
                let ket = if reset { plus } else { eigen };
                let matrix = [
                    ket[0] * eigen[0].conj(),
                    ket[0] * eigen[1].conj(),
                    ket[1] * eigen[0].conj(),
                    ket[1] * eigen[1].conj(),
                ];
                expected.push(Branch {
                    matrix,
                    record: vec![value],
                    bits: vec![value],
                });
            }
            close_instruments(choi(expected), exported_instrument(&output.text));
        }
    }
}

#[test]
fn reset_bases_are_unobserved_channels_with_no_added_measurement_records() {
    for (name, plus) in [
        ("R", [ONE, ZERO]),
        ("RZ", [ONE, ZERO]),
        ("RX", [C(FRAC_1_SQRT_2, 0.), C(FRAC_1_SQRT_2, 0.)]),
        ("RY", [C(FRAC_1_SQRT_2, 0.), C(0., FRAC_1_SQRT_2)]),
    ] {
        let output = export(&[op(name, vec![qubit(0)])]);
        assert!(output.classical_registers.is_empty());
        let expected = vec![
            Branch {
                matrix: [plus[0], ZERO, plus[1], ZERO],
                record: vec![],
                bits: vec![],
            },
            Branch {
                matrix: [ZERO, plus[0], ZERO, plus[1]],
                record: vec![],
                bits: vec![],
            },
        ];
        close_instruments(choi(expected), exported_instrument(&output.text));
    }
}

#[test]
fn extreme_finite_half_turns_fail_cleanly_or_export_without_panicking() {
    for angle in [
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        i64::MIN as f64,
        i64::MAX as f64,
        f64::MAX,
        -f64::MAX,
    ] {
        let result = export_stim(
            &[StimInstr::new("R_X", vec![angle], vec![qubit(0)])],
            &ExportOptions::default(),
            &QasmLimits::default(),
        );
        match result {
            Ok(output) => {
                from_qasm(&output.text).unwrap();
            }
            Err(error) => assert!(matches!(error.code, ErrorCode::Type | ErrorCode::Limit)),
        }
    }
}
