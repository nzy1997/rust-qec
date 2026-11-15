// SPDX-License-Identifier: MPL-2.0
// Full license: fixtures/original_many_record_input.LICENSE.
// Explicit finite-word portions of pinned Magna tests/native/rational_random.cpp.
// Full source-method numeric/proposal witnesses are added in this target before
// any ledger promotion; the existing masked stream is a separate public policy.
use num_bigint::BigUint;
use num_rational::BigRational;
use rmagna::noise::{
    ModuloRangeError, ModuloWordRangeSource, RandRangeSource, RandomSource, WordRangeSource,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
struct Words {
    values: Vec<u64>,
    position: usize,
}

fn fixture() -> Value {
    let raw = include_bytes!("data/original-numeric-contracts.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "e1676926e7cca648f76e0912b852bbb85b8e0b6514c4a14294b000ef7dccecc0"
    );
    let value: Value = serde_json::from_slice(raw).unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(value["upstream_commit"], manifest["upstream"]["commit"]);
    assert_eq!(
        value["reference_manifest_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/reference.json"
            ))
        )
    );
    assert_eq!(
        value["producer_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_numeric_contracts.py"
            ))
        )
    );
    for (path, hash) in value["source_sha256"].as_object().unwrap() {
        assert_eq!(*hash, manifest["upstream"]["files"][path]);
    }
    assert_eq!(value["original_methods_passed"], 5);
    assert_eq!(value["streams"].as_array().unwrap().len(), 7);
    value
}

struct Protocol {
    events: Vec<Value>,
    cursor: usize,
    affine_bits: bool,
}
impl Protocol {
    fn new(stream: &Value, affine_bits: bool) -> Self {
        Self {
            events: stream["events"].as_array().unwrap().clone(),
            cursor: 0,
            affine_bits,
        }
    }
    fn exhausted(&self) {
        assert_eq!(self.cursor, self.events.len());
    }
}
impl RandRangeSource for Protocol {
    type Error = std::convert::Infallible;
    fn randrange(&mut self, bound: &BigUint) -> Result<BigUint, Self::Error> {
        let event = &self.events[self.cursor];
        self.cursor += 1;
        let expected = if event["kind"] == "bits" {
            assert!(self.affine_bits);
            assert_eq!(event["args"][0], 2);
            BigUint::from(4u32)
        } else {
            assert_eq!(event["kind"], "range");
            event["args"][0].as_u64().unwrap().into()
        };
        assert_eq!(*bound, expected, "protocol request {}", self.cursor);
        Ok(event["value"].as_u64().unwrap().into())
    }
}

#[test]
fn all_original_independent_and_affine_draws_replay_exact_protocols_and_point_masses() {
    use rmagna::{gf2::BitVector, instrument::*};
    use std::collections::BTreeMap;
    let data = fixture();
    // Native and independent Python reference consumed exactly the same public
    // protocol and ended in the same state. Rust replays that explicit protocol;
    // it does not replace its default RNG with a Python Random implementation.
    assert_eq!(data["streams"][0], data["streams"][1]);
    let plan = record_plan(3);
    let proposal = OriginalIndependentRecordProposal::new(
        &plan,
        &BTreeMap::from([
            ("r0".into(), "2/7".parse().unwrap()),
            ("r2".into(), "3/11".parse().unwrap()),
        ]),
        Default::default(),
    )
    .unwrap();
    let affine = OriginalAffineRecordProposal::from_integer_equations(
        &plan,
        &[BitVector::from_indices(3, [0, 1]).unwrap()],
        &[1],
        "finite test",
        Default::default(),
    )
    .unwrap();
    let mut random = Protocol::new(&data["streams"][0], true);
    for (key, is_affine) in [("independent", false), ("affine", true)] {
        let rows = data["observations"][key].as_array().unwrap();
        assert_eq!(rows.len(), 30);
        for row in rows {
            let draw = if is_affine {
                affine
                    .draw(&mut random, OriginalSharedFaults::empty())
                    .unwrap()
            } else {
                proposal
                    .draw(&mut random, OriginalSharedFaults::empty())
                    .unwrap()
            };
            assert_eq!(draw.range_requests, if is_affine { 1 } else { 3 });
            assert_eq!(
                draw.proposal_probability,
                row["q"].as_str().unwrap().parse::<BigRational>().unwrap()
            );
            for (j, name) in ["a", "b", "c"].into_iter().enumerate() {
                assert_eq!(
                    draw.assignment.word().get(j).unwrap(),
                    row["true"][name] == 1
                );
            }
        }
    }
    random.exhausted();
    assert_eq!(random.cursor, 120);
    eprintln!(
        "original proposal witness: 30 independent + 30 affine draws, 120 exact protocol requests"
    );
    assert_eq!(plan.input_key_generations(), 0);
}

