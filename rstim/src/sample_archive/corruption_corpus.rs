use crate::ir::StimInstr;
use crate::parser::parse_lines;
use crate::sample_archive::format::{
    BlockHeader, SampleArchiveError, SampleArchiveErrorCode, ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN,
    BLOCK_MAGIC, FORMAT_MAJOR, GLOBAL_HEADER_LEN,
};
use crate::sample_archive::limits::ArchiveLimits;
use crate::sample_archive::reader::SampleArchiveReader;
use crate::sample_archive::zstd_frame::{compress_frame, decompress_frame};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};

pub const PASS_LINE: &str = "PASS rsmp corruption corpus valid=1 invalid>=12";
const CORPUS_FIXTURE_ID: &str = "compat_v1_two_block_sparse_dense";

#[derive(Clone, Debug)]
pub struct CorruptionCorpusOptions {
    pub catalog_path: PathBuf,
    pub fixture_manifest_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorruptionCorpusSummary {
    pub status: String,
    pub success_line: String,
    pub fixture_hash: String,
    pub fixture_byte_length: usize,
    pub valid_archives: usize,
    pub named_recipes: usize,
    pub truncation_points: usize,
    pub bit_flips: usize,
    pub counts_by_error_code: BTreeMap<String, usize>,
    pub unexpected_successes: usize,
    pub wrong_error_codes: usize,
    pub panics: usize,
    pub timeouts: usize,
    pub results: Vec<CorruptionCaseResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorruptionCaseResult {
    pub id: String,
    pub kind: String,
    pub expected_error: String,
    pub actual_error: Option<String>,
    pub status: String,
    pub blocks_returned: usize,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MaterializedCorruption {
    pub id: String,
    pub expected_error: String,
    pub archive: Vec<u8>,
    pub circuit_text: Option<String>,
    pub limits: ArchiveLimits,
}

#[derive(Debug)]
struct Fixture {
    archive: Vec<u8>,
    archive_sha256: String,
    circuit: Vec<StimInstr>,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    corruption_recipes: Vec<CatalogRecipe>,
    #[serde(default)]
    bit_flips: Vec<CatalogBitFlip>,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogRecipe {
    id: String,
    fixture_id: String,
    source_role: String,
    kind: String,
    locator: String,
    mutation: String,
    #[serde(default)]
    expected_error: Option<String>,
    #[serde(default)]
    expected_code: Option<String>,
    recompute: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogBitFlip {
    id: String,
    locator: String,
    expected_error: String,
    #[serde(default)]
    bit: Option<u8>,
}

#[derive(Clone, Debug)]
struct CaseInput {
    archive: Vec<u8>,
    circuit_text: Option<&'static str>,
    limits: ArchiveLimits,
}

#[derive(Clone, Debug)]
struct ArchiveLayout {
    blocks: Vec<BlockLayout>,
    trailer: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
struct BlockLayout {
    block: BlockHeader,
    all: std::ops::Range<usize>,
    header: std::ops::Range<usize>,
    syndrome: std::ops::Range<usize>,
    free: std::ops::Range<usize>,
}

#[derive(Clone, Copy, Debug)]
enum StreamKind {
    Syndrome,
    Free,
}

#[derive(Clone, Copy, Debug)]
struct RecipeMetadata {
    fixture_id: &'static str,
    source_role: &'static str,
    kind: &'static str,
    locator: &'static str,
    mutation: &'static str,
    recompute: &'static [&'static str],
}

pub fn run_corruption_corpus(
    options: CorruptionCorpusOptions,
) -> Result<CorruptionCorpusSummary, String> {
    let repo_root = repo_root_from_manifest(&options.fixture_manifest_path)?;
    let manifest = load_manifest(&options.fixture_manifest_path)?;
    let catalog = load_catalog(&options.catalog_path)?;
    let fixture = load_fixture(&repo_root, &manifest)?;

    let mut results = Vec::new();
    let mut counts_by_error_code = BTreeMap::new();
    let mut unexpected_successes = 0usize;
    let mut wrong_error_codes = 0usize;
    let mut panics = 0usize;
    let timeouts = 0usize;

    let valid_archives = match decode_archive(
        fixture.archive.clone(),
        &fixture.circuit,
        ArchiveLimits::default(),
    ) {
        Ok(_) => 1,
        Err((err, _blocks_returned)) => {
            results.push(CorruptionCaseResult {
                id: "valid_fixture".to_string(),
                kind: "valid".to_string(),
                expected_error: "success".to_string(),
                actual_error: Some(err.code().as_str().to_string()),
                status: "wrong_error_code".to_string(),
                blocks_returned: 0,
                message: Some(err.detail().to_string()),
            });
            wrong_error_codes += 1;
            0
        }
    };

    for recipe in &catalog.corruption_recipes {
        let expected = recipe_expected_error(recipe)?;
        let input = match materialize_recipe(recipe, &fixture.archive) {
            Ok(input) => input,
            Err(message) => {
                panics += 1;
                results.push(CorruptionCaseResult {
                    id: recipe.id.clone(),
                    kind: recipe.kind.clone(),
                    expected_error: expected,
                    actual_error: None,
                    status: "panic".to_string(),
                    blocks_returned: 0,
                    message: Some(message),
                });
                continue;
            }
        };
        let result = run_one_case(
            recipe.id.clone(),
            recipe.kind.clone(),
            expected,
            input,
            &fixture.circuit,
            &mut counts_by_error_code,
        );
        tally_result(
            &result,
            &mut unexpected_successes,
            &mut wrong_error_codes,
            &mut panics,
        );
        results.push(result);
    }

    for len in 0..fixture.archive.len() {
        let input = CaseInput {
            archive: fixture.archive[..len].to_vec(),
            circuit_text: None,
            limits: ArchiveLimits::default(),
        };
        let result = run_one_case(
            format!("truncate_at_{len}"),
            "truncation".to_string(),
            SampleArchiveErrorCode::Truncated.as_str().to_string(),
            input,
            &fixture.circuit,
            &mut counts_by_error_code,
        );
        tally_result(
            &result,
            &mut unexpected_successes,
            &mut wrong_error_codes,
            &mut panics,
        );
        results.push(result);
    }

    for bit_flip in &catalog.bit_flips {
        let input = materialize_bit_flip(bit_flip, &fixture.archive)?;
        let result = run_one_case(
            bit_flip.id.clone(),
            "bit_flip".to_string(),
            bit_flip.expected_error.clone(),
            input,
            &fixture.circuit,
            &mut counts_by_error_code,
        );
        tally_result(
            &result,
            &mut unexpected_successes,
            &mut wrong_error_codes,
            &mut panics,
        );
        results.push(result);
    }

    let status = if valid_archives == 1
        && catalog.corruption_recipes.len() >= 12
        && !catalog.bit_flips.is_empty()
        && unexpected_successes == 0
        && wrong_error_codes == 0
        && panics == 0
        && timeouts == 0
    {
        "pass"
    } else {
        "fail"
    };
    Ok(CorruptionCorpusSummary {
        status: status.to_string(),
        success_line: if status == "pass" {
            PASS_LINE.to_string()
        } else {
            String::new()
        },
        fixture_hash: fixture.archive_sha256,
        fixture_byte_length: fixture.archive.len(),
        valid_archives,
        named_recipes: catalog.corruption_recipes.len(),
        truncation_points: fixture.archive.len(),
        bit_flips: catalog.bit_flips.len(),
        counts_by_error_code,
        unexpected_successes,
        wrong_error_codes,
        panics,
        timeouts,
        results,
    })
}

pub fn materialize_named_corruption(
    catalog_path: &Path,
    fixture_manifest_path: &Path,
    id: &str,
) -> Result<MaterializedCorruption, String> {
    let repo_root = repo_root_from_manifest(fixture_manifest_path)?;
    let manifest = load_manifest(fixture_manifest_path)?;
    let catalog = load_catalog(catalog_path)?;
    let fixture = load_fixture(&repo_root, &manifest)?;
    let recipe = catalog
        .corruption_recipes
        .iter()
        .find(|recipe| recipe.id == id)
        .ok_or_else(|| format!("unknown corruption recipe {id}"))?;
    let expected_error = recipe_expected_error(recipe)?;
    let input = materialize_recipe(recipe, &fixture.archive)?;
    Ok(MaterializedCorruption {
        id: recipe.id.clone(),
        expected_error,
        archive: input.archive,
        circuit_text: input.circuit_text.map(str::to_string),
        limits: input.limits,
    })
}

pub fn write_summary_json(summary: &CorruptionCorpusSummary, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(summary).map_err(|err| err.to_string())?;
    fs::write(path, bytes).map_err(|err| format!("{}: {err}", path.display()))
}

fn run_one_case(
    id: String,
    kind: String,
    expected_error: String,
    input: CaseInput,
    fixture_circuit: &[StimInstr],
    counts_by_error_code: &mut BTreeMap<String, usize>,
) -> CorruptionCaseResult {
    let circuit = match input.circuit_text {
        Some(text) => match parse_lines(text) {
            Ok(circuit) => circuit,
            Err(err) => {
                return CorruptionCaseResult {
                    id,
                    kind,
                    expected_error,
                    actual_error: None,
                    status: "panic".to_string(),
                    blocks_returned: 0,
                    message: Some(format!("control circuit parse failed: {err}")),
                };
            }
        },
        None => fixture_circuit.to_vec(),
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        decode_archive(input.archive, &circuit, input.limits)
    }));
    match outcome {
        Err(_) => CorruptionCaseResult {
            id,
            kind,
            expected_error,
            actual_error: None,
            status: "panic".to_string(),
            blocks_returned: 0,
            message: Some("reader panicked".to_string()),
        },
        Ok(Ok(blocks_returned)) => CorruptionCaseResult {
            id,
            kind,
            expected_error,
            actual_error: None,
            status: "unexpected_success".to_string(),
            blocks_returned,
            message: None,
        },
        Ok(Err((err, blocks_returned))) => {
            let actual = err.code().as_str().to_string();
            *counts_by_error_code.entry(actual.clone()).or_insert(0) += 1;
            let status = if actual == expected_error {
                "matched_error_code"
            } else {
                "wrong_error_code"
            };
            CorruptionCaseResult {
                id,
                kind,
                expected_error,
                actual_error: Some(actual),
                status: status.to_string(),
                blocks_returned,
                message: Some(err.detail().to_string()),
            }
        }
    }
}

fn decode_archive(
    archive: Vec<u8>,
    circuit: &[StimInstr],
    limits: ArchiveLimits,
) -> Result<usize, (SampleArchiveError, usize)> {
    let mut reader = match SampleArchiveReader::open(Cursor::new(archive), circuit, limits) {
        Ok(reader) => reader,
        Err(err) => return Err((err, 0)),
    };
    let mut blocks = 0usize;
    loop {
        match reader.next_block() {
            Ok(Some(_block)) => blocks += 1,
            Ok(None) => break,
            Err(err) => return Err((err, blocks)),
        }
    }
    reader.finish().map(|_| blocks).map_err(|err| (err, blocks))
}

fn tally_result(
    result: &CorruptionCaseResult,
    unexpected_successes: &mut usize,
    wrong_error_codes: &mut usize,
    panics: &mut usize,
) {
    match result.status.as_str() {
        "unexpected_success" => *unexpected_successes += 1,
        "wrong_error_code" => *wrong_error_codes += 1,
        "panic" => *panics += 1,
        _ => {}
    }
}

fn validate_recipe_metadata(recipe: &CatalogRecipe) -> Result<(), String> {
    let metadata = recipe_metadata(&recipe.id)
        .ok_or_else(|| format!("unsupported corruption recipe {}", recipe.id))?;
    validate_recipe_field(
        &recipe.id,
        "fixture_id",
        &recipe.fixture_id,
        metadata.fixture_id,
    )?;
    validate_recipe_field(
        &recipe.id,
        "source_role",
        &recipe.source_role,
        metadata.source_role,
    )?;
    validate_recipe_field(&recipe.id, "kind", &recipe.kind, metadata.kind)?;
    validate_recipe_field(&recipe.id, "locator", &recipe.locator, metadata.locator)?;
    validate_recipe_field(&recipe.id, "mutation", &recipe.mutation, metadata.mutation)?;
    if recipe.recompute.len() != metadata.recompute.len()
        || !recipe
            .recompute
            .iter()
            .map(String::as_str)
            .eq(metadata.recompute.iter().copied())
    {
        return Err(format!(
            "{}.recompute must be {:?}",
            recipe.id, metadata.recompute
        ));
    }
    Ok(())
}

fn validate_recipe_field(
    recipe_id: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{recipe_id}.{field} must be {expected}, got {actual}"
        ))
    }
}

