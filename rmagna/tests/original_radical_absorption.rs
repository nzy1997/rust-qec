// SPDX-License-Identifier: MPL-2.0
// Original inputs and finite assertions adapted from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_native_global_phase.py (SHA256
// b9d596e34dd777a83ac9aa8feb81e4a246d56bd4fae9db9be663f091927d6b50).
// License: fixtures/original_many_record_input.LICENSE.
// The literal Rust path sum and additional generic cases are independent.
use rmagna::catalogue::{PhaseSelectionOptions, builtin_phase_catalogue};
use rmagna::gf2::BitVector;
use rmagna::phase::{
    NativePhaseOptions, ParityPhase, PathTensorError, PhaseError, PhasePathTensor,
    RadicalAbsorptionOptions,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Scalar {
    coefficients: [i64; 4],
    power: i64,
}
impl Scalar {
    fn exact(&self, a: &ScalarArithmetic) -> ExactScalar {
        a.scalar(self.coefficients.map(Into::into), self.power)
            .unwrap()
    }
}
#[derive(Deserialize)]
struct Tensor {
    n: usize,
    linear: Vec<u8>,
    upper: Vec<usize>,
    terms: Vec<(usize, u8)>,
    constant: u8,
    power: i64,
    boundary: Vec<(usize, u8)>,
}
#[derive(Deserialize)]
struct Amplitude {
    label: usize,
    #[serde(flatten)]
    scalar: Scalar,
}
#[derive(Deserialize)]
struct Case {
    tensor: Tensor,
    coefficient: Option<Scalar>,
    expected: Vec<Amplitude>,
    norm: Scalar,
}
#[derive(Deserialize)]
struct Fixture {
    upstream_commit: String,
    module: String,
    module_sha256: String,
    method: Option<String>,
    seed: u64,
    cases: Vec<Case>,
}
fn bits(n: usize, value: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|&j| value >> j & 1 != 0)).unwrap()
}
fn load(text: &str, hash: &str, count: usize, seed: u64) -> Fixture {
    assert_eq!(format!("{:x}", Sha256::digest(text)), hash);
    let fixture: Fixture = serde_json::from_str(text).unwrap();
    assert_eq!(
        fixture.upstream_commit,
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture.module, "tests/core/test_native_global_phase.py");
    assert_eq!(
        fixture.module_sha256,
        "b9d596e34dd777a83ac9aa8feb81e4a246d56bd4fae9db9be663f091927d6b50"
    );
    assert_eq!(fixture.seed, seed);
    assert_eq!(fixture.cases.len(), count);
    fixture
}
fn raw(case: &Case, a: &ScalarArithmetic) -> PhasePathTensor {
    let t = &case.tensor;
    let m = t.boundary.len();
    let offset = BitVector::from_indices(
        m,
        t.boundary
            .iter()
            .enumerate()
            .filter_map(|(j, &(_, b))| (b != 0).then_some(j)),
    )
    .unwrap();
    let columns = (0..t.n)
        .map(|j| {
            BitVector::from_indices(
                m,
                t.boundary
                    .iter()
                    .enumerate()
                    .filter_map(|(k, &(mask, _))| (mask >> j & 1 != 0).then_some(k)),
            )
            .unwrap()
        })
        .collect();
    let coefficient = case
        .coefficient
        .as_ref()
        .map_or_else(ExactScalar::one, |c| c.exact(a));
    PhasePathTensor::new(
        AffineKet::new(
            offset,
            columns,
            QuadraticForm::new(
                0,
                t.linear.clone(),
                t.upper.iter().map(|&v| bits(t.n, v)).collect(),
            )
            .unwrap(),
            a.multiply(&coefficient, &a.omega(0, t.power).unwrap())
                .unwrap(),
        )
        .unwrap(),
        ParityPhase::new(
            t.n,
            t.constant,
            t.terms.iter().map(|&(axis, c)| (bits(t.n, axis), c)),
        )
        .unwrap(),
    )
    .unwrap()
}

