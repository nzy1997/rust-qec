use crate::sample_archive::format::{SampleArchiveError, SampleArchiveErrorCode};
use crate::sim::bit_table::{checked_bit_table_storage_size, BitTableAllocError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::{self, ThreadId};

static TELEMETRY_OWNER: Mutex<Option<ThreadId>> = Mutex::new(None);
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
    *TELEMETRY_OWNER.lock().expect("telemetry owner lock") = Some(thread::current().id());
    MAX_BUFFERED_SHOTS.store(0, Ordering::Relaxed);
    MAX_LIVE_DECODED_BLOCKS.store(0, Ordering::Relaxed);
    MAX_TRANSFORM_PAYLOADS.store(0, Ordering::Relaxed);
    MAX_WRITER_LIVE_BYTES.store(0, Ordering::Relaxed);
    MAX_READER_LIVE_BYTES.store(0, Ordering::Relaxed);
    TRANSFORM_RETAINED_BYTES.store(0, Ordering::Relaxed);
    DIAGNOSTICS.lock().expect("telemetry lock").clear();
}

#[doc(hidden)]
pub fn disable_archive_telemetry() {
    let current = thread::current().id();
    let mut owner = TELEMETRY_OWNER.lock().expect("telemetry owner lock");
    if owner.as_ref().is_some_and(|owner| *owner != current) {
        return;
    }
    *owner = None;
    drop(owner);
    MAX_BUFFERED_SHOTS.store(0, Ordering::Relaxed);
    MAX_LIVE_DECODED_BLOCKS.store(0, Ordering::Relaxed);
    MAX_TRANSFORM_PAYLOADS.store(0, Ordering::Relaxed);
    MAX_WRITER_LIVE_BYTES.store(0, Ordering::Relaxed);
    MAX_READER_LIVE_BYTES.store(0, Ordering::Relaxed);
    TRANSFORM_RETAINED_BYTES.store(0, Ordering::Relaxed);
    DIAGNOSTICS.lock().expect("telemetry lock").clear();
}

pub fn archive_telemetry() -> ArchiveTelemetrySnapshot {
    if !telemetry_enabled() {
        return ArchiveTelemetrySnapshot::default();
    }
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
    if !telemetry_enabled() {
        return Vec::new();
    }
    DIAGNOSTICS.lock().expect("telemetry lock").clone()
}

pub(crate) fn record_buffered_input(
    measurement_rows: u64,
    buffered_shots: u64,
) -> Result<u64, SampleArchiveError> {
    if !telemetry_enabled() {
        return Ok(0);
    }
    update_max(&MAX_BUFFERED_SHOTS, buffered_shots);
    bit_table_bytes("writer.buffered_input", measurement_rows, buffered_shots)
}

pub(crate) fn record_transform_retained(bytes: u64) {
    if !telemetry_enabled() {
        return;
    }
    push_diagnostic(format!(
        "transform_retained_bytes immutable_compiled_transform = {bytes}"
    ));
    update_max(&TRANSFORM_RETAINED_BYTES, bytes);
}

pub(crate) fn record_transform_payloads(payloads: u64) {
    if !telemetry_enabled() {
        return;
    }
    push_diagnostic(format!(
        "transform_payloads live_logical_payloads = {payloads}"
    ));
    update_max(&MAX_TRANSFORM_PAYLOADS, payloads);
}

pub(crate) fn record_writer_live_bytes(parts: &[(&str, u64)]) -> Result<u64, SampleArchiveError> {
    if !telemetry_enabled() {
        return Ok(0);
    }
    let bytes = checked_sum("writer.live_bytes", parts)?;
    update_max(&MAX_WRITER_LIVE_BYTES, bytes);
    Ok(bytes)
}

pub(crate) fn record_reader_live_bytes(parts: &[(&str, u64)]) -> Result<u64, SampleArchiveError> {
    if !telemetry_enabled() {
        return Ok(0);
    }
    let bytes = checked_sum("reader.live_bytes", parts)?;
    update_max(&MAX_READER_LIVE_BYTES, bytes);
    Ok(bytes)
}

