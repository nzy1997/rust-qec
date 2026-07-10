use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sampler::{sample_batch_with_options, SampleOptions};
use rstim::sim::bit_table::BitTable;
use rstim::sim::measure_record_batch::MeasureRecordBatch;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn b8_fingerprint(table: &BitTable) -> (usize, u64) {
    let mut bytes = Vec::new();
    write_shots_b8(table, &mut bytes).expect("write b8");
    (bytes.len(), fnv64(&bytes))
}

#[test]
fn contiguous_storage_reports_expected_shape() {
    let mut batch = MeasureRecordBatch::new(130);

    batch.push_row(&[0x11, 0x22, 0x33, 0x44]);
    batch.push_row(&[0xaa, 0xbb]);

    assert_eq!(batch.batch_size(), 130);
    assert_eq!(batch.words_per_row(), 3);
    assert_eq!(batch.len(), 2);
    assert_eq!(batch.contiguous_words(), &[0x11, 0x22, 0x33, 0xaa, 0xbb, 0]);
    assert_eq!(
        batch.contiguous_words().len(),
        batch.len() * batch.words_per_row()
    );
}

#[test]
fn lookback_words_match_pushed_rows() {
    let mut batch = MeasureRecordBatch::new(129);

    batch.push_row(&[0b001, 0b010, 0b100]);
    batch.push_row(&[0xf0]);
    batch.push_row(&[0xa0, 0xb0, 0xc0, 0xd0]);

    assert_eq!(batch.words_per_row(), 3);
    assert_eq!(batch.lookback_words(1), &[0xa0, 0xb0, 0xc0]);
    assert_eq!(batch.lookback_words(2), &[0xf0, 0, 0]);
    assert_eq!(batch.lookback_words(3), &[0b001, 0b010, 0b100]);
    assert!(batch.lookback(3, 0));
    assert!(!batch.lookback(3, 1));
}

#[test]
fn xor_lookback_preserves_detector_parity_for_known_fixture() {
    let instrs = parse_lines(SURFACE_D11_R100).expect("parse checked fixture");
    let mut rng = StdRng::seed_from_u64(20260708);

    let out = sample_batch_with_options(&instrs, 130, &mut rng, SampleOptions::default())
        .expect("sample checked fixture");

    assert_eq!(out.measurements.num_major(), 12121);
    assert_eq!(out.detections.num_major(), 12000);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.measurements.num_minor(), 130);
    assert_eq!(out.detections.num_minor(), 130);
    assert_eq!(out.observable_flips.num_minor(), 130);
    assert_eq!(out.detector_materializations, 12000);
    assert_eq!(out.observable_materializations, 1);
    // The seeded fingerprint is implementation-specific; it guards this
    // fixture's storage/parity path without promising a stable RNG stream.
    assert_eq!(
        b8_fingerprint(&out.detections),
        (195000, 0x80598848ee51b1ec)
    );
    assert_eq!(
        b8_fingerprint(&out.observable_flips),
        (130, 0x3552b86e402fbc36)
    );
}

#[test]
fn push_zeros_preserves_row_alignment() {
    let mut batch = MeasureRecordBatch::new(65);

    batch.push_row(&[0x1111, 0x2222]);
    batch.push_zeros();
    batch.push_row(&[0x3333, 0x4444]);

    assert_eq!(batch.words_per_row(), 2);
    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch.contiguous_words(),
        &[0x1111, 0x2222, 0, 0, 0x3333, 0x4444]
    );

    let mut dest = vec![0xffff, 0xffff];
    batch.xor_lookback_into(2, &mut dest);
    assert_eq!(dest, &[0xffff, 0xffff]);
    batch.xor_lookback_into(1, &mut dest);
    assert_eq!(dest, &[0xcccc, 0xbbbb]);
}

#[test]
fn zero_shot_rows_remain_counted() {
    let mut batch = MeasureRecordBatch::new(0);

    batch.push_zeros();
    batch.push_row(&[0x1234, 0x5678]);

    assert_eq!(batch.words_per_row(), 0);
    assert_eq!(batch.len(), 2);
    assert!(batch.contiguous_words().is_empty());
    assert!(batch.lookback_words(1).is_empty());
    assert!(batch.lookback_words(2).is_empty());

    let mut dest = Vec::new();
    batch.xor_lookback_into(1, &mut dest);
    assert!(dest.is_empty());
}
