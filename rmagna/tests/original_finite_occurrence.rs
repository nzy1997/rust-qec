// SPDX-License-Identifier: MPL-2.0
// Finite occurrence portion of pinned Magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/native/rational_random.cpp,
// invoked by NoiseNumericFastpathTests.test_finite_noise_thresholds_and_general_conversion_in_native_module.
// The separate categorical-index and scalar-conversion assertions are not mapped here.
use num_traits::Zero;
use rmagna::probability::{OccurrenceThreshold, Probability};

#[allow(dead_code)]
#[path = "support/original_gf2.rs"]
mod original_gf2;
use original_gf2::Mt19937;

fn no_entropy() -> u64 {
    panic!("this endpoint or empty mask must not request entropy")
}

fn original_cases() -> [(&'static str, OccurrenceThreshold); 10] {
    use OccurrenceThreshold::{Always, Below, Never};
    [
        ("0", Never),
        ("1", Always),
        ("1/3", Below(0x5555_5555_5555_5555)),
        ("1/2", Below(1_u64 << 63)),
        ("1/18446744073709551616", Below(1)),
        ("1/36893488147419103232", Never),
        ("18446744073709551615/18446744073709551616", Below(u64::MAX)),
        (
            "18446744073709551614/18446744073709551615",
            Below(u64::MAX - 1),
        ),
        ("1/1000", Below(u64::MAX / 1000)),
        ("1/10000", Below(u64::MAX / 10000)),
    ]
}

#[test]
fn original_scalar_occurrence_is_lazy_and_preserves_exact_probabilities() {
    use OccurrenceThreshold::{Always, Below, Never};
    for (text, expected) in original_cases() {
        let probability: Probability = text.parse().unwrap();
        let exact_before = probability.value().clone();
        let threshold = probability.threshold_64();
        assert_eq!(threshold, expected, "original cutoff for {text}");
        match expected {
            Never => {
                assert!(!threshold.draw_with(&mut no_entropy));
                assert_eq!(threshold.draw_bits_with(u64::MAX, &mut no_entropy), 0);
            }
            Always => {
                assert!(threshold.draw_with(&mut no_entropy));
                assert_eq!(threshold.draw_bits_with(0x8123, &mut no_entropy), 0x8123);
            }
            Below(cutoff) => {
                let mut words = vec![(cutoff - 1, true), (cutoff, false)];
                if cutoff != u64::MAX {
                    words.push((cutoff + 1, false));
                }
                for (word, accepts) in words {
                    let mut calls = 0;
                    let mut provider = || {
                        calls += 1;
                        word
                    };
                    let dynamic: &mut dyn FnMut() -> u64 = &mut provider;
                    assert_eq!(threshold.draw_with(dynamic), accepts, "{text}, word={word}");
                    assert_eq!(calls, 1);
                    assert_eq!(threshold.accepts(word), accepts);
                }
            }
        }
        assert_eq!(threshold.draw_bits_with(0, &mut no_entropy), 0);
        assert_eq!(probability.value(), &exact_before);
        if text == "1/36893488147419103232" {
            assert!(!probability.value().is_zero());
            assert_eq!(threshold, Never);
        }
    }
    let zero_cutoff = Below(0);
    assert!(!zero_cutoff.draw_with(&mut no_entropy));
    assert_eq!(zero_cutoff.draw_bits_with(u64::MAX, &mut no_entropy), 0);
    assert_eq!(zero_cutoff.draw_bits_with(0, &mut no_entropy), 0);
}

// An ordinary scalar word determines a shrinking interval of possible values.
// This oracle uses interval endpoints, not the production pending-lane masks.
fn scalar_prefix_planes(word: u64, cutoff: u64) -> usize {
    for known in 0..=64 {
        let unknown = 64 - known;
        let low = (u128::from(word) >> unknown) << unknown;
        let high = low + ((1_u128 << unknown) - 1);
        if low >= u128::from(cutoff) || high < u128::from(cutoff) {
            return known;
        }
    }
    unreachable!("a complete scalar word is always decided")
}

#[test]
fn original_packed_occurrence_matches_all_560_scalar_prefix_trials() {
    use OccurrenceThreshold::{Always, Below, Never};
    let coins = [
        (Never, 0, false),
        (Below(1_u64 << 63), 1_u64 << 63, false),
        (Below(0x5555_5555_5555_5555), 0x5555_5555_5555_5555, false),
        (Below(1), 1, false),
        (Never, 0, false), // The original positive sub-bucket probability.
        (Below(u64::MAX), u64::MAX, false),
        (Always, 0, true),
    ];
    let mut rng = Mt19937::new(20260927);
    let mut comparisons = 0;
    let mut trials = 0;
    for (coin, cutoff, certain) in coins {
        for trial in 0..80 {
            let mut words: [u64; 64] = std::array::from_fn(|_| rng.next());
            words[0] = cutoff;
            words[1] = cutoff.wrapping_sub(1);
            words[2] = 0;
            words[3] = u64::MAX;
            let mut planes = [0_u64; 64];
            for (lane, &word) in words.iter().enumerate() {
                for (bit, plane) in planes.iter_mut().enumerate() {
                    *plane |= ((word >> (63 - bit)) & 1) << lane;
                }
            }
            let live = if trial == 0 { u64::MAX } else { rng.next() };
            let mut expected = 0;
            for (lane, &word) in words.iter().enumerate() {
                if certain || word < cutoff {
                    expected |= 1_u64 << lane;
                }
                comparisons += 1;
            }
            let expected_calls = if certain {
                0
            } else {
                words
                    .iter()
                    .enumerate()
                    .filter(|(lane, _)| live & (1_u64 << lane) != 0)
                    .map(|(_, &word)| scalar_prefix_planes(word, cutoff))
                    .max()
                    .unwrap_or(0)
            };
            let mut calls = 0;
            let actual = coin.draw_bits_with(live, &mut || {
                let plane = planes[calls];
                calls += 1;
                plane
            });
            assert_eq!(actual, expected & live, "coin={coin:?}, trial={trial}");
            assert_eq!(actual & !live, 0);
            assert_eq!(calls, expected_calls, "coin={coin:?}, trial={trial}");
            trials += 1;
        }
    }
    assert_eq!(trials, 560);
    assert_eq!(comparisons, 35_840);
    assert_eq!(rng.draws, 36_393);

    // Half resolves after one plane even when some prefixes remain equal.
    // Inactive lane bits cannot change either the answer or consumption.
    let half = Below(1_u64 << 63);
    let live = 0b101101;
    for plane in [0b100100, 0b100100 | !live] {
        let mut calls = 0;
        let actual = half.draw_bits_with(live, &mut || {
            calls += 1;
            assert_eq!(calls, 1, "no lower bit can affect a half-probability draw");
            plane
        });
        assert_eq!(actual, 0b001001);
        assert_eq!(calls, 1);
    }
    let mut calls = 0;
    let cutoff = u64::MAX - 1;
    assert_eq!(
        Below(cutoff).draw_bits_with(1, &mut || {
            assert!(calls < 63, "the trailing zero never needs a plane");
            let plane = (cutoff >> (63 - calls)) & 1;
            calls += 1;
            plane
        }),
        0
    );
    assert_eq!(calls, 63);
    assert_eq!(half.draw_bits_with(0, &mut no_entropy), 0);
}