#[test]
fn all_original_unbiased_noise_draws_keep_public_range_protocol_and_scaled_identity() {
    use rmagna::noise::{
        NoiseSampling, SequentialNoiseOptions, SequentialNoiseSampler, SequentialNoiseSite,
    };
    let data = fixture();
    for (left, right) in [(3, 4), (5, 6)] {
        assert_eq!(data["streams"][left], data["streams"][right]);
    }
    eprintln!(
        "original noise witness: 100 physical + 20 effective-rate + 20 unit-scale draws; exact protocol exhaustion and per-draw weights"
    );
    let options = SequentialNoiseOptions::default();
    let sites = || {
        vec![
            SequentialNoiseSite::new(3, "1/3".parse().unwrap(), None, &options).unwrap(),
            SequentialNoiseSite::new(1, "2/5".parse().unwrap(), None, &options).unwrap(),
        ]
    };
    for (stream, count, rate) in [
        (3, 100, None),
        (5, 20, Some("1/8".parse().unwrap())),
        (6, 20, Some("1/8".parse().unwrap())),
    ] {
        let policy = if stream == 6 {
            NoiseSampling::Importance {
                proposal_scale: BigRational::from_integer(1.into()),
            }
        } else {
            NoiseSampling::Physical
        };
        let sampler = SequentialNoiseSampler::configured(sites(), rate, &policy, options).unwrap();
        let mut random = Protocol::new(&data["streams"][stream], false);
        for j in 0..count {
            let before = random.cursor;
            let draw = sampler.draw(&mut random).unwrap();
            assert_eq!(draw.range_calls, random.cursor - before);
            let mut faults = Vec::new();
            if let Some(index) = draw.channels[0] {
                faults.push(json!({"after":"a","qubit":0,"pauli":(["X","Y","Z"][index])}));
            }
            let flips = if draw.channels[1].is_some() {
                vec!["r"]
            } else {
                vec![]
            };
            let index = if stream == 3 {
                j
            } else {
                100 + 2 * j + usize::from(stream == 6)
            };
            let observed = &data["observations"]["noise"][index];
            assert_eq!(
                json!([faults, flips, draw.likelihood_ratio.to_string()]),
                observed["result"]
            );
            assert_eq!(draw.physical_probability, draw.proposal_probability);
            assert_eq!(draw.likelihood_ratio, BigRational::from_integer(1.into()));
        }
        random.exhausted();
    }
    assert!(
        SequentialNoiseSampler::configured(
            sites(),
            Some(BigRational::from_integer(2.into())),
            &NoiseSampling::Physical,
            options
        )
        .is_err()
    );
}

