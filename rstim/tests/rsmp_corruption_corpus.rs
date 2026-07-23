use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sample_archive::corruption_corpus::{
    run_corruption_corpus, CorruptionCaseResult, CorruptionCorpusOptions, CorruptionCorpusSummary,
    PASS_LINE,
};
use rstim::sample_archive::format::{
    BlockHeader, SampleArchiveErrorCode, ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, BLOCK_MAGIC,
    GLOBAL_HEADER_LEN,
};
use rstim::sample_archive::{ArchiveLimits, SampleArchiveReader};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::ops::Range;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE_CIRCUIT: &str = "rstim/tests/fixtures/rsmp/v1/compat.stim";
const FIXTURE_ARCHIVE: &str = "rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp";
const CATALOG: &str = "rstim/tests/fixtures/rsmp/catalog.json";
const FIXTURE_MANIFEST: &str = "rstim/tests/fixtures/rsmp/v1/manifest.toml";

#[test]
fn exact_recipe_mapping_and_summary_pass() {
    let summary = run_committed_corpus().expect("corruption corpus must pass");
    assert_eq!(summary.status, "pass");
    assert_eq!(summary.success_line, PASS_LINE);
    assert_eq!(summary.valid_archives, 1);
    assert!(summary.named_recipes >= 12);
    assert_eq!(summary.truncation_points, summary.fixture_byte_length);
    assert!(summary.bit_flips > 0);
    assert_eq!(summary.unexpected_successes, 0);
    assert_eq!(summary.wrong_error_codes, 0);
    assert_eq!(summary.panics, 0);
    assert_eq!(summary.timeouts, 0);
}

#[test]
fn exhaustive_truncation_mapping() {
    let summary = run_committed_corpus().expect("corruption corpus must pass");
    let truncations: Vec<_> = summary
        .results
        .iter()
        .filter(|result| result.kind == "truncation")
        .collect();
    assert_eq!(summary.truncation_points, summary.fixture_byte_length);
    assert_eq!(truncations.len(), summary.fixture_byte_length);
    for (offset, result) in truncations.iter().enumerate() {
        assert_eq!(result.id, format!("truncate_at_{offset}"));
        assert_eq!(result.expected_error, "RSMP_TRUNCATED");
        assert_eq!(result.actual_error.as_deref(), Some("RSMP_TRUNCATED"));
        assert_eq!(result.status, "matched_error_code");
    }
}

#[test]
fn format_aware_bit_flips() {
    let summary = run_committed_corpus().expect("corruption corpus must pass");
    let bit_flips: Vec<_> = summary
        .results
        .iter()
        .filter(|result| result.kind == "bit_flip")
        .collect();
    assert_eq!(bit_flips.len(), summary.bit_flips);
    assert!(summary.bit_flips > 0);
    for result in bit_flips {
        assert_eq!(result.status, "matched_error_code");
        assert_eq!(
            result.actual_error.as_deref(),
            Some(result.expected_error.as_str())
        );
    }
    for id in [
        "global_magic_bit",
        "block0_index_bit",
        "block1_index_bit",
        "sparse_syndrome_stream_bit",
        "dense_syndrome_stream_bit",
        "block0_logical_digest_bit",
        "trailer_block_count_bit",
        "archive_digest_bit",
    ] {
        assert!(summary.results.iter().any(|result| result.id == id));
    }
}

#[test]
fn corrupt_current_block_is_not_returned() {
    let summary = run_committed_corpus().expect("corruption corpus must pass");
    let first_block = corpus_result(&summary, "sparse_syndrome_stream_bit");
    assert_eq!(
        first_block.actual_error.as_deref(),
        Some("RSMP_DECOMPRESSION_FAILED")
    );
    assert_eq!(first_block.blocks_returned, 0);

    let second_block = corpus_result(&summary, "dense_syndrome_stream_bit");
    assert_eq!(
        second_block.actual_error.as_deref(),
        Some("RSMP_DECOMPRESSION_FAILED")
    );
    assert_eq!(second_block.blocks_returned, 1);
}

