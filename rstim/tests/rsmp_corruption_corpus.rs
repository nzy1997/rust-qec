use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sample_archive::corruption_corpus::{
    run_corruption_corpus, write_summary_json, CorruptionCaseResult, CorruptionCorpusOptions,
    CorruptionCorpusSummary, PASS_LINE,
};
use rstim::sample_archive::format::{
    BlockHeader, SampleArchiveErrorCode, ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, BLOCK_MAGIC,
    GLOBAL_HEADER_LEN,
};
use rstim::sample_archive::{ArchiveLimits, SampleArchiveReader};
use serde_json::{json, Value};
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

#[test]
fn summary_json_writer_creates_parent_directory() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let out = temp.path().join("nested").join("summary.json");
    let summary = run_committed_corpus().expect("corruption corpus must pass");

    write_summary_json(&summary, &out).expect("write summary json");

    let written: Value =
        serde_json::from_slice(&fs::read(&out).expect("read summary")).expect("parse summary");
    assert_eq!(written["status"], "pass");
    assert_eq!(written["success_line"], PASS_LINE);
}

#[test]
fn catalog_recipe_metadata_errors_are_reported() {
    let temp = tempfile::tempdir().expect("create temp dir");

    let unknown = json!({
        "id": "unknown_recipe",
        "fixture_id": "compat_v1_two_block_sparse_dense",
        "source_role": "nonzero_reference",
        "kind": "byte_mutation",
        "locator": "global.magic",
        "mutation": "set(global.magic, 0)",
        "expected_error": "RSMP_BAD_MAGIC",
        "recompute": []
    });
    let unknown_summary = run_catalog(
        &temp,
        json!({
            "corruption_recipes": [unknown],
            "bit_flips": []
        }),
    )
    .expect("unknown recipe is recorded as a corpus case failure");
    let unknown_result = corpus_result(&unknown_summary, "unknown_recipe");
    assert_eq!(unknown_result.status, "panic");
    assert_eq!(
        unknown_result.message.as_deref(),
        Some("unsupported corruption recipe unknown_recipe")
    );

    let mut wrong_field = committed_recipe("bad_magic");
    wrong_field["source_role"] = Value::String("wrong_role".to_string());
    let wrong_field_summary = run_catalog(&temp, catalog_for_recipes(vec![wrong_field]))
        .expect("metadata mismatch is recorded as a corpus case failure");
    let wrong_field_result = corpus_result(&wrong_field_summary, "bad_magic");
    assert_eq!(wrong_field_result.status, "panic");
    assert_eq!(
        wrong_field_result.message.as_deref(),
        Some("bad_magic.source_role must be nonzero_reference, got wrong_role")
    );

    let mut wrong_recompute = committed_recipe("bad_magic");
    wrong_recompute["recompute"] = json!(["trailer.archive_sha256"]);
    let wrong_recompute_summary = run_catalog(&temp, catalog_for_recipes(vec![wrong_recompute]))
        .expect("recompute mismatch is recorded as a corpus case failure");
    let wrong_recompute_result = corpus_result(&wrong_recompute_summary, "bad_magic");
    assert_eq!(wrong_recompute_result.status, "panic");
    assert_eq!(
        wrong_recompute_result.message.as_deref(),
        Some("bad_magic.recompute must be []")
    );
}

#[test]
fn invalid_catalog_expected_error_is_rejected() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let mut recipe = committed_recipe("bad_magic");
    recipe["expected_error"] = Value::String("NOT_A_PUBLIC_CODE".to_string());

    let err = run_catalog(&temp, catalog_for_recipes(vec![recipe]))
        .expect_err("invalid public error code must reject catalog");

    assert_eq!(err, "bad_magic.expected_error is not a public rsmp code");
}

#[test]
fn bit_flip_metadata_errors_are_rejected() {
    let temp = tempfile::tempdir().expect("create temp dir");

    let err = run_catalog(
        &temp,
        json!({
            "corruption_recipes": [],
            "bit_flips": [{
                "id": "invalid_bit",
                "locator": "global.magic",
                "expected_error": "RSMP_BAD_MAGIC",
                "bit": 8
            }]
        }),
    )
    .expect_err("bit index outside a byte must reject catalog");
    assert_eq!(err, "invalid_bit.bit must be between 0 and 7");

    let err = run_catalog(
        &temp,
        json!({
            "corruption_recipes": [],
            "bit_flips": [{
                "id": "unknown_locator",
                "locator": "global.unknown",
                "expected_error": "RSMP_BAD_MAGIC"
            }]
        }),
    )
    .expect_err("unknown bit-flip locator must reject catalog");
    assert_eq!(err, "unknown bit-flip locator global.unknown");
}

