use crate::sample_archive::format::{
    SampleArchiveError, SampleArchiveErrorCode, checked_dense_bit_bytes,
};
use crate::sim::bit_table::{BitTable, BitTableAllocError};

pub(crate) fn pack_dense(table: &BitTable) -> Result<Vec<u8>, SampleArchiveError> {
    let rows = table.num_major() as u64;
    let shots = table.num_minor() as u64;
    let len = checked_dense_bit_bytes(rows, shots)?;
    let len = usize::try_from(len).map_err(|_| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "dense stream too large",
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(|_| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "dense stream reservation failed",
        )
    })?;
    bytes.resize(len, 0);
    for shot in 0..table.num_minor() {
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                let bit = shot
                    .checked_mul(table.num_major())
                    .and_then(|base| base.checked_add(row))
                    .ok_or_else(|| {
                        SampleArchiveError::with_code(
                            SampleArchiveErrorCode::LimitExceeded,
                            "dense bit offset overflow",
                        )
                    })?;
                bytes[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn unpack_dense(
    bytes: &[u8],
    rows: usize,
    shots: usize,
) -> Result<BitTable, SampleArchiveError> {
    validate_dense_shape(bytes, rows as u64, shots as u64)?;
    validate_final_padding(bytes, rows as u64, shots as u64)?;
    let mut table = BitTable::try_new(rows, shots).map_err(map_alloc)?;
    for shot in 0..shots {
        for row in 0..rows {
            let bit = shot
                .checked_mul(rows)
                .and_then(|base| base.checked_add(row))
                .ok_or_else(|| {
                    SampleArchiveError::with_code(
                        SampleArchiveErrorCode::LimitExceeded,
                        "dense bit offset overflow",
                    )
                })?;
            if (bytes[bit / 8] >> (bit % 8)) & 1 == 1 {
                table.set(row, shot, true);
            }
        }
    }
    Ok(table)
}

pub(crate) fn validate_dense_shape(
    bytes: &[u8],
    rows: u64,
    shots: u64,
) -> Result<(), SampleArchiveError> {
    let expected = checked_dense_bit_bytes(rows, shots)?;
    let actual = u64::try_from(bytes.len()).map_err(|_| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "dense stream length too large",
        )
    })?;
    if actual != expected {
        Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::ShapeMismatch,
            "dense stream length does not match shape",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn validate_final_padding(
    bytes: &[u8],
    rows: u64,
    shots: u64,
) -> Result<(), SampleArchiveError> {
    let bits = rows.checked_mul(shots).ok_or_else(|| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "dense bit count overflow",
        )
    })?;
    if bits == 0 || bits % 8 == 0 {
        return Ok(());
    }
    let used = (bits % 8) as u8;
    let mask = !((1u16 << used) as u8 - 1);
    if bytes.last().copied().unwrap_or(0) & mask != 0 {
        Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::MalformedArchive,
            "nonzero dense final padding",
        ))
    } else {
        Ok(())
    }
}

fn map_alloc(_err: BitTableAllocError) -> SampleArchiveError {
    SampleArchiveError::with_code(
        SampleArchiveErrorCode::LimitExceeded,
        "bit table allocation failed",
    )
}
