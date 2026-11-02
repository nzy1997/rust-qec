// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original finite identity and invalid-scale contracts from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_weighted_noise.py.
//! Exclusive masses retain residual identity; conditional weights must sum to one.
//! Typed Rust faults/CLI errors replace native labels/keywords, without RNG parity.
use num_rational::BigRational;
use num_traits::One;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{
    ConditionalLawError, ConditionalSiteLaw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler,
    NoiseSampling, RandomSource, SplitMix64,
};
use rmagna::probability::{Probability, ProbabilityError, ProbabilityParser};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const INVALID_SCALE_TOKENS: [&str; 6] = ["0", "-1", "True", "nan", "inf", "1/0"];
const ORIGINAL_SITE: &str = "X_ERROR(1/3) 0\n";

fn q(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn p(text: &str) -> Probability {
    text.parse().unwrap()
}
fn exclusive(weights: &[&str], options: &NoiseOptions) -> NoiseModel {
    let weights: Vec<_> = weights.iter().map(|text| p(text)).collect();
    let residual = BigRational::one() - weights.iter().map(Probability::value).sum::<BigRational>();
    let categories = std::iter::once(Probability::from_rational(residual).unwrap())
        .chain(weights)
        .collect();
    NoiseModel::new(vec![categories], options).unwrap()
}

struct Quarter {
    word: u64,
    calls: usize,
}
impl RandomSource for Quarter {
    fn next_u64(&mut self) -> u64 {
        assert_eq!(self.calls, 0, "a literal quarter needs exactly one word");
        self.calls += 1;
        self.word
    }
}

#[test]
fn zero_and_identity() {
    let options = NoiseOptions {
        max_entropy_words: 0,
        ..Default::default()
    };
    // Preserve the original seed3 endpoint vectors and returned labels1/-1.
    let mut rng = SplitMix64::new(3);
    for (weights, native_label, canonical) in [
        (["0", "1"], 1, ["0", "0", "1"]),
        (["0", "0"], -1, ["1", "0", "0"]),
    ] {
        let model = exclusive(&weights, &options);
        assert_eq!(
            model.probabilities(0).unwrap(),
            canonical.map(p),
            "exclusive weights {weights:?}"
        );
        let draw = NoiseSampler::physical(model.clone(), options)
            .draw(&mut rng)
            .unwrap();
        let expected = if native_label == -1 {
            vec![]
        } else {
            vec![FaultSelection {
                site: 0,
                alternative: 1,
            }]
        };
        assert_eq!(draw.faults, expected);
        assert_eq!(draw.physical_probability, q(1, 1));
        assert_eq!(draw.proposal_probability, q(1, 1));
        assert_eq!(draw.likelihood_ratio, q(1, 1));
        assert_eq!(model.probability(&draw.faults).unwrap(), q(1, 1));
        assert_eq!(draw.entropy_words, 0);
        assert_eq!(rng.state(), 3);
        for alternative in 0..2 {
            assert_eq!(
                model
                    .probability(&[FaultSelection {
                        site: 0,
                        alternative
                    }])
                    .unwrap(),
                if native_label == 1 && alternative == 1 {
                    q(1, 1)
                } else {
                    q(0, 1)
                }
            );
        }
    }
    // All-zero exclusive masses are valid residual identity, whereas all-zero
    // conditional weights violate that distinct interface's unit-sum contract.
    assert_eq!(
        ConditionalSiteLaw::new(2, q(1, 1), vec![q(0, 1), q(0, 1)]),
        Err(ConditionalLawError::WeightSum { sum: q(0, 1) })
    );

    let options = NoiseOptions {
        max_entropy_words: 1,
        ..Default::default()
    };
    let model = exclusive(&["1/4", "1/2"], &options);
    assert_eq!(
        model.probabilities(0).unwrap(),
        [p("1/4"), p("1/4"), p("1/2")]
    );
    let sampler = NoiseSampler::physical(model.clone(), options);
    let mut counts = [0; 3];
    for (word, choice, mass) in [
        (0, 0, q(1, 4)),
        (1, 1, q(1, 4)),
        (2, 2, q(1, 2)),
        (3, 2, q(1, 2)),
    ] {
        let mut rng = Quarter { word, calls: 0 };
        let draw = sampler.draw(&mut rng).unwrap();
        let expected = if choice == 0 {
            vec![]
        } else {
            vec![FaultSelection {
                site: 0,
                alternative: choice - 1,
            }]
        };
        assert_eq!(draw.faults, expected, "literal uniform quarter {word}");
        assert_eq!(draw.physical_probability, mass);
        assert_eq!(draw.proposal_probability, mass);
        assert_eq!(model.probability(&draw.faults).unwrap(), mass);
        assert_eq!(draw.likelihood_ratio, q(1, 1));
        assert_eq!((draw.entropy_words, rng.calls), (1, 1));
        counts[choice] += 1;
    }
    assert_eq!(counts, [1, 1, 2]);
}

struct CliSite(PathBuf);
impl CliSite {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-weighted-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("original.magna"), ORIGINAL_SITE).unwrap();
        Self(path)
    }
    fn run(&self, extra: &[&str], code: i32) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(["run", "original.magna", "--seed", "3", "--json"])
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "args={extra:?} stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["schema_version"], 2);
        value
    }
}
impl Drop for CliSite {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn invalid_scales_and_scaled_support() {
    let options = NoiseOptions::default();
    let law = ConditionalSiteLaw::new(1, q(1, 3), vec![q(1, 1)]).unwrap();
    let physical =
        NoiseModel::from_conditional_laws(std::slice::from_ref(&law), None, &options).unwrap();
    assert_eq!(physical.probabilities(0).unwrap(), [p("2/3"), p("1/3")]);
    for scale in [q(0, 1), q(-1, 1)] {
        assert_eq!(
            physical
                .scaled_proposal(&scale, &BTreeSet::new(), &options)
                .unwrap_err(),
            NoiseError::InvalidProposalScale
        );
        assert_eq!(
            NoiseSampler::configured(
                physical.clone(),
                &NoiseSampling::Importance {
                    proposal_scale: scale
                },
                &BTreeSet::new(),
                options
            )
            .unwrap_err(),
            NoiseError::InvalidProposalScale
        );
    }
    assert_eq!(
        ProbabilityParser::default().rational("1/0"),
        Err(ProbabilityError::ZeroDenominator)
    );
    for text in ["True", "nan", "inf"] {
        assert_eq!(
            ProbabilityParser::default().rational(text),
            Err(ProbabilityError::InvalidSyntax)
        );
    }
    // Original scales3 and4 are integers; q=1 and q=4/3 respectively lose
    // physical support or exceed unit mass. Neither is silently clipped.
    for scale in [q(3, 1), q(4, 1)] {
        assert_eq!(
            physical
                .scaled_proposal(&scale, &BTreeSet::new(), &options)
                .unwrap_err(),
            NoiseError::ScaledProbability { site: 0 }
        );
    }
    let valid = physical
        .scaled_proposal(&q(2, 1), &BTreeSet::new(), &options)
        .unwrap();
    assert_eq!(
        valid.model().probabilities(0).unwrap(),
        [p("1/3"), p("2/3")]
    );
    let overridden =
        NoiseModel::from_conditional_laws(std::slice::from_ref(&law), Some(q(1, 2)), &options)
            .unwrap();
    assert_eq!(overridden.probabilities(0).unwrap(), [p("1/2"), p("1/2")]);
    assert_eq!(
        overridden
            .scaled_proposal(&q(2, 1), &BTreeSet::new(), &options)
            .unwrap_err(),
        NoiseError::ScaledProbability { site: 0 }
    );
    assert_eq!(law.occurrence_probability(), &p("1/3"));
    assert_eq!(law.conditional_weights(), [p("1")]);
    assert_eq!(physical.probabilities(0).unwrap(), [p("2/3"), p("1/3")]);