fn recipe_metadata(id: &str) -> Option<RecipeMetadata> {
    match id {
        "bad_magic" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.magic",
            "set(global.magic, 0x52534d00)",
            &[],
        )),
        "unsupported_version" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.format_major",
            "set(global.format_major, 2)",
            &[],
        )),
        "unknown_required_feature" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.required_flags",
            "set(global.required_flags, unknown_required_feature)",
            &[],
        )),
        "nonzero_reserved_field" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.reserved0",
            "set(global.reserved0, 1)",
            &[],
        )),
        "header_digest_mismatch" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.header_sha256",
            "set(global.header_sha256, alternate_digest)",
            &[],
        )),
        "circuit_mismatch" => Some(metadata(
            "nonzero_reference",
            "different_circuit_control",
            "external.circuit",
            "control(different_circuit)",
            &[],
        )),
        "unsupported_sweep_control" => Some(metadata(
            "nonzero_reference",
            "unsupported_sweep_control",
            "external.circuit",
            "control(unsupported_sweep)",
            &[],
        )),
        "shape_mismatch" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global.measurement_count",
            "set(global.measurement_count, 11)",
            &["global.header_sha256", "trailer.archive_sha256"],
        )),
        "truncated_header" => Some(metadata(
            "nonzero_reference",
            "byte_mutation",
            "global_header",
            "truncate(global_header)",
            &[],
        )),
        "truncated_block" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].header",
            "truncate(block)",
            &[],
        )),
        "truncated_zstd_frame" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].syndrome_stream",
            "truncate(block.zstd_frame)",
            &[],
        )),
        "zstd_decode_failure" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].free_stream",
            "set(block.zstd_frame.payload, invalid_zstandard_frame)",
            &["trailer.archive_sha256"],
        )),
        "truncated_trailer" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "trailer",
            "truncate(trailer)",
            &[],
        )),
        "overlong_varint" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].sparse_syndrome_payload.hit_count_uleb128",
            "set(block.sparse_syndrome_payload.hit_count_uleb128, overlong_encoding(1))",
            &[
                "block.syndrome_uncompressed_len",
                "block.syndrome_compressed_len",
                "block.syndrome_zstd_frame.checksum",
                "trailer.archive_sha256",
            ],
        )),
        "sparse_index_out_of_range" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].sparse_syndrome_payload.detector_index_delta",
            "set(block.sparse_syndrome_payload.detector_index_delta, detector_count)",
            &[
                "block.syndrome_uncompressed_len",
                "block.syndrome_compressed_len",
                "block.syndrome_zstd_frame.checksum",
                "trailer.archive_sha256",
            ],
        )),
        "duplicate_block" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0]",
            "duplicate(block)",
            &["trailer.block_count", "trailer.archive_sha256"],
        )),
        "omitted_block" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0]",
            "omit(block)",
            &["trailer.block_count", "trailer.archive_sha256"],
        )),
        "reordered_blocks" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "blocks[0..2]",
            "reorder(blocks)",
            &["trailer.archive_sha256"],
        )),
        "skipped_block" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].block_index",
            "set(block.block_index, 1)",
            &["trailer.archive_sha256"],
        )),
        "changed_compressed_payload" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].free_stream",
            "flip(block.zstd_frame.payload.bit)",
            &["trailer.archive_sha256"],
        )),
        "checksum_mismatch" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "trailer.archive_sha256",
            "set(trailer.archive_sha256, alternate_digest)",
            &[],
        )),
        "logical_payload_mismatch" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].free_stream",
            "flip(block.canonical_logical_payload.free_bits.bit)",
            &[
                "block.free_compressed_len",
                "block.free_zstd_frame.checksum",
                "trailer.archive_sha256",
            ],
        )),
        "declared_length_mismatch" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].syndrome_uncompressed_len",
            "set(block.syndrome_uncompressed_len, 0)",
            &["trailer.archive_sha256"],
        )),
        "resource_limit_exceeded" => Some(metadata(
            "surface_d11_r100",
            "custom_limit_control",
            "limits.max_archive_bytes",
            "limit(max_archive_bytes, global_header_plus_trailer)",
            &[],
        )),
        "nonzero_padding" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[1].syndrome_padding_bits",
            "set(block.syndrome_padding_bits, 1)",
            &[
                "block.syndrome_compressed_len",
                "block.syndrome_zstd_frame.checksum",
                "trailer.archive_sha256",
            ],
        )),
        "unknown_syndrome_codec" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "block[0].syndrome_codec_id",
            "set(block.syndrome_codec_id, 99)",
            &["trailer.archive_sha256"],
        )),
        "trailing_data" => Some(metadata(
            "surface_d11_r100",
            "byte_mutation",
            "archive.eof",
            "append_trailing_byte(0)",
            &[],
        )),
        _ => None,
    }
}

