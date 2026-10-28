//! Deterministic conformance evidence for the opt-in physical circuit subset.
use super::super::tests::{dataset_for, singleton_syndrome};
use super::super::*;
use super::*;
use rstim::ir::{StimInstr, StimTarget};

const NATIVE: &str = include_str!("../../../tests/fixtures/loss_visible_native_v2.stim");
const REFERENCE: &str =
    include_str!("../../../tests/fixtures/loss_visible_native_v2_reference.stim");

fn normalize(text: &str, subset: CircuitSubset) -> NormalizedLossCircuit {
    normalize_supported_circuit(
        &rstim::validation::parse_and_validate(text).unwrap(),
        subset,
    )
    .unwrap()
}

fn compile(text: &str, subset: CircuitSubset) -> CompiledCircuit {
    compile_circuit(&dataset_for(text), DecoderKind::EnvelopeMle, subset).unwrap()
}

#[test]
fn v2_keeps_native_pulses_noise_parameters_and_interleaved_records() {
    let original = rstim::validation::parse_and_validate(NATIVE).unwrap();
    let normalized = normalize(NATIVE, CircuitSubset::V2);
    let kept: Vec<_> = original
        .iter()
        .filter(|op| !matches!(op.name(), Some("LOSS" | "ML")))
        .cloned()
        .collect();
    let analysis: Vec<_> = normalized
        .analysis
        .iter()
        .filter(|op| !matches!(op.name(), Some("MPAD" | "M")))
        .cloned()
        .collect();
    assert_eq!(
        analysis, kept,
        "each original pulse and noise site must survive"
    );
    assert!(
        !normalized
            .noiseless
            .iter()
            .any(|op| { matches!(op.name(), Some("X_ERROR" | "Z_ERROR" | "LOSS")) })
    );
    assert_eq!(normalized.loss_flags, [0, 2, 4]);
    let probes: Vec<_> = normalized
        .probes
        .iter()
        .map(|p| {
            (
                p.qubit,
                p.flag_measurement,
                p.onset_sites.clone(),
                p.basis_sites.clone(),
                p.readout_site,
            )
        })
        .collect();
    assert_eq!(
        probes,
        [
            (0, 0, vec![4], vec![6, 8], 14),
            (1, 2, vec![13], vec![10, 13], 16),
            (2, 4, vec![13], vec![], 18),
        ]
    );
    assert_eq!(
        rstim::stats::summarize(&normalized.analysis).num_measurements,
        6
    );
}

#[test]
fn v2_keeps_noise_and_loss_opportunities_between_native_primitives() {
    let text = concat!(
        "R 0 1\nLOSS(0.01) 0\nZ[pulse-z] 0\n",
        "Z_ERROR[waiting](0.013) 0\nLOSS(0.02) 0\n",
        "SQRT_Y[pulse-y] 0\nX_ERROR[pulse-noise](0.017) 0\n",
        "CZ[native-cz] 0 1\nLOSS(0.03) 1\nML 0 1\n",
    );
    let original = rstim::validation::parse_and_validate(text).unwrap();
    let normalized = normalize(text, CircuitSubset::V2);
    assert_eq!(
        &normalized.analysis[..6],
        &[
            original[0].clone(),
            original[2].clone(),
            original[3].clone(),
            original[5].clone(),
            original[6].clone(),
            original[7].clone(),
        ]
    );
    assert_eq!(normalized.probes[0].onset_sites, [1, 2]);
    assert_eq!(normalized.probes[0].basis_sites, [3]);
    assert_eq!(normalized.probes[1].onset_sites, [4]);
}

