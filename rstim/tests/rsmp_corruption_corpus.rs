use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sample_archive::format::{
    ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, BLOCK_MAGIC, BlockHeader, GLOBAL_HEADER_LEN,
    SampleArchiveErrorCode,
};
use rstim::sample_archive::{ArchiveLimits, SampleArchiveReader};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::ops::Range;
use std::path::PathBuf;

const FIXTURE_CIRCUIT: &str = "rstim/tests/fixtures/rsmp/v1/compat.stim";
const FIXTURE_ARCHIVE: &str = "rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp";

#[test]
fn terminal_reader_error_is_latched() {
    let circuit = fixture_circuit();
    let mut archive = fixture_archive();
    let blocks = block_ranges(&archive);
    archive[blocks[0].free.end - 1] ^= 0x80;
    recompute_trailer_digest(&mut archive);

    let mut reader =
        SampleArchiveReader::open(Cursor::new(&archive), &circuit, ArchiveLimits::default())
            .expect("open mutated fixture");
    let first = reader
        .next_block()
        .expect_err("corrupt current block must error");
    assert_eq!(first.code(), SampleArchiveErrorCode::DecompressionFailed);
    let second = reader
        .next_block()
        .expect_err("reader must stay terminal after error");
    assert_eq!(second.code(), first.code());
    let finish = reader
        .finish()
        .expect_err("finish must stay terminal after error");
    assert_eq!(finish.code(), first.code());
}

fn fixture_circuit() -> Vec<StimInstr> {
    let text = fs::read_to_string(repo_path(FIXTURE_CIRCUIT)).expect("read fixture circuit");
    parse_lines(&text).expect("parse fixture circuit")
}

fn fixture_archive() -> Vec<u8> {
    fs::read(repo_path(FIXTURE_ARCHIVE)).expect("read fixture archive")
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(relative)
}

#[derive(Debug)]
struct BlockRanges {
    free: Range<usize>,
}

fn block_ranges(archive: &[u8]) -> Vec<BlockRanges> {
    let mut ranges = Vec::new();
    let mut offset = GLOBAL_HEADER_LEN;
    while archive[offset..offset + 8] == BLOCK_MAGIC[..] {
        let header_end = offset + BLOCK_HEADER_LEN;
        let header = BlockHeader::from_bytes(&archive[offset..header_end]).expect("block header");
        let syndrome_start = header_end;
        let syndrome_end = syndrome_start + header.syndrome_compressed_len as usize;
        let free_end = syndrome_end + header.free_compressed_len as usize;
        ranges.push(BlockRanges {
            free: syndrome_end..free_end,
        });
        offset = free_end;
    }
    ranges
}

fn recompute_trailer_digest(archive: &mut [u8]) {
    let trailer_start = archive.len() - ARCHIVE_TRAILER_LEN;
    let digest: [u8; 32] = Sha256::digest(&archive[..trailer_start + 32]).into();
    archive[trailer_start + 32..trailer_start + 64].copy_from_slice(&digest);
}
