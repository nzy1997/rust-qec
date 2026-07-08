use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::FrameSimulator;

fn measurement_words(program: &str, batch_size: usize, seed: u64) -> Vec<u64> {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(1, batch_size);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    frame.measurements(&ref_sample).row_words(0).to_vec()
}

fn count_ones(words: &[u64]) -> u32 {
    words.iter().map(|word| word.count_ones()).sum()
}

fn valid_word_mask(batch_size: usize, word: usize) -> u64 {
    let remaining = batch_size.saturating_sub(word * 64);
    if remaining >= 64 {
        !0u64
    } else if remaining == 0 {
        0
    } else {
        (1u64 << remaining) - 1
    }
}

fn frame_source() -> &'static str {
    include_str!("../src/sim/frame.rs")
}

fn match_arm(source: &str, start_marker: &str, end_marker: &str) -> String {
    let start = source.find(start_marker).expect("start marker present");
    let tail = &source[start..];
    let end = tail.find(end_marker).expect("end marker present");
    tail[..end].to_string()
}

#[test]
fn noise_mask_p_zero_is_empty() {
    let words = measurement_words("X_ERROR(0) 0\nM 0\n", 130, 7);
    assert_eq!(words, vec![0, 0, 0]);
}

#[test]
fn noise_mask_p_one_is_all_ones() {
    let batch_size = 130;
    let words = measurement_words("X_ERROR(1) 0\nM 0\n", batch_size, 7);
    assert_eq!(words.len(), 3);
    assert_eq!(words[0], valid_word_mask(batch_size, 0));
    assert_eq!(words[1], valid_word_mask(batch_size, 1));
    assert_eq!(words[2], valid_word_mask(batch_size, 2));
}

#[test]
fn low_probability_noise_mask_has_expected_frequency_bounds() {
    let batch_size = 65_536;
    let words = measurement_words("X_ERROR(0.01) 0\nM 0\n", batch_size, 123);
    let hits = count_ones(&words);
    assert!(
        (550..=760).contains(&hits),
        "expected roughly 655 hits for p=0.01 over {batch_size} shots, got {hits}"
    );
}

#[test]
fn noise_mask_is_reproducible_for_seeded_rng() {
    let program = "X_ERROR(0.037) 0\nM 0\n";
    let first = measurement_words(program, 257, 99);
    let second = measurement_words(program, 257, 99);
    assert_eq!(first, second);
}

#[test]
fn depolarize1_event_mask_uses_integer_threshold_path() {
    let arm = match_arm(frame_source(), "\"DEPOLARIZE1\"", "\"DEPOLARIZE2\"");
    assert!(arm.contains("random_bits_with_prob"), "{arm}");
    assert!(!arm.contains("gen::<f64>() < p"), "{arm}");
    assert!(!arm.contains("r#gen::<f64>() < p"), "{arm}");
}

#[test]
fn depolarize2_event_mask_uses_integer_threshold_path() {
    let arm = match_arm(frame_source(), "\"DEPOLARIZE2\"", "\"CORRELATED_ERROR\"");
    assert!(arm.contains("random_bits_with_prob"), "{arm}");
    assert!(!arm.contains("gen::<f64>() < p"), "{arm}");
    assert!(!arm.contains("r#gen::<f64>() < p"), "{arm}");
}