#[test]
fn all_original_real_and_weighted_scalar_inputs_keep_exact_fields_and_rounded_bits() {
    use num_bigint::BigInt;
    use num_traits::Zero;
    use rmagna::{
        field::FieldArithmetic,
        numeric::{NumericOptions, approximate_real},
        scalar::ScalarArithmetic,
    };
    let data = fixture();
    let rows = data["observations"]["decimals"].as_array().unwrap();
    assert_eq!(rows.len(), 70);
    eprintln!(
        "original scalar precision witness: 35 inputs, 70 exact/unweighted and weighted fields, rounded decimal reference bits"
    );
    let arithmetic = FieldArithmetic::default();
    for pair in rows.chunks_exact(2) {
        assert!(pair[0]["weight"].is_null());
        assert_eq!(pair[0]["coefficients"], pair[1]["coefficients"]);
        assert_eq!(pair[0]["power"], pair[1]["power"]);
        for row in pair {
            let c: [BigInt; 4] =
                std::array::from_fn(|j| row["coefficients"][j].as_str().unwrap().parse().unwrap());
            assert!(c[2].is_zero());
            assert_eq!(c[1], -c[3].clone());
            let power = row["power"].as_i64().unwrap();
            let scalar = ScalarArithmetic::default()
                .scalar(c.clone(), power)
                .unwrap();
            let weight = row["weight"].as_str().map_or_else(
                || BigRational::from_integer(1.into()),
                |s| s.parse().unwrap(),
            );
            let exact = arithmetic
                .scale(&arithmetic.from_scalar(&scalar).unwrap(), &weight)
                .unwrap();
            // Independent coefficient formula, including normalized power and
            // exact rational weighting; no intermediate Decimal rounding.
            let factor = if power >= 0 {
                BigRational::from_integer(BigInt::from(1) << power as usize)
            } else {
                BigRational::new(1.into(), BigInt::from(1) << (-power) as usize)
            };
            for (j, coefficient) in c.into_iter().enumerate() {
                assert_eq!(
                    exact.coefficients()[j],
                    BigRational::from_integer(coefficient) * &factor * &weight
                );
            }
            let got = approximate_real(&exact, NumericOptions::default())
                .unwrap()
                .scientific;
            let expected = &row["rounded_decimal_frexp"];
            let native_bits = if got.mantissa == 0.0 {
                0
            } else {
                (got.mantissa * 0.5).to_bits()
            };
            assert_eq!(format!("{native_bits:016x}"), expected["mantissa_bits"]);
            assert_eq!(
                if got.mantissa == 0.0 {
                    0
                } else {
                    got.exponent + 1
                },
                expected["exponent"].as_i64().unwrap()
            );
        }
    }
}

fn decimal_ratio(text: &str) -> BigRational {
    use num_bigint::BigInt;
    let (base, exponent) = text
        .split_once(['e', 'E'])
        .map_or((text, 0), |(a, b)| (a, b.parse::<i32>().unwrap()));
    let digits = base.split_once('.').map_or(0, |(_, b)| b.len()) as i32;
    let numerator: BigInt = base.replace('.', "").parse().unwrap();
    let power = exponent - digits;
    if power >= 0 {
        BigRational::from_integer(numerator * BigInt::from(10).pow(power as u32))
    } else {
        BigRational::new(numerator, BigInt::from(10).pow((-power) as u32))
    }
}

