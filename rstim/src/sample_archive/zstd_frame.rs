use crate::sample_archive::format::{SampleArchiveError, SampleArchiveErrorCode};
use crate::sample_archive::limits::ArchiveLimits;
use std::io::Write;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const ZSTD_BLOCK_HEADER_LEN: usize = 3;
const ZSTD_FRAME_CHECKSUM_LEN: usize = 4;
const ZSTD_DECODER_CONTEXT_BYTES: u64 = 256 * 1024;

type ArchiveResult<T> = Result<T, SampleArchiveError>;

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
        .long_distance_matching(false)
        .map_err(|_| compression_failed("failed to disable zstd long-distance matching"))?;
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
    let frame_len = single_frame_len(compressed, header.3, header.1)?;
    if frame_len != compressed.len() && starts_with_zstd_frame_magic(&compressed[frame_len..]) {
        return Err(malformed_archive(
            "zstd stream must contain exactly one frame",
        ));
    }
    if header.2 > limits.max_zstd_window_bytes {
        return Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd window exceeds limit",
        ));
    }
    let content_size = header
        .0
        .ok_or_else(|| decompression_failed("zstd frame content size is missing"))?;
    let decoder_memory = checked_decoder_memory_bytes(compressed.len(), content_size, header.2)?;
    if decoder_memory > limits.max_zstd_decoder_memory_bytes {
        return Err(SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd decoder memory exceeds limit",
        ));
    }
    if content_size != declared_len {
        return Err(decompression_failed(
            "zstd frame content size does not match declaration",
        ));
    }
    if !header.1 {
        return Err(decompression_failed("zstd frame checksum is missing"));
    }
    if frame_len != compressed.len() {
        return Err(malformed_archive(
            "zstd stream must contain exactly one frame",
        ));
    }
    let capacity = usize::try_from(declared_len).map_err(|_| {
        SampleArchiveError::with_code(
            SampleArchiveErrorCode::LimitExceeded,
            "zstd content size too large",
        )
    })?;
    let mut decoder = zstd::bulk::Decompressor::new()
        .map_err(|_| decompression_failed("failed to initialize zstd decoder"))?;
    decoder
        .window_log_max(window_log_max_for_limit(limits.max_zstd_window_bytes))
        .map_err(|_| decompression_failed("failed to configure zstd window limit"))?;
    let decoded = decoder
        .decompress(compressed, capacity)
        .map_err(|_| decompression_failed("zstd decompression failed"))?;
    if decoded.len() as u64 != declared_len {
        return Err(decompression_failed(
            "zstd decoded length does not match declaration",
        ));
    }
    Ok(decoded)
}

fn checked_decoder_memory_bytes(input: usize, content: u64, window: u64) -> ArchiveResult<u64> {
    let parts = [ZSTD_DECODER_CONTEXT_BYTES, input as u64, content, window];
    parts.into_iter().try_fold(0u64, |acc, value| {
        acc.checked_add(value)
            .ok_or_else(|| limit_exceeded("zstd decoder memory overflow"))
    })
}
fn window_log_max_for_limit(max_window_bytes: u64) -> u32 {
    let bounded = max_window_bytes.max(1024);
    (u64::BITS - bounded.saturating_sub(1).leading_zeros()).min(31)
}
fn starts_with_zstd_frame_magic(bytes: &[u8]) -> bool {
    if bytes.len() < ZSTD_MAGIC.len() {
        return false;
    }
    bytes[..ZSTD_MAGIC.len()] == ZSTD_MAGIC
        || (bytes[1..4] == [0x2a, 0x4d, 0x18] && (0x50..=0x5f).contains(&bytes[0]))
}
fn parse_frame_header(bytes: &[u8]) -> ArchiveResult<(Option<u64>, bool, u64, usize)> {
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
        return Ok((
            content_size,
            content_checksum,
            window_size.unwrap_or(0),
            offset,
        ));
    };
    Ok((
        content_size,
        content_checksum,
        window_size.unwrap_or(content_size_value),
        offset,
    ))
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

fn malformed_archive(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::MalformedArchive, detail)
}