fn metadata(
    source_role: &'static str,
    kind: &'static str,
    locator: &'static str,
    mutation: &'static str,
    recompute: &'static [&'static str],
) -> RecipeMetadata {
    RecipeMetadata {
        fixture_id: CORPUS_FIXTURE_ID,
        source_role,
        kind,
        locator,
        mutation,
        recompute,
    }
}

fn materialize_recipe(recipe: &CatalogRecipe, archive: &[u8]) -> Result<CaseInput, String> {
    validate_recipe_metadata(recipe)?;
    let mut bytes = archive.to_vec();
    let limits = ArchiveLimits::default();
    match recipe.id.as_str() {
        "bad_magic" => bytes[0] ^= 0xff,
        "unsupported_version" => put_u16(&mut bytes, 8, FORMAT_MAJOR + 1),
        "unknown_required_feature" => put_u32(&mut bytes, 16, 1),
        "nonzero_reserved_field" => put_u32(&mut bytes, 24, 1),
        "header_digest_mismatch" => bytes[GLOBAL_HEADER_LEN - 1] ^= 0x55,
        "circuit_mismatch" => {
            return Ok(CaseInput {
                archive: bytes,
                circuit_text: Some("M 0\n"),
                limits,
            });
        }
        "unsupported_sweep_control" => {
            return Ok(CaseInput {
                archive: bytes,
                circuit_text: Some("M sweep[0]\n"),
                limits,
            });
        }
        "shape_mismatch" => {
            put_u64(&mut bytes, 48, 11);
            recompute_header_digest(&mut bytes);
            recompute_trailer_digest(&mut bytes);
        }
        "truncated_header" => bytes.truncate(GLOBAL_HEADER_LEN - 1),
        "truncated_block" => bytes.truncate(GLOBAL_HEADER_LEN + 16),
        "truncated_zstd_frame" => {
            let layout = derive_layout(&bytes)?;
            bytes.truncate(layout.blocks[0].syndrome.start + 3);
        }
        "truncated_trailer" => {
            let trailer_start = bytes.len() - ARCHIVE_TRAILER_LEN;
            bytes.truncate(trailer_start + 12);
        }
        "zstd_decode_failure" | "changed_compressed_payload" => {
            let layout = derive_layout(&bytes)?;
            bytes[layout.blocks[0].free.end - 1] ^= 0x80;
            recompute_trailer_digest(&mut bytes);
        }
        "overlong_varint" => {
            replace_stream(&mut bytes, 0, StreamKind::Syndrome, vec![0x80, 0x00, 0x00])?
        }
        "sparse_index_out_of_range" => {
            replace_stream(&mut bytes, 0, StreamKind::Syndrome, vec![0x01, 0x09, 0x00])?
        }
        "duplicate_block" => {
            let layout = derive_layout(&bytes)?;
            let block = bytes[layout.blocks[0].all.clone()].to_vec();
            let trailer_start = layout.trailer.start;
            bytes.splice(trailer_start..trailer_start, block);
        }
        "omitted_block" => {
            let layout = derive_layout(&bytes)?;
            bytes.drain(layout.blocks[0].all.clone());
        }
        "reordered_blocks" => {
            let layout = derive_layout(&bytes)?;
            let block0 = bytes[layout.blocks[0].all.clone()].to_vec();
            let block1 = bytes[layout.blocks[1].all.clone()].to_vec();
            bytes.splice(
                layout.blocks[0].all.start..layout.blocks[1].all.end,
                [block1, block0].concat(),
            );
        }
        "skipped_block" => {
            put_u64(&mut bytes, GLOBAL_HEADER_LEN + 12, 1);
            recompute_trailer_digest(&mut bytes);
        }
        "checksum_mismatch" => {
            let trailer_start = bytes.len() - ARCHIVE_TRAILER_LEN;
            bytes[trailer_start + 63] ^= 0x5a;
        }
        "logical_payload_mismatch" => {
            let layout = derive_layout(&bytes)?;
            let mut free = decompress_frame(
                &bytes[layout.blocks[0].free.clone()],
                layout.blocks[0].block.free_uncompressed_len,
                ArchiveLimits::default(),
            )
            .map_err(|err| err.to_string())?;
            free[0] ^= 0x01;
            replace_stream(&mut bytes, 0, StreamKind::Free, free)?;
        }
        "declared_length_mismatch" => {
            let layout = derive_layout(&bytes)?;
            put_u64(&mut bytes, layout.blocks[0].header.start + 44, 0);
            recompute_trailer_digest(&mut bytes);
        }
        "resource_limit_exceeded" => {
            let mut limits = limits;
            limits.max_archive_bytes = (GLOBAL_HEADER_LEN + ARCHIVE_TRAILER_LEN) as u64;
            return Ok(CaseInput {
                archive: bytes,
                circuit_text: None,
                limits,
            });
        }
        "nonzero_padding" => {
            let layout = derive_layout(&bytes)?;
            let mut syndrome = decompress_frame(
                &bytes[layout.blocks[1].syndrome.clone()],
                layout.blocks[1].block.syndrome_uncompressed_len,
                ArchiveLimits::default(),
            )
            .map_err(|err| err.to_string())?;
            let last = syndrome
                .last_mut()
                .ok_or_else(|| "dense syndrome stream unexpectedly empty".to_string())?;
            *last |= 0x80;
            replace_stream(&mut bytes, 1, StreamKind::Syndrome, syndrome)?;
        }
        "unknown_syndrome_codec" => {
            put_u16(&mut bytes, GLOBAL_HEADER_LEN + 36, 99);
            recompute_trailer_digest(&mut bytes);
        }
        "trailing_data" => bytes.push(0),
        _ => return Err(format!("unsupported corruption recipe {}", recipe.id)),
    }
    Ok(CaseInput {
        archive: bytes,
        circuit_text: None,
        limits,
    })
}

