// SPDX-License-Identifier: MPL-2.0
// Three complete UnifiedTests methods from itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_unified_simulator.py.
#[allow(dead_code)]
mod common;
use common::{bits, dense_gate};
use rmagna::catalogue::builtin_phase_catalogue;
use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::instrument::{
    OriginalAssignment as Assignment, OriginalBranch, OriginalBranchCertificateOptions as Options,
    OriginalCallbackError, OriginalControl, OriginalError, OriginalFault, OriginalGate as G,
    OriginalInstrument, OriginalLimits, OriginalOperation, OriginalPauli as P,
};
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const COMPILERS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn phase(wire: i64, turns: i64) -> G {
    G::Phase {
        wires: vec![wire],
        turns,
    }
}
fn sum(terms: &[AffineKet], word: usize, width: usize, a: &ScalarArithmetic) -> ExactScalar {
    terms.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(
            &v,
            &k.prepare()
                .unwrap()
                .amplitude(&bits(width, word), a)
                .unwrap(),
        )
        .unwrap()
    })
}
fn compare(
    branch: &OriginalBranch,
    expected: &[ExactScalar],
    options: &Options<'_>,
    a: &ScalarArithmetic,
) {
    let ordinary = branch.expansion(a).unwrap();
    let cert = branch.compile_after_absorption(options, a).unwrap();
    let mut norm = ExactScalar::zero();
    assert_eq!(expected.len(), 1 << branch.width());
    for (word, value) in expected.iter().enumerate() {
        assert_eq!(sum(&ordinary, word, branch.width(), a), *value);
        assert_eq!(
            branch
                .amplitude(&bits(branch.width(), word), 4096, a)
                .unwrap(),
            *value
        );
        assert_eq!(sum(cert.states(), word, branch.width(), a), *value);
        norm = a
            .add(&norm, &a.multiply(&value.conjugate(), value).unwrap())
            .unwrap();
    }
    assert_eq!(branch.probability(a).unwrap(), norm);
    assert_eq!(*cert.probability(), norm);
}
#[test]
fn original_gaussian_internal_contraction_all_140_literal_path_sums() {
    let a = ScalarArithmetic::default();
    let mut rng = SplitMix64::new(9001);
    let mut cases = 0;
    let mut zeros = 0;
    let mut dependent = 0;
    let mut widths = [false; 4];
    for n in 0..7 {
        for _ in 0..20 {
            let width = (rng.next_u64() % 4) as usize;
            widths[width] = true;
            let mask = (1 << width) - 1;
            let columns = (0..n)
                .map(|_| rng.next_u64() as usize & mask)
                .collect::<Vec<_>>();
            let linear = (0..n)
                .map(|_| (rng.next_u64() % 4) as u8)
                .collect::<Vec<_>>();
            let upper = (0..n)
                .map(|j| rng.next_u64() as usize & ((1 << n) - 1) & !((1 << (j + 1)) - 1))
                .collect::<Vec<_>>();
            let base = rng.next_u64() as usize & mask;
            let global = (rng.next_u64() % 8) as i64;
            let mut expected = vec![ExactScalar::zero(); 1 << width];
            for path in 0usize..1 << n {
                let mut word = base;
                let mut q = 0i64;
                for j in 0..n {
                    if path >> j & 1 != 0 {
                        word ^= columns[j];
                        q += i64::from(linear[j]) + 2 * (upper[j] & path).count_ones() as i64;
                    }
                }
                expected[word] = a
                    .add(
                        &expected[word],
                        &a.omega(global + 2 * q, -(n as i64)).unwrap(),
                    )
                    .unwrap();
            }
            let ket = AffineKet::new(
                bits(width, base),
                columns.iter().map(|&x| bits(width, x)).collect(),
                QuadraticForm::new(0, linear, upper.iter().map(|&x| bits(n, x)).collect()).unwrap(),
                a.omega(global, -(n as i64)).unwrap(),
            )
            .unwrap();
            let contracted = ket.contract(&a).unwrap();
            if let Some(k) = &contracted {
                dependent += usize::from(k.columns().len() < n);
            } else {
                zeros += 1;
            }
            for (word, value) in expected.iter().enumerate() {
                assert_eq!(
                    contracted.as_ref().map_or(ExactScalar::zero(), |k| k
                        .prepare()
                        .unwrap()
                        .amplitude(&bits(width, word), &a)
                        .unwrap()),
                    *value,
                    "paths={n},width={width},word={word}"
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 140);
    assert_eq!(widths, [true; 4]);
    assert!(zeros > 0);
    assert!(dependent > 0);
}
#[test]
fn original_fixed_input_absorption_noise_and_two_term_ccz() {
    let a = ScalarArithmetic::default();
    let options = Options {
        catalogue: Some(builtin_phase_catalogue().unwrap()),
        ..Default::default()
    };
    for compiler in COMPILERS {
        let zero = CliffordCircuit::computational(bits(1, 0)).unwrap();
        let ir = OriginalInstrument::new(
            vec![31],
            vec![31],
            zero,
            vec![op("t", phase(31, 1))],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        let branch = ir.compile(&Assignment::default(), compiler, &a).unwrap();
        compare(
            &branch,
            &[ExactScalar::one(), ExactScalar::zero()],
            &options,
            &a,
        );
        assert_eq!(
            branch
                .compile_after_absorption(&options, &a)
                .unwrap()
                .statistics()
                .quotient_rank,
            Some(0)
        );
        for s in [2, -2] {
            let ir = OriginalInstrument::new(
                vec![0],
                vec![0],
                CliffordCircuit::computational(bits(1, 0)).unwrap(),
                vec![
                    op("h0", G::H(0)),
                    op("s", phase(0, s)),
                    op("h1", G::H(0)),
                    op("t", phase(0, 1)),
                ],
                vec![],
                OriginalLimits::default(),
                &a,
            )
            .unwrap();
            let mut values = vec![ExactScalar::one(), ExactScalar::zero()];
            dense_gate(C::H(0), &mut values, &a);
            dense_gate(
                C::S {
                    qubit: 0,
                    turns: if s == 2 { 1 } else { 3 },
                },
                &mut values,
                &a,
            );
            // Literal Y: (a,b) -> (-i*b,i*a). The negative-Y state changes sign.
            values = vec![
                a.multiply(&a.omega(-2, 0).unwrap(), &values[1]).unwrap(),
                a.multiply(&a.omega(2, 0).unwrap(), &values[0]).unwrap(),
            ];
            dense_gate(C::H(0), &mut values, &a);
            values[1] = a.multiply(&values[1], &a.omega(1, 0).unwrap()).unwrap();
            let assigned = Assignment {
                faults: vec![OriginalFault {
                    after: "s".into(),
                    wire: 0,
                    pauli: P::Y,
                }],
                ..Default::default()
            };
            compare(
                &ir.compile(&assigned, compiler, &a).unwrap(),
                &values,
                &options,
                &a,
            );
        }
        let plus = AffineKet::new(
            bits(3, 0),
            (0..3).map(|j| bits(3, 1 << j)).collect(),
            QuadraticForm::new(0, vec![0; 3], vec![bits(3, 0); 3]).unwrap(),
            a.omega(0, -3).unwrap(),
        )
        .unwrap();
        let ir = OriginalInstrument::new(
            vec![0, 1, 2],
            vec![0, 1, 2],
            plus,
            vec![op("ccz", G::Ccz(0, 1, 2))],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        let expected = (0..8)
            .map(|word| a.omega(if word == 7 { 4 } else { 0 }, -3).unwrap())
            .collect::<Vec<_>>();
        let branch = ir.compile(&Assignment::default(), compiler, &a).unwrap();
        compare(&branch, &expected, &options, &a);
        let cert = branch.compile_after_absorption(&options, &a).unwrap();
        assert_eq!(cert.statistics().quotient_rank, Some(3));
        assert_eq!(cert.statistics().certified_terms, 2);
        assert_eq!(cert.states().len(), 2);
    }
}
#[test]
fn original_causal_callback_rejects_future_and_hidden_parents_without_cached_results() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let future = OriginalControl {
        operation: "c".into(),
        parents: vec!["future".into()],
        version: "v1".into(),
        callback: Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        }),
    };
    assert!(
        matches!(OriginalInstrument::new(vec![0],vec![0],CliffordCircuit::computational(bits(1,0)).unwrap(),vec![op("c",G::Control)],vec![future],OriginalLimits::default(),&a),Err(OriginalError::Metadata{kind:"unknown or future parent",identity})if identity=="future")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let counter = calls.clone();
    let hidden = OriginalControl {
        operation: "c".into(),
        parents: vec!["a".into()],
        version: "v1".into(),
        callback: Arc::new(move |inputs| {
            counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(inputs.entries(), &[("a".into(), false)]);
            assert!(!inputs.get("a")?);
            inputs.get("hidden")?;
            Ok(vec![])
        }),
    };
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![
            op(
                "m",
                G::Measure {
                    factors: vec![(0, P::Z)],
                    inverted: false,
                    record: "a".into(),
                },
            ),
            op("c", G::Control),
        ],
        vec![hidden],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let assigned = Assignment {
        true_values: vec![("a".into(), false)],
        ..Default::default()
    };
    for compiler in COMPILERS {
        for _ in 0..2 {
            let result = ir.compile(&assigned, compiler, &a);
            assert!(
                matches!(result,Err(OriginalError::Callback{operation,error:OriginalCallbackError::UndeclaredParent(parent)})if operation=="c"&&parent=="hidden")
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}