#[test]
fn manifest_path_and_hash_errors_are_rejected() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let manifest = write_temp_manifest(
        &temp,
        r#"
[paths]
archive = ""
circuit = "rstim/tests/fixtures/rsmp/v1/compat.stim"

[hashes]
archive_sha256 = "unused"
"#,
    );
    let err = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: write_catalog_value(
            &temp,
            json!({
                "corruption_recipes": [],
                "bit_flips": []
            }),
        ),
        fixture_manifest_path: manifest,
    })
    .expect_err("empty archive path must be rejected");
    assert_eq!(
        err,
        "paths.archive must be a non-empty POSIX repo-relative path"
    );

    let manifest = write_temp_manifest(
        &temp,
        r#"
[paths]
archive = "../compat-v1.rsmp"
circuit = "rstim/tests/fixtures/rsmp/v1/compat.stim"

[hashes]
archive_sha256 = "unused"
"#,
    );
    let err = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: write_catalog_value(
            &temp,
            json!({
                "corruption_recipes": [],
                "bit_flips": []
            }),
        ),
        fixture_manifest_path: manifest,
    })
    .expect_err("parent path must be rejected");
    assert_eq!(err, "paths.archive must be repo-relative without '..'");

    let manifest = temp_manifest_with_fixture(&temp, "0".repeat(64), &fixture_archive());
    let err = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: write_catalog_value(
            &temp,
            json!({
                "corruption_recipes": [],
                "bit_flips": []
            }),
        ),
        fixture_manifest_path: manifest,
    })
    .expect_err("hash mismatch must be rejected");
    assert!(err.starts_with("archive_sha256 mismatch: got "), "{err}");
    assert!(err
        .ends_with(", expected 0000000000000000000000000000000000000000000000000000000000000000"));
}

#[test]
fn invalid_base_archive_can_make_recipe_unexpectedly_succeed() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let mut archive = fixture_archive();
    archive[0] ^= 0xff;
    let manifest = temp_manifest_with_fixture(&temp, sha256_hex(&archive), &archive);
    let summary = run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: write_catalog_value(
            &temp,
            catalog_for_recipes(vec![committed_recipe("bad_magic")]),
        ),
        fixture_manifest_path: manifest,
    })
    .expect("run corpus over intentionally invalid base fixture");

    assert_eq!(summary.status, "fail");
    assert_eq!(summary.valid_archives, 0);
    assert!(summary.wrong_error_codes >= 1);
    let valid = corpus_result(&summary, "valid_fixture");
    assert_eq!(valid.status, "wrong_error_code");
    assert_eq!(valid.actual_error.as_deref(), Some("RSMP_BAD_MAGIC"));
    let bad_magic = corpus_result(&summary, "bad_magic");
    assert_eq!(bad_magic.status, "unexpected_success");
    assert_eq!(bad_magic.blocks_returned, 2);
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

fn run_catalog(
    temp: &tempfile::TempDir,
    catalog: Value,
) -> Result<CorruptionCorpusSummary, String> {
    run_corruption_corpus(CorruptionCorpusOptions {
        catalog_path: write_catalog_value(temp, catalog),
        fixture_manifest_path: repo_path(FIXTURE_MANIFEST),
    })
}

fn write_catalog_value(temp: &tempfile::TempDir, catalog: Value) -> PathBuf {
    let path = temp.path().join("catalog.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&catalog).expect("serialize catalog"),
    )
    .expect("write catalog");
    path
}

fn catalog_for_recipes(recipes: Vec<Value>) -> Value {
    json!({
        "corruption_recipes": recipes,
        "bit_flips": []
    })
}

fn committed_recipe(id: &str) -> Value {
    let catalog_text = fs::read_to_string(repo_path(CATALOG)).expect("read committed catalog");
    let catalog: Value = serde_json::from_str(&catalog_text).expect("parse committed catalog");
    catalog
        .get("corruption_recipes")
        .and_then(Value::as_array)
        .expect("catalog recipes")
        .iter()
        .find(|recipe| recipe.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("missing committed recipe {id}"))
        .clone()
}

fn write_temp_manifest(temp: &tempfile::TempDir, text: &str) -> PathBuf {
    let dir = temp.path().join("rstim/tests/fixtures/rsmp/v1");
    fs::create_dir_all(&dir).expect("create manifest fixture dir");
    let path = dir.join("manifest.toml");
    fs::write(&path, text).expect("write manifest");
    path
}

fn temp_manifest_with_fixture(
    temp: &tempfile::TempDir,
    archive_sha256: String,
    archive: &[u8],
) -> PathBuf {
    let dir = temp.path().join("rstim/tests/fixtures/rsmp/v1");
    fs::create_dir_all(&dir).expect("create temp fixture dir");
    fs::write(dir.join("compat-v1.rsmp"), archive).expect("write temp archive");
    fs::write(
        dir.join("compat.stim"),
        fs::read_to_string(repo_path(FIXTURE_CIRCUIT)).expect("read fixture circuit"),
    )
    .expect("write temp circuit");
    let manifest = format!(
        r#"
[paths]
archive = "rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp"
circuit = "rstim/tests/fixtures/rsmp/v1/compat.stim"

[hashes]
archive_sha256 = "{archive_sha256}"
"#
    );
    write_temp_manifest(temp, &manifest)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
