// SPDX-License-Identifier: MPL-2.0
// Complete KernelContractTests.test_random_full_records_states_faults_and_moments_against_independent_matrices,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_kernel_contract.py.
use num_traits::ToPrimitive;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment as Assignment, OriginalBranchCertificateOptions, OriginalFault,
    OriginalGate as G, OriginalInstrument, OriginalLimits, OriginalNetworkOptions,
    OriginalOperation, OriginalOrigin, OriginalPauli as P, OriginalRecordParity,
    OriginalTraceAssignment, ReadoutBasis,
};
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| mask >> j & 1 != 0)).unwrap()
}
fn number(v: &Value) -> usize {
    v.as_u64().unwrap().try_into().unwrap()
}
fn string(v: &Value) -> String {
    v.as_str().unwrap().into()
}
fn pauli(v: char) -> P {
    match v {
        'X' => P::X,
        'Y' => P::Y,
        'Z' => P::Z,
        _ => panic!("unknown literal Pauli {v}"),
    }
}
fn integers(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|q| q.as_i64().unwrap())
        .collect()
}
fn operations(row: &Value) -> Vec<OriginalOperation> {
    row["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|op| {
            let qs = integers(&op["qubits"]);
            let gate = match op["kind"].as_str().unwrap() {
                "H" => G::H(qs[0]),
                "S" | "T" | "PHASE" => G::Phase {
                    wires: qs,
                    turns: op["exponent"].as_i64().unwrap(),
                },
                "CX" => G::Cx(qs[0], qs[1]),
                "CZ" => G::Cz(qs[0], qs[1]),
                "CCZ" => G::Ccz(qs[0], qs[1], qs[2]),
                "MP" => G::Measure {
                    factors: qs
                        .into_iter()
                        .zip(op["paulis"].as_str().unwrap().chars().map(pauli))
                        .collect(),
                    inverted: op["sign"] == 1,
                    record: string(&op["record"]),
                },
                "RESET_PLUS" => G::Reset {
                    wire: qs[0],
                    plus: true,
                },
                "MX_DESTRUCT" => G::Destructive {
                    wire: qs[0],
                    basis: ReadoutBasis::X,
                    record: string(&op["record"]),
                },
                kind => panic!("unknown original operation {kind}"),
            };
            OriginalOperation {
                id: string(&op["id"]),
                gate,
            }
        })
        .collect()
}
fn instrument(row: &Value, a: &ScalarArithmetic) -> OriginalInstrument {
    let k = &row["initial"];
    let width = number(&k["width"]);
    let columns = k["columns"].as_array().unwrap();
    let rank = columns.len();
    let initial = AffineKet::new(
        bits(width, number(&k["base"])),
        columns.iter().map(|c| bits(width, number(c))).collect(),
        QuadraticForm::new(
            0,
            k["linear"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| number(c) as u8)
                .collect(),
            k["upper"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| bits(rank, number(c)))
                .collect(),
        )
        .unwrap(),
        a.omega(k["phase"].as_i64().unwrap(), k["power"].as_i64().unwrap())
            .unwrap(),
    )
    .unwrap();
    OriginalInstrument::new(
        integers(&row["inputs"]),
        integers(&row["outputs"]),
        initial,
        operations(row),
        vec![],
        OriginalLimits::default(),
        a,
    )
    .unwrap()
}
fn assignment(row: &Value) -> Assignment {
    Assignment {
        true_values: row["true_values"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v == 1))
            .collect(),
        faults: row["faults"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| OriginalFault {
                after: string(&f["after"]),
                wire: f["qubit"].as_i64().unwrap(),
                pauli: pauli(f["pauli"].as_str().unwrap().chars().next().unwrap()),
            })
            .collect(),
        report_flips: row["report_flips"]
            .as_array()
            .unwrap()
            .iter()
            .map(string)
            .collect(),
    }
}
fn complex(value: &ExactScalar) -> [f64; 2] {
    let c = value.coefficients().each_ref().map(|c| c.to_f64().unwrap());
    let scale = 2.0_f64.powi(value.power().try_into().unwrap());
    [
        (c[0] + (c[1] - c[3]) * std::f64::consts::FRAC_1_SQRT_2) * scale,
        (c[2] + (c[1] + c[3]) * std::f64::consts::FRAC_1_SQRT_2) * scale,
    ]
}
fn pair(value: &Value) -> [f64; 2] {
    [value[0].as_f64().unwrap(), value[1].as_f64().unwrap()]
}
fn close(actual: [f64; 2], expected: [f64; 2], tolerance: f64, context: &str) {
    for (x, y) in actual.into_iter().zip(expected) {
        assert!(x.is_finite() && y.is_finite());
        assert!(
            (x - y).abs() <= tolerance,
            "{context}: {actual:?} != {expected:?}"
        );
    }
}
fn sum(terms: &[AffineKet], word: &BitVector, a: &ScalarArithmetic) -> ExactScalar {
    terms.iter().fold(ExactScalar::zero(), |total, k| {
        a.add(&total, &k.prepare().unwrap().amplitude(word, a).unwrap())
            .unwrap()
    })
}
#[test]
fn original_random_full_records_states_faults_and_moments_against_independent_matrices() {
    let data: Value =
        serde_json::from_str(include_str!("fixtures/original_kernel_matrices.json")).unwrap();
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(data["seed"], 26092393);
    assert_eq!(data["source_files"], 475);
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 96);
    // Independent global-phase anchor, not a probability-only norm comparison.
    close(
        pair(&rows[0]["vector"][0]),
        [-0.125, -0.125],
        4e-13,
        "literal original phase anchor",
    );
    let a = ScalarArithmetic::default();
    let mut counts = [0; 5];
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for trial in 0..24 {
            let cases = &rows[trial * 4..trial * 4 + 4];
            let ir = instrument(&cases[0], &a);
            let trace = ir.compile_trace(strategy, &a).unwrap();
            let events = [
                OriginalRecordParity {
                    records: vec!["a".into()],
                    inverted: false,
                },
                OriginalRecordParity {
                    records: vec!["b".into()],
                    inverted: false,
                },
            ];
            let query = trace
                .prepare_network_probability(&events, &OriginalNetworkOptions::default(), &a)
                .unwrap();
            let mut total = ExactScalar::zero();
            for (case, row) in cases.iter().enumerate() {
                assert_eq!(row["trial"], trial);
                let assignment = assignment(row);
                let branch = ir.compile(&assignment, strategy, &a).unwrap();
                assert_eq!(branch.outputs(), &[1, 0]);
                assert_eq!(branch.width(), 3);
                assert_eq!(branch.environment().len(), 1);
                assert_eq!(branch.environment()[0].wire, 1);
                assert_eq!(
                    branch.environment()[0].retired_by,
                    OriginalOrigin::Operation("reset".into())
                );
                assert_eq!(branch.records().len(), 2);
                for (record, (name, value)) in branch
                    .records()
                    .iter()
                    .zip([("a", case & 2 != 0), ("b", case & 1 != 0)])
                {
                    let flipped = row["report_flips"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|r| r.as_str().unwrap() == name)
                        .count()
                        % 2
                        != 0;
                    assert_eq!(record.true_value, value);
                    assert_eq!(record.reported_value, value ^ flipped);
                }
                let probability = branch.probability(&a).unwrap();
                close(
                    complex(&probability),
                    [row["probability"].as_f64().unwrap(), 0.],
                    4e-13,
                    "branch probability",
                );
                let view = trace
                    .with_assignment(
                        &OriginalTraceAssignment {
                            faults: assignment.faults.clone(),
                            report_flips: assignment.report_flips.clone(),
                        },
                        &a,
                    )
                    .unwrap();
                let outcome = BitVector::from_indices(
                    2,
                    branch
                        .records()
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| r.reported_value)
                        .map(|(i, _)| i),
                )
                .unwrap();
                assert_eq!(
                    query.evaluate(&view, &outcome, &a).unwrap().value,
                    probability
                );
                counts[0] += 1;
                total = a.add(&total, &probability).unwrap();
                let projected = branch.expansion(&a).unwrap();
                for word in 0..8 {
                    let actual = sum(&projected, &bits(3, word), &a);
                    close(
                        complex(&actual),
                        pair(&row["vector"][word]),
                        3e-12,
                        "projected branch amplitude",
                    );
                    counts[1] += 1;
                    if trial < 4 {
                        assert_eq!(
                            branch.amplitude(&bits(3, word), 1 << 20, &a).unwrap(),
                            actual
                        );
                        counts[4] += 1;
                    }
                }
                for (index, (x, z)) in [(1, 0), (1, 1), (0, 1), (3, 2)].into_iter().enumerate() {
                    let phase = 2 * ((x & z) as u32).count_ones() as u8;
                    let pauli = PauliWord::new(bits(2, x), bits(2, z), phase).unwrap();
                    close(
                        complex(&branch.output_moment(&pauli, &a).unwrap()),
                        pair(&row["moments"][index]),
                        4e-13,
                        "original output moment",
                    );
                    counts[2] += 1;
                }
                if trial < 4 {
                    let options = OriginalBranchCertificateOptions {
                        max_quotient_rank: 10,
                        max_terms: 1024,
                        ..Default::default()
                    };
                    let certificate = branch.compile_after_absorption(&options, &a).unwrap();
                    assert_eq!(certificate.probability(), &probability);
                    for word in 0..8 {
                        close(
                            complex(&sum(certificate.states(), &bits(3, word), &a)),
                            pair(&row["vector"][word]),
                            3e-12,
                            "absorbed certificate amplitude",
                        );
                    }
                    counts[3] += 1;
                }
            }
            assert_eq!(total, ExactScalar::one());
        }
    }
    assert_eq!(counts, [192, 1536, 768, 32, 256]);
}
