#[path = "../examples/support/cultivation_flow.rs"]
mod flow;

use rmagna::circuit::{Basis, ParseOptions, parse_circuit};
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};

fn prepare(text: &str) -> flow::DetectorClosure {
    let circuit = parse_circuit(text, &ParseOptions::default()).unwrap();
    let categories = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    flow::prepare(&circuit, &categories).unwrap()
}

#[test]
fn selected_signs_and_reported_feedback_resolve_to_the_correct_noise_signatures() {
    let text = "RX 0\nX 1\nX_ERROR(1/7) 0\nMX(1/7) 0\nCX rec[-1] 1\nM(1/7) 1\nDETECTOR rec[-1] rec[-2]\nPOSTSELECT !det[0]";
    let result = prepare(text);
    assert_eq!(result.retained, [0]);
    assert!(result.unsafe_t.is_empty());
    assert!(result.offset.is_zero());
    assert_eq!(result.signature_words, 3);
    assert_eq!(result.signatures.len(), 3);
    assert!(result.signatures[0][0].is_zero());
    // Flipping the first reported record also flips its feedback target, so
    // its two appearances cancel. True-record feedback would fail this check.
    assert!(result.signatures[1][0].is_zero());
    assert_eq!(result.signatures[2][0].ones().collect::<Vec<_>>(), [0]);
    let changed = prepare(&text.replace("!det[0]", "det[0]"));
    assert_eq!(changed.offset.ones().collect::<Vec<_>>(), [0]);
    assert_eq!(changed.signatures, result.signatures);
}

#[test]
fn constraints_that_anticommute_at_t_are_separate_from_exact_safe_constraints() {
    let result = prepare(
        "R 0\nX_ERROR(1/7) 0\nT 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nRX 1\nZ_ERROR(1/7) 1\nT_DAG 1\nMX 1\nDETECTOR rec[-1]\nPOSTSELECT det[1]",
    );
    assert_eq!(result.retained, [0]);
    assert_eq!(result.unsafe_t, [1]);
    assert!(result.offset.is_zero());
    assert_eq!(result.signatures[0][0].ones().collect::<Vec<_>>(), [0]);
    assert!(result.signatures[1][0].is_zero());
}

#[test]
fn detector_words_keep_unsafe_lanes_and_fault_signs_across_word_boundaries() {
    let mut text = String::from("R 0\nX_ERROR(1/7) 0\nM 0\nRX 1\nT 1\nMX 1\n");
    for index in 0..130 {
        let record = if index == 63 || index == 128 {
            "-1"
        } else {
            "-2"
        };
        text.push_str(&format!("DETECTOR rec[{record}]\n"));
    }
    text.push_str("POSTSELECT !det[0] det[63]");
    let result = prepare(&text);
    let safe: Vec<_> = (0..130)
        .filter(|index| ![63, 128].contains(index))
        .collect();
    assert_eq!(result.unsafe_t, [63, 128]);
    assert_eq!(result.retained, safe);
    assert_eq!(result.offset.ones().collect::<Vec<_>>(), safe);
    assert_eq!(result.signatures[0][0].ones().collect::<Vec<_>>(), safe);
    assert_eq!(result.signature_words, 3);
}

#[test]
fn y_preparation_and_adjoint_clifford_phases_keep_the_selected_baseline() {
    for prefix in ["RY 0", "R 0\nH 0\nS 0"] {
        let result = prepare(&format!(
            "{prefix}\nZ_ERROR(1/7) 0\nMY 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]"
        ));
        assert_eq!(result.retained, [0]);
        assert!(result.unsafe_t.is_empty());
        assert!(result.offset.is_zero());
        assert_eq!(result.signatures[0][0].ones().collect::<Vec<_>>(), [0]);
    }
}

#[test]
fn all_fifteen_two_qubit_channels_match_independent_bell_state_eigenvalues() {
    let probabilities = vec!["1/150"; 15].join(",");
    let text = format!(
        "R 0 1\nH 0\nCX 0 1\nPAULI_CHANNEL_2({probabilities}) 0 1\nMPP X0*X1 Z0*Z1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nPOSTSELECT det[0] det[1]"
    );
    let circuit = parse_circuit(&text, &ParseOptions::default()).unwrap();
    let categories = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    let result = flow::prepare(&circuit, &categories).unwrap();
    assert_eq!(result.retained, [0, 1]);
    assert!(result.offset.is_zero());
    assert!(result.unsafe_t.is_empty());
    assert_eq!(result.signatures[0].len(), 15);
    let mut syndromes = [0; 4];
    for (channel, signature) in categories[0].channels.iter().zip(&result.signatures[0]) {
        let OriginalNoiseAction::Pauli { factors } = &channel.action else {
            panic!("Pauli channel required")
        };
        // Independent dense amplitudes of |00>+|11>, using integer complex
        // pairs and the standard 2x2 X/Y/Z matrices. No flow helper is used.
        let mut state = [(1i32, 0i32), (0, 0), (0, 0), (1, 0)];
        for &(qubit, basis) in factors {
            let mut next = [(0, 0); 4];
            for (index, (real, imag)) in state.into_iter().enumerate() {
                let bit = (index >> qubit) & 1;
                let (target, value) = match basis {
                    Basis::X => (index ^ (1 << qubit), (real, imag)),
                    Basis::Z => (
                        index,
                        if bit == 0 {
                            (real, imag)
                        } else {
                            (-real, -imag)
                        },
                    ),
                    Basis::Y => (
                        index ^ (1 << qubit),
                        if bit == 0 {
                            (-imag, real)
                        } else {
                            (imag, -real)
                        },
                    ),
                };
                next[target] = value;
            }
            state = next;
        }
        let index = state.iter().position(|&value| value != (0, 0)).unwrap();
        let zz_negative = index.count_ones() % 2 == 1;
        let a = state[index];
        let xx_negative = state[index ^ 3] == (-a.0, -a.1);
        assert!(state[index ^ 3] == a || xx_negative);
        assert_eq!(signature.get(0).unwrap(), zz_negative);
        assert_eq!(signature.get(1).unwrap(), xx_negative);
        syndromes[usize::from(zz_negative) + 2 * usize::from(xx_negative)] += 1;
    }
    assert_eq!(syndromes, [3, 4, 4, 4]);
}

#[test]
fn random_unselected_and_oversized_constraints_fail_explicitly() {
    for (text, message) in [
        (
            "RX 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]".to_string(),
            "nondeterministic",
        ),
        (
            "R 0\nM 0\nDETECTOR rec[-1]".to_string(),
            "unselected detector",
        ),
        (
            format!("R 0\nM 0\n{}", "DETECTOR rec[-1]\n".repeat(4097)),
            "limits",
        ),
    ] {
        let circuit = parse_circuit(&text, &ParseOptions::default()).unwrap();
        let categories =
            original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
        let error = flow::prepare(&circuit, &categories).err().unwrap();
        assert!(error.to_string().contains(message), "{error}");
    }
}
