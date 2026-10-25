use num_rational::BigRational;
use rmagna::BigInt;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::statistics::{
    CheckpointOptions, ExactStatistics, HistoryEfficiency, StatisticsError, StatisticsOptions,
};
use serde_json::{Value, json};
fn options() -> StatisticsOptions {
    StatisticsOptions::default()
}
fn scalar(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn push(
    stats: &mut ExactStatistics,
    efficiency: &mut HistoryEfficiency,
    begin: i64,
    end: i64,
    missing: bool,
) {
    let f = FieldArithmetic::default();
    for i in begin..end {
        let b = BigRational::new((i - 7).into(), 31.into());
        let wide = f
            .value([
                BigRational::new((BigInt::from(1) << 512) + i, 17.into()),
                b.clone(),
                BigRational::from_integer(0.into()),
                -b,
            ])
            .unwrap();
        let zero = i % 5 == 0;
        let weight = if zero {
            ExactField::zero()
        } else {
            scalar(i + 3, 11)
        };
        let accepted = if zero {
            ExactField::zero()
        } else {
            scalar(i % 4 + 1, 7)
        };
        let error = f.multiply(&accepted, &scalar((i % 3) + 1, 5)).unwrap();
        stats
            .push(&[accepted.clone(), wide, scalar(2 * i - 9, 13)], &weight)
            .unwrap();
        efficiency
            .push(
                &accepted,
                if missing && i % 7 == 0 {
                    None
                } else {
                    Some(&error)
                },
                &weight,
                &scalar(i + 1, 17),
            )
            .unwrap();
    }
}
fn fresh() -> (ExactStatistics, HistoryEfficiency) {
    (
        ExactStatistics::new(3, options()).unwrap(),
        HistoryEfficiency::new(FieldArithmetic::default()).unwrap(),
    )
}
#[test]
fn exact_checkpoints_preserve_raw_totals_covariance_ratios_weights_and_efficiency() {
    for missing in [false, true] {
        let (mut full, mut efficiency) = fresh();
        push(&mut full, &mut efficiency, 0, 19, missing);
        for cut in [0, 1, 5, 18, 19] {
            let (mut prefix, mut diagnostic) = fresh();
            push(&mut prefix, &mut diagnostic, 0, cut, missing);
            let stats_text = prefix.checkpoint_json(Default::default()).unwrap();
            let efficiency_text = diagnostic.checkpoint_json(Default::default()).unwrap();
            let mut resumed =
                ExactStatistics::from_checkpoint_json(&stats_text, options(), Default::default())
                    .unwrap();
            let mut continued = HistoryEfficiency::from_checkpoint_json(
                &efficiency_text,
                FieldArithmetic::default(),
                Default::default(),
            )
            .unwrap();
            assert_eq!(
                resumed.checkpoint_json(Default::default()).unwrap(),
                stats_text
            );
            assert_eq!(
                continued.checkpoint_json(Default::default()).unwrap(),
                efficiency_text
            );
            push(&mut resumed, &mut continued, cut, 19, missing);
            assert_eq!(
                resumed.checkpoint_json(Default::default()).unwrap(),
                full.checkpoint_json(Default::default()).unwrap()
            );
            assert_eq!(resumed.summarize().unwrap(), full.summarize().unwrap());
            assert_eq!(
                resumed.paired_ratio(1, 0).unwrap(),
                full.paired_ratio(1, 0).unwrap()
            );
            assert_eq!(
                continued.summarize().unwrap(),
                efficiency.summarize().unwrap()
            );
            assert_eq!(
                continued.checkpoint_json(Default::default()).unwrap(),
                efficiency.checkpoint_json(Default::default()).unwrap()
            );
        }
    }
}
#[test]
fn saved_checkpoints_continue_in_a_fresh_process() {
    const CHILD: &str = "RMAGNA_STATISTICS_CHECKPOINT_CHILD_DIRECTORY";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = std::path::PathBuf::from(root);
        let mut stats = ExactStatistics::from_checkpoint_json(
            &std::fs::read_to_string(root.join("stats.json")).unwrap(),
            options(),
            Default::default(),
        )
        .unwrap();
        let mut efficiency = HistoryEfficiency::from_checkpoint_json(
            &std::fs::read_to_string(root.join("efficiency.json")).unwrap(),
            FieldArithmetic::default(),
            Default::default(),
        )
        .unwrap();
        push(&mut stats, &mut efficiency, 8, 19, false);
        std::fs::write(
            root.join("stats-after.json"),
            stats.checkpoint_json(Default::default()).unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("efficiency-after.json"),
            efficiency.checkpoint_json(Default::default()).unwrap(),
        )
        .unwrap();
        return;
    }
    let root = std::env::temp_dir().join(format!("rmagna-checkpoint-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let (mut stats, mut efficiency) = fresh();
    push(&mut stats, &mut efficiency, 0, 8, false);
    std::fs::write(
        root.join("stats.json"),
        stats.checkpoint_json(Default::default()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("efficiency.json"),
        efficiency.checkpoint_json(Default::default()).unwrap(),
    )
    .unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "saved_checkpoints_continue_in_a_fresh_process",
            "--nocapture",
        ])
        .env(CHILD, &root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    push(&mut stats, &mut efficiency, 8, 19, false);
    assert_eq!(
        std::fs::read_to_string(root.join("stats-after.json")).unwrap(),
        stats.checkpoint_json(Default::default()).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("efficiency-after.json")).unwrap(),
        efficiency.checkpoint_json(Default::default()).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn low_bits_beyond_five_hundred_bits_and_byte_arithmetic_count_endpoints_are_exact() {
    let f = FieldArithmetic::default();
    let a: BigInt = (BigInt::from(1) << 512) + 3;
    let b = &a + 2;
    let mut stats = ExactStatistics::new(1, options()).unwrap();
    for (n, w) in [(a.clone(), 0), (b, 2)] {
        stats
            .push(
                &[f.from_rational(BigRational::from_integer(n)).unwrap()],
                &scalar(w, 1),
            )
            .unwrap();
    }
    let text = stats.checkpoint_json(Default::default()).unwrap();
    let exact = CheckpointOptions {
        max_json_bytes: text.len(),
    };
    assert_eq!(stats.checkpoint_json(exact).unwrap(), text);
    let restored = ExactStatistics::from_checkpoint_json(&text, options(), exact).unwrap();
    let s = restored.summarize().unwrap().unwrap();
    assert_eq!(
        s.means[0].coefficients()[0],
        BigRational::from_integer(a + 1)
    );
    assert_eq!(s.covariance_of_mean.unwrap()[0][0], ExactField::one());
    assert_eq!(s.effective_sample_size, ExactField::one());
    let short = CheckpointOptions {
        max_json_bytes: text.len() - 1,
    };
    assert!(stats.checkpoint_json(short).is_err());
    assert!(ExactStatistics::from_checkpoint_json(&text, options(), short).is_err());
    assert!(
        ExactStatistics::from_checkpoint_json(
            &text,
            StatisticsOptions {
                max_dimensions: 0,
                ..options()
            },
            Default::default()
        )
        .is_err()
    );
    assert!(
        ExactStatistics::from_checkpoint_json(
            &text,
            StatisticsOptions {
                arithmetic: FieldArithmetic::new(128),
                ..options()
            },
            Default::default()
        )
        .is_err()
    );
    assert_eq!(stats.checkpoint_json(Default::default()).unwrap(), text);
    let empty = ExactStatistics::new(1, options())
        .unwrap()
        .checkpoint_json(Default::default())
        .unwrap();
    let mut value: Value = serde_json::from_str(&empty).unwrap();
    value["count"] = json!(u64::MAX.to_string());
    let mut maximum =
        ExactStatistics::from_checkpoint_json(&value.to_string(), options(), Default::default())
            .unwrap();
    assert_eq!(maximum.count(), u64::MAX);
    let before = maximum.checkpoint_json(Default::default()).unwrap();
    assert!(matches!(
        maximum.push(&[ExactField::zero()], &ExactField::zero()),
        Err(StatisticsError::CountOverflow)
    ));
    assert_eq!(maximum.checkpoint_json(Default::default()).unwrap(), before);
    assert!(
        ExactStatistics::from_checkpoint_json(
            &empty,
            StatisticsOptions {
                arithmetic: FieldArithmetic::new(0),
                ..options()
            },
            Default::default()
        )
        .is_ok()
    );
}
#[test]
fn typed_json_rejects_schema_container_private_tag_and_exact_coefficient_corruption() {
    let text = ExactStatistics::new(2, options())
        .unwrap()
        .checkpoint_json(Default::default())
        .unwrap();
    let base: Value = serde_json::from_str(&text).unwrap();
    let mut cases = vec![
        "null".to_owned(),
        "[]".to_owned(),
        text.replacen('{', "{\"count\":\"0\",", 1),
    ];
    for (key, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("schema_version", json!(1.0)),
        ("kind", json!("history_efficiency")),
        ("count", json!(0)),
        ("count", json!("-1")),
        ("count", json!("01")),
        ("count", json!("18446744073709551616")),
        ("count", json!({"$serde_json::private::Number":"0"})),
        ("sums", json!({"$serde_json::private::RawValue":"[]"})),
        ("products", json!([])),
        ("unknown", json!(0)),
    ] {
        let mut bad = base.clone();
        bad[key] = value;
        cases.push(bad.to_string());
    }
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("weight");
    cases.push(missing.to_string());
    for coefficient in [
        json!(["1", "0"]),
        json!(["01", "1"]),
        json!(["-0", "1"]),
        json!(["1", "-3"]),
        json!(["2", "4"]),
        json!(["0", "2"]),
        json!(["\u{0661}", "1"]),
        json!([1, "1"]),
        json!([{"$serde_json::private::Number":"0"},"1"]),
    ] {
        let mut bad = base.clone();
        bad["sums"][0][0] = coefficient;
        cases.push(bad.to_string());
    }
    for (field, index, n) in [
        ("sums", 1, "1"),
        ("products", 0, "-1"),
        ("weight", 0, "-1"),
        ("weight_squared", 0, "1"),
    ] {
        let mut bad = base.clone();
        if field == "sums" || field == "products" {
            bad[field][0][index][0] = json!(n);
        } else {
            bad[field][index][0] = json!(n);
        }
        cases.push(bad.to_string());
    }
    let mut nonempty = base.clone();
    nonempty["sums"][0][0][0] = json!("1");
    cases.push(nonempty.to_string());
    for bad in cases {
        assert!(
            ExactStatistics::from_checkpoint_json(&bad, options(), Default::default()).is_err(),
            "accepted: {bad}"
        );
    }
    assert_eq!(
        ExactStatistics::from_checkpoint_json(&text, options(), Default::default())
            .unwrap()
            .checkpoint_json(Default::default())
            .unwrap(),
        text
    );
}
#[test]
fn efficiency_checkpoints_preserve_empty_zero_weight_missing_error_states_and_reject_bad_counts() {
    let f = FieldArithmetic::default();
    let mut diagnostic = HistoryEfficiency::new(f).unwrap();
    for missing in [false, true] {
        let text = diagnostic.checkpoint_json(Default::default()).unwrap();
        let cap = CheckpointOptions {
            max_json_bytes: text.len(),
        };
        assert_eq!(diagnostic.checkpoint_json(cap).unwrap(), text);
        let restored = HistoryEfficiency::from_checkpoint_json(&text, f, cap).unwrap();
        assert_eq!(
            restored.summarize().unwrap(),
            diagnostic.summarize().unwrap()
        );
        assert!(
            diagnostic
                .checkpoint_json(CheckpointOptions {
                    max_json_bytes: text.len() - 1
                })
                .is_err()
        );
        let zero = ExactField::zero();
        diagnostic
            .push(
                &ExactField::zero(),
                if missing { None } else { Some(&zero) },
                &ExactField::zero(),
                &ExactField::zero(),
            )
            .unwrap();
    }
    let text = diagnostic.checkpoint_json(Default::default()).unwrap();
    let base: Value = serde_json::from_str(&text).unwrap();
    let mut cases = vec![text.replacen('{', "{\"missing_error\":false,", 1)];
    for (key, value) in [
        ("schema_version", json!(0)),
        ("kind", json!("exact_statistics")),
        ("zero_weights", json!("3")),
        ("zero_weights", json!("0")),
        ("missing_error", json!(0)),
        ("reference", json!([])),
        ("statistics", json!({"$serde_json::private::RawValue":"{}"})),
    ] {
        let mut bad = base.clone();
        bad[key] = value;
        cases.push(bad.to_string());
    }
    let mut negative = base.clone();
    negative["reference"][0][0][0] = json!("-1");
    cases.push(negative.to_string());
    let mut dimensions = base.clone();
    dimensions["statistics"]["sums"] = json!([]);
    dimensions["statistics"]["products"] = json!([]);
    cases.push(dimensions.to_string());
    for bad in cases {
        assert!(
            HistoryEfficiency::from_checkpoint_json(&bad, f, Default::default()).is_err(),
            "accepted: {bad}"
        );
    }
}

#[test]
fn oversized_decimal_work_and_all_zero_efficiency_contributions_fail_before_restoration() {
    let text = ExactStatistics::new(1, options())
        .unwrap()
        .checkpoint_json(Default::default())
        .unwrap();
    let mut bad: Value = serde_json::from_str(&text).unwrap();
    bad["sums"][0][0][0] = json!("9".repeat(50000));
    let error = ExactStatistics::from_checkpoint_json(
        &bad.to_string(),
        StatisticsOptions {
            arithmetic: FieldArithmetic::new(32),
            ..options()
        },
        Default::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error.field, "sums");
    assert!(error.message.contains("integer text"));
    let huge = FieldArithmetic::default()
        .from_rational(BigRational::from_integer(BigInt::from(1) << 8192))
        .unwrap();
    let mut stats = ExactStatistics::new(1, options()).unwrap();
    stats.push(&[huge], &ExactField::zero()).unwrap();
    let error = stats
        .checkpoint_json(CheckpointOptions {
            max_json_bytes: 128,
        })
        .unwrap_err();
    assert_eq!(error.field, "max_json_bytes");
    assert!(error.message.contains("integer text"));
    let mut efficiency: Value = serde_json::from_str(
        &HistoryEfficiency::new(FieldArithmetic::default())
            .unwrap()
            .checkpoint_json(Default::default())
            .unwrap(),
    )
    .unwrap();
    efficiency["statistics"]["count"] = json!("2");
    efficiency["zero_weights"] = json!("2");
    efficiency["statistics"]["sums"][0][0][0] = json!("1");
    assert_eq!(
        HistoryEfficiency::from_checkpoint_json(
            &efficiency.to_string(),
            FieldArithmetic::default(),
            Default::default()
        )
        .err()
        .unwrap()
        .field,
        "zero_weights"
    );
}

#[test]
fn complete_positional_checkpoint_arrays_are_rejected_at_roots_and_nested_statistics() {
    let stats = ExactStatistics::new(2, options()).unwrap();
    let efficiency = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    let stats_text = stats.checkpoint_json(Default::default()).unwrap();
    let efficiency_text = efficiency.checkpoint_json(Default::default()).unwrap();
    let s: Value = serde_json::from_str(&stats_text).unwrap();
    let e: Value = serde_json::from_str(&efficiency_text).unwrap();
    let stats_array = json!([
        s["schema_version"],
        s["kind"],
        s["count"],
        s["sums"],
        s["products"],
        s["weight"],
        s["weight_squared"]
    ]);
    let efficiency_array = json!([
        e["schema_version"],
        e["kind"],
        e["statistics"],
        e["reference"],
        e["zero_weights"],
        e["missing_error"]
    ]);
    assert!(
        ExactStatistics::from_checkpoint_json(
            &stats_array.to_string(),
            options(),
            Default::default()
        )
        .is_err()
    );
    let mut nested = e.clone();
    nested["statistics"] = stats_array.clone();
    let mut both = efficiency_array.clone();
    both[2] = stats_array;
    for invalid in [efficiency_array, nested, both] {
        assert!(
            HistoryEfficiency::from_checkpoint_json(
                &invalid.to_string(),
                FieldArithmetic::default(),
                Default::default()
            )
            .is_err()
        );
    }
    assert_eq!(
        ExactStatistics::from_checkpoint_json(&stats_text, options(), Default::default())
            .unwrap()
            .checkpoint_json(Default::default())
            .unwrap(),
        stats_text
    );
    assert_eq!(
        HistoryEfficiency::from_checkpoint_json(
            &efficiency_text,
            FieldArithmetic::default(),
            Default::default()
        )
        .unwrap()
        .checkpoint_json(Default::default())
        .unwrap(),
        efficiency_text
    );
}