fn materialize_bit_flip(bit_flip: &CatalogBitFlip, archive: &[u8]) -> Result<CaseInput, String> {
    let mut bytes = archive.to_vec();
    let bit = bit_flip.bit.unwrap_or(0);
    if bit > 7 {
        return Err(format!("{}.bit must be between 0 and 7", bit_flip.id));
    }
    let offset = locator_offset(&bit_flip.locator, archive)?;
    bytes[offset] ^= 1u8 << bit;
    if matches!(
        bit_flip.locator.as_str(),
        "global.circuit_sha256"
            | "global.measurement_count"
            | "global.detector_count"
            | "global.observable_count"
            | "global.detector_rank"
            | "global.total_shots"
    ) {
        recompute_header_digest(&mut bytes);
        recompute_trailer_digest(&mut bytes);
    } else if bit_flip.locator.starts_with("block[") || bit_flip.locator.ends_with("_stream") {
        recompute_trailer_digest(&mut bytes);
    }
    Ok(CaseInput {
        archive: bytes,
        circuit_text: None,
        limits: ArchiveLimits::default(),
    })
}

fn locator_offset(locator: &str, archive: &[u8]) -> Result<usize, String> {
    let layout = derive_layout(archive)?;
    match locator {
        "global.magic" => Ok(0),
        "global.required_flags" => Ok(16),
        "global.header_sha256" => Ok(120),
        "block[0].block_index" => Ok(layout.blocks[0].header.start + 12),
        "block[1].block_index" => Ok(layout.blocks[1].header.start + 12),
        "block[0].logical_payload_sha256" => Ok(layout.blocks[0].header.start + 76),
        "block[1].logical_payload_sha256" => Ok(layout.blocks[1].header.start + 76),
        "block[0].syndrome_stream" => Ok(layout.blocks[0].syndrome.end - 1),
        "block[1].syndrome_stream" => Ok(layout.blocks[1].syndrome.end - 1),
        "block[0].free_stream" => Ok(layout.blocks[0].free.end - 1),
        "block[1].free_stream" => Ok(layout.blocks[1].free.end - 1),
        "trailer.block_count" => Ok(layout.trailer.start + 16),
        "trailer.archive_sha256" => Ok(layout.trailer.start + 32),
        _ => Err(format!("unknown bit-flip locator {locator}")),
    }
}

