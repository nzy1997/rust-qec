use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sim::frame::FrameSimulator;

fn measurement_words(program: &str, batch_size: usize, seed: u64) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(seed);
    measurement_words_with_rng_impl(program, batch_size, &mut rng)
}

fn measurement_words_with_rng_impl(
    program: &str,
    batch_size: usize,
    rng: &mut impl RngCore,
) -> Vec<u64> {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut frame = FrameSimulator::new(1, batch_size);
    frame.run(&instrs, &ref_sample, rng).unwrap();
    frame.measurements(&ref_sample).row_words(0).to_vec()
}

struct CountingRng {
    inner: StdRng,
    core_calls: usize,
}

struct ScriptedRng {
    draws: Vec<u64>,
    next: usize,
}

impl ScriptedRng {
    fn from_u64s(draws: Vec<u64>) -> Self {
        Self { draws, next: 0 }
    }
}

impl CountingRng {
    fn seed_from_u64(seed: u64) -> Self {
        Self {
            inner: StdRng::seed_from_u64(seed),
            core_calls: 0,
        }
    }

    fn core_calls(&self) -> usize {
        self.core_calls
    }
}

impl RngCore for CountingRng {
    fn next_u32(&mut self) -> u32 {
        self.core_calls += 1;
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.core_calls += 1;
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_ne_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl RngCore for ScriptedRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let value = self.draws.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        value
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_ne_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn measurement_words_with_counting_rng(
    program: &str,
    batch_size: usize,
    seed: u64,
) -> (Vec<u64>, usize) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = CountingRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(1, batch_size);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    (
        frame.measurements(&ref_sample).row_words(0).to_vec(),
        rng.core_calls(),
    )
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
    for probability in [0.001, 0.037] {
        let program = format!("X_ERROR({probability}) 0\nM 0\n");
        let first = measurement_words(&program, 257, 99);
        let second = measurement_words(&program, 257, 99);
        assert_eq!(first, second, "p={probability}");
    }
}

#[test]
fn low_probability_noise_mask_uses_sparse_path() {
    let batch_size = 65_536;
    let (words, core_calls) =
        measurement_words_with_counting_rng("X_ERROR(0.001) 0\nM 0\n", batch_size, 123);
    let hits = count_ones(&words);
    assert!(
        (30..=110).contains(&hits),
        "expected roughly 66 hits for p=0.001 over {batch_size} shots, got {hits}"
    );
    assert!(
        core_calls < 8_192,
        "low-probability mask should jump between events instead of drawing once per bit; saw {core_calls} RNG core calls for {batch_size} shots"
    );
}

#[test]
fn medium_probability_noise_mask_keeps_dense_path() {
    let batch_size = 4_096;
    let (words, core_calls) =
        measurement_words_with_counting_rng("X_ERROR(0.3) 0\nM 0\n", batch_size, 321);
    let hits = count_ones(&words);
    assert!(
        (1_100..=1_350).contains(&hits),
        "expected roughly 1229 hits for p=0.3 over {batch_size} shots, got {hits}"
    );
    assert!(
        core_calls >= batch_size,
        "medium-probability mask should stay on the dense path; saw {core_calls} RNG core calls for {batch_size} shots"
    );
}

#[test]
fn sparse_path_can_set_measurement_bit_zero() {
    let mut rng = ScriptedRng::from_u64s(vec![u64::MAX, 1u64 << 11]);
    let words = measurement_words_with_rng_impl("X_ERROR(0.001) 0\nM 0\n", 1, &mut rng);
    assert_eq!(words, vec![1]);
}

#[test]
fn sparse_path_retries_zero_uniform_draw() {
    let mut rng = ScriptedRng::from_u64s(vec![0, u64::MAX, 1u64 << 11]);
    let words = measurement_words_with_rng_impl("X_ERROR(0.001) 0\nM 0\n", 1, &mut rng);
    assert_eq!(words, vec![1]);
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