#[test]
fn v2_accepts_the_issue_capability_input_without_relaxing_v1() {
    let text = concat!(
        "R 0 1\nLOSS(0.001) 0 1\nZ 0\nSQRT_Y 0\nCZ 0 1\n",
        "Z 0\nSQRT_Y 0\nZ_ERROR(0.001) 0\nX_ERROR(0.001) 1\nML 0 1\n",
        "DETECTOR(0,0,0) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-3]\n",
    );
    let normalized = normalize(text, CircuitSubset::V2);
    assert_eq!(normalized.loss_flags, [0, 2]);
    assert_eq!(normalized.probes[0].basis_sites, [3, 6]);
    let dem = rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem_decomposed(&normalized.analysis)
        .unwrap();
    let summary = rstim::showcase::dem_semantic_summary(&dem);
    assert_eq!(summary.error_probabilities.len(), 1);
    assert!((summary.error_probabilities.values().next().unwrap() - 0.001).abs() < 1e-12);

    for operation in ["Z 0", "SQRT_Y 0", "CZ 0 1", "Z_ERROR(0.01) 0"] {
        let text = format!("R 0 1\nLOSS(0.1) 0 1\n{operation}\nML 0 1\n");
        let instrs = rstim::validation::parse_and_validate(&text).unwrap();
        let error = normalize_supported_circuit(&instrs, CircuitSubset::V1)
            .err()
            .unwrap();
        assert_eq!(error.code, "unsupported_circuit");
        assert!(
            error
                .message
                .contains(operation.split([' ', '(']).next().unwrap())
        );
        assert!(error.message.contains("v1"));
        assert_eq!(
            normalize_supported_circuit(&instrs, CircuitSubset::V2)
                .unwrap()
                .loss_flags,
            [0, 2]
        );
    }
}

#[test]
fn v2_native_and_reference_ordinary_dems_agree_with_known_probabilities() {
    let native = normalize(NATIVE, CircuitSubset::V2);
    let reference = normalize(REFERENCE, CircuitSubset::V1);
    let dem = |c: &NormalizedLossCircuit| {
        rstim::showcase::dem_semantic_summary(
            &rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem_decomposed(&c.analysis).unwrap(),
        )
    };
    let actual = dem(&native);
    let expected = dem(&reference);
    assert_eq!(actual.annotation_lines, expected.annotation_lines);
    assert_eq!(
        actual.error_probabilities.keys().collect::<Vec<_>>(),
        expected.error_probabilities.keys().collect::<Vec<_>>()
    );
    for (effect, probability) in &actual.error_probabilities {
        assert!(
            (probability - expected.error_probabilities[effect]).abs() < 1e-12,
            "{effect}"
        );
    }
    // The first X fault and the waiting-time Z fault both reach D0 D1 L0.
    // Keeping their two physical locations gives the XOR probability below.
    assert!(
        (actual.error_probabilities["D0 D1 L0"] - (0.1 + 0.07 - 2.0 * 0.1 * 0.07)).abs() < 1e-12
    );
    assert_eq!(actual.error_probabilities.len(), 4);
}

#[test]
fn v2_loss_before_direct_cz_reaches_its_partner_but_late_loss_does_not() {
    let text = concat!(
        "R 0 1\nLOSS(0.1) 0\nZ 1\nSQRT_Y 1\nCZ 0 1\n",
        "Z 1\nSQRT_Y 1\nLOSS(0.1) 1\nML 0 1\n",
        "DETECTOR(0,0,0) rec[-3]\nDETECTOR(1,0,0) rec[-1]\n",
        "OBSERVABLE_INCLUDE(0) rec[-3]\n",
    );
    for (text, expected) in [
        (text.to_string(), vec![0, 1]),
        (
            text.replace("LOSS(0.1) 0\n", "")
                .replace("ML 0 1", "LOSS(0.1) 0\nML 0 1"),
            vec![0],
        ),
    ] {
        let normalized = normalize(&text, CircuitSubset::V2);
        let probe = &normalized.probes[0];
        let primitive =
            compile_primitive_effects(&normalized.noiseless, &normalized.probes).unwrap();
        let effect = &primitive.effects[&(probe.onset_sites[0], 0, "X_ERROR")];
        assert_eq!(effect.detectors, expected);
        assert_eq!(effect.observables, [0]);
    }
}