#[test]
fn all_six_original_estimator_families_keep_tiny_covariances_and_extreme_weights() {
    use rmagna::{
        field::{ExactField, FieldArithmetic},
        numeric::{Binary64, NumericOptions, approximate_real, approximate_sqrt},
        statistics::ExactStatistics,
    };
    let data = fixture();
    let families = data["observations"]["estimators"].as_array().unwrap();
    assert_eq!(families.len(), 6);
    eprintln!(
        "original estimator witness: all 6 families, 1E-2000/1E2000 and 78-digit covariance, exact pairwise and closed-form oracles"
    );
    let a = FieldArithmetic::default();
    let field = |r: BigRational| a.from_rational(r).unwrap();
    let compare = |exact: &ExactField, native: &Value, sqrt: bool| {
        if native.is_null() {
            return;
        }
        let got = if sqrt {
            approximate_sqrt(exact, NumericOptions::default())
        } else {
            approximate_real(exact, NumericOptions::default())
        }
        .unwrap();
        match got.binary64 {
            Binary64::Finite(v) => assert_eq!(v.to_bits(), native.as_f64().unwrap().to_bits()),
            Binary64::Underflow { negative: false } => assert_eq!(native.as_f64(), Some(0.0)),
            Binary64::Overflow { negative: false } => {
                assert_eq!(native["native_float"], "positive_infinity")
            }
            v => panic!("unexpected signed estimator export: {v:?}"),
        }
        if !exact.is_zero() {
            assert!((1.0..2.0).contains(&got.scientific.mantissa.abs()));
        }
    };
    for (index, family) in families.iter().enumerate() {
        let rows = family["rows"].as_array().unwrap();
        let n = rows.len();
        let weights: Vec<_> = rows
            .iter()
            .map(|r| decimal_ratio(r["weight"].as_str().unwrap()))
            .collect();
        let x: Vec<_> = rows
            .iter()
            .zip(&weights)
            .map(|(r, w)| w * decimal_ratio(r["acceptance"].as_str().unwrap()))
            .collect();
        let y: Vec<_> = rows
            .iter()
            .zip(&x)
            .map(|(r, x)| x * decimal_ratio(r["diagnostic"].as_str().unwrap()))
            .collect();
        let mut statistics = ExactStatistics::new(2, Default::default()).unwrap();
        for j in 0..n {
            statistics
                .push(
                    &[field(x[j].clone()), field(y[j].clone())],
                    &field(weights[j].clone()),
                )
                .unwrap();
        }
        let summary = statistics.summarize().unwrap().unwrap();
        let nr = BigRational::from_integer(n.into());
        let sumx: BigRational = x.iter().cloned().sum();
        let sumy: BigRational = y.iter().cloned().sum();
        let sumw: BigRational = weights.iter().cloned().sum();
        assert_eq!(summary.count, n as u64);
        assert_eq!(summary.means, vec![field(&sumx / &nr), field(&sumy / &nr)]);
        assert_eq!(summary.mean_weight, field(&sumw / &nr));
        let squarew: BigRational = weights.iter().map(|w| w * w).sum();
        let ess = if squarew == BigRational::from_integer(0.into()) {
            squarew
        } else {
            &sumw * &sumw / squarew
        };
        assert_eq!(summary.effective_sample_size, field(ess));
        if n > 1 {
            // Independent pairwise-difference identity, avoiding the streaming
            // implementation's subtraction of large raw second moments.
            let vectors = [&x, &y];
            for i in 0..2 {
                for j in 0..2 {
                    let mut sum = BigRational::from_integer(0.into());
                    for left in 0..n {
                        for right in left + 1..n {
                            sum += (&vectors[i][left] - &vectors[i][right])
                                * (&vectors[j][left] - &vectors[j][right]);
                        }
                    }
                    let covariance = sum / BigRational::from_integer((n * n * (n - 1)).into());
                    assert_eq!(
                        summary.covariance_of_mean.as_ref().unwrap()[i][j],
                        field(covariance)
                    );
                }
            }
        } else {
            assert!(summary.covariance_of_mean.is_none());
        }
        let ratio = statistics.paired_ratio(1, 0).unwrap();
        if sumx == BigRational::from_integer(0.into()) {
            assert!(ratio.is_none());
        } else {
            let ratio = ratio.as_ref().unwrap();
            let q = &sumy / &sumx;
            assert_eq!(ratio.ratio, field(q.clone()));
            let residuals: BigRational = y
                .iter()
                .zip(&x)
                .map(|(y, x)| {
                    let r = y - &q * x;
                    &r * &r
                })
                .sum();
            let variance =
                residuals / BigRational::from_integer((n * (n - 1)).into()) / (&sumx / &nr).pow(2);
            assert_eq!(ratio.variance, Some(field(variance)));
            compare(
                &ratio.ratio,
                &family["result"]["conditional_diagnostic"],
                false,
            );
            compare(
                ratio.variance.as_ref().unwrap(),
                &family["result"]["conditional_standard_error"],
                true,
            );
        }
        for (key, value) in [
            ("acceptance", &summary.means[0]),
            ("joint_diagnostic", &summary.means[1]),
            ("mean_weight", &summary.mean_weight),
            ("effective_sample_size", &summary.effective_sample_size),
        ] {
            compare(value, &family["result"][key], false);
        }
        if let Some(cov) = &summary.covariance_of_mean {
            compare(
                &cov[0][0],
                &family["result"]["acceptance_standard_error"],
                true,
            );
            compare(&cov[1][1], &family["result"]["joint_standard_error"], true);
        }
        // Independent closed forms for the two extreme scaled families and
        // the nearly equal 78-digit weights retain information lost in f64.
        if matches!(index, 3 | 4) {
            let t = &weights[0];
            assert_eq!(
                summary.means[0],
                field(t * BigRational::new(4.into(), 3.into()))
            );
            assert_eq!(
                summary.means[1],
                field(t / BigRational::from_integer(3.into()))
            );
            assert_eq!(
                ratio.unwrap().variance,
                Some(field(BigRational::new(27.into(), 256.into())))
            );
        }
        if index == 5 {
            let u = decimal_ratio("1E-78");
            assert_eq!(
                summary.means[0],
                field(
                    (BigRational::from_integer(3.into())
                        + &u * BigRational::from_integer(5.into()))
                        / BigRational::from_integer(6.into())
                )
            );
            assert_eq!(
                summary.covariance_of_mean.as_ref().unwrap()[0][1],
                field(-(&u + &u * &u) / BigRational::from_integer(36.into()))
            );
        }
    }
}

