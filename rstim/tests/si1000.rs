use rand::{SeedableRng, rngs::StdRng};
use rstim::codegen::surface_code::{rotated_memory_x_si1000, rotated_memory_z_si1000};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::{StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;
use rstim::showcase::dem_semantic_summary;
use rstim::transforms::{flattened, without_noise};

const P: f64 = 0.002;
const CASES: [Reference; 2] = [
    Reference {
        basis: "X",
        generate: rotated_memory_x_si1000,
        circuit: include_str!("fixtures/si1000/rotated_memory_x_d3_r3.stim"),
        dem: include_str!("fixtures/si1000/rotated_memory_x_d3_r3.dem"),
    },
    Reference {
        basis: "Z",
        generate: rotated_memory_z_si1000,
        circuit: include_str!("fixtures/si1000/rotated_memory_z_d3_r3.stim"),
        dem: include_str!("fixtures/si1000/rotated_memory_z_d3_r3.dem"),
    },
];

struct Reference {
    basis: &'static str,
    generate: fn(usize, usize, f64) -> Result<Vec<StimInstr>, String>,
    circuit: &'static str,
    dem: &'static str,
}

fn absolute_coords(circuit: &[StimInstr]) -> Vec<StimInstr> {
    let mut shift = [0.0; 3];
    let mut instructions = flattened(circuit);
    instructions.retain_mut(|instr| {
        if let StimInstr::Op {
            name,
            args,
            targets,
            ..
        } = instr
        {
            if name == "SHIFT_COORDS" {
                for (offset, delta) in shift.iter_mut().zip(args) {
                    *offset += *delta;
                }
                return false;
            }
            if name == "DETECTOR" {
                for (coord, offset) in args.iter_mut().zip(shift) {
                    *coord += offset;
                }
            }
            if matches!(name.as_str(), "DETECTOR" | "OBSERVABLE_INCLUDE") {
                targets.sort_unstable_by_key(|target| match target {
                    StimTarget::Rec(offset) => *offset,
                    _ => unreachable!("expected a measurement record"),
                });
            }
        }
        true
    });
    instructions
}

fn assert_schedule_matches(actual: &[StimInstr], expected: &[StimInstr], basis: &str) {
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual, expected,
            "memory-{basis} schedule instruction {index}"
        );
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "memory-{basis} schedule length"
    );
}

#[test]
fn noisy_schedules_match_pinned_stim_circuits() {
    for case in &CASES {
        let circuit = (case.generate)(3, 3, P).unwrap();
        let expected = parse_lines(case.circuit).unwrap();
        assert_schedule_matches(&absolute_coords(&circuit), &expected, case.basis);
    }
}

#[test]
fn noisy_detector_error_models_match_pinned_stim_models() {
    for case in &CASES {
        let circuit = (case.generate)(3, 3, P).unwrap();
        let actual = ErrorAnalyzer::circuit_to_dem(&absolute_coords(&circuit)).unwrap();
        let expected = DetectorErrorModel::parse(case.dem).unwrap();
        assert_eq!(actual.num_detectors(), expected.num_detectors());
        assert_eq!(actual.num_observables(), expected.num_observables());
        let actual = dem_semantic_summary(&actual);
        let expected = dem_semantic_summary(&expected);
        assert_eq!(actual.annotation_lines, expected.annotation_lines);
        assert_eq!(
            actual.error_probabilities.keys().collect::<Vec<_>>(),
            expected.error_probabilities.keys().collect::<Vec<_>>()
        );
        for (targets, p) in &expected.error_probabilities {
            let actual_p = actual.error_probabilities[targets];
            assert!(
                (actual_p - p).abs() < 1e-12,
                "memory-{} DEM {targets}: expected {p}, got {actual_p}",
                case.basis
            );
        }
    }
}

fn raw_parities(circuit: &[StimInstr]) -> Vec<Vec<bool>> {
    let shots = 32;
    let mut rng = StdRng::seed_from_u64(1327);
    let samples = sample_batch(circuit, shots, &mut rng).unwrap();
    let mut measurements = 0;
    let mut parities = Vec::new();
    for instr in circuit {
        let StimInstr::Op { name, targets, .. } = instr else {
            unreachable!("a reference schedule is flat");
        };
        match name.as_str() {
            "M" => measurements += targets.len(),
            "DETECTOR" | "OBSERVABLE_INCLUDE" => {
                parities.push(
                    (0..shots)
                        .map(|shot| {
                            targets.iter().fold(false, |parity, target| {
                                let StimTarget::Rec(offset) = target else {
                                    unreachable!("expected a measurement record");
                                };
                                let index =
                                    measurements.checked_add_signed(*offset as isize).unwrap();
                                parity ^ samples.measurements.get(index, shot)
                            })
                        })
                        .collect(),
                );
            }
            _ => {}
        }
    }
    parities
}

#[test]
fn noiseless_schedules_and_raw_parities_match_pinned_stim_circuits() {
    for case in &CASES {
        let circuit = absolute_coords(&(case.generate)(3, 3, 0.0).unwrap());
        let expected = without_noise(&parse_lines(case.circuit).unwrap());
        assert_schedule_matches(&circuit, &expected, case.basis);
        let actual = raw_parities(&circuit);
        assert_eq!(actual, raw_parities(&expected));
        assert_eq!(actual, vec![vec![false; 32]; 25]);
    }
}

#[test]
fn validates_si1000_base_probability_and_dimensions() {
    for case in &CASES {
        for p in [-0.001, 0.2001, f64::NAN, f64::INFINITY] {
            assert!((case.generate)(3, 3, p).unwrap_err().contains("[0, 0.2]"));
        }
        assert!((case.generate)(1, 3, P).unwrap_err().contains("distance"));
        assert!((case.generate)(3, 0, P).unwrap_err().contains("rounds"));
        assert!((case.generate)(2, 1, 0.2).is_ok());
    }
}
