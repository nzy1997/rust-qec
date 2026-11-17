//! Standalone driver for the paper's pinned PPVM release source.
use ppvm_pauli_sum::config::indexmap::ByteFxHashF64;
use ppvm_stim::{parse_extended, sample_serial_validated, validate};
use ppvm_tableau::prelude::GeneralizedTableau;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

type Tableau = GeneralizedTableau<ByteFxHashF64<8>, usize>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scoring {
    detectors: Vec<Vec<usize>>,
    expected: Vec<bool>,
    target_record: usize,
}

fn tally(
    rows: &[Vec<Option<bool>>],
    records: usize,
    scoring: &Scoring,
) -> Result<(usize, usize), &'static str> {
    let (mut accepted, mut failed) = (0, 0);
    for row in rows {
        if row.len() != records || row.iter().any(Option::is_none) {
            return Err("PPVM draw is missing a measurement record");
        }
        if scoring
            .detectors
            .iter()
            .zip(&scoring.expected)
            .all(|(terms, &expected)| {
                terms
                    .iter()
                    .fold(false, |value, &record| value ^ row[record].unwrap())
                    == expected
            })
        {
            accepted += 1;
            failed += usize::from(row[scoring.target_record].unwrap());
        }
    }
    Ok((accepted, failed))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let [circuit, scoring, shots, seed]: [String; 4] = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "usage: ppvm_cultivation STIM SCORING SHOTS SEED")?;
    let shots: usize = shots.parse()?;
    let seed: u64 = seed.parse()?;
    if !matches!(shots, 128 | 1024) || seed == 0 || seed > u64::from(u32::MAX) {
        return Err("unexpected PPVM timing quota or seed".into());
    }
    let text = std::fs::read_to_string(circuit)?;
    let scoring: Scoring = serde_json::from_str(&std::fs::read_to_string(scoring)?)?;
    let start = Instant::now();
    let program = parse_extended(&text)?;
    validate(&program)?;
    let records = program.measurement_count();
    let qubits = program.num_qubits();
    if qubits == 0
        || qubits > 64
        || scoring.detectors.len() != scoring.expected.len()
        || scoring.target_record.checked_add(1) != Some(records)
        || scoring.detectors.iter().flatten().any(|&r| r >= records)
    {
        return Err("PPVM scoring does not match the full circuit measurement layout".into());
    }
    let preparation_seconds = start.elapsed().as_secs_f64();
    let draw = |shots, seed| {
        let values = sample_serial_validated(&program.instructions, records, shots, |index| {
            Tableau::new_with_seed(qubits, 1e-10, (seed << 32) | index as u64)
        });
        if values.len() != shots {
            return Err("PPVM draw count differs");
        }
        tally(&values, records, &scoring)
    };
    draw(1, 0)?;
    let start = Instant::now();
    let (accepted, failed) = draw(shots, seed)?;
    let sampling_seconds = start.elapsed().as_secs_f64();
    println!(
        "{}",
        json!({"schema":"rmagna.cultivation-ppvm-timing.v1", "completed":shots,
        "accepted":accepted,"accepted_failure":failed,"preparation_seconds":preparation_seconds,
        "sampling_seconds":sampling_seconds,"coefficient_threshold":1e-10,"threads":1,
        "upstream_commit":env!("PPVM_CULTIVATION_SOURCE_COMMIT")})
    );
    Ok(())
}
