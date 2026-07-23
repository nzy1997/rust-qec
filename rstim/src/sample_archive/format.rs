//! The fixed-width RSMP v1 structural envelope.

use std::error::Error;
use std::fmt;

pub const FORMAT_MAJOR: u16 = 1;
pub const FORMAT_MINOR: u16 = 0;
pub const GLOBAL_MAGIC: &[u8; 8] = b"RSTMSMP\0";
pub const BLOCK_MAGIC: &[u8; 8] = b"RSMPBLK\0";
pub const TRAILER_MAGIC: &[u8; 8] = b"RSMPEND\0";
pub const GLOBAL_HEADER_LEN: usize = 152;
pub const BLOCK_HEADER_LEN: usize = 108;
pub const ARCHIVE_TRAILER_LEN: usize = 64;
pub const MAX_GLOBAL_HEADER_LEN: u32 = 65_535;
pub const DEFAULT_MAX_SHOTS_PER_BLOCK: u64 = 4096;
pub const CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1: u16 = 1;
pub const FINGERPRINT_SHA256_CANONICAL_CIRCUIT: u16 = 1;
pub const TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1: u16 = 1;
pub const REFERENCE_SIMULATE_NOISELESS: u16 = 1;
pub const CODEC_SUITE_ZSTD_FRAMES_V1: u16 = 1;
pub const STREAM_CODEC_EMPTY: u16 = 0;
pub const STREAM_CODEC_SYNDROME_DENSE_V1: u16 = 1;
pub const STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1: u16 = 2;
pub const STREAM_CODEC_FREE_DENSE_V1: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldSpec {
    pub name: &'static str,
    pub offset: usize,
    pub width: usize,
}

pub const GLOBAL_FIELDS: [FieldSpec; 21] = [
    FieldSpec {
        name: "magic",
        offset: 0,
        width: 8,
    },
    FieldSpec {
        name: "format_major",
        offset: 8,
        width: 2,
    },
    FieldSpec {
        name: "format_minor",
        offset: 10,
        width: 2,
    },
    FieldSpec {
        name: "header_len",
        offset: 12,
        width: 4,
    },
    FieldSpec {
        name: "required_flags",
        offset: 16,
        width: 4,
    },
    FieldSpec {
        name: "optional_flags",
        offset: 20,
        width: 4,
    },
    FieldSpec {
        name: "reserved_flags",
        offset: 24,
        width: 4,
    },
    FieldSpec {
        name: "canonicalization_id",
        offset: 28,
        width: 2,
    },
    FieldSpec {
        name: "fingerprint_id",
        offset: 30,
        width: 2,
    },
    FieldSpec {
        name: "transform_id",
        offset: 32,
        width: 2,
    },
    FieldSpec {
        name: "reference_id",
        offset: 34,
        width: 2,
    },
    FieldSpec {
        name: "codec_suite_id",
        offset: 36,
        width: 2,
    },
    FieldSpec {
        name: "reserved0",
        offset: 38,
        width: 2,
    },
    FieldSpec {
        name: "max_shots_per_block",
        offset: 40,
        width: 8,
    },
    FieldSpec {
        name: "measurement_count",
        offset: 48,
        width: 8,
    },
    FieldSpec {
        name: "detector_count",
        offset: 56,
        width: 8,
    },
    FieldSpec {
        name: "observable_count",
        offset: 64,
        width: 8,
    },
    FieldSpec {
        name: "detector_rank",
        offset: 72,
        width: 8,
    },
    FieldSpec {
        name: "total_shots",
        offset: 80,
        width: 8,
    },
    FieldSpec {
        name: "circuit_sha256",
        offset: 88,
        width: 32,
    },
    FieldSpec {
        name: "header_sha256",
        offset: 120,
        width: 32,
    },
];

