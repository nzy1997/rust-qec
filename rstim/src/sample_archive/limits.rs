use crate::measurement_transform::MeasurementTransformLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub transform: MeasurementTransformLimits,
    pub max_total_shots: u64,
    pub max_detector_rank: u64,
    pub max_free_measurements: u64,
    pub max_compressed_bytes_per_stream: u64,
    pub max_decompressed_bytes_per_stream: u64,
    pub max_compressed_bytes_per_archive: u64,
    pub max_decompressed_bytes_per_archive: u64,
    pub max_zstd_window_bytes: u64,
    pub max_zstd_decoder_memory_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            transform: MeasurementTransformLimits::default(),
            max_total_shots: crate::sample_archive::format::DEFAULT_MAX_SHOTS_PER_BLOCK,
            max_detector_rank: 10_000_000,
            max_free_measurements: 10_000_000,
            max_compressed_bytes_per_stream: 64 * 1024 * 1024,
            max_decompressed_bytes_per_stream: 64 * 1024 * 1024,
            max_compressed_bytes_per_archive: 256 * 1024 * 1024,
            max_decompressed_bytes_per_archive: 256 * 1024 * 1024,
            max_zstd_window_bytes: 8 * 1024 * 1024,
            max_zstd_decoder_memory_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleArchiveOptions {
    pub compression_level: i32,
}

impl Default for SampleArchiveOptions {
    fn default() -> Self {
        Self {
            compression_level: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_defaults_keep_transform_limits_embedded() {
        let limits = ArchiveLimits::default();
        assert_eq!(limits.transform, MeasurementTransformLimits::default());
        assert_eq!(
            limits.max_total_shots,
            crate::sample_archive::format::DEFAULT_MAX_SHOTS_PER_BLOCK
        );
        assert_eq!(limits.max_detector_rank, 10_000_000);
        assert_eq!(limits.max_free_measurements, 10_000_000);
        assert_eq!(limits.max_compressed_bytes_per_stream, 64 * 1024 * 1024);
        assert_eq!(limits.max_decompressed_bytes_per_stream, 64 * 1024 * 1024);
        assert_eq!(limits.max_compressed_bytes_per_archive, 256 * 1024 * 1024);
        assert_eq!(limits.max_decompressed_bytes_per_archive, 256 * 1024 * 1024);
        assert_eq!(limits.max_zstd_window_bytes, 8 * 1024 * 1024);
        assert_eq!(limits.max_zstd_decoder_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(SampleArchiveOptions::default().compression_level, 3);
    }
}