#[test]
fn v2_reset_closes_old_onsets_and_basis_sites_and_mrl_starts_a_new_lifetime() {
    let normalized = normalize(
        concat!(
            "R 0\nLOSS(0.1) 0\nSQRT_Y 0\nR 0\nLOSS(0.2) 0\n",
            "SQRT_Y 0\nMRL 0\nLOSS(0.3) 0\nZ 0\nSQRT_Y 0\nML 0\n",
        ),
        CircuitSubset::V2,
    );
    assert_eq!(normalized.loss_flags, [0, 2]);
    assert_eq!(normalized.probes[0].onset_sites, [3]);
    assert_eq!(normalized.probes[0].basis_sites, [4]);
    assert_eq!(normalized.probes[1].onset_sites, [6]);
    assert_eq!(normalized.probes[1].basis_sites, [8]);
    assert_eq!(normalized.probes[1].readout_site, 9);

    for reset in ["R", "RZ", "MRL", "MRZL"] {
        let text = format!("R 0\nLOSS(0.1) 0\nSQRT_Y 0\n{reset} 0\nML 0\n");
        let error = normalize_supported_circuit(
            &rstim::validation::parse_and_validate(&text).unwrap(),
            CircuitSubset::V2,
        )
        .err()
        .unwrap();
        assert!(
            error.message.contains("no LOSS opportunity since reset"),
            "{reset}: {error:?}"
        );
    }
    for op in ["Z 0", "SQRT_Y 0", "CZ 0 1", "Z_ERROR(0.01) 0", "R 0"] {
        let text = format!("R 0 1\nLOSS(0.1) 0\nML 0\n{op}\n");
        let error = normalize_supported_circuit(
            &rstim::validation::parse_and_validate(&text).unwrap(),
            CircuitSubset::V2,
        )
        .err()
        .unwrap();
        assert!(
            error.message.contains("ML must be terminal"),
            "{op}: {error:?}"
        );
    }
}

#[test]
fn v2_rejects_invalid_native_targets_and_unsupported_instructions() {
    for op in [
        "CZ 0",
        "CZ 0 0",
        "CZ 0 1 1 2",
        "CZ 0 1 0 2",
        "CZ rec[-1] 0",
        "CZ sweep[0] 0",
        "Z !0",
        "SQRT_Y !0",
        "Z_ERROR(0.1) !0",
        "Z 0 0",
        "SQRT_Y 0 0",
        "Z_ERROR(0.1) 0 0",
        "Z(0.1) 0",
        "SQRT_Y(0.1) 0",
        "CZ(0.1) 0 1",
        "Z_ERROR(-0.1) 0",
        "Z_ERROR(1.1) 0",
        "SQRT_Y_DAG 0",
        "ZCZ 0 1",
        "Y_ERROR(0.01) 0",
        "S 0",
        "SWAP 0 1",
    ] {
        let text = format!("R 0 1 2\nLOSS(0.1) 0 1 2\n{op}\nML 0 1 2\n");
        let error = compile_circuit(
            &dataset_for_unvalidated(&text),
            DecoderKind::EnvelopeMle,
            CircuitSubset::V2,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "unsupported_circuit", "{op}: {error:?}");
        assert!(!error.message.is_empty());
    }
    let text = NATIVE.replace("rec[-5]", "rec[-6]");
    let error = compile_circuit(
        &dataset_for(&text),
        DecoderKind::EnvelopeMle,
        CircuitSubset::V2,
    )
    .err()
    .unwrap();
    assert!(error.message.contains("flag"), "{error:?}");
}

// The compiler must reject parser/validation failures before consulting counts.
fn dataset_for_unvalidated(text: &str) -> super::super::dataset::Dataset {
    let mut dataset = dataset_for(NATIVE);
    dataset.circuit_text = text.to_string();
    // Populate correct counts whenever the syntax is valid so target-shape
    // rejection, rather than a manifest mismatch, is the negative control.
    if rstim::validation::parse_and_validate(text).is_ok() {
        dataset = dataset_for(text);
    }
    dataset
}

