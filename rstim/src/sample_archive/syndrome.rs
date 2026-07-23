use crate::sample_archive::dense::{pack_dense, unpack_dense};
use crate::sample_archive::format::{
    STREAM_CODEC_EMPTY, STREAM_CODEC_SYNDROME_DENSE_V1, STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1,
    SampleArchiveError, SampleArchiveErrorCode, checked_dense_bit_bytes,
};
use crate::sim::bit_table::{BitTable, BitTableAllocError};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

static CURRENT_MATERIALIZED_CANDIDATES: AtomicUsize = AtomicUsize::new(0);
static MAX_MATERIALIZED_CANDIDATES: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyndromePlan {
    pub codec_id: u16,
    pub raw_len: u64,
    pub dense_len: u64,
    pub sparse_len: u64,
}

pub struct SyndromeEncoding {
    pub codec_id: u16,
    pub raw_len: u64,
    pub raw: Vec<u8>,
    pub dense_len: u64,
    pub sparse_len: u64,
}

pub fn encode_syndrome(table: &BitTable) -> Result<SyndromeEncoding, SampleArchiveError> {
    let plan = plan_syndrome(table)?;
    let raw = materialize_syndrome(table, plan)?;

    Ok(SyndromeEncoding {
        codec_id: plan.codec_id,
        raw_len: plan.raw_len,
        raw,
        dense_len: plan.dense_len,
        sparse_len: plan.sparse_len,
    })
}

pub fn plan_syndrome(table: &BitTable) -> Result<SyndromePlan, SampleArchiveError> {
    let rows = table.num_major() as u64;
    let shots = table.num_minor() as u64;
    if rows == 0 || shots == 0 {
        return Ok(SyndromePlan {
            codec_id: STREAM_CODEC_EMPTY,
            raw_len: 0,
            dense_len: 0,
            sparse_len: 0,
        });
    }

    let dense_len = checked_dense_bit_bytes(rows, shots)?;
    let sparse_len = checked_sparse_len(table)?;
    let (codec_id, raw_len) = if dense_len <= sparse_len {
        (STREAM_CODEC_SYNDROME_DENSE_V1, dense_len)
    } else {
        (STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1, sparse_len)
    };

    Ok(SyndromePlan {
        codec_id,
        raw_len,
        dense_len,
        sparse_len,
    })
}

pub fn materialize_syndrome(
    table: &BitTable,
    plan: SyndromePlan,
) -> Result<Vec<u8>, SampleArchiveError> {
    begin_materialization_window();
    let raw = match plan.codec_id {
        STREAM_CODEC_EMPTY => Vec::new(),
        STREAM_CODEC_SYNDROME_DENSE_V1 => materialize_dense(table)?,
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1 => materialize_sparse(table, plan.raw_len)?,
        _ => return Err(malformed("unknown syndrome codec")),
    };
    let raw_len = u64::try_from(raw.len()).map_err(|_| limit("syndrome stream too large"))?;
    if raw_len != plan.raw_len {
        return Err(limit(
            "syndrome stream length changed during materialization",
        ));
    }
    Ok(raw)
}

pub fn decode_syndrome_raw(
    codec_id: u16,
    declared_raw_len: u64,
    raw: &[u8],
    rows: usize,
    shots: usize,
) -> Result<BitTable, SampleArchiveError> {
    validate_declared_raw_len(raw, declared_raw_len)?;
    match codec_id {
        STREAM_CODEC_EMPTY => {
            if declared_raw_len != 0 || !raw.is_empty() {
                return Err(malformed("noncanonical empty syndrome stream"));
            }
            if rows != 0 && shots != 0 {
                return Err(malformed("nonempty syndrome uses empty codec"));
            }
            BitTable::try_new(rows, shots).map_err(map_alloc)
        }
        STREAM_CODEC_SYNDROME_DENSE_V1 => {
            if rows == 0 || shots == 0 {
                return Err(malformed("empty syndrome uses nonempty codec"));
            }
            let expected = checked_dense_bit_bytes(rows as u64, shots as u64)?;
            if declared_raw_len != expected {
                return Err(malformed("dense stream length does not match shape"));
            }
            unpack_dense(raw, rows, shots)
        }
        STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1 => {
            if rows == 0 || shots == 0 {
                return Err(malformed("empty syndrome uses nonempty codec"));
            }
            parse_sparse_syndrome(raw, rows as u64, shots as u64, |_, _| {})?;
            let mut table = BitTable::try_new(rows, shots).map_err(map_alloc)?;
            parse_sparse_syndrome(raw, rows as u64, shots as u64, |shot, detector| {
                table.set(detector as usize, shot as usize, true);
            })?;
            Ok(table)
        }
        _ => Err(malformed("unknown syndrome codec")),
    }
}

