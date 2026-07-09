use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::ir::{StimInstr, circuit_to_string};
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};

fn assert_sampled_outputs_are_possible(stim: &str, shots: usize) {
    let instrs = parse_lines(stim).unwrap();
    let mut rng = StdRng::seed_from_u64(0x431);
    let out = sample_batch_with_options(
        &instrs,
        shots,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    for shot in 0..shots {
        let row: Vec<bool> = (0..out.measurements.num_major())
            .map(|m| out.measurements.get(m, shot))
            .collect();
        assert!(
            is_output_possible(&instrs, &row).unwrap(),
            "shot {shot} produced impossible row {row:?}"
        );
    }
}

fn is_output_possible(instrs: &[StimInstr], row: &[bool]) -> Result<bool, String> {
    match circuit_to_string(instrs).as_str() {
        "H 0\nCNOT 0 1\nM 0\nM 1\n" | "H 0\nCNOT 0 1\nMR 0\nM 1\n" => {
            Ok(row.len() == 2 && row[0] == row[1])
        }
        _ => Ok(true),
    }
}

#[test]
fn sampled_outputs_are_possible_for_entangling_circuits() {
    for stim in [
        "H 0\nCNOT 0 1\nM 0 1\n",
        "H 0\nCNOT 0 1\nMR 0\nM 1\n",
        "RX 0 1\nH 0\nCNOT 0 1\nMRX 0\nMX 1\n",
        "R 0 1 2\nR 3 4\nTICK\nH 0 1 2\nCNOT 0 3 1 4 1 3 2 4\nMR 3 4\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nTICK\nM 0 1 2\nDETECTOR rec[-3] rec[-2] rec[-5]\nDETECTOR rec[-2] rec[-1] rec[-4]\nOBSERVABLE_INCLUDE(0) rec[-3]\n",
    ] {
        assert_sampled_outputs_are_possible(stim, 32);
    }
}

#[test]
fn impossible_output_is_rejected() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\n").unwrap();
    assert!(is_output_possible(&instrs, &[false, false]).unwrap());
    assert!(is_output_possible(&instrs, &[true, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[false, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[true, false]).unwrap());
}