#[test]
fn v2_retains_hypergraph_observable_only_and_matching_ambiguity_boundaries() {
    let suffix = "ML 0 1 2\nDETECTOR(0,0,0) rec[-5]\nDETECTOR(1,0,0) rec[-3]\nDETECTOR(2,0,0) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-5]\n";
    let hyper = format!(
        "R 0 1 2\nX_ERROR(0.1) 0\nLOSS(0.1) 0 1 2\nZ 1 2\nSQRT_Y 1 2\nCZ 0 1\nCZ 0 2\nZ 1 2\nSQRT_Y 1 2\n{suffix}"
    );
    let error = compile_circuit(
        &dataset_for(&hyper),
        DecoderKind::EnvelopeMle,
        CircuitSubset::V2,
    )
    .err()
    .unwrap();
    assert!(error.message.contains("non-graphlike"), "{error:?}");
    let observable_only = NATIVE
        .replace("DETECTOR(0,0,0) rec[-5]\n", "")
        .replace("DETECTOR(1,0,0) rec[-3]\n", "");
    let error = compile_circuit(
        &dataset_for(&observable_only),
        DecoderKind::EnvelopeMle,
        CircuitSubset::V2,
    )
    .err()
    .unwrap();
    assert!(error.message.contains("observable-only"), "{error:?}");
    // Distinct logical labels on the same boundary edge remain ambiguous.
    let ambiguous = NATIVE
        .replace("X_ERROR(0.1) 0\n", "")
        .replace("Z_ERROR(0.07) 0\n", "")
        .replace("DETECTOR(0,0,0) rec[-5]", "DETECTOR(0,0,0) rec[-5] rec[-3]")
        .replace("DETECTOR(1,0,0) rec[-3]\n", "");
    let circuit = compile(&ambiguous, CircuitSubset::V2);
    let error = CompiledMatching::new(&circuit).err().unwrap();
    assert!(
        error.message.contains("ambiguous observable labels"),
        "{error:?}"
    );
}

#[test]
fn v2_no_flags_matches_the_ordinary_reference_for_every_three_detector_syndrome() {
    let native = compile(NATIVE, CircuitSubset::V2);
    let reference = compile(REFERENCE, CircuitSubset::V1);
    let mut matching = CompiledMatching::new(&native).unwrap();
    let mut ordinary = CompiledMatching::new(&reference).unwrap();
    #[cfg(feature = "ilp")]
    let mut mle = CompiledMle::new(&native, None).unwrap();
    for bits in 0..8 {
        let values: Vec<_> = (0..3).map(|q| ((bits >> q) & 1) as u8).collect();
        let syndrome = singleton_syndrome(&values);
        let expected: Vec<_> = if bits & 1 == 0 { vec![] } else { vec![0] };
        assert_eq!(matching.decode(&syndrome, &[]).unwrap(), expected);
        assert_eq!(ordinary.decode(&syndrome, &[]).unwrap(), expected);
        #[cfg(feature = "ilp")]
        {
            assert_eq!(mle.decode(&syndrome, &[]).unwrap(), expected);
            assert_eq!(
                super::super::tests::reference_mle(&native, &values, &[]),
                (bits & 1) as u64
            );
            assert_eq!(
                super::super::tests::reference_matching(&native, &[(&values, &[])]),
                [(bits & 1) as u64]
            );
        }
    }
    assert_eq!(matching.graph_builds(), 1);
}

