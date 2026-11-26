#[allow(dead_code)]
#[path = "../examples/support/cultivation_flow.rs"]
mod cultivation_flow;
#[path = "../examples/support/cultivation_proposal.rs"]
mod proposal;

use cultivation_flow::DetectorClosure;
use num_rational::BigRational;
use proposal::{Sampler, WeightKey};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::noise::{OriginalNoiseOptions, RandomSource, SplitMix64, original_noise_categories};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Fixture {
    schema: String,
    generator_sha256: String,
    law_sha256: String,
    protocol_sha256: String,
    models: Vec<Model>,
}
#[derive(Deserialize)]
struct Model {
    name: String,
    signatures: Vec<Vec<u64>>,
    physical_rate: String,
    closing_sites: usize,
    offset: u64,
    complete_patterns: usize,
    transcripts: Vec<Transcript>,
}
type Transcript = (Vec<(u64, u64)>, Vec<usize>, String);

fn sampler(signatures: &[Vec<u64>], offset: u64, closing: Option<usize>, p: &str) -> Sampler {
    let circuit = parse_circuit("X_ERROR(1/8) 0", &ParseOptions::default()).unwrap();
    let template = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    let categories = signatures
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let mut site = template[0].clone();
            // Gaps model internal undeclared readout sites. Returned faults must
            // use these identities rather than the compact original-site index.
            site.internal_site = 2 * index + 1;
            site.channels = vec![site.channels[0].clone(); row.len()];
            site
        })
        .collect::<Vec<_>>();
    let bits = |value| BitVector::from_words(4, vec![value]).unwrap();
    let closure = closing.map(|count| {
        (
            DetectorClosure {
                signatures: signatures
                    .iter()
                    .map(|row| row.iter().copied().map(bits).collect())
                    .collect(),
                offset: bits(offset),
                retained: (0..4).collect(),
                unsafe_t: Vec::new(),
                signature_words: signatures.iter().map(Vec::len).sum(),
            },
            count,
        )
    });
    Sampler::new(&categories, p.parse().unwrap(), closure).unwrap()
}

#[test]
fn closing_draws_match_independent_complete_branch_and_cdf_oracles() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("data/cultivation-proposal-ranges.json")).unwrap();
    assert_eq!(fixture.schema, "rmagna.cultivation-range-oracles.v1");
    for (bytes, expected) in [
        (
            include_bytes!("../../benchmarks/magna_reproduction/generate_cultivation_ranges.py")
                .as_slice(),
            &fixture.generator_sha256,
        ),
        (
            include_bytes!("../../benchmarks/magna_reproduction/paper_proposals.py").as_slice(),
            &fixture.law_sha256,
        ),
        (
            include_bytes!("../../benchmarks/magna_reproduction/paper_proposal_draws.py")
                .as_slice(),
            &fixture.protocol_sha256,
        ),
    ] {
        assert_eq!(&format!("{:x}", Sha256::digest(bytes)), expected);
    }
    let mut total = 0;
    for model in fixture.models {
        let sampler = sampler(
            &model.signatures,
            model.offset,
            Some(model.closing_sites),
            &model.physical_rate,
        );
        let mut patterns = BTreeSet::new();
        for (requests, expected, weight) in model.transcripts {
            let mut requests = requests.into_iter();
            let (faults, key) = sampler
                .draw_ranges(|bound| {
                    let (expected_bound, value) = requests.next().expect("extra range request");
                    assert_eq!(bound, expected_bound, "{}", model.name);
                    Ok(value)
                })
                .unwrap();
            assert!(requests.next().is_none(), "unconsumed range request");
            let mut pattern = vec![0; model.signatures.len()];
            for fault in faults {
                assert_eq!(fault.site % 2, 1);
                pattern[(fault.site - 1) / 2] = fault.alternative + 1;
            }
            assert_eq!(pattern, expected, "{}", model.name);
            assert_eq!(
                sampler.likelihood(key).unwrap(),
                weight.parse::<BigRational>().unwrap()
            );
            patterns.insert(pattern);
            total += 1;
        }
        if model.name == "all225" {
            assert_eq!(patterns.len(), 3 * 225);
        } else {
            assert_eq!(patterns.len(), model.complete_patterns);
        }
    }
    assert_eq!(total, 4737);
}

struct Words {
    words: Vec<u64>,
    used: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let result = self.words[self.used];
        self.used += 1;
        result
    }
}