fn replace_stream(
    archive: &mut Vec<u8>,
    block_index: usize,
    stream: StreamKind,
    raw: Vec<u8>,
) -> Result<(), String> {
    let layout = derive_layout(archive)?;
    let block_layout = layout
        .blocks
        .get(block_index)
        .ok_or_else(|| format!("missing block {block_index}"))?;
    let mut block = block_layout.block.clone();
    let frame = if raw.is_empty() {
        Vec::new()
    } else {
        compress_frame(&raw, 3).map_err(|err| err.to_string())?
    };
    match stream {
        StreamKind::Syndrome => {
            block.syndrome_uncompressed_len = raw.len() as u64;
            block.syndrome_compressed_len = frame.len() as u64;
        }
        StreamKind::Free => {
            block.free_uncompressed_len = raw.len() as u64;
            block.free_compressed_len = frame.len() as u64;
        }
    }
    let header = block.to_bytes().map_err(|err| err.to_string())?;
    archive[block_layout.header.clone()].copy_from_slice(&header);
    let stream_range = match stream {
        StreamKind::Syndrome => block_layout.syndrome.clone(),
        StreamKind::Free => block_layout.free.clone(),
    };
    archive.splice(stream_range, frame);
    recompute_trailer_digest(archive);
    Ok(())
}