pub fn for_each_sparse_syndrome_hit(
    raw: &[u8],
    declared_raw_len: u64,
    rows: u64,
    shots: u64,
    mut on_hit: impl FnMut(u64, u64),
) -> Result<(), SampleArchiveError> {
    validate_declared_raw_len(raw, declared_raw_len)?;
    parse_sparse_syndrome(raw, rows, shots, |_, _| {})?;
    parse_sparse_syndrome(raw, rows, shots, |shot, detector| on_hit(shot, detector))
}

pub fn reset_materialization_telemetry() {
    CURRENT_MATERIALIZED_CANDIDATES.store(0, Ordering::Relaxed);
    MAX_MATERIALIZED_CANDIDATES.store(0, Ordering::Relaxed);
}

pub fn max_materialized_candidates() -> usize {
    MAX_MATERIALIZED_CANDIDATES.load(Ordering::Relaxed)
}

#[allow(dead_code)] // Consumed by archive integration in the next task.
pub(crate) fn update_dense_syndrome_hash(
    table: &BitTable,
    hasher: &mut Sha256,
) -> Result<(), SampleArchiveError> {
    checked_dense_bit_bytes(table.num_major() as u64, table.num_minor() as u64)?;
    let mut byte = 0u8;
    let mut bit_in_byte = 0u8;
    for shot in 0..table.num_minor() {
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                byte |= 1 << bit_in_byte;
            }
            bit_in_byte += 1;
            if bit_in_byte == 8 {
                hasher.update([byte]);
                byte = 0;
                bit_in_byte = 0;
            }
        }
    }
    if bit_in_byte != 0 {
        hasher.update([byte]);
    }
    Ok(())
}

fn checked_sparse_len(table: &BitTable) -> Result<u64, SampleArchiveError> {
    let mut len = 0u64;
    for shot in 0..table.num_minor() {
        let mut count = 0u64;
        let mut previous = 0u64;
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                let row = row as u64;
                if count == 0 {
                    len = checked_add_uleb_len(len, row)?;
                } else {
                    let delta = row
                        .checked_sub(previous)
                        .and_then(|value| value.checked_sub(1))
                        .ok_or_else(|| limit("sparse detector order overflow"))?;
                    len = checked_add_uleb_len(len, delta)?;
                }
                count = count
                    .checked_add(1)
                    .ok_or_else(|| limit("sparse hit count overflow"))?;
                previous = row;
            }
        }
        len = checked_add_uleb_len(len, count)?;
    }
    Ok(len)
}

fn materialize_dense(table: &BitTable) -> Result<Vec<u8>, SampleArchiveError> {
    let raw = pack_dense(table)?;
    record_materialized_candidate();
    Ok(raw)
}