fn limit_exceeded(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_header(desc: u8, tail: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::from(ZSTD_MAGIC);
        bytes.push(desc);
        bytes.extend_from_slice(tail);
        bytes
    }

    fn assert_code(result: Result<Vec<u8>, SampleArchiveError>, code: SampleArchiveErrorCode) {
        assert_eq!(result.unwrap_err().code(), code);
    }

    #[test]
    fn decompress_rejects_missing_checksum_and_content_size() {
        let frame = compress_frame(b"abc", 3).expect("compress test frame");

        let mut no_checksum = frame.clone();
        no_checksum[4] &= !0x04;
        assert_code(
            decompress_frame(&no_checksum, 3, ArchiveLimits::default()),
            SampleArchiveErrorCode::DecompressionFailed,
        );

        let no_content_size = frame_header(0x04, &[0, 1, 0, 0, 0, 0, 0, 0]);
        assert_code(
            decompress_frame(&no_content_size, 0, ArchiveLimits::default()),
            SampleArchiveErrorCode::DecompressionFailed,
        );

        let mut wrong_size = frame;
        wrong_size[5] ^= 1;
        assert_code(
            decompress_frame(&wrong_size, 3, ArchiveLimits::default()),
            SampleArchiveErrorCode::DecompressionFailed,
        );

        let mut trailing_junk = compress_frame(b"abc", 3).expect("compress test frame");
        trailing_junk.push(0);
        assert_code(
            decompress_frame(&trailing_junk, 3, ArchiveLimits::default()),
            SampleArchiveErrorCode::MalformedArchive,
        );
    }

    #[test]
    fn decompress_enforces_window_and_decoder_memory_limits() {
        let frame = compress_frame(b"abc", 3).expect("compress test frame");

        let mut limits = ArchiveLimits::default();
        limits.max_zstd_window_bytes = 1;
        assert_code(
            decompress_frame(&frame, 3, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let mut limits = ArchiveLimits::default();
        limits.max_zstd_window_bytes = u64::MAX;
        limits.max_zstd_decoder_memory_bytes = 1;
        assert_code(
            decompress_frame(&frame, 3, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );

        let huge_content = frame_header(
            0xc4,
            &[
                0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1, 0, 0, 0, 0, 0, 0,
            ],
        );
        assert_code(
            decompress_frame(&huge_content, u64::MAX, limits),
            SampleArchiveErrorCode::LimitExceeded,
        );
    }

    #[test]
    fn frame_header_parser_covers_descriptor_shapes() {
        assert_eq!(
            parse_frame_header(&[]).unwrap_err().code(),
            SampleArchiveErrorCode::DecompressionFailed
        );

        let mut reserved = frame_header(0x2c, &[0]);
        reserved.resize(8, 0);
        assert_eq!(
            parse_frame_header(&reserved).unwrap_err().code(),
            SampleArchiveErrorCode::DecompressionFailed
        );

        let dictionary_truncated = frame_header(0x27, &[0]);
        assert_eq!(
            parse_frame_header(&dictionary_truncated)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::DecompressionFailed
        );

        let no_content_size = parse_frame_header(&frame_header(0x04, &[0])).unwrap();
        assert_eq!(no_content_size.0, None);
        assert_eq!(no_content_size.1, true);
        assert_eq!(no_content_size.2, 1024);
        assert_eq!(no_content_size.3, 6);

        let one_byte_content = parse_frame_header(&frame_header(0x24, &[7])).unwrap();
        assert_eq!(one_byte_content.0, Some(7));
        assert_eq!(one_byte_content.2, 7);

        let two_byte_content = parse_frame_header(&frame_header(0x64, &[1, 0])).unwrap();
        assert_eq!(two_byte_content.0, Some(257));

        let four_byte_content = parse_frame_header(&frame_header(0xa4, &[1, 0, 0, 0])).unwrap();
        assert_eq!(four_byte_content.0, Some(1));

        let eight_byte_content =
            parse_frame_header(&frame_header(0xe4, &[1, 0, 0, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!(eight_byte_content.0, Some(1));

        let dict_one = parse_frame_header(&frame_header(0x25, &[0, 3])).unwrap();
        assert_eq!(dict_one.0, Some(3));
        let dict_two = parse_frame_header(&frame_header(0x26, &[0, 0, 3])).unwrap();
        assert_eq!(dict_two.0, Some(3));
        let dict_four = parse_frame_header(&frame_header(0x27, &[0, 0, 0, 0, 3])).unwrap();
        assert_eq!(dict_four.0, Some(3));

        assert!(!starts_with_zstd_frame_magic(&[0]));
    }

    #[test]
    fn single_frame_len_covers_block_shapes_and_truncation() {
        assert_eq!(single_frame_len(&[3, 0, 0, 0xaa], 0, false).unwrap(), 4);

        assert_eq!(
            single_frame_len(&[7, 0, 0], 0, false).unwrap_err().code(),
            SampleArchiveErrorCode::DecompressionFailed
        );
        assert_eq!(
            single_frame_len(&[17, 0, 0, 0xaa], 0, false)
                .unwrap_err()
                .code(),
            SampleArchiveErrorCode::DecompressionFailed
        );
        assert_eq!(
            single_frame_len(&[1, 0, 0], 0, true).unwrap_err().code(),
            SampleArchiveErrorCode::DecompressionFailed
        );
    }

    #[test]
    fn compression_error_helper_maps_to_io() {
        assert_eq!(
            compression_failed("test compression failure").code(),
            SampleArchiveErrorCode::Io
        );
    }
}