fn derive_layout(archive: &[u8]) -> Result<ArchiveLayout, String> {
    if archive.len() < GLOBAL_HEADER_LEN + ARCHIVE_TRAILER_LEN {
        return Err("archive too short for layout".to_string());
    }
    let trailer_start = archive.len() - ARCHIVE_TRAILER_LEN;
    let mut offset = GLOBAL_HEADER_LEN;
    let mut blocks = Vec::new();
    while offset < trailer_start {
        let header_end = offset
            .checked_add(BLOCK_HEADER_LEN)
            .ok_or_else(|| "block header offset overflow".to_string())?;
        if header_end > trailer_start || archive[offset..offset + 8] != BLOCK_MAGIC[..] {
            return Err(format!("invalid block layout at offset {offset}"));
        }
        let block =
            BlockHeader::from_bytes(&archive[offset..header_end]).map_err(|err| err.to_string())?;
        let syndrome_start = header_end;
        let syndrome_end = syndrome_start
            .checked_add(block.syndrome_compressed_len as usize)
            .ok_or_else(|| "syndrome end overflow".to_string())?;
        let free_end = syndrome_end
            .checked_add(block.free_compressed_len as usize)
            .ok_or_else(|| "free end overflow".to_string())?;
        if free_end > trailer_start {
            return Err("block streams exceed trailer".to_string());
        }
        blocks.push(BlockLayout {
            block,
            all: offset..free_end,
            header: offset..header_end,
            syndrome: syndrome_start..syndrome_end,
            free: syndrome_end..free_end,
        });
        offset = free_end;
    }
    Ok(ArchiveLayout {
        blocks,
        trailer: trailer_start..archive.len(),
    })
}

