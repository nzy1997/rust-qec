use crate::measurement_transform::{MeasurementTransform, MeasurementTransformError};
use crate::sample_archive::dense::pack_dense;
use crate::sample_archive::format::{
    BlockHeader, CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1, CODEC_SUITE_ZSTD_FRAMES_V1,
    FINGERPRINT_SHA256_CANONICAL_CIRCUIT, GlobalHeader, REFERENCE_SIMULATE_NOISELESS,
    STREAM_CODEC_EMPTY, STREAM_CODEC_FREE_DENSE_V1, STREAM_CODEC_SYNDROME_DENSE_V1,
    SampleArchiveError, SampleArchiveErrorCode, TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
    checked_dense_bit_bytes,
};
use crate::sample_archive::integrity::{finalize_header, finalize_trailer};
use crate::sample_archive::limits::{ArchiveLimits, SampleArchiveOptions};
use crate::sample_archive::zstd_frame::compress_frame;
use crate::sim::bit_table::BitTable;
use sha2::{Digest, Sha256};
use std::io::Write;

pub struct SampleArchiveWriter<W: Write> {
    output: W,
    transform: MeasurementTransform,
    total_shots: u64,
    options: SampleArchiveOptions,
    limits: ArchiveLimits,
    archive_hasher: Sha256,
    wrote_block: bool,
}

impl<W: Write> SampleArchiveWriter<W> {
    pub fn new(
        mut output: W,
        transform: MeasurementTransform,
        total_shots: u64,
        options: SampleArchiveOptions,
        limits: ArchiveLimits,
    ) -> Result<Self, SampleArchiveError> {
        validate_transform_and_archive_shape(&transform, total_shots, limits)?;
        let identity = transform.identity();
        let mut header = GlobalHeader {
            required_flags: 0,
            optional_flags: 0,
            canonicalization_id: CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
            fingerprint_id: FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
            transform_id: TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
            reference_id: REFERENCE_SIMULATE_NOISELESS,
            codec_suite_id: CODEC_SUITE_ZSTD_FRAMES_V1,
            max_shots_per_block: limits.transform.max_shots_per_block,
            measurement_count: identity.measurement_count,
            detector_count: identity.detector_count,
            observable_count: identity.observable_count,
            detector_rank: identity.detector_rank,
            total_shots,
            circuit_sha256: identity.circuit_sha256,
            header_sha256: [0; 32],
        };
        let header_bytes = finalize_header(&mut header)?;
        output.write_all(&header_bytes).map_err(map_io)?;
        let mut archive_hasher = Sha256::new();
        archive_hasher.update(header_bytes);
        Ok(Self {
            output,
            transform,
            total_shots,
            options,
            limits,
            archive_hasher,
            wrote_block: false,
        })
    }

    pub fn write_measurements(
        &mut self,
        measurements: &BitTable,
    ) -> Result<(), SampleArchiveError> {
        if self.total_shots == 0 {
            return Err(shape("zero-shot archive cannot contain a block"));
        }
        if self.wrote_block {
            return Err(shape("positive-shot archive already has its one block"));
        }
        if measurements.num_major() != self.transform.num_measurements()
            || measurements.num_minor() as u64 != self.total_shots
        {
            return Err(shape(
                "measurement table shape does not match archive header",
            ));
        }
        self.transform
            .validate_actual_usage(self.limits.transform, Some(measurements.num_minor()))
            .map_err(map_transform_error)?;

        let encoded = self
            .transform
            .encode_block(measurements)
            .map_err(map_transform_error)?;
        let syndrome_len = checked_dense_bit_bytes(
            encoded.selected_detectors.num_major() as u64,
            measurements.num_minor() as u64,
        )?;
        let free_len = checked_dense_bit_bytes(
            encoded.free_measurements.num_major() as u64,
            measurements.num_minor() as u64,
        )?;
        validate_decompressed_streams(syndrome_len, free_len, self.limits)?;
        let syndrome = pack_dense(&encoded.selected_detectors)?;
        let free = pack_dense(&encoded.free_measurements)?;
        let mut logical_hasher = Sha256::new();
        logical_hasher.update(&syndrome);
        logical_hasher.update(&free);
        let logical_payload_sha256: [u8; 32] = logical_hasher.finalize().into();

        let syndrome_frame = if syndrome.is_empty() {
            Vec::new()
        } else {
            compress_frame(&syndrome, self.options.compression_level)?
        };
        let free_frame = if free.is_empty() {
            Vec::new()
        } else {
            compress_frame(&free, self.options.compression_level)?
        };
        validate_compressed_streams(
            syndrome_frame.len() as u64,
            free_frame.len() as u64,
            self.limits,
        )?;

        let block_header = BlockHeader {
            block_index: 0,
            first_shot: 0,
            shot_count: self.total_shots,
            syndrome_codec_id: if syndrome.is_empty() {
                STREAM_CODEC_EMPTY
            } else {
                STREAM_CODEC_SYNDROME_DENSE_V1
            },
            free_codec_id: if free.is_empty() {
                STREAM_CODEC_EMPTY
            } else {
                STREAM_CODEC_FREE_DENSE_V1
            },
            syndrome_uncompressed_len: syndrome.len() as u64,
            syndrome_compressed_len: syndrome_frame.len() as u64,
            free_uncompressed_len: free.len() as u64,
            free_compressed_len: free_frame.len() as u64,
            logical_payload_sha256,
        };
        let block_bytes = block_header.to_bytes()?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &block_bytes)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &syndrome_frame)?;
        write_and_hash(&mut self.output, &mut self.archive_hasher, &free_frame)?;
        self.wrote_block = true;
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, SampleArchiveError> {
        if self.total_shots > 0 && !self.wrote_block {
            return Err(shape("positive-shot archive is missing its block"));
        }
        let block_count = u64::from(self.wrote_block);
        let trailer = finalize_trailer(block_count, self.total_shots, self.archive_hasher.clone())?;
        self.output.write_all(&trailer).map_err(map_io)?;
        self.output.flush().map_err(map_io)?;
        Ok(self.output)
    }
}