pub const BLOCK_FIELDS: [FieldSpec; 14] = [
    FieldSpec {
        name: "magic",
        offset: 0,
        width: 8,
    },
    FieldSpec {
        name: "format_major",
        offset: 8,
        width: 2,
    },
    FieldSpec {
        name: "format_minor",
        offset: 10,
        width: 2,
    },
    FieldSpec {
        name: "block_index",
        offset: 12,
        width: 8,
    },
    FieldSpec {
        name: "first_shot",
        offset: 20,
        width: 8,
    },
    FieldSpec {
        name: "shot_count",
        offset: 28,
        width: 8,
    },
    FieldSpec {
        name: "syndrome_codec_id",
        offset: 36,
        width: 2,
    },
    FieldSpec {
        name: "free_codec_id",
        offset: 38,
        width: 2,
    },
    FieldSpec {
        name: "reserved0",
        offset: 40,
        width: 4,
    },
    FieldSpec {
        name: "syndrome_uncompressed_len",
        offset: 44,
        width: 8,
    },
    FieldSpec {
        name: "syndrome_compressed_len",
        offset: 52,
        width: 8,
    },
    FieldSpec {
        name: "free_uncompressed_len",
        offset: 60,
        width: 8,
    },
    FieldSpec {
        name: "free_compressed_len",
        offset: 68,
        width: 8,
    },
    FieldSpec {
        name: "logical_payload_sha256",
        offset: 76,
        width: 32,
    },
];