#[test]
fn already_returned_prefix_requires_finish() {
    let circuit = fixture_circuit();
    let mut archive = fixture_archive();
    archive.push(0);

    let mut reader =
        SampleArchiveReader::open(Cursor::new(&archive), &circuit, ArchiveLimits::default())
            .expect("open trailing-data fixture");
    assert!(reader.next_block().expect("block 0").is_some());
    assert!(reader.next_block().expect("block 1").is_some());
    assert!(reader.next_block().expect("read trailer").is_none());
    let err = reader
        .finish()
        .expect_err("finish must reject trailing data");
    assert_eq!(err.code(), SampleArchiveErrorCode::TrailingData);

    let summary = run_committed_corpus().expect("corruption corpus must pass");
    let trailing = corpus_result(&summary, "trailing_data");
    assert_eq!(trailing.actual_error.as_deref(), Some("RSMP_TRAILING_DATA"));
    assert_eq!(trailing.blocks_returned, 2);
}

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

#[test]
fn wrong_expected_code_is_rejected() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let catalog_path = temp.path().join("catalog.json");
    let summary_path = temp.path().join("summary.json");
    let catalog_text = fs::read_to_string(repo_path(CATALOG)).expect("read committed catalog");
    let mut catalog: Value = serde_json::from_str(&catalog_text).expect("parse committed catalog");
    let recipes = catalog
        .get_mut("corruption_recipes")
        .and_then(Value::as_array_mut)
        .expect("catalog recipes");
    let recipe = recipes
        .iter_mut()
        .find(|recipe| recipe.get("id").and_then(Value::as_str) == Some("bad_magic"))
        .expect("bad_magic recipe");
    recipe["expected_error"] = Value::String("RSMP_IO".to_string());
    fs::write(
        &catalog_path,
        serde_json::to_vec_pretty(&catalog).expect("serialize mutated catalog"),
    )
    .expect("write mutated catalog");

    let summary = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: catalog_path.clone(),
        fixture_manifest_path: repo_path(FIXTURE_MANIFEST),
    })
    .expect("run negative-control corpus");
    assert_eq!(summary.status, "fail");
    assert_eq!(summary.success_line, "");
    assert_eq!(summary.wrong_error_codes, 1);
    let bad_magic = corpus_result(&summary, "bad_magic");
    assert_eq!(bad_magic.status, "wrong_error_code");
    assert_eq!(bad_magic.expected_error, "RSMP_IO");
    assert_eq!(bad_magic.actual_error.as_deref(), Some("RSMP_BAD_MAGIC"));

    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(repo_root())
        .args([
            "run",
            "--locked",
            "--quiet",
            "-p",
            "rstim",
            "--example",
            "rsmp_corruption_corpus",
            "--",
            "--catalog",
        ])
        .arg(&catalog_path)
        .arg("--fixture-manifest")
        .arg(repo_path(FIXTURE_MANIFEST))
        .arg("--out")
        .arg(&summary_path)
        .output()
        .expect("run corpus example");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(PASS_LINE));
    assert!(stderr.contains("bad_magic"), "{stderr}");
    assert!(stderr.contains("expected=RSMP_IO"), "{stderr}");
    assert!(stderr.contains("actual=RSMP_BAD_MAGIC"), "{stderr}");
}

fn fixture_circuit() -> Vec<StimInstr> {
    let text = fs::read_to_string(repo_path(FIXTURE_CIRCUIT)).expect("read fixture circuit");
    parse_lines(&text).expect("parse fixture circuit")
}

fn fixture_archive() -> Vec<u8> {
    fs::read(repo_path(FIXTURE_ARCHIVE)).expect("read fixture archive")
}

fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn run_committed_corpus() -> Result<CorruptionCorpusSummary, String> {
    run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: repo_path(CATALOG),
        fixture_manifest_path: repo_path(FIXTURE_MANIFEST),
    })
}

fn corpus_result<'a>(summary: &'a CorruptionCorpusSummary, id: &str) -> &'a CorruptionCaseResult {
    summary
        .results
        .iter()
        .find(|result| result.id == id)
        .unwrap_or_else(|| panic!("missing corpus result {id}"))
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
