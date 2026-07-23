use crate::sample_archive::format::{
    ArchiveTrailer, GLOBAL_HEADER_LEN, GlobalHeader, SampleArchiveError,
};
use sha2::{Digest, Sha256};

pub(crate) fn finalize_header(
    header: &mut GlobalHeader,
) -> Result<[u8; GLOBAL_HEADER_LEN], SampleArchiveError> {
    header.header_sha256 = [0; 32];
    let mut bytes = header.to_bytes()?;
    let digest = header_digest(&bytes);
    header.header_sha256 = digest;
    bytes[GLOBAL_HEADER_LEN - 32..GLOBAL_HEADER_LEN].copy_from_slice(&digest);
    Ok(bytes)
}

pub(crate) fn header_digest(header_bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(&header_bytes[..GLOBAL_HEADER_LEN - 32]).into()
}

pub(crate) fn trailer_prefix(
    block_count: u64,
    total_shots: u64,
) -> Result<[u8; 32], SampleArchiveError> {
    let trailer = ArchiveTrailer {
        block_count,
        total_shots,
        archive_sha256: [0; 32],
    };
    let bytes = trailer.to_bytes()?;
    Ok(bytes[..32].try_into().expect("trailer prefix width"))
}

pub(crate) fn finalize_trailer(
    block_count: u64,
    total_shots: u64,
    archive_hasher: Sha256,
) -> Result<[u8; crate::sample_archive::format::ARCHIVE_TRAILER_LEN], SampleArchiveError> {
    let prefix = trailer_prefix(block_count, total_shots)?;
    let mut hasher = archive_hasher;
    hasher.update(prefix);
    let digest: [u8; 32] = hasher.finalize().into();
    ArchiveTrailer {
        block_count,
        total_shots,
        archive_sha256: digest,
    }
    .to_bytes()
}