fn load_catalog(path: &Path) -> Result<Catalog, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse catalog: {err}"))
}

fn load_manifest(path: &Path) -> Result<toml::Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse manifest: {err}"))
}

fn load_fixture(repo_root: &Path, manifest: &toml::Value) -> Result<Fixture, String> {
    let archive_path = repo_path(
        repo_root,
        toml_str(manifest, &["paths", "archive"])?,
        "paths.archive",
    )?;
    let circuit_path = repo_path(
        repo_root,
        toml_str(manifest, &["paths", "circuit"])?,
        "paths.circuit",
    )?;
    let archive =
        fs::read(&archive_path).map_err(|err| format!("{}: {err}", archive_path.display()))?;
    let archive_sha256 = hex(&Sha256::digest(&archive));
    let expected_archive_sha256 = toml_str(manifest, &["hashes", "archive_sha256"])?;
    if archive_sha256 != expected_archive_sha256 {
        return Err(format!(
            "archive_sha256 mismatch: got {archive_sha256}, expected {expected_archive_sha256}"
        ));
    }
    let circuit_text = fs::read_to_string(&circuit_path)
        .map_err(|err| format!("{}: {err}", circuit_path.display()))?;
    let circuit = parse_lines(&circuit_text).map_err(|err| format!("parse circuit: {err}"))?;
    Ok(Fixture {
        archive,
        archive_sha256,
        circuit,
    })
}

