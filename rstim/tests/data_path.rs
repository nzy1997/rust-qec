use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::data_path::{build_reference_sample, ReferenceSampleMode};
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch, sample_batch_with_options, SampleOptions};
use rstim::sim::bit_table::BitTable;

fn assert_same_table(actual: &BitTable, expected: &BitTable) {
    assert_eq!(actual.num_major(), expected.num_major());
    assert_eq!(actual.num_minor(), expected.num_minor());
    for major in 0..actual.num_major() {
        for minor in 0..actual.num_minor() {
            assert_eq!(actual.get(major, minor), expected.get(major, minor));
        }
    }
}

#[test]
fn zero_reference_mode_returns_expected_measurement_width() {
    let instrs = parse_lines("X 0\nM 0\nM 0\n").unwrap();
    let sample = build_reference_sample(&instrs, ReferenceSampleMode::AssumeAllZero).unwrap();
    assert_eq!(sample, vec![false, false]);
}

#[test]
fn sample_batch_wrapper_matches_default_options() {
    let instrs = parse_lines("R 0\nM 0\n").unwrap();
    let mut rng_a = StdRng::seed_from_u64(7);
    let mut rng_b = StdRng::seed_from_u64(7);
    let wrapped = sample_batch(&instrs, 4, &mut rng_a).unwrap();
    let explicit = sample_batch_with_options(&instrs, 4, &mut rng_b, SampleOptions::default()).unwrap();
    assert_same_table(&wrapped.measurements, &explicit.measurements);
    assert_same_table(&wrapped.detections, &explicit.detections);
    assert_same_table(&wrapped.observable_flips, &explicit.observable_flips);
}