pub(crate) fn record_reader_decoded_blocks(blocks: u64) {
    if !telemetry_enabled() {
        return;
    }
    push_diagnostic(format!("reader.live_decoded_blocks = {blocks}"));
    update_max(&MAX_LIVE_DECODED_BLOCKS, blocks);
}

pub(crate) fn bit_table_bytes(
    label: &'static str,
    rows: u64,
    shots: u64,
) -> Result<u64, SampleArchiveError> {
    if !telemetry_enabled() {
        return Ok(0);
    }
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
    let size = checked_bit_table_storage_size(rows_to_usize(rows)?, rows_to_usize(shots)?)
        .map_err(|err| match err {
            BitTableAllocError::SizeOverflow => limit("telemetry bit-table total-byte overflow"),
            BitTableAllocError::ReservationFailed => {
                limit("telemetry bit-table reservation limit exceeded")
            }
        })?;
    u64::try_from(size.total_bytes).map_err(|_| limit("telemetry bit-table total-byte overflow"))
}

pub(crate) fn checked_sum(
    label: &'static str,
    parts: &[(&str, u64)],
) -> Result<u64, SampleArchiveError> {
    if !telemetry_enabled() {
        return Ok(0);
    }
    let mut total = 0u64;
    for (name, value) in parts {
        push_diagnostic(format!("{label} checked_add {name}: {total} + {value}"));
        total = total
            .checked_add(*value)
            .ok_or_else(|| limit("telemetry byte sum overflow"))?;
    }
    Ok(total)
}

fn telemetry_enabled() -> bool {
    let current = thread::current().id();
    TELEMETRY_OWNER
        .lock()
        .expect("telemetry owner lock")
        .as_ref()
        .is_some_and(|owner| *owner == current)
}

fn update_max(cell: &AtomicU64, value: u64) {
    cell.fetch_max(value, Ordering::Relaxed);
}

fn push_diagnostic(line: String) {
    if !telemetry_enabled() {
        return;
    }
    let mut diagnostics = DIAGNOSTICS.lock().expect("telemetry lock");
    if !diagnostics.iter().any(|existing| existing == &line) {
        diagnostics.push(line);
    }
}

fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}

fn rows_to_usize(value: u64) -> Result<usize, SampleArchiveError> {
    usize::try_from(value).map_err(|_| limit("telemetry dimension exceeds usize"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn telemetry_is_disabled_until_reset() {
        let _guard = TEST_LOCK.lock().expect("telemetry test lock");
        reset_archive_telemetry();
        disable_archive_telemetry();
        assert_eq!(bit_table_bytes("disabled.table", 3, 65).unwrap(), 0);
        record_buffered_input(3, 65).expect("disabled buffer accounting");
        record_transform_payloads(2);
        record_reader_decoded_blocks(1);
        record_writer_live_bytes(&[("buffer", 48)]).expect("disabled writer accounting");
        record_reader_live_bytes(&[("decoded", 48)]).expect("disabled reader accounting");
        record_transform_retained(123);
        push_diagnostic("disabled push is ignored".to_string());

        assert_eq!(archive_telemetry(), ArchiveTelemetrySnapshot::default());
        assert!(diagnostic_lines().is_empty());
    }

    #[test]
    fn telemetry_records_checked_byte_formulas() {
        let _guard = TEST_LOCK.lock().expect("telemetry test lock");
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
        let initial_diagnostics = diagnostic_lines().len();
        let _ = bit_table_bytes("test.table", 3, 65).expect("repeat bit-table bytes");
        assert_eq!(diagnostic_lines().len(), initial_diagnostics);

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
        let _guard = TEST_LOCK.lock().expect("telemetry test lock");
        reset_archive_telemetry();
        assert_eq!(
            bit_table_bytes("overflow", 1, u64::MAX).unwrap_err().code(),
            SampleArchiveErrorCode::LimitExceeded
        );
        assert_eq!(
            bit_table_bytes("overflow_size", u64::MAX, 64)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::LimitExceeded
        );
        assert_eq!(
            bit_table_bytes("reservation", (isize::MAX as u64 / 8) + 1, 1)
                .unwrap_err()
                .code(),
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