fn materialize_sparse(table: &BitTable, len: u64) -> Result<Vec<u8>, SampleArchiveError> {
    let len = usize::try_from(len).map_err(|_| limit("sparse stream too large"))?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(len)
        .map_err(|_| limit("sparse stream reservation failed"))?;

    for shot in 0..table.num_minor() {
        let mut count = 0u64;
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| limit("sparse hit count overflow"))?;
            }
        }
        encode_uleb(count, &mut raw);

        let mut previous = 0u64;
        let mut first = true;
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                let row = row as u64;
                let value = if first {
                    first = false;
                    row
                } else {
                    row.checked_sub(previous)
                        .and_then(|value| value.checked_sub(1))
                        .ok_or_else(|| limit("sparse detector order overflow"))?
                };
                encode_uleb(value, &mut raw);
                previous = row;
            }
        }
    }
    debug_assert_eq!(raw.len(), len);
    record_materialized_candidate();
    Ok(raw)
}

fn parse_sparse_syndrome(
    raw: &[u8],
    rows: u64,
    shots: u64,
    mut on_hit: impl FnMut(u64, u64),
) -> Result<(), SampleArchiveError> {
    let mut offset = 0usize;
    for shot in 0..shots {
        let count = decode_uleb(raw, &mut offset)?;
        if count > rows {
            return Err(malformed("sparse hit count exceeds detector count"));
        }

        let mut previous = 0u64;
        for hit in 0..count {
            let delta = decode_uleb(raw, &mut offset)?;
            let detector = if hit == 0 {
                delta
            } else {
                previous
                    .checked_add(delta)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(|| malformed("sparse detector delta overflows"))?
            };
            if detector >= rows {
                return Err(malformed("sparse detector index exceeds shape"));
            }
            on_hit(shot, detector);
            previous = detector;
        }
    }
    if offset != raw.len() {
        return Err(malformed("bytes after sparse shot records"));
    }
    Ok(())
}

fn encode_uleb(mut value: u64, raw: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        raw.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn decode_uleb(raw: &[u8], offset: &mut usize) -> Result<u64, SampleArchiveError> {
    let mut value = 0u64;
    for index in 0..10 {
        let byte = *raw
            .get(*offset)
            .ok_or_else(|| malformed("unterminated ULEB128 value"))?;
        *offset += 1;
        let payload = u64::from(byte & 0x7f);
        if index == 9 && (payload > 1 || byte & 0x80 != 0) {
            return Err(malformed("ULEB128 value overflows u64"));
        }
        value |= payload << (index * 7);
        if byte & 0x80 == 0 {
            if uleb_len(value) != index + 1 {
                return Err(malformed("noncanonical ULEB128 value"));
            }
            return Ok(value);
        }
    }
    Err(malformed("ULEB128 value overflows u64"))
}

fn checked_add_uleb_len(len: u64, value: u64) -> Result<u64, SampleArchiveError> {
    len.checked_add(uleb_len(value) as u64)
        .ok_or_else(|| limit("sparse stream length overflow"))
}

fn uleb_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

fn validate_declared_raw_len(raw: &[u8], declared_raw_len: u64) -> Result<(), SampleArchiveError> {
    let actual = u64::try_from(raw.len()).map_err(|_| malformed("raw stream length too large"))?;
    if actual == declared_raw_len {
        Ok(())
    } else {
        Err(malformed("declared raw length does not match stream"))
    }
}

fn begin_materialization_window() {
    CURRENT_MATERIALIZED_CANDIDATES.store(0, Ordering::Relaxed);
}

fn record_materialized_candidate() {
    let current = CURRENT_MATERIALIZED_CANDIDATES.fetch_add(1, Ordering::Relaxed) + 1;
    let mut observed = MAX_MATERIALIZED_CANDIDATES.load(Ordering::Relaxed);
    while observed < current {
        match MAX_MATERIALIZED_CANDIDATES.compare_exchange_weak(
            observed,
            current,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(current) => observed = current,
        }
    }
}

fn map_alloc(_err: BitTableAllocError) -> SampleArchiveError {
    limit("bit table allocation failed")
}

fn malformed(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::MalformedArchive, detail)
}

fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_telemetry_tracks_multiple_candidates_in_one_window() {
        reset_materialization_telemetry();
        begin_materialization_window();
        record_materialized_candidate();
        record_materialized_candidate();
        assert_eq!(max_materialized_candidates(), 2);
    }
}