pub const TRAILER_FIELDS: [FieldSpec; 7] = [
    FieldSpec {
        name: "magic",
        offset: 0,
        width: 8,
    },
    FieldSpec {
        name: "format_major",
        offset: 8,
        width: 2,
    },
    FieldSpec {
        name: "format_minor",
        offset: 10,
        width: 2,
    },
    FieldSpec {
        name: "reserved0",
        offset: 12,
        width: 4,
    },
    FieldSpec {
        name: "block_count",
        offset: 16,
        width: 8,
    },
    FieldSpec {
        name: "total_shots",
        offset: 24,
        width: 8,
    },
    FieldSpec {
        name: "archive_sha256",
        offset: 32,
        width: 32,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleArchiveErrorCode {
    BadMagic,
    UnsupportedVersion,
    UnsupportedFeature,
    UnsupportedSweep,
    CircuitMismatch,
    ShapeMismatch,
    LimitExceeded,
    Truncated,
    MalformedArchive,
    DecompressionFailed,
    ChecksumMismatch,
    LogicalDigestMismatch,
    TrailingData,
    Io,
}

impl SampleArchiveErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadMagic => "RSMP_BAD_MAGIC",
            Self::UnsupportedVersion => "RSMP_UNSUPPORTED_VERSION",
            Self::UnsupportedFeature => "RSMP_UNSUPPORTED_FEATURE",
            Self::UnsupportedSweep => "RSMP_UNSUPPORTED_SWEEP",
            Self::CircuitMismatch => "RSMP_CIRCUIT_MISMATCH",
            Self::ShapeMismatch => "RSMP_SHAPE_MISMATCH",
            Self::LimitExceeded => "RSMP_LIMIT_EXCEEDED",
            Self::Truncated => "RSMP_TRUNCATED",
            Self::MalformedArchive => "RSMP_MALFORMED_ARCHIVE",
            Self::DecompressionFailed => "RSMP_DECOMPRESSION_FAILED",
            Self::ChecksumMismatch => "RSMP_CHECKSUM_MISMATCH",
            Self::LogicalDigestMismatch => "RSMP_LOGICAL_DIGEST_MISMATCH",
            Self::TrailingData => "RSMP_TRAILING_DATA",
            Self::Io => "RSMP_IO",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleArchiveError {
    code: SampleArchiveErrorCode,
    detail: &'static str,
}

impl SampleArchiveError {
    pub const fn code(&self) -> SampleArchiveErrorCode {
        self.code
    }

    pub(crate) const fn with_code(code: SampleArchiveErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }

    const fn new(code: SampleArchiveErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl fmt::Display for SampleArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl Error for SampleArchiveError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalHeader {
    pub required_flags: u32,
    pub optional_flags: u32,
    pub canonicalization_id: u16,
    pub fingerprint_id: u16,
    pub transform_id: u16,
    pub reference_id: u16,
    pub codec_suite_id: u16,
    pub max_shots_per_block: u64,
    pub measurement_count: u64,
    pub detector_count: u64,
    pub observable_count: u64,
    pub detector_rank: u64,
    pub total_shots: u64,
    pub circuit_sha256: [u8; 32],
    pub header_sha256: [u8; 32],
}

impl GlobalHeader {
    pub const fn known_vector_v1() -> Self {
        Self {
            required_flags: 0,
            optional_flags: 0,
            canonicalization_id: CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
            fingerprint_id: FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
            transform_id: TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
            reference_id: REFERENCE_SIMULATE_NOISELESS,
            codec_suite_id: CODEC_SUITE_ZSTD_FRAMES_V1,
            max_shots_per_block: DEFAULT_MAX_SHOTS_PER_BLOCK,
            measurement_count: 258,
            detector_count: 513,
            observable_count: 2,
            detector_rank: 257,
            total_shots: 0,
            circuit_sha256: [
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
                0x1c, 0x1d, 0x1e, 0x1f,
            ],
            header_sha256: [
                0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
                0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb,
                0xbc, 0xbd, 0xbe, 0xbf,
            ],
        }
    }

    pub fn to_bytes(&self) -> Result<[u8; GLOBAL_HEADER_LEN], SampleArchiveError> {
        self.validate()?;
        let mut bytes = [0; GLOBAL_HEADER_LEN];
        bytes[0..8].copy_from_slice(GLOBAL_MAGIC);
        put_u16(&mut bytes, 8, FORMAT_MAJOR);
        put_u16(&mut bytes, 10, FORMAT_MINOR);
        put_u32(&mut bytes, 12, GLOBAL_HEADER_LEN as u32);
        put_u32(&mut bytes, 16, self.required_flags);
        put_u32(&mut bytes, 20, self.optional_flags);
        put_u16(&mut bytes, 28, self.canonicalization_id);
        put_u16(&mut bytes, 30, self.fingerprint_id);
        put_u16(&mut bytes, 32, self.transform_id);
        put_u16(&mut bytes, 34, self.reference_id);
        put_u16(&mut bytes, 36, self.codec_suite_id);
        put_u64(&mut bytes, 40, self.max_shots_per_block);
        put_u64(&mut bytes, 48, self.measurement_count);
        put_u64(&mut bytes, 56, self.detector_count);
        put_u64(&mut bytes, 64, self.observable_count);
        put_u64(&mut bytes, 72, self.detector_rank);
        put_u64(&mut bytes, 80, self.total_shots);
        bytes[88..120].copy_from_slice(&self.circuit_sha256);
        bytes[120..152].copy_from_slice(&self.header_sha256);
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SampleArchiveError> {
        require_len(bytes, GLOBAL_HEADER_LEN)?;
        validate_magic(bytes, GLOBAL_MAGIC)?;
        validate_version(bytes)?;
        let header_len = get_u32(bytes, 12);
        if !(GLOBAL_HEADER_LEN as u32..=MAX_GLOBAL_HEADER_LEN).contains(&header_len)
            || header_len != GLOBAL_HEADER_LEN as u32
        {
            return Err(malformed("invalid global header length"));
        }
        let header = Self {
            required_flags: get_u32(bytes, 16),
            optional_flags: get_u32(bytes, 20),
            canonicalization_id: get_u16(bytes, 28),
            fingerprint_id: get_u16(bytes, 30),
            transform_id: get_u16(bytes, 32),
            reference_id: get_u16(bytes, 34),
            codec_suite_id: get_u16(bytes, 36),
            max_shots_per_block: get_u64(bytes, 40),
            measurement_count: get_u64(bytes, 48),
            detector_count: get_u64(bytes, 56),
            observable_count: get_u64(bytes, 64),
            detector_rank: get_u64(bytes, 72),
            total_shots: get_u64(bytes, 80),
            circuit_sha256: array_at(bytes, 88),
            header_sha256: array_at(bytes, 120),
        };
        if get_u32(bytes, 24) != 0 || get_u16(bytes, 38) != 0 {
            return Err(malformed("nonzero reserved global field"));
        }
        header.validate()?;
        Ok(header)
    }

    fn validate(&self) -> Result<(), SampleArchiveError> {
        if self.required_flags != 0 {
            return Err(SampleArchiveError::new(
                SampleArchiveErrorCode::UnsupportedFeature,
                "unknown required feature",
            ));
        }
        if self.optional_flags != 0 {
            return Err(malformed("unknown optional feature"));
        }
        if self.canonicalization_id != CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1
            || self.fingerprint_id != FINGERPRINT_SHA256_CANONICAL_CIRCUIT
            || self.transform_id != TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1
            || self.reference_id != REFERENCE_SIMULATE_NOISELESS
            || self.codec_suite_id != CODEC_SUITE_ZSTD_FRAMES_V1
        {
            return Err(malformed("unsupported v1 identifier"));
        }
        if self.max_shots_per_block == 0 {
            return Err(malformed("zero max shots per block"));
        }
        if self.detector_rank > self.detector_count || self.detector_rank > self.measurement_count {
            return Err(malformed("invalid detector rank"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockHeader {
    pub block_index: u64,
    pub first_shot: u64,
    pub shot_count: u64,
    pub syndrome_codec_id: u16,
    pub free_codec_id: u16,
    pub syndrome_uncompressed_len: u64,
    pub syndrome_compressed_len: u64,
    pub free_uncompressed_len: u64,
    pub free_compressed_len: u64,
    pub logical_payload_sha256: [u8; 32],
}

impl BlockHeader {
    pub const fn known_vector_v1() -> Self {
        Self {
            block_index: 0x0102_0304_0506_0708,
            first_shot: 0x1112_1314_1516_1718,
            shot_count: 33,
            syndrome_codec_id: STREAM_CODEC_SYNDROME_DENSE_V1,
            free_codec_id: STREAM_CODEC_FREE_DENSE_V1,
            syndrome_uncompressed_len: 5,
            syndrome_compressed_len: 13,
            free_uncompressed_len: 2,
            free_compressed_len: 9,
            logical_payload_sha256: [
                0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xcb, 0xcc, 0xcd,
                0xce, 0xcf, 0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xdb,
                0xdc, 0xdd, 0xde, 0xdf,
            ],
        }
    }

    pub fn to_bytes(&self) -> Result<[u8; BLOCK_HEADER_LEN], SampleArchiveError> {
        self.validate()?;
        let mut bytes = [0; BLOCK_HEADER_LEN];
        bytes[0..8].copy_from_slice(BLOCK_MAGIC);
        put_u16(&mut bytes, 8, FORMAT_MAJOR);
        put_u16(&mut bytes, 10, FORMAT_MINOR);
        put_u64(&mut bytes, 12, self.block_index);
        put_u64(&mut bytes, 20, self.first_shot);
        put_u64(&mut bytes, 28, self.shot_count);
        put_u16(&mut bytes, 36, self.syndrome_codec_id);
        put_u16(&mut bytes, 38, self.free_codec_id);
        put_u64(&mut bytes, 44, self.syndrome_uncompressed_len);
        put_u64(&mut bytes, 52, self.syndrome_compressed_len);
        put_u64(&mut bytes, 60, self.free_uncompressed_len);
        put_u64(&mut bytes, 68, self.free_compressed_len);
        bytes[76..108].copy_from_slice(&self.logical_payload_sha256);
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SampleArchiveError> {
        require_len(bytes, BLOCK_HEADER_LEN)?;
        validate_magic(bytes, BLOCK_MAGIC)?;
        validate_version(bytes)?;
        if get_u32(bytes, 40) != 0 {
            return Err(malformed("nonzero reserved block field"));
        }
        let header = Self {
            block_index: get_u64(bytes, 12),
            first_shot: get_u64(bytes, 20),
            shot_count: get_u64(bytes, 28),
            syndrome_codec_id: get_u16(bytes, 36),
            free_codec_id: get_u16(bytes, 38),
            syndrome_uncompressed_len: get_u64(bytes, 44),
            syndrome_compressed_len: get_u64(bytes, 52),
            free_uncompressed_len: get_u64(bytes, 60),
            free_compressed_len: get_u64(bytes, 68),
            logical_payload_sha256: array_at(bytes, 76),
        };
        header.validate()?;
        Ok(header)
    }

    fn validate(&self) -> Result<(), SampleArchiveError> {
        if self.shot_count == 0 {
            return Err(malformed("zero block shot count"));
        }
        self.first_shot
            .checked_add(self.shot_count)
            .ok_or_else(|| malformed("block shot range overflow"))?;
        validate_stream(
            self.syndrome_codec_id,
            self.syndrome_uncompressed_len,
            self.syndrome_compressed_len,
            true,
        )?;
        validate_stream(
            self.free_codec_id,
            self.free_uncompressed_len,
            self.free_compressed_len,
            false,
        )?;
        self.syndrome_uncompressed_len
            .checked_add(self.free_uncompressed_len)
            .ok_or_else(|| malformed("uncompressed length overflow"))?;
        self.syndrome_compressed_len
            .checked_add(self.free_compressed_len)
            .ok_or_else(|| malformed("compressed length overflow"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveTrailer {
    pub block_count: u64,
    pub total_shots: u64,
    pub archive_sha256: [u8; 32],
}

impl ArchiveTrailer {
    pub const fn known_vector_v1() -> Self {
        Self {
            block_count: 0,
            total_shots: 0,
            archive_sha256: [
                0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d,
                0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
                0x9c, 0x9d, 0x9e, 0x9f,
            ],
        }
    }

    pub fn to_bytes(&self) -> Result<[u8; ARCHIVE_TRAILER_LEN], SampleArchiveError> {
        let mut bytes = [0; ARCHIVE_TRAILER_LEN];
        bytes[0..8].copy_from_slice(TRAILER_MAGIC);
        put_u16(&mut bytes, 8, FORMAT_MAJOR);
        put_u16(&mut bytes, 10, FORMAT_MINOR);
        put_u64(&mut bytes, 16, self.block_count);
        put_u64(&mut bytes, 24, self.total_shots);
        bytes[32..64].copy_from_slice(&self.archive_sha256);
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SampleArchiveError> {
        require_len(bytes, ARCHIVE_TRAILER_LEN)?;
        validate_magic(bytes, TRAILER_MAGIC)?;
        validate_version(bytes)?;
        if get_u32(bytes, 12) != 0 {
            return Err(malformed("nonzero reserved trailer field"));
        }
        Ok(Self {
            block_count: get_u64(bytes, 16),
            total_shots: get_u64(bytes, 24),
            archive_sha256: array_at(bytes, 32),
        })
    }
}

pub fn checked_dense_bit_bytes(bits_per_shot: u64, shots: u64) -> Result<u64, SampleArchiveError> {
    let bits = bits_per_shot
        .checked_mul(shots)
        .ok_or_else(|| limit("dense bit count overflow"))?;
    bits.checked_add(7)
        .ok_or_else(|| limit("dense byte count overflow"))
        .map(|value| value / 8)
}

pub fn checked_logical_payload_bytes(
    selected_detector_count: u64,
    free_measurement_count: u64,
    shots: u64,
) -> Result<u64, SampleArchiveError> {
    let selected_bytes = checked_dense_bit_bytes(selected_detector_count, shots)?;
    let free_bytes = checked_dense_bit_bytes(free_measurement_count, shots)?;
    selected_bytes
        .checked_add(free_bytes)
        .ok_or_else(|| limit("logical payload byte count overflow"))
}

fn validate_stream(
    codec: u16,
    uncompressed: u64,
    compressed: u64,
    syndrome: bool,
) -> Result<(), SampleArchiveError> {
    if uncompressed == 0 {
        if codec != STREAM_CODEC_EMPTY || compressed != 0 {
            return Err(malformed("noncanonical empty stream"));
        }
    } else if syndrome {
        if !matches!(
            codec,
            STREAM_CODEC_SYNDROME_DENSE_V1 | STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1
        ) || compressed == 0
        {
            return Err(malformed("invalid syndrome stream codec"));
        }
    } else if codec != STREAM_CODEC_FREE_DENSE_V1 || compressed == 0 {
        return Err(malformed("invalid free stream codec"));
    }
    Ok(())
}

fn require_len(bytes: &[u8], len: usize) -> Result<(), SampleArchiveError> {
    if bytes.len() < len {
        Err(SampleArchiveError::new(
            SampleArchiveErrorCode::Truncated,
            "truncated fixed-width record",
        ))
    } else if bytes.len() > len {
        Err(malformed("unexpected bytes after fixed-width record"))
    } else {
        Ok(())
    }
}
fn validate_magic(bytes: &[u8], magic: &[u8; 8]) -> Result<(), SampleArchiveError> {
    if bytes[0..8] == magic[..] {
        Ok(())
    } else {
        Err(SampleArchiveError::new(
            SampleArchiveErrorCode::BadMagic,
            "invalid record magic",
        ))
    }
}
fn validate_version(bytes: &[u8]) -> Result<(), SampleArchiveError> {
    if get_u16(bytes, 8) == FORMAT_MAJOR && get_u16(bytes, 10) == FORMAT_MINOR {
        Ok(())
    } else {
        Err(SampleArchiveError::new(
            SampleArchiveErrorCode::UnsupportedVersion,
            "unsupported format version",
        ))
    }
}
fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated field width"),
    )
}
fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated field width"),
    )
}
fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("validated field width"),
    )
}
fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn array_at(bytes: &[u8], offset: usize) -> [u8; 32] {
    bytes[offset..offset + 32]
        .try_into()
        .expect("validated digest width")
}
const fn malformed(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::new(SampleArchiveErrorCode::MalformedArchive, detail)
}
const fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::new(SampleArchiveErrorCode::LimitExceeded, detail)
}
