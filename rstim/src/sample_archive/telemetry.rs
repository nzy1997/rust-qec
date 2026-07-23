use crate::sample_archive::format::{SampleArchiveError, SampleArchiveErrorCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static MAX_BUFFERED_SHOTS: AtomicU64 = AtomicU64::new(0);
static MAX_LIVE_DECODED_BLOCKS: AtomicU64 = AtomicU64::new(0);
static MAX_TRANSFORM_PAYLOADS: AtomicU64 = AtomicU64::new(0);
static MAX_WRITER_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static MAX_READER_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static TRANSFORM_RETAINED_BYTES: AtomicU64 = AtomicU64::new(0);
static DIAGNOSTICS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveTelemetrySnapshot {
    pub max_buffered_shots: u64,
    pub max_live_decoded_blocks: u64,
    pub max_transform_payloads: u64,
    pub max_writer_live_bytes: u64,
    pub max_reader_live_bytes: u64,
    pub transform_retained_bytes: u64,
}

pub fn reset_archive_telemetry() {
    MAX_BUFFERED_SHOTS.store(0, Ordering::Relaxed);
    MAX_LIVE_DECODED_BLOCKS.store(0, Ordering::Relaxed);
    MAX_TRANSFORM_PAYLOADS.store(0, Ordering::Relaxed);
    MAX_WRITER_LIVE_BYTES.store(0, Ordering::Relaxed);
    MAX_READER_LIVE_BYTES.store(0, Ordering::Relaxed);
    TRANSFORM_RETAINED_BYTES.store(0, Ordering::Relaxed);
    DIAGNOSTICS.lock().expect("telemetry lock").clear();
}

pub fn archive_telemetry() -> ArchiveTelemetrySnapshot {
    ArchiveTelemetrySnapshot {
        max_buffered_shots: MAX_BUFFERED_SHOTS.load(Ordering::Relaxed),
        max_live_decoded_blocks: MAX_LIVE_DECODED_BLOCKS.load(Ordering::Relaxed),
        max_transform_payloads: MAX_TRANSFORM_PAYLOADS.load(Ordering::Relaxed),
        max_writer_live_bytes: MAX_WRITER_LIVE_BYTES.load(Ordering::Relaxed),
        max_reader_live_bytes: MAX_READER_LIVE_BYTES.load(Ordering::Relaxed),
        transform_retained_bytes: TRANSFORM_RETAINED_BYTES.load(Ordering::Relaxed),
    }
}

pub fn diagnostic_lines() -> Vec<String> {
    DIAGNOSTICS.lock().expect("telemetry lock").clone()
}

pub(crate) fn record_buffered_input(
    measurement_rows: u64,
    buffered_shots: u64,
) -> Result<u64, SampleArchiveError> {
    update_max(&MAX_BUFFERED_SHOTS, buffered_shots);
    bit_table_bytes("writer.buffered_input", measurement_rows, buffered_shots)
}

pub(crate) fn record_transform_retained(bytes: u64) {
    push_diagnostic(format!(
        "transform_retained_bytes immutable_compiled_transform = {bytes}"
    ));
    update_max(&TRANSFORM_RETAINED_BYTES, bytes);
}

pub(crate) fn record_transform_payloads(payloads: u64) {
    push_diagnostic(format!("transform_payloads live_logical_payloads = {payloads}"));
    update_max(&MAX_TRANSFORM_PAYLOADS, payloads);
}

pub(crate) fn record_writer_live_bytes(parts: &[(&str, u64)]) -> Result<u64, SampleArchiveError> {
    let bytes = checked_sum("writer.live_bytes", parts)?;
    update_max(&MAX_WRITER_LIVE_BYTES, bytes);
    Ok(bytes)
}

pub(crate) fn record_reader_live_bytes(parts: &[(&str, u64)]) -> Result<u64, SampleArchiveError> {
    let bytes = checked_sum("reader.live_bytes", parts)?;
    update_max(&MAX_READER_LIVE_BYTES, bytes);
    Ok(bytes)
}

pub(crate) fn record_reader_decoded_blocks(blocks: u64) {
    push_diagnostic(format!("reader.live_decoded_blocks = {blocks}"));
    update_max(&MAX_LIVE_DECODED_BLOCKS, blocks);
}

pub(crate) fn bit_table_bytes(
    label: &'static str,
    rows: u64,
    shots: u64,
) -> Result<u64, SampleArchiveError> {
    push_diagnostic(format!(
        "{label}.words_per_row checked_add shots_plus_63 = {shots} + 63"
    ));
    let words_numerator = shots
        .checked_add(63)
        .ok_or_else(|| limit("telemetry bit-table row-word overflow"))?;
    let words_per_row = words_numerator / 64;
    push_diagnostic(format!(
        "{label}.total_words checked_mul rows_x_words = {rows} * {words_per_row}"
    ));
    let total_words = rows
        .checked_mul(words_per_row)
        .ok_or_else(|| limit("telemetry bit-table total-word overflow"))?;
    push_diagnostic(format!(
        "{label}.total_bytes checked_mul words_x_8 = {total_words} * 8"
    ));
    total_words
        .checked_mul(8)
        .ok_or_else(|| limit("telemetry bit-table total-byte overflow"))
}

pub(crate) fn checked_sum(
    label: &'static str,
    parts: &[(&str, u64)],
) -> Result<u64, SampleArchiveError> {
    let mut total = 0u64;
    for (name, value) in parts {
        push_diagnostic(format!(
            "{label} checked_add {name}: {total} + {value}"
        ));
        total = total
            .checked_add(*value)
            .ok_or_else(|| limit("telemetry byte sum overflow"))?;
    }
    Ok(total)
}

fn update_max(cell: &AtomicU64, value: u64) {
    let mut observed = cell.load(Ordering::Relaxed);
    while observed < value {
        match cell.compare_exchange_weak(observed, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(current) => observed = current,
        }
    }
}

fn push_diagnostic(line: String) {
    DIAGNOSTICS.lock().expect("telemetry lock").push(line);
}

fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_records_checked_byte_formulas() {
        reset_archive_telemetry();
        let bytes = bit_table_bytes("test.table", 3, 65).expect("bit-table bytes");
        assert_eq!(bytes, 48);
        let total = checked_sum("test.sum", &[("a", bytes), ("b", 7)]).expect("sum bytes");
        assert_eq!(total, 55);
        record_buffered_input(3, 65).expect("buffer bytes");
        record_transform_payloads(2);
        record_reader_decoded_blocks(1);
        record_writer_live_bytes(&[("buffer", bytes)]).expect("writer bytes");
        record_reader_live_bytes(&[("decoded", bytes)]).expect("reader bytes");
        record_transform_retained(123);

        let snapshot = archive_telemetry();
        assert_eq!(snapshot.max_buffered_shots, 65);
        assert_eq!(snapshot.max_transform_payloads, 2);
        assert_eq!(snapshot.max_live_decoded_blocks, 1);
        assert_eq!(snapshot.max_writer_live_bytes, bytes);
        assert_eq!(snapshot.max_reader_live_bytes, bytes);
        assert_eq!(snapshot.transform_retained_bytes, 123);
        let diagnostics = diagnostic_lines();
        assert!(diagnostics.iter().any(|line| line.contains("checked_mul")));
        assert!(diagnostics.iter().any(|line| line.contains("checked_add")));
    }

    #[test]
    fn telemetry_checked_arithmetic_reports_limit_exceeded() {
        reset_archive_telemetry();
        assert_eq!(
            bit_table_bytes("overflow", 1, u64::MAX).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );
        assert_eq!(
            checked_sum("overflow", &[("a", u64::MAX), ("b", 1)])
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );
    }
}