fn scalar_512() -> Value {
    let raw = include_bytes!("data/original-numeric-512.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "816e98e4e5826a28b1bc319aad166c35f70173a25c4fb9576765ae39797ee72a"
    );
    let value: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        value["producer_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_numeric_512.py"
            ))
        )
    );
    let reference: Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(
        value["source_sha256"],
        reference["upstream"]["files"]["tests/native/rational_random.cpp"]
    );
    assert_eq!(value["upstream_commit"], reference["upstream"]["commit"]);
    assert_eq!(
        value["reference_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/reference.json"
            ))
        )
    );
    assert_eq!(value["standalone_exit_code"], 0);
    assert_eq!(value["original_assertions_retained"], true);
    assert_eq!(value["cpp_checks_include_fe_upward"], true);
    value
}

fn check_512() {
    use num_bigint::BigInt;
    use rmagna::{
        numeric::{NumericOptions, approximate_scalar_ratio},
        scalar::ScalarArithmetic,
    };
    let data = scalar_512();
    let rows = data["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 512);
    eprintln!("original finite scalar witness: all 512 pairs match independent Fraction rounding");
    let a = ScalarArithmetic::default();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["index"], index);
        let make = |name: &str| {
            a.scalar(
                [
                    row[name].as_str().unwrap().parse::<BigInt>().unwrap(),
                    0.into(),
                    0.into(),
                    0.into(),
                ],
                row[format!("{name}_power")].as_i64().unwrap(),
            )
            .unwrap()
        };
        let got = approximate_scalar_ratio(
            &make("numerator"),
            &make("denominator"),
            NumericOptions::default(),
        )
        .unwrap();
        assert_eq!(
            format!("{:016x}", (got.mantissa * 0.5).to_bits()),
            row["mantissa_bits"]
        );
        assert_eq!(got.exponent + 1, row["exponent"].as_i64().unwrap());
    }
}

#[test]
fn all_512_original_scalar_ratios_match_independent_fraction_rounding() {
    check_512();
}