fn validate_transform_and_archive_shape(
    transform: &MeasurementTransform,
    total_shots: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if total_shots > limits.max_total_shots {
        return Err(limit("archive total shots exceed limit"));
    }
    if total_shots > usize::MAX as u64 {
        return Err(limit("archive shot count exceeds usize"));
    }
    transform
        .validate_actual_usage(limits.transform, Some(total_shots as usize))
        .map_err(map_transform_error)?;
    let rank = transform.rank() as u64;
    if rank > limits.max_detector_rank {
        return Err(limit("detector rank exceeds archive limit"));
    }
    let free = transform
        .num_measurements()
        .checked_sub(transform.rank())
        .ok_or_else(|| shape("rank exceeds measurement count"))? as u64;
    if free > limits.max_free_measurements {
        return Err(limit("free measurement width exceeds archive limit"));
    }
    Ok(())
}

fn validate_decompressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_decompressed_bytes_per_stream
        || free_len > limits.max_decompressed_bytes_per_stream
    {
        return Err(limit("decompressed stream exceeds limit"));
    }
    if syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("decompressed archive bytes overflow"))?
        > limits.max_decompressed_bytes_per_archive
    {
        return Err(limit("decompressed archive bytes exceed limit"));
    }
    Ok(())
}

fn validate_compressed_streams(
    syndrome_len: u64,
    free_len: u64,
    limits: ArchiveLimits,
) -> Result<(), SampleArchiveError> {
    if syndrome_len > limits.max_compressed_bytes_per_stream
        || free_len > limits.max_compressed_bytes_per_stream
    {
        return Err(limit("compressed stream exceeds limit"));
    }
    if syndrome_len
        .checked_add(free_len)
        .ok_or_else(|| limit("compressed archive bytes overflow"))?
        > limits.max_compressed_bytes_per_archive
    {
        return Err(limit("compressed archive bytes exceed limit"));
    }
    Ok(())
}

pub(crate) fn map_transform_error(err: MeasurementTransformError) -> SampleArchiveError {
    match err {
        MeasurementTransformError::UnsupportedSweep => SampleArchiveError::with_code(
            SampleArchiveErrorCode::UnsupportedSweep,
            "sweep-bit circuits are not supported",
        ),
        MeasurementTransformError::LimitExceeded { .. }
        | MeasurementTransformError::Allocation(_) => limit("measurement transform limit exceeded"),
        MeasurementTransformError::ShapeMismatch { .. } => {
            shape("measurement transform shape mismatch")
        }
        MeasurementTransformError::InvalidRecordTarget { .. }
        | MeasurementTransformError::Reference { .. } => SampleArchiveError::with_code(
            SampleArchiveErrorCode::MalformedArchive,
            "measurement transform construction failed",
        ),
    }
}

fn write_and_hash(
    output: &mut impl Write,
    hasher: &mut Sha256,
    bytes: &[u8],
) -> Result<(), SampleArchiveError> {
    output.write_all(bytes).map_err(map_io)?;
    hasher.update(bytes);
    Ok(())
}

fn map_io(_err: std::io::Error) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::Io, "archive I/O failed")
}

pub(crate) fn shape(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::ShapeMismatch, detail)
}

pub(crate) fn limit(detail: &'static str) -> SampleArchiveError {
    SampleArchiveError::with_code(SampleArchiveErrorCode::LimitExceeded, detail)
}