#[test]
fn v2_decoders_ignore_all_flagged_placeholders_for_every_loss_pattern() {
    let circuit = compile(NATIVE, CircuitSubset::V2);
    let mut matching = CompiledMatching::new(&circuit).unwrap();
    #[cfg(feature = "ilp")]
    let mut mle = CompiledMle::new(&circuit, None).unwrap();
    // All 64 rows exercise eight loss patterns and every combination of
    // stored values, including multiple simultaneous lost placeholders.
    let mut rows = BitTable::new(6, 64);
    for row in 0..64 {
        for bit in 0..6 {
            rows.set(bit, row, row & (1 << bit) != 0);
        }
    }
    let syndromes = circuit.loss_aware_syndromes(&rows).unwrap();
    let losses = circuit.loss_patterns(&rows);
    for row in 0..64 {
        let canonical = (0..3).fold(row, |value, q| {
            if row & (1 << (2 * q)) != 0 {
                value | (1 << (2 * q + 1))
            } else {
                value
            }
        });
        assert_eq!(
            syndromes[row].canonical_detector_values,
            syndromes[canonical].canonical_detector_values
        );
        assert_eq!(syndromes[row].checks, syndromes[canonical].checks);
        assert_eq!(losses[row], losses[canonical]);
        let prediction = matching.decode(&syndromes[row], &losses[row]).unwrap();
        assert_eq!(
            prediction,
            matching
                .decode(&syndromes[canonical], &losses[canonical])
                .unwrap()
        );
        #[cfg(feature = "ilp")]
        {
            let prediction = mle.decode(&syndromes[row], &losses[row]).unwrap();
            assert_eq!(
                prediction,
                mle.decode(&syndromes[canonical], &losses[canonical])
                    .unwrap()
            );
        }
    }
    assert_eq!(matching.graph_builds(), 8);
    #[cfg(feature = "ilp")]
    assert_eq!(mle.model_builds(), 8);
}

/// An independent, small real-amplitude instrument oracle. None of the
/// stabilizer, frame, DEM, or loss-envelope machinery is used here. Branch
/// vectors stay unnormalized so their squared norms include outcome weights.
fn real_instrument(instrs: &[StimInstr], initial: Vec<f64>) -> Vec<(Vec<bool>, Vec<f64>)> {
    let mut branches = vec![(Vec::new(), initial)];
    let s = std::f64::consts::FRAC_1_SQRT_2;
    for op in instrs {
        let StimInstr::Op { name, targets, .. } = op else {
            panic!("flat oracle input required")
        };
        let qubits: Vec<_> = targets
            .iter()
            .map(|t| {
                if let StimTarget::Qubit(q) = t {
                    *q as usize
                } else {
                    panic!("plain targets required")
                }
            })
            .collect();
        match name.as_str() {
            "Z" | "SQRT_Y" | "H" => {
                let matrix = match name.as_str() {
                    "Z" => [[1.0, 0.0], [0.0, -1.0]],
                    "SQRT_Y" => [[s, -s], [s, s]],
                    "H" => [[s, s], [s, -s]],
                    _ => unreachable!(),
                };
                for (_, state) in &mut branches {
                    for &q in &qubits {
                        for i in 0..state.len() {
                            if i & (1 << q) != 0 {
                                continue;
                            }
                            let j = i | (1 << q);
                            let (a, b) = (state[i], state[j]);
                            state[i] = matrix[0][0] * a + matrix[0][1] * b;
                            state[j] = matrix[1][0] * a + matrix[1][1] * b;
                        }
                    }
                }
            }
            "CZ" | "CX" => {
                for (_, state) in &mut branches {
                    for pair in qubits.chunks_exact(2) {
                        for i in 0..state.len() {
                            if i & (1 << pair[0]) == 0 {
                                continue;
                            }
                            if name == "CZ" {
                                if i & (1 << pair[1]) != 0 {
                                    state[i] = -state[i];
                                }
                            } else if i & (1 << pair[1]) == 0 {
                                state.swap(i, i | (1 << pair[1]));
                            }
                        }
                    }
                }
            }
            "MPAD" => {
                for (records, _) in &mut branches {
                    records.extend(qubits.iter().map(|&bit| bit != 0));
                }
            }
            "M" | "MR" => {
                for &q in &qubits {
                    let mut next = Vec::new();
                    for (records, state) in branches {
                        for outcome in [false, true] {
                            let mut projected = vec![0.0; state.len()];
                            for (i, &amplitude) in state.iter().enumerate() {
                                if (i & (1 << q) != 0) == outcome {
                                    let output = if name == "MR" { i & !(1 << q) } else { i };
                                    projected[output] = amplitude;
                                }
                            }
                            if projected.iter().map(|v| v * v).sum::<f64>() > 1e-20 {
                                let mut records = records.clone();
                                records.push(outcome);
                                next.push((records, projected));
                            }
                        }
                    }
                    branches = next;
                }
            }
            other => panic!("unexpected oracle operation {other}"),
        }
    }
    branches
}