// The platform header supplies the rounding-mode value; it is not guessed from
// the CPU architecture. Run the mutation in a separate process, never a parallel
// test runner thread, and restore the saved mode even through unwinding.
#[cfg(unix)]
#[test]
fn scalar_ratio_rounding_is_independent_of_process_mode_and_restored_after_panic() {
    use std::{path::PathBuf, process::Command};
    #[link(name = "m")]
    unsafe extern "C" {
        fn fegetround() -> std::os::raw::c_int;
        fn fesetround(mode: std::os::raw::c_int) -> std::os::raw::c_int;
    }
    struct SavedMode(i32);
    impl Drop for SavedMode {
        fn drop(&mut self) {
            // SAFETY: the mode was returned by the platform's fegetround.
            assert_eq!(unsafe { fesetround(self.0) }, 0);
        }
    }
    const MODE: &str = "RMAGNA_TEST_HEADER_FE_UPWARD";
    if let Ok(mode) = std::env::var(MODE) {
        let mode: i32 = mode.parse().unwrap();
        // SAFETY: fegetround has no preconditions; FE_UPWARD comes from fenv.h
        // compiled by the active platform C compiler in the parent process.
        let saved = unsafe { fegetround() };
        assert_ne!(saved, -1);
        let guard = SavedMode(saved);
        assert_eq!(unsafe { fesetround(mode) }, 0);
        assert_eq!(unsafe { fegetround() }, mode);
        check_512();
        let third = rmagna::numeric::approximate_scalar_ratio(
            &rmagna::scalar::ExactScalar::one(),
            &rmagna::scalar::ScalarArithmetic::default()
                .scalar([3.into(), 0.into(), 0.into(), 0.into()], 0)
                .unwrap(),
            Default::default(),
        )
        .unwrap();
        assert_eq!((third.mantissa * 0.5).to_bits(), 0x3fe5_5555_5555_5555);
        assert_eq!(third.exponent + 1, -1);
        assert_eq!(unsafe { fegetround() }, mode);
        drop(guard);
        assert_eq!(unsafe { fegetround() }, saved);
        let result = std::panic::catch_unwind(|| {
            let _guard = SavedMode(saved);
            assert_eq!(unsafe { fesetround(mode) }, 0);
            assert_eq!(unsafe { fegetround() }, mode);
            panic!("injected rounding guard unwind");
        });
        assert!(result.is_err());
        assert_eq!(unsafe { fegetround() }, saved);
        return;
    }
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(std::env::temp_dir().join(format!("rmagna-fenv-{}", std::process::id())));
    std::fs::create_dir(&scratch.0).unwrap();
    let source = scratch.0.join("mode.c");
    let binary = scratch.0.join("mode");
    std::fs::write(&source,"#include <fenv.h>\n#include <stdio.h>\nint main(void){printf(\"%d\\n\",(int)FE_UPWARD);return 0;}\n").unwrap();
    let compiler = Command::new("cc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let header = Command::new(&binary).output().unwrap();
    assert!(header.status.success());
    let mode = String::from_utf8(header.stdout).unwrap();
    mode.trim().parse::<i32>().unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "scalar_ratio_rounding_is_independent_of_process_mode_and_restored_after_panic",
            "--test-threads=1",
        ])
        .env(MODE, mode.trim())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed"));
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = self.values[self.position];
        self.position += 1;
        word
    }
}
#[test]
fn original_uniform_index_rejects_the_incomplete_bucket_and_bound_one_uses_no_entropy() {
    let mut words = Words {
        values: vec![0, 1],
        position: 0,
    };
    let mut source = ModuloWordRangeSource::new(&mut words, 2);
    assert_eq!(
        source.randrange(&15u32.into()).unwrap(),
        BigUint::from(1u32)
    );
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(source.randrange(&1u32.into()).unwrap(), BigUint::from(0u32));
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(
        source.randrange(&0u32.into()),
        Err(ModuloRangeError::EmptyRange)
    );
    assert_eq!(
        source.randrange(&(BigUint::from(1u32) << 64)),
        Err(ModuloRangeError::RangeTooWide)
    );
    assert_eq!(
        source.randrange(&2u32.into()),
        Err(ModuloRangeError::EntropyLimit { limit: 2 })
    );
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(words.position, 2);
}
#[test]
fn modulo_and_masked_conventions_are_explicit_and_preserve_boundary_words() {
    for count in [2u64, 3, 7, 15, 16, 257, u64::MAX] {
        let threshold = count.wrapping_neg() % count;
        let mut candidates = vec![threshold];
        if threshold > 0 {
            candidates.insert(0, threshold - 1);
        }
        let mut words = Words {
            values: candidates.clone(),
            position: 0,
        };
        let mut source = ModuloWordRangeSource::new(&mut words, candidates.len());
        assert_eq!(
            source.randrange(&count.into()).unwrap(),
            BigUint::from(threshold % count)
        );
        assert_eq!(source.entropy_words(), candidates.len());
        assert_eq!(words.position, candidates.len());
        let mut maximum = Words {
            values: vec![u64::MAX],
            position: 0,
        };
        assert_eq!(
            ModuloWordRangeSource::new(&mut maximum, 1)
                .randrange(&count.into())
                .unwrap(),
            BigUint::from(u64::MAX % count)
        );
    }
    let mut words = Words {
        values: vec![0, 1],
        position: 0,
    };
    assert_eq!(
        WordRangeSource::new(&mut words, 2)
            .randrange(&15u32.into())
            .unwrap(),
        BigUint::from(0u32)
    );
    assert_eq!(words.position, 1); // Intentionally distinct from full-word modulo rejection.
    let mut exhausted = Words {
        values: vec![0],
        position: 0,
    };
    let mut source = ModuloWordRangeSource::new(&mut exhausted, 1);
    assert_eq!(
        source.randrange(&15u32.into()),
        Err(ModuloRangeError::EntropyLimit { limit: 1 })
    );
    assert_eq!(source.entropy_words(), 1);
}

