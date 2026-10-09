use rand::{SeedableRng, rngs::StdRng};
use rmatching::Matching;
use rstim::{error_analyzer::ErrorAnalyzer, parser::parse_lines, sampler::sample_batch};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = StdRng::seed_from_u64(0x5eed);

    // Parsing and seeded sampling work from an ordinary downstream crate.
    let bell = parse_lines("H 0\nCNOT 0 1\nM 0 1")?;
    let bell_samples = sample_batch(&bell, 32, &mut rng)?;
    for shot in 0..32 {
        assert_eq!(
            bell_samples.measurements.get(0, shot),
            bell_samples.measurements.get(1, shot),
            "Bell measurements must have even parity in shot {shot}"
        );
    }

    // This error has the same detector and observable symptom. A fixed seed
    // makes the example reproducible while still exercising both outcomes.
    let circuit = parse_lines(
        "R 0\n\
         X_ERROR(0.25) 0\n\
         M 0\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]",
    )?;
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)?;
    assert_eq!(dem.num_detectors(), 1);
    assert_eq!(dem.num_observables(), 1);

    let mut matching = Matching::from_dem(&dem.to_string())?;
    let samples = sample_batch(&circuit, 128, &mut rng)?;
    let mut observed_zero = false;
    let mut observed_one = false;
    for shot in 0..128 {
        let syndrome: Vec<u8> = (0..samples.detections.num_major())
            .map(|detector| u8::from(samples.detections.get(detector, shot)))
            .collect();
        let prediction = matching.decode(&syndrome);
        let actual = u8::from(samples.observable_flips.get(0, shot));

        assert_eq!(syndrome, vec![actual], "D0 and L0 differ in shot {shot}");
        assert_eq!(prediction, vec![actual], "decoder mismatch in shot {shot}");
        observed_zero |= actual == 0;
        observed_one |= actual == 1;
    }
    assert!(
        observed_zero && observed_one,
        "seeded run must exercise both outcomes"
    );

    println!("validated Bell parity and 128/128 observable predictions");
    Ok(())
}
