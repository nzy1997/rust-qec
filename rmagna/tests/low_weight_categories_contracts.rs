//! Complete categorical/mandatory-tail contracts from pinned
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_low_weight.py. Native binding ABI identity is not claimed.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::enumeration::{EnumerationOptions, enumerate_noise};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampling};
use rmagna::probability::Probability;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::TaskEnumeration;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-low-weight-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "I 0\n").unwrap();
        std::fs::write(path.join("task.json"), r#"{"ideal_state":"target.magna"}"#).unwrap();
        Self(path)
    }
    fn load(&self, source: &str) -> FileTask {
        std::fs::write(self.0.join("physical.magna"), source).unwrap();
        FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
fn fraction(n: i64, d: i64) -> Value {
    json!({"numerator":n.to_string(),"denominator":d.to_string()})
}
fn pauli(basis: &str) -> Value {
    json!({"kind":"pauli","factors":[{"qubit":0,"basis":basis}]})
}
fn options(weight: usize) -> FileEnumerationOptions {
    let mut options = FileEnumerationOptions::default();
    options.enumeration.noise.max_weight = weight;
    options.enumeration.include_patterns = true;
    options
}
type Choices = Vec<(usize, usize)>;
fn pattern_masses(raw: &TaskEnumeration) -> BTreeMap<Choices, BigRational> {
    let rows = raw.pattern_responses.as_ref().unwrap();
    let result = rows
        .iter()
        .map(|row| {
            (
                row.choices
                    .iter()
                    .map(|choice| (choice.site, choice.alternative))
                    .collect(),
                row.probability.value().clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(result.len(), rows.len());
    result
}

#[test]
fn original_distinct_sites_preserve_truncated_masses_patterns_and_metadata() {
    let files = Files::new();
    let file = files.load("X_ERROR(1/10) 0\nX_ERROR(1/5) 0");
    let prepared = file.compile().unwrap();
    let fresh = file.enumerate(&options(1)).unwrap();
    let reused = prepared.enumerate(&options(1)).unwrap();
    assert!(fresh.is_complete() && reused.is_complete());
    assert_eq!(fresh.raw_result(), reused.raw_result());
    let raw = fresh.raw_result().unwrap();
    assert_eq!(raw.patterns, 3);
    assert_eq!(raw.covered_probability.value(), &rational(49, 50));
    assert_eq!(raw.omitted_probability.value(), &rational(1, 50));
    assert_eq!(raw.masses.accepted, field(49, 50));
    assert_eq!(raw.masses.error, field(13, 50));
    assert_eq!(raw.full_error.lower, field(13, 50));
    assert_eq!(raw.full_error.upper, field(7, 25));
    let oracle = BTreeMap::from([
        (vec![], rational(18, 25)),
        (vec![(0, 0)], rational(2, 25)),
        (vec![(1, 0)], rational(9, 50)),
    ]);
    assert_eq!(pattern_masses(raw), oracle);
    let rows = raw.pattern_responses.as_ref().unwrap();
    assert_eq!(rows.iter().filter(|row| row.choices.len() == 1).count(), 2);
    for row in rows {
        assert_eq!(row.response.accepted, ExactField::one());
        assert_eq!(
            row.response.error,
            field(i64::from(row.choices.len() == 1), 1)
        );
    }
    let value = fresh.to_json().unwrap();
    assert_eq!(value["status"], "complete");
    let categories = value["original_noise_categories"].as_array().unwrap();
    assert_eq!(categories.len(), 2);
    for (index, (n, d)) in [(1, 10), (1, 5)].into_iter().enumerate() {
        let category = &categories[index];
        assert_eq!(category["original_category"], index);
        assert_eq!(category["internal_site"], index);
        assert_eq!(category["channels"].as_array().unwrap().len(), 1);
        assert_eq!(category["channels"][0]["probability"], fraction(n, d));
        assert_eq!(category["channels"][0]["action"], pauli("X"));
        assert_eq!(category["source_operations"][0]["line"], index + 1);
    }
    assert_eq!(categories[0]["identity_probability"], fraction(9, 10));
    assert_eq!(categories[1]["identity_probability"], fraction(4, 5));
    let fingerprint = value["fingerprints"]["circuit"].as_str().unwrap();
    assert_eq!(fingerprint.len(), 64);
    assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let reused_json = reused.to_json().unwrap();
    assert_eq!(value["fingerprints"], reused_json["fingerprints"]);
    assert_eq!(
        value["original_noise_categories"],
        reused_json["original_noise_categories"]
    );
    let retained = value["result"]["pattern_responses"].as_array().unwrap();
    assert_eq!(
        retained
            .iter()
            .filter(|row| row["choices"].as_array().unwrap().len() == 1)
            .count(),
        2
    );
    let mut full_options = options(2);
    full_options.enumeration.include_patterns = false;
    let full = file.enumerate(&full_options).unwrap();
    let full_raw = full.raw_result().unwrap();
    assert_eq!(full_raw.patterns, 4);
    assert_eq!(full_raw.covered_probability, Probability::one());
    assert_eq!(full_raw.omitted_probability, Probability::zero());
    assert_eq!(full_raw.masses.error, field(13, 50));
    assert_eq!(full_raw.masses.accepted, ExactField::one());
    // The fingerprint also changes with the physical probability, while this
    // already owned report retains its original snapshot.
    let altered = files.load("X_ERROR(1/11) 0\nX_ERROR(1/5) 0");
    let altered = altered.enumerate(&options(1)).unwrap().to_json().unwrap();
    assert_ne!(
        value["fingerprints"]["circuit"],
        altered["fingerprints"]["circuit"]
    );
    assert_eq!(
        fresh.to_json().unwrap()["fingerprints"]["circuit"],
        fingerprint
    );
}

#[test]
fn original_pauli_channel_and_else_chain_keep_exclusive_exact_weights() {
    for (source, error, identity, probabilities, bases, errors) in [
        (
            "PAULI_CHANNEL_1(1/10,1/5,3/10) 0",
            (3, 10),
            (2, 5),
            vec![(1, 10), (1, 5), (3, 10)],
            vec!["X", "Y", "Z"],
            vec![true, true, false],
        ),
        (
            "CORRELATED_ERROR(1/10) X0\nELSE_CORRELATED_ERROR(2/9) Z0",
            (1, 10),
            (7, 10),
            vec![(1, 10), (1, 5)],
            vec!["X", "Z"],
            vec![true, false],
        ),
    ] {
        let files = Files::new();
        let file = files.load(source);
        let prepared = file.compile().unwrap();
        let fresh = file.enumerate(&options(1)).unwrap();
        let reused = prepared.enumerate(&options(1)).unwrap();
        assert!(fresh.is_complete() && reused.is_complete());
        assert_eq!(fresh.raw_result(), reused.raw_result());
        let raw = fresh.raw_result().unwrap();
        assert_eq!(raw.patterns, probabilities.len() + 1);
        assert_eq!(raw.covered_probability, Probability::one());
        assert_eq!(raw.omitted_probability, Probability::zero());
        assert_eq!(raw.masses.accepted, ExactField::one());
        assert_eq!(raw.masses.error, field(error.0, error.1));
        let mut oracle = BTreeMap::from([(vec![], rational(identity.0, identity.1))]);
        for (channel, &(n, d)) in probabilities.iter().enumerate() {
            oracle.insert(vec![(0, channel)], rational(n, d));
        }
        assert_eq!(pattern_masses(raw), oracle);
        for row in raw.pattern_responses.as_ref().unwrap() {
            assert!(row.choices.len() <= 1);
            let expected = row
                .choices
                .first()
                .is_some_and(|choice| errors[choice.alternative]);
            assert_eq!(row.response.accepted, ExactField::one());
            assert_eq!(row.response.error, field(i64::from(expected), 1));
        }
        let value = fresh.to_json().unwrap();
        let table = value["original_noise_categories"].as_array().unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(
            table[0]["identity_probability"],
            fraction(identity.0, identity.1)
        );
        let channels = table[0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), probabilities.len());
        for (channel, &(n, d)) in probabilities.iter().enumerate() {
            assert_eq!(channels[channel]["probability"], fraction(n, d));
            assert_eq!(channels[channel]["action"], pauli(bases[channel]));
        }
        assert_eq!(
            table[0]["source_operations"].as_array().unwrap().len(),
            if probabilities.len() == 2 { 2 } else { 1 }
        );
        assert_eq!(
            value["original_noise_categories"],
            reused.to_json().unwrap()["original_noise_categories"]
        );
    }
}

#[test]
fn original_204_categories_prune_mandatory_tail_before_final_pattern_limit() {
    let mut categories = vec![vec!["9/10".parse().unwrap(), "1/10".parse().unwrap()]; 200];
    categories.extend((0..4).map(|_| vec![Probability::zero(), Probability::one()]));
    assert_eq!(categories.len(), 204);
    let model = NoiseModel::new(categories, &NoiseOptions::default()).unwrap();
    let options = EnumerationOptions {
        max_weight: 3,
        max_patterns: 1,
        ..Default::default()
    };
    let empty = enumerate_noise(&model, &options).unwrap();
    assert!(empty.patterns().is_empty());
    assert_eq!(empty.covered_probability(), &Probability::zero());
    assert_eq!(empty.omitted_probability(), &Probability::one());
    let result = enumerate_noise(
        &model,
        &EnumerationOptions {
            max_weight: 4,
            ..options
        },
    )
    .unwrap();
    assert_eq!(result.patterns().len(), 1);
    let pattern = &result.patterns()[0];
    assert_eq!(pattern.weight(), 4);
    assert_eq!(
        pattern
            .faults
            .iter()
            .map(|fault| (fault.site, fault.alternative))
            .collect::<Vec<_>>(),
        vec![(200, 0), (201, 0), (202, 0), (203, 0)]
    );
    // All 200 optional categories must remain identity; all four mandatory
    // categories have unit probability. No Cartesian support is enumerated.
    let expected = BigRational::new(BigInt::from(9).pow(200), BigInt::from(10).pow(200));
    assert_eq!(pattern.probability.value(), &expected);
    assert_eq!(result.covered_probability().value(), &expected);
    assert_eq!(
        result.omitted_probability().value(),
        &(rational(1, 1) - &expected)
    );
    assert_eq!(result.by_weight().len(), 5);
    for mass in &result.by_weight()[..4] {
        assert_eq!(mass, &Probability::zero());
    }
    assert_eq!(result.by_weight()[4].value(), &expected);
}