// Enumerate the caller's literal paths, preserving dependent labels and all
// complex interference. No quotient, Gaussian contraction or resource is used.
fn dense(t: &PhasePathTensor, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let ket = t.boundary();
    let mut result = vec![ExactScalar::zero(); 1 << ket.width()];
    for u in 0..1usize << t.phase().num_variables() {
        let mut label = ket.offset().clone();
        for (j, col) in ket.columns().iter().enumerate() {
            if u >> j & 1 != 0 {
                label.xor_assign(col).unwrap();
            }
        }
        let y = label.ones().fold(0, |v, j| v | (1 << j));
        let q = ket.phase();
        let mut exponent = 2 * i64::from(q.constant()) + i64::from(t.phase().constant());
        for j in 0..q.num_variables() {
            if u >> j & 1 != 0 {
                exponent += 2 * i64::from(q.linear()[j]);
                for k in q.upper()[j].ones() {
                    exponent += 4 * ((u >> k & 1) as i64);
                }
            }
        }
        for (axis, c) in t.phase().terms() {
            let parity = axis.ones().fold(0, |s, j| s ^ (u >> j & 1));
            exponent += i64::from(*c) * parity as i64;
        }
        result[y] = a
            .add(
                &result[y],
                &a.multiply(ket.coefficient(), &a.omega(exponent, 0).unwrap())
                    .unwrap(),
            )
            .unwrap();
    }
    result
}
fn check_fixture(f: Fixture) {
    let a = ScalarArithmetic::default();
    let database = builtin_phase_catalogue().unwrap();
    let mut contractions = 0;
    let mut catalogue_routes = 0;
    let mut nonreal = 0;
    for (index, case) in f.cases.iter().enumerate() {
        let tensor = raw(case, &a);
        let before = tensor.clone();
        let mut expected = vec![ExactScalar::zero(); 1 << case.tensor.boundary.len()];
        for amp in &case.expected {
            expected[amp.label] = amp.scalar.exact(&a);
        }
        assert_eq!(dense(&tensor, &a), expected, "literal input {index}");
        let absorbed = tensor
            .absorb_clifford_radical(Default::default(), &a)
            .unwrap();
        contractions += absorbed.diagnostics().clifford_contractions;
        assert_eq!(tensor, before);
        let output = absorbed.tensor();
        assert_eq!(output.boundary().width(), tensor.boundary().width());
        assert_eq!(dense(output, &a), expected, "absorbed {index}");
        let again = output
            .absorb_clifford_radical(Default::default(), &a)
            .unwrap();
        assert_eq!(again.tensor(), output);
        assert_eq!(again.diagnostics().clifford_contractions, 0);
        let expected_norm = case.norm.exact(&a);
        for db in [None, Some(database)] {
            let resource = output
                .prepare_native_resource(256, Default::default(), db, &a)
                .unwrap();
            let Some(resource) = resource else {
                assert!(expected.iter().all(ExactScalar::is_zero));
                assert!(expected_norm.is_zero());
                continue;
            };
            catalogue_routes += usize::from(
                resource.native_resource().unwrap().receipt().route
                    == rmagna::catalogue::PhaseSelectionRoute::CertifiedCatalogue,
            );
            let states = resource
                .expand(output.phase(), output.boundary(), &a)
                .unwrap();
            let prepared = states
                .iter()
                .map(|s| s.prepare().unwrap())
                .collect::<Vec<_>>();
            for (y, value) in expected.iter().enumerate() {
                nonreal += usize::from(!value.is_real());
                let got = prepared.iter().fold(ExactScalar::zero(), |sum, ket| {
                    a.add(
                        &sum,
                        &ket.amplitude(&bits(output.boundary().width(), y), &a)
                            .unwrap(),
                    )
                    .unwrap()
                });
                assert_eq!(
                    &got,
                    value,
                    "expanded {index}, label {y}, catalogue {}",
                    db.is_some()
                );
            }
            // Include every cross term. Summing individual term norms would
            // lose interference in these deliberately noninjective tensors.
            let mut norm = ExactScalar::zero();
            for left in &states {
                for right in &states {
                    norm = a.add(&norm, &left.overlap(right, &a).unwrap()).unwrap();
                }
            }
            assert_eq!(norm, expected_norm, "coherent norm {index}");
        }
    }
    assert!(contractions > 0);
    assert!(catalogue_routes > 0);
    assert!(nonreal > 0);
}

#[test]
fn original_actual_absorption_and_decomposition_against_independent_dense_sums() {
    let fixture = load(
        include_str!("fixtures/original_global_phase192.json"),
        "384db19d4a91870e4958fea31bd534e4fc8f47fe09dc43b70cbd9befac710242",
        192,
        26091484,
    );
    assert_eq!(
        fixture.method.as_deref(),
        Some("test_actual_absorption_and_decomposition_against_independent_dense_sums")
    );
    check_fixture(fixture);
}
#[test]
fn generic384_complex_scalars_preserve_absorption_expansions_and_coherent_norms() {
    let fixture = load(
        include_str!("fixtures/generic_radical_absorption384.json"),
        "5c5bb80fb0d76bd657af1bbaae491aedd38733f123a0369bc3f775492d1893a7",
        384,
        2609148401,
    );
    assert!(fixture.method.is_none());
    check_fixture(fixture);
}

