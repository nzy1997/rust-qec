use crate::sample_archive::format::{SampleArchiveError, SampleArchiveErrorCode};
use crate::sample_archive::limits::ArchiveLimits;
use std::io::Write;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const ZSTD_BLOCK_HEADER_LEN: usize = 3;
const ZSTD_FRAME_CHECKSUM_LEN: usize = 4;

pub(crate) fn compress_frame(bytes: &[u8], level: i32) -> Result<Vec<u8>, SampleArchiveError> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), level)
        .map_err(|_| compression_failed("failed to initialize zstd encoder"))?;
    encoder
        .include_checksum(true)
        .map_err(|_| compression_failed("failed to enable zstd checksum"))?;
    encoder
        .include_contentsize(true)
        .map_err(|_| compression_failed("failed to enable zstd content size"))?;
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .map_err(|_| compression_failed("failed to set zstd content size"))?;
    encoder
        .write_all(bytes)
        .map_err(|_| compression_failed("failed to write zstd frame"))?;
    encoder
        .finish()
        .map_err(|_| compression_failed("failed to finish zstd frame"))
}

pub(crate) fn decompress_frame(
    compressed: &[u8],
    declared_len: u64,
    limits: ArchiveLimits,
) -> Result<Vec<u8>, SampleArchiveError> {
    let header = parse_frame_header(compressed)?;
    if !header.content_checksum {
        return Err(decompression_failed("zstd frame checksum is missing"));
    }
    let Some(content_size) = header.content_size else {
        return Err(decompression_failed("zstd frame content size is missing"));
    };
    if content_size != declared_len {
        return Err(decompression_failed(
            "zstd frame content size does not match declaration",
        ));
    }
    let frame_len = single_frame_len(compressed, header.header_len, header.content_checksum)?;
    if frame_len != compressed.len() {
        return Err(decompression_failed(
            "zstd stream must contain exactly one frame",
        ));
    }
    if header.window_size > limits.max_zstd_window_bytes {
        return Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd window exceeds limit",
        ));
    }
    if header
        .window_size
        .checked_add(content_size)
        .ok_or_else(|| {
            SampleArchiveError::with_code(
                SampleArchiveErrorCode::LimitExceeded,
                "zstd decoder memory overflow",
            )
        })?
        > limits.max_zstd_decoder_memory_bytes
    {
        return Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd decoder memory exceeds limit",
        ));
    }
    let capacity = usize::try_from(declared_len).map_err(|_| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd content size too large",
        )
    })?;
    let decoded = zstd::bulk::decompress(compressed, capacity)
        .map_err(|_| decompression_failed("zstd decompression failed"))?;
    if decoded.len() as u64 != declared_len {
        return Err(decompression_failed(
            "zstd decoded length does not match declaration",
        ));
    }
    Ok(decoded)
}

#[derive(Debug, Clone, Copy)]
struct FrameHeader {
    content_size: Option<u64>,
    content_checksum: bool,
    window_size: u64,
    header_len: usize,
}

fn parse_frame_header(bytes: &[u8]) -> Result<FrameHeader, SampleArchiveError> {
    if bytes.len() < 6 || bytes[0..4] != ZSTD_MAGIC {
        return Err(decompression_failed("invalid zstd frame magic"));
    }
    let desc = bytes[4];
    if desc & 0x08 != 0 {
        return Err(decompression_failed("reserved zstd descriptor bit is set"));
    }
    let fcs_flag = desc >> 6;
    let single_segment = desc & 0x20 != 0;
    let content_checksum = desc & 0x04 != 0;
    let dict_flag = desc & 0x03;
    let mut offset = 5usize;

    let window_size = if single_segment {
        None
    } else {
        let descriptor = *bytes
            .get(offset)
            .ok_or_else(|| decompression_failed("truncated zstd window descriptor"))?;
        offset += 1;
        let exponent = descriptor >> 3;
        let mantissa = descriptor & 0x07;
        let window_log = 10 + u64::from(exponent);
        let shift = u32::try_from(window_log)
            .map_err(|_| decompression_failed("zstd window size overflow"))?;
        let window_base = 1u64
            .checked_shl(shift)
            .ok_or_else(|| decompression_failed("zstd window size overflow"))?;
        Some(window_base + (window_base / 8) * u64::from(mantissa))
    };

    let dict_len = match dict_flag {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        _ => unreachable!("two-bit dictionary flag"),
    };
    offset = offset
        .checked_add(dict_len)
        .ok_or_else(|| decompression_failed("zstd dictionary offset overflow"))?;
    if bytes.len() < offset {
        return Err(decompression_failed("truncated zstd dictionary id"));
    }

    let fcs_len = match fcs_flag {
        0 if single_segment => 1,
        0 => 0,
        1 => 2,
        2 => 4,
        3 => 8,
        _ => unreachable!("two-bit fcs flag"),
    };
    let content_size = if fcs_len == 0 {
        None
    } else {
        let end = offset
            .checked_add(fcs_len)
            .ok_or_else(|| decompression_failed("zstd content-size offset overflow"))?;
        let fcs = bytes
            .get(offset..end)
            .ok_or_else(|| decompression_failed("truncated zstd content size"))?;
        let mut raw = 0u64;
        for (i, byte) in fcs.iter().enumerate() {
            raw |= u64::from(*byte) << (8 * i);
        }
        if fcs_len == 2 {
            raw += 256;
        }
        offset = end;
        Some(raw)
    };
    let Some(content_size_value) = content_size else {
        return Ok(FrameHeader {
            content_size,
            content_checksum,
            window_size: window_size.unwrap_or(0),
            header_len: offset,
        });
    };
    Ok(FrameHeader {
        content_size,
        content_checksum,
        window_size: window_size.unwrap_or(content_size_value),
        header_len: offset,
    })
}

fn single_frame_len(
    bytes: &[u8],
    mut offset: usize,
    content_checksum: bool,
) -> Result<usize, SampleArchiveError> {
    loop {
        let header_end = offset
            .checked_add(ZSTD_BLOCK_HEADER_LEN)
            .ok_or_else(|| decompression_failed("zstd block offset overflow"))?;
        let block_header = bytes
            .get(offset..header_end)
            .ok_or_else(|| decompression_failed("truncated zstd block header"))?;
        let raw = u32::from(block_header[0])
            | (u32::from(block_header[1]) << 8)
            | (u32::from(block_header[2]) << 16);
        offset = header_end;

        let last_block = raw & 1 == 1;
        let block_type = (raw >> 1) & 0x3;
        let block_size = usize::try_from(raw >> 3)
            .map_err(|_| decompression_failed("zstd block size too large"))?;
        let payload_len = match block_type {
            0 | 2 => block_size,
            1 => 1,
            3 => return Err(decompression_failed("reserved zstd block type")),
            _ => unreachable!("two-bit block type"),
        };
        offset = offset
            .checked_add(payload_len)
            .ok_or_else(|| decompression_failed("zstd block payload offset overflow"))?;
        if bytes.len() < offset {
            return Err(decompression_failed("truncated zstd block payload"));
        }
        if last_block {
            break;
        }
    }

    if content_checksum {
        offset = offset
            .checked_add(ZSTD_FRAME_CHECKSUM_LEN)
            .ok_or_else(|| decompression_failed("zstd checksum offset overflow"))?;
        if bytes.len() < offset {
            return Err(decompression_failed("truncated zstd frame checksum"));
        }
    }
    Ok(offset)
}

fn compression_failed(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::Io, detail)
}

fn decompression_failed(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::DecompressionFailed, detail)
}