fn record_plan(width: usize) -> rmagna::instrument::OriginalAssignmentPlan {
    use rmagna::{
        clifford::CliffordCircuit, gf2::BitVector, instrument::*, scalar::ScalarArithmetic,
        unitary::CompilerStrategy,
    };
    let a = ScalarArithmetic::default();
    let source = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(BitVector::zeros(1).unwrap()).unwrap(),
        (0..width)
            .map(|j| OriginalOperation {
                id: format!("m{j}"),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: format!("r{j}"),
                },
            })
            .collect(),
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    OriginalAssignmentPlan::from_owned(source, CompilerStrategy::ParityCopies, a)
}
#[test]
fn independent_record_proposals_own_probabilities_and_all_eight_exact_point_masses() {
    use rmagna::instrument::*;
    use std::{collections::BTreeMap, convert::Infallible};
    let source = record_plan(3);
    let mut probabilities = BTreeMap::from([
        ("r0".into(), "2/7".parse().unwrap()),
        ("r2".into(), "3/11".parse().unwrap()),
    ]);
    let proposal =
        OriginalIndependentRecordProposal::new(&source, &probabilities, Default::default())
            .unwrap();
    probabilities.insert("r0".into(), "1/3".parse().unwrap());
    let numerators = [40, 16, 40, 16, 15, 6, 15, 6];
    let mut sum = num_rational::BigRational::from_integer(0.into());
    for (word, numerator) in numerators.into_iter().enumerate() {
        let mut requests = Vec::new();
        let mut random = |bound: &BigUint| -> Result<BigUint, Infallible> {
            let j = requests.len();
            requests.push(bound.clone());
            Ok(if word & (1 << j) != 0 {
                0u32.into()
            } else {
                bound - BigUint::from(1u32)
            })
        };
        let result = proposal
            .draw(&mut random, OriginalSharedFaults::empty())
            .unwrap();
        assert_eq!(result.range_requests, 3);
        assert_eq!(requests, vec![7u32.into(), 2u32.into(), 11u32.into()]);
        for j in 0..3 {
            assert_eq!(
                result.assignment.word().get(j).unwrap(),
                word & (1 << j) != 0
            );
        }
        assert_eq!(
            result.proposal_probability,
            num_rational::BigRational::new(numerator.into(), 154.into())
        );
        sum += &result.proposal_probability;
        let expected = if word == 0 {
            rmagna::scalar::ExactScalar::one()
        } else {
            rmagna::scalar::ExactScalar::zero()
        };
        assert_eq!(source.probability(result.assignment).unwrap(), expected);
    }
    assert_eq!(sum, num_rational::BigRational::from_integer(1.into()));
    assert_eq!(source.input_key_generations(), 0);
}
#[test]
fn affine_real_record_proposals_own_equations_and_sample_the_checked_nullspace() {
    use rmagna::{gf2::BitVector, instrument::*};
    use std::convert::Infallible;
    let source = record_plan(3);
    let mut rows = vec![BitVector::from_indices(3, [0, 1]).unwrap()];
    let mut certificate = "finite test".to_owned();
    let proposal = OriginalAffineRecordProposal::from_integer_equations(
        &source,
        &rows,
        &[1],
        &certificate,
        Default::default(),
    )
    .unwrap();
    rows[0].set(0, false).unwrap();
    certificate.clear();
    assert_eq!(proposal.coverage_certificate(), "finite test");
    assert_eq!(proposal.family().dimension(), 2);
    for (coordinate, point) in [1usize, 2, 5, 6].into_iter().enumerate() {
        let mut calls = 0;
        let mut random = |bound: &BigUint| -> Result<BigUint, Infallible> {
            assert_eq!(*bound, 4u32.into());
            calls += 1;
            Ok(coordinate.into())
        };
        let result = proposal
            .draw(&mut random, OriginalSharedFaults::empty())
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(result.range_requests, 1);
        for j in 0..3 {
            assert_eq!(
                result.assignment.word().get(j).unwrap(),
                point & (1 << j) != 0
            );
        }
        assert_eq!(
            result.proposal_probability,
            num_rational::BigRational::new(1.into(), 4.into())
        );
        // The class checks algebra/caller evidence,not the physical coverage
        // promise: this repeated-MZ source has zero mass on the declared family.
        assert!(source.probability(result.assignment).unwrap().is_zero());
    }
    let identities = (0..3)
        .map(|j| BitVector::from_indices(3, [j]).unwrap())
        .collect::<Vec<_>>();
    let deterministic = OriginalAffineRecordProposal::from_integer_equations(
        &source,
        &identities,
        &[0, 0, 0],
        "deterministic",
        OriginalProposalOptions {
            max_range_calls: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut never = |_: &BigUint| -> Result<BigUint, Infallible> {
        panic!("deterministic family must not consume entropy")
    };
    let result = deterministic
        .draw(&mut never, OriginalSharedFaults::empty())
        .unwrap();
    assert_eq!(result.range_requests, 0);
    assert_eq!(
        result.proposal_probability,
        num_rational::BigRational::from_integer(1.into())
    );
    assert_eq!(
        source.probability(result.assignment).unwrap(),
        rmagna::scalar::ExactScalar::one()
    );
}
#[test]
fn affine_record_proposals_keep_coordinates_and_probability_beyond_a_machine_word() {
    use rmagna::instrument::*;
    let source = record_plan(65);
    let proposal = OriginalAffineRecordProposal::from_integer_equations(
        &source,
        &[],
        &[],
        "explicit full record space",
        Default::default(),
    )
    .unwrap();
    let mut calls = 0;
    let mut random = |bound: &BigUint| -> Result<BigUint, std::convert::Infallible> {
        calls += 1;
        assert_eq!(*bound, BigUint::from(1u32) << 65);
        Ok(BigUint::from(1u32) << 64)
    };
    let result = proposal
        .draw(&mut random, OriginalSharedFaults::empty())
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.assignment.word().width(), 65);
    for j in 0..65 {
        assert_eq!(result.assignment.word().get(j).unwrap(), j == 64);
    }
    assert_eq!(
        result.proposal_probability,
        BigRational::new(1.into(), num_bigint::BigInt::from(1) << 65)
    );
}
#[test]
fn record_proposal_errors_refuse_invalid_support_quotas_and_range_results() {
    use rmagna::{gf2::BitVector, instrument::*};
    use std::{collections::BTreeMap, convert::Infallible};
    let source = record_plan(3);
    let rows = vec![BitVector::from_indices(3, [0, 1]).unwrap()];
    for rhs in [2, -1] {
        assert!(
            OriginalAffineRecordProposal::from_integer_equations(
                &source,
                &rows,
                &[rhs],
                "finite",
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        OriginalAffineRecordProposal::from_integer_equations(
            &source,
            &rows,
            &[1],
            "",
            Default::default()
        )
        .is_err()
    );
    assert!(
        OriginalAffineRecordProposal::from_integer_equations(
            &source,
            &[BitVector::zeros(3).unwrap()],
            &[1],
            "finite",
            Default::default()
        )
        .is_err()
    );
    for text in ["0", "1"] {
        assert!(
            OriginalIndependentRecordProposal::new(
                &source,
                &BTreeMap::from([("r0".into(), text.parse().unwrap())]),
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        OriginalIndependentRecordProposal::new(
            &source,
            &BTreeMap::from([("foreign".into(), "1/2".parse().unwrap())]),
            Default::default()
        )
        .is_err()
    );
    assert!(
        OriginalIndependentRecordProposal::new(
            &source,
            &BTreeMap::new(),
            OriginalProposalOptions {
                max_probability_bits: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    for options in [
        OriginalProposalOptions {
            max_geometry_words: 0,
            ..Default::default()
        },
        OriginalProposalOptions {
            max_elimination_work: 0,
            ..Default::default()
        },
        OriginalProposalOptions {
            max_certificate_bytes: 0,
            ..Default::default()
        },
    ] {
        assert!(
            OriginalAffineRecordProposal::from_integer_equations(
                &source,
                &rows,
                &[1],
                "finite",
                options
            )
            .is_err()
        );
    }
    let proposal = OriginalIndependentRecordProposal::new(
        &source,
        &BTreeMap::new(),
        OriginalProposalOptions {
            max_range_calls: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let mut never =
        |_: &BigUint| -> Result<BigUint, Infallible> { panic!("quota must reject before RNG") };
    assert!(matches!(
        proposal.draw(&mut never, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::Original(_))
    ));
    let proposal =
        OriginalIndependentRecordProposal::new(&source, &BTreeMap::new(), Default::default())
            .unwrap();
    let mut bad = |bound: &BigUint| -> Result<BigUint, Infallible> { Ok(bound.clone()) };
    assert!(matches!(
        proposal.draw(&mut bad, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::OutOfRange)
    ));
    let mut calls = 0;
    let mut failed = |_: &BigUint| -> Result<BigUint, &'static str> {
        calls += 1;
        Err("range failure")
    };
    assert!(matches!(
        proposal.draw(&mut failed, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::Source("range failure"))
    ));
    assert_eq!(calls, 1);
}