    let cli = CliSite::new();
    for token in INVALID_SCALE_TOKENS {
        let value = cli.run(&["--noise-proposal-scale", token], 2);
        assert_eq!(value["status"], "failed");
        let failures = value["failures"].as_array().unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0]["phase"], "arguments");
        assert_eq!(failures[0]["type"], "CliError");
        let message = failures[0]["message"].as_str().unwrap();
        assert!(
            message.contains("--noise-proposal-scale"),
            "{token}: {message}"
        );
        let diagnostic = match token {
            "0" | "-1" => "must be positive",
            "1/0" => "zero denominator",
            _ => "decimal",
        };
        assert!(message.contains(diagnostic), "{token}: {message}");
    }
    let unsupported = cli.run(&["--proposal-rate", "1/2"], 2);
    assert_eq!(unsupported["status"], "failed");
    assert_eq!(unsupported["failures"].as_array().unwrap().len(), 1);
    assert_eq!(unsupported["failures"][0]["phase"], "arguments");
    assert_eq!(unsupported["failures"][0]["type"], "CliError");
    let message = unsupported["failures"][0]["message"].as_str().unwrap();
    assert!(message.contains("unknown option") && message.contains("--proposal-rate"));
    let positive = cli.run(&["--noise-proposal-scale", "2"], 0);
    assert_eq!(positive["status"], "complete");
    assert_eq!(positive["completed"], 1);
    assert_eq!(positive["sampling"]["method"], "importance");
    assert_eq!(
        positive["proposal_summary"]["proposal_scale"],
        json!({"numerator":"2","denominator":"1"})
    );
    assert_eq!(positive["proposal_summary"]["scaled_categories"], 1);
    assert_eq!(positive["proposal_summary"]["endpoint_categories"], 0);
}
