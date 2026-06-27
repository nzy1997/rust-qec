use rstim::codegen::surface_code::{
    rotated_memory_x, rotated_memory_z, unrotated_memory_x, unrotated_memory_z,
};
use rstim::stats;

#[test]
fn rotated_memory_x_d3_r1_qubit_count() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    // d=3 rotated: 9 data + 8 ancilla = 17 qubits
    assert_eq!(stats::num_qubits(&instrs), 17);
}

#[test]
fn rotated_memory_x_d3_r1_measurement_count() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    // 1 round: 8 ancilla MR + 9 final data M = 17 measurements
    assert_eq!(stats::num_measurements(&instrs), 17);
}

#[test]
fn rotated_memory_x_has_observable() {
    let instrs = rotated_memory_x(3, 1, 0.0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn rotated_memory_z_d3_r1() {
    let instrs = rotated_memory_z(3, 1, 0.0);
    assert_eq!(stats::num_qubits(&instrs), 17);
    assert_eq!(stats::num_measurements(&instrs), 17);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn rotated_memory_x_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = rotated_memory_x(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}

#[test]
fn rotated_memory_x_with_noise() {
    use rstim::ir::StimInstr;
    let instrs = rotated_memory_x(3, 1, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}

#[test]
fn unrotated_memory_x_d3_r1() {
    let instrs = unrotated_memory_x(3, 1, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn unrotated_memory_z_d3_r1() {
    let instrs = unrotated_memory_z(3, 1, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn unrotated_memory_x_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = unrotated_memory_x(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}

#[test]
fn cli_gen_surface_code_rotated_memory_x() {
    use rstim::codegen::surface_code::rotated_memory_x;
    // Verify the function is callable with the same signature as CLI would use
    let instrs = rotated_memory_x(3, 1, 0.0);
    assert!(!instrs.is_empty());
}

#[test]
fn surface_code_after_clifford_atom_loss() {
    use rstim::codegen::surface_code::rotated_memory_x_with_params;
    use rstim::codegen::NoiseParams;

    let instrs = rotated_memory_x_with_params(
        3,
        3,
        NoiseParams {
            after_clifford_loss_probability: 0.01,
            ..NoiseParams::none()
        },
    );

    assert_after_clifford_loss_layers(&instrs, 6, 12);
}

#[test]
fn unrotated_surface_code_after_clifford_atom_loss() {
    use rstim::codegen::surface_code::unrotated_memory_x_with_params;
    use rstim::codegen::NoiseParams;

    let instrs = unrotated_memory_x_with_params(
        3,
        3,
        NoiseParams {
            after_clifford_loss_probability: 0.01,
            ..NoiseParams::none()
        },
    );

    assert_after_clifford_loss_layers(&instrs, 6, 12);
}

fn assert_after_clifford_loss_layers(
    instrs: &[rstim::ir::StimInstr],
    expected_h_layers: usize,
    expected_cx_layers: usize,
) {
    use rstim::ir::StimInstr;

    let mut h_layers = 0usize;
    let mut cx_layers = 0usize;

    let mut index = 0usize;
    while index < instrs.len() {
        let StimInstr::Op { name, targets, .. } = &instrs[index] else {
            index += 1;
            continue;
        };

        if name == "H" {
            let mut layer_targets = Vec::new();
            while let Some(StimInstr::Op {
                name: h_name,
                targets,
                ..
            }) = instrs.get(index)
            {
                if h_name != "H" {
                    break;
                }
                layer_targets.extend_from_slice(targets);
                index += 1;
            }
            let Some(StimInstr::Op {
                name: loss_name,
                args: loss_args,
                targets: loss_targets,
                ..
            }) = instrs.get(index)
            else {
                panic!("H layer was not followed by an op near index {index}");
            };
            assert_eq!(loss_name, "LOSS", "H layer should be followed by LOSS");
            assert_eq!(
                loss_args,
                &vec![0.01],
                "LOSS should keep the configured probability"
            );
            assert_eq!(
                loss_targets, &layer_targets,
                "LOSS after H should target exactly the H layer targets"
            );
            h_layers += 1;
            continue;
        }

        if name == "CX" {
            let Some(StimInstr::Op {
                name: loss_name,
                args: loss_args,
                targets: loss_targets,
                ..
            }) = instrs.get(index + 1)
            else {
                panic!("CX layer was not followed by an op near index {index}");
            };
            assert_eq!(loss_name, "LOSS", "CX layer should be followed by LOSS");
            assert_eq!(
                loss_args,
                &vec![0.01],
                "LOSS should keep the configured probability"
            );
            assert_eq!(
                loss_targets, targets,
                "LOSS after CX should target exactly the CX layer targets"
            );
            cx_layers += 1;
        }

        index += 1;
    }

    assert_eq!(
        h_layers, expected_h_layers,
        "three rounds should each have two H layers"
    );
    assert_eq!(
        cx_layers, expected_cx_layers,
        "three rounds should each have four CX layers"
    );
}