fn original_example(physical: bool, a: &ScalarArithmetic) -> PhasePathTensor {
    PhasePathTensor::new(
        AffineKet::new(
            bits(usize::from(physical) * 2, usize::from(physical) * 2),
            if physical {
                vec![bits(2, 1), bits(2, 2), bits(2, 0)]
            } else {
                vec![bits(0, 0); 3]
            },
            QuadraticForm::new(0, vec![0; 3], vec![bits(3, 4), bits(3, 4), bits(3, 0)]).unwrap(),
            a.omega(0, -3).unwrap(),
        )
        .unwrap(),
        ParityPhase::new(3, 5, [(bits(3, 1), 1), (bits(3, 2), 1)]).unwrap(),
    )
    .unwrap()
}
#[test]
fn absorption_precedes_final_resource_selection_without_claiming_full_native_exception_contract() {
    let a = ScalarArithmetic::default();
    let tensor = original_example(true, &a);
    assert_eq!(tensor.phase().quotient().unwrap().rank(), 2);
    let caps = NativePhaseOptions {
        selection: PhaseSelectionOptions {
            max_quotient_rank: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    // Existing streaming selection rejects the first independent direction.
    // Native reports needed=2, so its complete exception/certificate method
    // remains unmapped even though the numerical absorption is reproduced.
    assert!(tensor.prepare_native_resource(1, caps, None, &a).is_err());
    let result = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    assert_eq!(result.tensor().phase().num_variables(), 1);
    assert_eq!(result.tensor().phase().quotient().unwrap().rank(), 0);
    let resource = result
        .tensor()
        .prepare_native_resource(1, caps, None, &a)
        .unwrap()
        .unwrap();
    assert_eq!(resource.quotient_rank(), 0);
    assert_eq!(resource.num_terms(), 1);
    assert_eq!(dense(result.tensor(), &a), dense(&tensor, &a));
    assert_eq!(result.diagnostics().stages.len(), 1);
    assert_eq!(result.diagnostics().stages[0].hidden_paths, 1);
    assert_eq!(result.diagnostics().stages[0].signature_rank, 2);
    assert!(
        result
            .tensor()
            .prepare_native_resource(0, caps, None, &a)
            .is_err()
    );
    let table_zero = NativePhaseOptions {
        selection: PhaseSelectionOptions {
            max_core_table_bytes: 0,
            ..caps.selection
        },
        ..caps
    };
    assert!(
        result
            .tensor()
            .prepare_native_resource(1, table_zero, None, &a)
            .is_err()
    );
    assert!(
        result
            .tensor()
            .prepare_resource(1, Default::default(), None, &a)
            .is_ok()
    );
}

#[test]
fn only_jointly_hidden_paths_are_summed_and_shrinking_repeats_to_a_fixed_point() {
    let a = ScalarArithmetic::default();
    let tensor = PhasePathTensor::new(
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 0), bits(1, 1)],
            QuadraticForm::new(0, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
            ExactScalar::one(),
        )
        .unwrap(),
        ParityPhase::new(2, 3, [(bits(2, 1), 1)]).unwrap(),
    )
    .unwrap();
    let unchanged = tensor
        .absorb_clifford_radical(
            RadicalAbsorptionOptions {
                max_stages: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert_eq!(unchanged.tensor(), &tensor);
    assert_eq!(unchanged.diagnostics().joint_factorizations, 1);
    assert_eq!(unchanged.diagnostics().clifford_contractions, 0);
    let hidden = original_example(false, &a);
    let reduced = hidden
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    assert_eq!(reduced.diagnostics().stages.len(), 2);
    assert_eq!(reduced.diagnostics().clifford_contractions, 2);
    assert_eq!(reduced.tensor().phase().num_variables(), 0);
    assert_eq!(dense(reduced.tensor(), &a), dense(&hidden, &a));
    assert!(
        hidden
            .absorb_clifford_radical(
                RadicalAbsorptionOptions {
                    max_stages: 1,
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert!(
        hidden
            .absorb_clifford_radical(
                RadicalAbsorptionOptions {
                    max_stages: 2,
                    ..Default::default()
                },
                &a
            )
            .is_ok()
    );
}

#[test]
fn limits_are_typed_atomic_and_allow_exact_boundary_retries() {
    let a = ScalarArithmetic::default();
    let tensor = original_example(true, &a);
    let before = tensor.clone();
    let result = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    let d = result.diagnostics();
    for (options, expected) in [
        (
            RadicalAbsorptionOptions {
                max_physical_bits: 1,
                ..Default::default()
            },
            "radical_absorption.max_physical_bits",
        ),
        (
            RadicalAbsorptionOptions {
                max_path_bits: 2,
                ..Default::default()
            },
            "radical_absorption.max_path_bits",
        ),
        (
            RadicalAbsorptionOptions {
                max_stages: 0,
                ..Default::default()
            },
            "radical_absorption.max_stages",
        ),
        (
            RadicalAbsorptionOptions {
                max_work_units: d.declared_work_units - 1,
                ..Default::default()
            },
            "radical_absorption.max_work_units",
        ),
        (
            RadicalAbsorptionOptions {
                max_geometry_words: d.declared_peak_geometry_words - 1,
                ..Default::default()
            },
            "radical_absorption.max_geometry_words",
        ),
    ] {
        assert!(
            matches!(tensor.absorb_clifford_radical(options, &a), Err(PathTensorError::Algebra(PhaseError::ResourceLimit { option, .. })) if option == expected)
        );
        assert_eq!(tensor, before);
        assert_eq!(
            tensor
                .absorb_clifford_radical(Default::default(), &a)
                .unwrap(),
            result
        );
    }
    let exact = RadicalAbsorptionOptions {
        max_physical_bits: 2,
        max_path_bits: 3,
        max_stages: 1,
        max_work_units: d.declared_work_units,
        max_geometry_words: d.declared_peak_geometry_words,
    };
    assert_eq!(tensor.absorb_clifford_radical(exact, &a).unwrap(), result);
    let large = PhasePathTensor::new(
        AffineKet::new(
            tensor.boundary().offset().clone(),
            tensor.boundary().columns().to_vec(),
            tensor.boundary().phase().clone(),
            a.integer(7).unwrap(),
        )
        .unwrap(),
        tensor.phase().clone(),
    )
    .unwrap();
    let saved = large.clone();
    assert!(
        large
            .absorb_clifford_radical(Default::default(), &ScalarArithmetic::new(1).unwrap())
            .is_err()
    );
    assert_eq!(large, saved);
    assert!(
        large
            .absorb_clifford_radical(Default::default(), &a)
            .is_ok()
    );
}

#[test]
fn actual_signature_cancellation_scales_across_packed_words_without_enumeration() {
    let a = ScalarArithmetic::default();
    let n = 192;
    let coordinates = [0, 64, 127, 191];
    let phase = ParityPhase::new(
        n,
        7,
        (1..16).map(|mask| {
            (
                BitVector::from_indices(
                    n,
                    coordinates
                        .iter()
                        .enumerate()
                        .filter_map(|(j, &k)| (mask >> j & 1 != 0).then_some(k)),
                )
                .unwrap(),
                1,
            )
        }),
    )
    .unwrap();
    assert_eq!(phase.terms().len(), 15);
    assert_eq!(phase.quotient().unwrap().rank(), 0);
    let coefficient = a
        .scalar([1.into(), 2.into(), (-1).into(), 3.into()], -2)
        .unwrap();
    let tensor = PhasePathTensor::new(
        AffineKet::new(
            bits(1, 1),
            (0..n).map(|j| bits(1, usize::from(j == 0))).collect(),
            QuadraticForm::new(0, vec![0; n], vec![BitVector::zeros(n).unwrap(); n]).unwrap(),
            coefficient.clone(),
        )
        .unwrap(),
        phase,
    )
    .unwrap();
    let output = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    assert_eq!(output.diagnostics().stages[0].hidden_paths, 191);
    assert_eq!(output.diagnostics().stages[0].signature_rank, 0);
    assert_eq!(output.tensor().phase().num_variables(), 1);
    let expected = a
        .multiply(
            &coefficient,
            &a.multiply(
                &a.omega(7, 0).unwrap(),
                &a.scalar([1.into(), 0.into(), 0.into(), 0.into()], 191)
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(dense(output.tensor(), &a), vec![expected.clone(), expected]);
}

#[test]
fn explicit_and_interference_zeros_and_empty_paths_remain_raw_owned_tensors() {
    let a = ScalarArithmetic::default();
    for (coefficient, linear) in [(ExactScalar::zero(), 0), (ExactScalar::one(), 2)] {
        let tensor = PhasePathTensor::new(
            AffineKet::new(
                bits(2, 3),
                vec![bits(2, 0)],
                QuadraticForm::new(0, vec![linear], vec![bits(1, 0)]).unwrap(),
                coefficient,
            )
            .unwrap(),
            ParityPhase::new(1, 7, []).unwrap(),
        )
        .unwrap();
        let before = tensor.clone();
        let result = tensor
            .absorb_clifford_radical(Default::default(), &a)
            .unwrap();
        assert_eq!(tensor, before);
        assert_eq!(result.tensor().boundary().width(), 2);
        assert_eq!(result.tensor().phase().num_variables(), 0);
        assert!(result.tensor().boundary().coefficient().is_zero());
        assert_eq!(dense(&tensor, &a), dense(result.tensor(), &a));
        assert!(
            result
                .tensor()
                .prepare_native_resource(0, Default::default(), None, &a)
                .unwrap()
                .is_none()
        );
        assert_eq!(result.clone().into_tensor(), *result.tensor());
    }
    let empty = PhasePathTensor::new(
        AffineKet::new(
            bits(0, 0),
            vec![],
            QuadraticForm::new(3, vec![], vec![]).unwrap(),
            a.omega(1, -3).unwrap(),
        )
        .unwrap(),
        ParityPhase::new(0, 5, []).unwrap(),
    )
    .unwrap();
    let result = empty
        .absorb_clifford_radical(
            RadicalAbsorptionOptions {
                max_stages: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert_eq!(result.tensor(), &empty);
    assert_eq!(result.diagnostics().quotient_analyses, 1);
    assert_eq!(result.diagnostics().joint_factorizations, 0);
}

#[test]
fn packed_physical_labels_keep_order_and_nonreal_global_quadratic_scalars() {
    let a = ScalarArithmetic::default();
    let m = 193;
    let offset = BitVector::from_indices(m, [0, 100]).unwrap();
    let left = BitVector::from_indices(m, [0, 64]).unwrap();
    let right = BitVector::from_indices(m, [65, 192]).unwrap();
    let coefficient = a
        .scalar([2.into(), (-1).into(), 3.into(), 1.into()], -4)
        .unwrap();
    let tensor = PhasePathTensor::new(
        AffineKet::new(
            offset.clone(),
            vec![left.clone(), right.clone(), BitVector::zeros(m).unwrap()],
            QuadraticForm::new(3, vec![0; 3], vec![bits(3, 4), bits(3, 4), bits(3, 0)]).unwrap(),
            coefficient.clone(),
        )
        .unwrap(),
        ParityPhase::new(3, 5, [(bits(3, 1), 1), (bits(3, 2), 1)]).unwrap(),
    )
    .unwrap();
    let reduced = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    assert_eq!(reduced.tensor().boundary().width(), m);
    assert_eq!(reduced.tensor().phase().num_variables(), 1);
    let ket = reduced.tensor().contract_clifford(&a).unwrap().unwrap();
    let prepared = ket.prepare().unwrap();
    let mut other = offset.clone();
    other.xor_assign(&left).unwrap();
    other.xor_assign(&right).unwrap();
    // Sum x2 imposes x0=x1; each supported label receives twice its original
    // coefficient, including i^3, omega^5 and the remaining S phase.
    for (label, exponent) in [(offset.clone(), 11), (other, 13)] {
        let expected = a
            .multiply(
                &coefficient,
                &a.multiply(&a.integer(2).unwrap(), &a.omega(exponent, 0).unwrap())
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(prepared.amplitude(&label, &a).unwrap(), expected);
    }
    let mut impossible = offset;
    impossible.set(11, true).unwrap();
    assert!(prepared.amplitude(&impossible, &a).unwrap().is_zero());
}

#[test]
fn scalar_exponent_overflow_during_hidden_sum_is_typed_and_preserves_input() {
    let a = ScalarArithmetic::default();
    let tensor = PhasePathTensor::new(
        AffineKet::new(
            bits(0, 0),
            vec![bits(0, 0); 2],
            QuadraticForm::new(0, vec![0; 2], vec![bits(2, 0); 2]).unwrap(),
            a.scalar([1.into(), 0.into(), 0.into(), 0.into()], i64::MAX)
                .unwrap(),
        )
        .unwrap(),
        ParityPhase::new(2, 0, []).unwrap(),
    )
    .unwrap();
    let before = tensor.clone();
    let error = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap_err();
    assert!(matches!(
        error,
        PathTensorError::Algebra(PhaseError::Scalar(
            rmagna::scalar::ScalarError::ExponentOverflow
        )) | PathTensorError::Algebra(PhaseError::Stabilizer(
            rmagna::stabilizer::StabilizerError::Scalar(
                rmagna::scalar::ScalarError::ExponentOverflow
            )
        ))
    ));
    assert_eq!(tensor, before);
}