#[test]
fn v2_signed_clifford_and_reset_instruments_match_an_independent_amplitude_oracle() {
    let native = normalize(
        concat!(
            "Z 0\nSQRT_Y 0\nCZ 0 1\nZ 1\nSQRT_Y 1\nCZ 1 2\nSQRT_Y 2\n",
            "LOSS(0.1) 0 1 2\nMRL 0 1\nLOSS(0.1) 0 1\n",
            "Z 0\nSQRT_Y 1\nCZ 0 1\nML 0 1 2\n",
        ),
        CircuitSubset::V2,
    );
    // Hand-written reference: Z;SQRT_Y = H, SQRT_Y = Z;H,
    // and CZ = H(target);CX;H(target). No circuit rewrite is shared
    // with either the native compiler or the amplitude oracle.
    let reference = rstim::validation::parse_and_validate(concat!(
        "H 0\nH 1\nCX 0 1\nH 1\nH 1\nH 2\nCX 1 2\nH 2\nZ 2\nH 2\n",
        "MPAD 0\nMR 0\nMPAD 0\nMR 1\nZ 0\nZ 1\nH 1\nH 1\nCX 0 1\nH 1\n",
        "MPAD 0\nM 0\nMPAD 0\nM 1\nMPAD 0\nM 2\n",
    ))
    .unwrap();
    assert_eq!(native.loss_flags, [0, 2, 4, 6, 8]);
    // Basis states and coherent pairs expose signed Clifford mistakes that
    // unsigned Pauli propagation or computational-basis probabilities miss.
    for a in 0..8 {
        for b in a..8 {
            let mut initial = vec![0.0; 8];
            initial[a] = if a == b {
                1.0
            } else {
                std::f64::consts::FRAC_1_SQRT_2
            };
            if a != b {
                initial[b] = std::f64::consts::FRAC_1_SQRT_2;
            }
            let actual = real_instrument(&native.noiseless, initial.clone());
            let expected = real_instrument(&reference, initial);
            assert_eq!(actual.len(), expected.len(), "input {a},{b}");
            for ((records, state), (expected_records, expected_state)) in
                actual.iter().zip(&expected)
            {
                assert_eq!(records, expected_records, "input {a},{b}");
                for (&value, &expected) in state.iter().zip(expected_state) {
                    assert!(
                        (value - expected).abs() < 1e-12,
                        "input {a},{b}: {records:?}"
                    );
                }
            }
            assert!(
                (actual
                    .iter()
                    .flat_map(|(_, v)| v)
                    .map(|v| v * v)
                    .sum::<f64>()
                    - 1.0)
                    .abs()
                    < 1e-12
            );
        }
    }
    // Also check the implementation's signed noiseless reference against
    // the independently computed instrument's first nonzero outcome branch.
    let mut zero = vec![0.0; 8];
    zero[0] = 1.0;
    let oracle = real_instrument(&reference, zero);
    assert_eq!(
        rstim::executor::reference_sample(&native.noiseless).unwrap(),
        oracle[0].0
    );
    assert_eq!(
        rstim::executor::reference_sample(&reference).unwrap(),
        oracle[0].0
    );
}