#[test]
fn physical_ranges_retain_exact_endpoints_and_bounded_rejection_entropy() {
    let signatures = vec![vec![1], vec![0], vec![0], vec![2]];
    let sampler = sampler(&signatures, 0, None, "1/3");
    assert_eq!(sampler.num_sites(), 4);
    let mut random = Words {
        words: vec![3, 0, 1, 2, 0],
        used: 0,
    };
    let draw = sampler.draw(&mut random, 5).unwrap();
    assert_eq!(draw.entropy_words, 5);
    assert_eq!(
        draw.faults.iter().map(|f| f.site).collect::<Vec<_>>(),
        [1, 7]
    );
    assert_eq!(
        sampler.likelihood(draw.weight).unwrap(),
        "1".parse().unwrap()
    );
    let bits = |value| BitVector::from_words(4, vec![value]).unwrap();
    let mut closure = DetectorClosure {
        signatures: signatures
            .iter()
            .map(|row| row.iter().copied().map(bits).collect())
            .collect(),
        offset: bits(3),
        retained: (0..4).collect(),
        unsafe_t: Vec::new(),
        signature_words: 4,
    };
    let mut residual = [0];
    assert!(sampler.passes_detector_constraints(&closure, &draw, &mut residual));
    closure.offset = bits(2);
    assert!(!sampler.passes_detector_constraints(&closure, &draw, &mut residual));
    let mut random = Words {
        words: vec![3; 4],
        used: 0,
    };
    assert!(
        sampler
            .draw(&mut random, 4)
            .err()
            .unwrap()
            .to_string()
            .contains("max_entropy_words")
    );
    assert_eq!(random.used, 4);
    assert!(sampler.draw_ranges(Ok).is_err());
    for (p, count) in [("0", 0), ("1", 4)] {
        let sampler = self::sampler(&signatures, 0, None, p);
        let mut random = Words {
            words: Vec::new(),
            used: 0,
        };
        let draw = sampler.draw(&mut random, 0).unwrap();
        assert_eq!(draw.faults.len(), count);
        assert_eq!(draw.entropy_words, 0);
    }
}

#[test]
fn physical_blocks_preserve_ranges_and_final_entropy_state() {
    let signatures = vec![vec![0, 1, 2]; 21];
    for rate in [
        "1/3",
        "1/1000",
        "3/4",
        "15000000000000000000/18446744073709551613",
    ] {
        let sampler = sampler(&signatures, 0, None, rate);
        for seed in 0..32 {
            for limit in [3, 4, 20, 21, 40, 100] {
                let mut expected_random = SplitMix64::new(seed);
                let mut used = 0;
                let expected = sampler.draw_ranges(|bound| {
                    let mask = u64::MAX >> (bound - 1).leading_zeros();
                    loop {
                        if used == limit {
                            return Err("reference entropy exhausted".into());
                        }
                        used += 1;
                        let value = expected_random.next_u64() & mask;
                        if value < bound {
                            return Ok(value);
                        }
                    }
                });
                let mut random = SplitMix64::new(seed);
                let draw = sampler.draw(&mut random, limit);
                match (draw, expected) {
                    (Ok(draw), Ok((faults, weight))) => {
                        assert_eq!(draw.faults, faults);
                        assert_eq!(draw.weight, weight);
                        assert_eq!(draw.entropy_words, used);
                    }
                    (Err(error), Err(_)) => {
                        assert!(error.to_string().contains("max_entropy_words"))
                    }
                    _ => {
                        panic!("physical block differs at rate {rate}, seed {seed}, limit {limit}")
                    }
                }
                assert_eq!(random.state(), expected_random.state());
            }
        }
    }
}

#[test]
fn impossible_closing_weight_descriptors_are_rejected() {
    let sampler = sampler(&vec![vec![0]; 4], 0, Some(2), "1/32");
    for key in [
        WeightKey {
            occurrences: 5,
            tail_candidates: 1,
            passing_candidates: 1,
            passes: true,
        },
        WeightKey {
            occurrences: 0,
            tail_candidates: 1,
            passing_candidates: 0,
            passes: false,
        },
        WeightKey {
            occurrences: 1,
            tail_candidates: 1,
            passing_candidates: 1,
            passes: false,
        },
        WeightKey {
            occurrences: 1,
            tail_candidates: 1,
            passing_candidates: 0,
            passes: true,
        },
        WeightKey {
            occurrences: 1,
            tail_candidates: 226,
            passing_candidates: 1,
            passes: true,
        },
    ] {
        assert!(sampler.likelihood(key).is_err());
    }
}