fn recipe_expected_error(recipe: &CatalogRecipe) -> Result<String, String> {
    let expected = recipe
        .expected_error
        .as_deref()
        .or(recipe.expected_code.as_deref())
        .ok_or_else(|| format!("{}.expected_error missing", recipe.id))?;
    if !is_public_code(expected) {
        return Err(format!(
            "{}.expected_error is not a public rsmp code",
            recipe.id
        ));
    }
    Ok(expected.to_string())
}

fn is_public_code(code: &str) -> bool {
    matches!(
        code,
        "RSMP_BAD_MAGIC"
            | "RSMP_UNSUPPORTED_VERSION"
            | "RSMP_UNSUPPORTED_FEATURE"
            | "RSMP_UNSUPPORTED_SWEEP"
            | "RSMP_CIRCUIT_MISMATCH"
            | "RSMP_SHAPE_MISMATCH"
            | "RSMP_LIMIT_EXCEEDED"
            | "RSMP_TRUNCATED"
            | "RSMP_MALFORMED_ARCHIVE"
            | "RSMP_DECOMPRESSION_FAILED"
            | "RSMP_CHECKSUM_MISMATCH"
            | "RSMP_LOGICAL_DIGEST_MISMATCH"
            | "RSMP_TRAILING_DATA"
            | "RSMP_IO"
    )
}

fn repo_root_from_manifest(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path)
    };
    let mut root = absolute.as_path();
    for _ in 0..6 {
        root = root
            .parent()
            .ok_or_else(|| format!("cannot infer repo root from {}", absolute.display()))?;
    }
    Ok(root.to_path_buf())
}

fn repo_path(repo_root: &Path, relative: &str, label: &str) -> Result<PathBuf, String> {
    if relative.is_empty() || relative.contains('\\') {
        return Err(format!(
            "{label} must be a non-empty POSIX repo-relative path"
        ));
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{label} must be repo-relative without '..'"));
    }
    Ok(repo_root.join(path))
}

fn toml_str<'a>(value: &'a toml::Value, path: &[&str]) -> Result<&'a str, String> {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .ok_or_else(|| format!("missing TOML field {}", path.join(".")))?;
    }
    current
        .as_str()
        .ok_or_else(|| format!("{} must be a string", path.join(".")))
}

fn recompute_header_digest(bytes: &mut [u8]) {
    let digest: [u8; 32] = Sha256::digest(&bytes[..GLOBAL_HEADER_LEN - 32]).into();
    bytes[GLOBAL_HEADER_LEN - 32..GLOBAL_HEADER_LEN].copy_from_slice(&digest);
}

fn recompute_trailer_digest(bytes: &mut [u8]) {
    let trailer_start = bytes.len() - ARCHIVE_TRAILER_LEN;
    let digest: [u8; 32] = Sha256::digest(&bytes[..trailer_start + 32]).into();
    bytes[trailer_start + 32..trailer_start + 64].copy_from_slice(&digest);
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
