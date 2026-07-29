use rstim::ir::{circuit_to_string, StimInstr};
use rstim::measurement_transform::{DecodedSampleBlock, MeasurementTransform};
use rstim::output::{read_shots_01, read_shots_b8};
use rstim::parser::parse_lines;
use rstim::sample_archive::format::{
    ArchiveTrailer, BlockHeader, GlobalHeader, ARCHIVE_TRAILER_LEN, BLOCK_HEADER_LEN, BLOCK_MAGIC,
    CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1, CODEC_SUITE_ZSTD_FRAMES_V1,
    FINGERPRINT_SHA256_CANONICAL_CIRCUIT, FORMAT_MAJOR, FORMAT_MINOR, GLOBAL_HEADER_LEN,
    REFERENCE_SIMULATE_NOISELESS, STREAM_CODEC_FREE_DENSE_V1, STREAM_CODEC_SYNDROME_DENSE_V1,
    STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1, TRAILER_MAGIC,
    TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
};
use rstim::sample_archive::{ArchiveLimits, SampleArchiveReader};
use rstim::sim::bit_table::BitTable;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use tempfile::TempDir;
use toml::Value as TomlValue;

const FIXTURE_ID: &str = "compat_v1_two_block_sparse_dense";
const SUCCESS_LINE: &str = "PASS rsmp v1 compatibility fixtures=1 blocks=2 codecs=sparse,dense";
const CATALOG_PATH: &str = "rstim/tests/fixtures/rsmp/catalog.json";
const MANIFEST_PATH: &str = "rstim/tests/fixtures/rsmp/v1/manifest.toml";
const ARCHIVE_PATH: &str = "rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp";
const ZERO_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const EXPECTED_ZSTD_LEVEL: i64 = 3;

type CheckResult<T> = Result<T, String>;

#[test]
fn rsmp_v1_compatibility_fixture_decodes() {
    let report = verify_fixture(&repo_root()).expect("compatibility fixture");
    assert_eq!(report.success_line, SUCCESS_LINE);
    println!("{}", report.success_line);
}

#[test]
fn changed_archive_payload_byte_is_rejected() {
    let temp = CopiedFixtureTree::new();
    flip_first_compressed_stream_byte(&temp.archive_path);
    let err = verify_fixture(temp.root.path()).expect_err("changed archive payload must fail");
    assert!(
        err.contains("archive_sha256"),
        "unexpected rejection for changed archive payload: {err}"
    );
}

#[test]
fn historical_generation_lock_is_not_current_reader_lock() {
    let temp = CopiedFixtureTree::new();
    let cargo_lock = temp.root.path().join("Cargo.lock");
    fs::write(
        &cargo_lock,
        fs::read_to_string(&cargo_lock).expect("read copied Cargo.lock")
            + "\n# simulated release metadata-only lock change\n",
    )
    .expect("write copied Cargo.lock");

    let report = verify_fixture(temp.root.path()).expect("compatibility fixture");

    assert_eq!(report.success_line, SUCCESS_LINE);
}

#[test]
fn changed_expected_measurement_hash_is_rejected() {
    let temp = CopiedFixtureTree::new();
    rewrite_manifest_measurement_hash(&temp.manifest_path, ZERO_SHA256);
    refresh_catalog_manifest_hash(&temp.catalog_path, &temp.manifest_path);
    let err = verify_fixture(temp.root.path()).expect_err("changed measurement hash must fail");
    assert!(
        err.contains("measurements_01_sha256"),
        "unexpected rejection for changed measurement hash: {err}"
    );
}

#[test]
fn changed_format_codec_id_is_rejected() {
    let temp = CopiedFixtureTree::new();
    rewrite_manifest_literal(
        &temp.manifest_path,
        "syndrome_codec_sparse_id = 2",
        "syndrome_codec_sparse_id = 99",
    );
    refresh_catalog_manifest_hash(&temp.catalog_path, &temp.manifest_path);
    let err = verify_fixture(temp.root.path()).expect_err("changed format codec id must fail");
    assert!(
        err.contains("format.syndrome_codec_sparse_id"),
        "unexpected rejection for changed format codec id: {err}"
    );
}

fn verify_fixture(repo_root: &Path) -> CheckResult<CompatReport> {
    let catalog = load_catalog(repo_root)?;
    let catalog_entry = catalog_entry(&catalog)?;
    verify_catalog_consumers(catalog_entry)?;
    let compat = json_object(
        catalog_entry.get("rsmp_v1_compatibility"),
        "catalog.rsmp_v1_compatibility",
    )?;
    let manifest_rel = json_string(compat.get("manifest_path"), "catalog.manifest_path")?;
    require_eq(manifest_rel, MANIFEST_PATH, "catalog.manifest_path")?;
    let archive_rel = json_string(compat.get("archive_path"), "catalog.archive_path")?;
    require_eq(archive_rel, ARCHIVE_PATH, "catalog.archive_path")?;
    require_eq(
        json_u64(compat.get("block_shots"), "catalog.block_shots")?,
        2,
        "catalog.block_shots",
    )?;
    require_eq(
        json_u64(compat.get("blocks"), "catalog.blocks")?,
        2,
        "catalog.blocks",
    )?;
    require_json_string_list(
        compat.get("syndrome_codecs"),
        &["sparse", "dense"],
        "catalog.syndrome_codecs",
    )?;

    let manifest_path = checked_repo_path(repo_root, manifest_rel, "catalog.manifest_path")?;
    let archive_path = checked_repo_path(repo_root, archive_rel, "catalog.archive_path")?;
    let manifest_sha256 = sha256_file(&manifest_path)?;
    let archive_file_sha256 = sha256_file(&archive_path)?;
    require_eq(
        json_string(compat.get("manifest_sha256"), "catalog.manifest_sha256")?,
        manifest_sha256.as_str(),
        "catalog.manifest_sha256",
    )?;
    require_eq(
        json_string(
            catalog_entry
                .get("hashes")
                .and_then(|hashes| hashes.get("manifest_sha256")),
            "catalog.hashes.manifest_sha256",
        )?,
        manifest_sha256.as_str(),
        "catalog.hashes.manifest_sha256",
    )?;
    require_eq(
        json_string(compat.get("archive_sha256"), "catalog.archive_sha256")?,
        archive_file_sha256.as_str(),
        "catalog.archive_sha256",
    )?;

    let manifest = load_manifest(&manifest_path)?;
    verify_manifest_identity(&manifest)?;
    verify_manifest_consumers(&manifest)?;
    verify_manifest_generation(&manifest)?;

    let shape = FixtureShape::from_manifest(&manifest)?;
    let paths = FixturePaths::from_manifest(repo_root, &manifest)?;
    verify_fixture_hashes(
        repo_root,
        &manifest,
        &paths,
        &manifest_sha256,
        &archive_file_sha256,
    )?;
    verify_catalog_shape_and_hashes(
        catalog_entry,
        &shape,
        &archive_file_sha256,
        &manifest_sha256,
    )?;

    let circuit_text = fs::read_to_string(&paths.circuit)
        .map_err(|error| format!("{}: {error}", paths.circuit.display()))?;
    let circuit = parse_lines(&circuit_text).map_err(|error| format!("parse circuit: {error}"))?;
    verify_circuit_identity(&manifest, &circuit)?;
    let transform =
        MeasurementTransform::from_circuit(&circuit).map_err(|error| error.to_string())?;
    verify_transform_identity(&manifest, &shape, &transform)?;

    let expected_measurements = read_01_table(&paths.measurements_01, shape.measurements)?;
    let measurement_b8_bytes = fs::read(&paths.measurements_b8)
        .map_err(|error| format!("{}: {error}", paths.measurements_b8.display()))?;
    let measurements_from_b8 = read_shots_b8(&measurement_b8_bytes, shape.measurements)
        .map_err(|error| format!("measurements b8: {error}"))?;
    assert_tables_eq(
        &expected_measurements,
        &measurements_from_b8,
        "measurement 01 and b8",
    )?;
    let expected_detectors = read_01_table(&paths.expected_detectors_01, shape.detectors)?;
    let expected_observables = read_01_table(&paths.expected_observables_01, shape.observables)?;

    let archive = fs::read(&paths.archive)
        .map_err(|error| format!("{}: {error}", paths.archive.display()))?;
    let archive_layout = verify_archive_structure(&manifest, &shape, &archive)?;
    let decoded = decode_archive(&archive, &circuit, &shape)?;
    assert_tables_eq(
        &expected_measurements,
        &decoded.measurements,
        "decoded measurements",
    )?;
    assert_tables_eq(&expected_detectors, &decoded.detectors, "decoded detectors")?;
    assert_tables_eq(
        &expected_observables,
        &decoded.observables,
        "decoded observables",
    )?;
    verify_decoded_logical_payloads(&manifest, &transform, &decoded, &archive_layout)?;

    Ok(CompatReport {
        success_line: SUCCESS_LINE.to_string(),
    })
}

fn verify_manifest_identity(manifest: &TomlValue) -> CheckResult<()> {
    require_eq(
        toml_i64(manifest, &["schema_version"])?,
        1,
        "schema_version",
    )?;
    require_eq(
        toml_str(manifest, &["fixture_id"])?,
        FIXTURE_ID,
        "fixture_id",
    )?;
    Ok(())
}

fn verify_manifest_consumers(manifest: &TomlValue) -> CheckResult<()> {
    require_toml_string_list(
        manifest.get("consumers"),
        &[
            "compatibility",
            "corruption_corpus",
            "cli_publication_tests",
            "readiness",
        ],
        "consumers",
    )?;
    Ok(())
}

fn verify_manifest_generation(manifest: &TomlValue) -> CheckResult<()> {
    let argv = toml_array(manifest, &["generation", "argv"])?;
    let argv_strings = argv
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| "generation.argv entries must be strings".to_string())
        })
        .collect::<CheckResult<Vec<_>>>()?;
    require_eq(
        argv_strings,
        vec![
            "cargo",
            "test",
            "--locked",
            "-p",
            "rstim",
            "--test",
            "_tmp_generate_compat_fixture",
            "generate_compat_fixture",
            "--",
            "--exact",
            "--nocapture",
        ],
        "generation.argv",
    )?;
    require_eq(
        toml_str(manifest, &["generation", "generator_repository_revision"])?,
        "31f9f456151d0794b2bc91244f5f4fc46eb5f6de",
        "generation.generator_repository_revision",
    )?;
    require_eq(
        toml_str(manifest, &["generation", "writer_source_base_revision"])?,
        "98a203f64fbd9b758d8768b68e73162234b50434",
        "generation.writer_source_base_revision",
    )?;
    require_eq(
        toml_str(manifest, &["generation", "zstandard_crate"])?,
        "zstd 0.13.3",
        "generation.zstandard_crate",
    )?;
    require_eq(
        toml_str(manifest, &["generation", "zstandard_backend"])?,
        "zstd-sys 2.0.16+zstd.1.5.7",
        "generation.zstandard_backend",
    )?;
    let cargo_lock_sha256 = toml_str(manifest, &["generation", "cargo_lock_sha256"])?;
    require_sha256(cargo_lock_sha256, "generation.cargo_lock_sha256")?;
    require_eq(
        toml_str(manifest, &["hashes", "cargo_lock_sha256"])?,
        cargo_lock_sha256,
        "hashes.cargo_lock_sha256",
    )?;
    let statement = toml_str(manifest, &["generation", "statement"])?;
    if !statement.contains("provenance") || !statement.contains("not required") {
        return Err(
            "generation.statement must record provenance and writer-byte policy".to_string(),
        );
    }
    Ok(())
}

fn verify_fixture_hashes(
    repo_root: &Path,
    manifest: &TomlValue,
    paths: &FixturePaths,
    manifest_sha256: &str,
    archive_file_sha256: &str,
) -> CheckResult<()> {
    let file_checks = [
        (
            &paths.circuit,
            "hashes.circuit_source_sha256",
            ["hashes", "circuit_source_sha256"],
        ),
        (
            &paths.measurements_01,
            "hashes.measurements_01_sha256",
            ["hashes", "measurements_01_sha256"],
        ),
        (
            &paths.measurements_b8,
            "hashes.measurements_b8_sha256",
            ["hashes", "measurements_b8_sha256"],
        ),
        (
            &paths.archive,
            "hashes.archive_sha256",
            ["hashes", "archive_sha256"],
        ),
        (
            &paths.expected_detectors_01,
            "hashes.expected_detectors_01_sha256",
            ["hashes", "expected_detectors_01_sha256"],
        ),
        (
            &paths.expected_observables_01,
            "hashes.expected_observables_01_sha256",
            ["hashes", "expected_observables_01_sha256"],
        ),
    ];
    for (path, label, key) in file_checks {
        require_eq(
            sha256_file(path)?.as_str(),
            toml_str(manifest, &key)?,
            label,
        )?;
    }
    require_eq(
        archive_file_sha256,
        toml_str(manifest, &["hashes", "whole_archive_sha256"])?,
        "hashes.whole_archive_sha256",
    )?;
    require_eq(
        archive_file_sha256,
        toml_str(manifest, &["hashes", "archive_sha256"])?,
        "hashes.archive_sha256",
    )?;
    require_eq(
        sha256_file(&checked_repo_path(repo_root, MANIFEST_PATH, MANIFEST_PATH)?)?.as_str(),
        manifest_sha256,
        "manifest_sha256",
    )?;
    Ok(())
}

fn verify_catalog_shape_and_hashes(
    catalog_entry: &JsonValue,
    shape: &FixtureShape,
    archive_file_sha256: &str,
    manifest_sha256: &str,
) -> CheckResult<()> {
    require_eq(
        json_string(
            catalog_entry.get("circuit_sha256"),
            "catalog.circuit_sha256",
        )?,
        "18a857fb71f44eb28144a6a0e3aad17cce675daa2737690432efe305ed5777a2",
        "catalog.circuit_sha256",
    )?;
    require_eq(
        json_u64(catalog_entry.get("shots"), "catalog.shots")?,
        shape.shots,
        "catalog.shots",
    )?;
    require_eq(
        json_u64(
            catalog_entry.get("measurement_count"),
            "catalog.measurement_count",
        )?,
        shape.measurements as u64,
        "catalog.measurement_count",
    )?;
    require_eq(
        json_u64(
            catalog_entry.get("detector_count"),
            "catalog.detector_count",
        )?,
        shape.detectors as u64,
        "catalog.detector_count",
    )?;
    require_eq(
        json_u64(
            catalog_entry.get("observable_count"),
            "catalog.observable_count",
        )?,
        shape.observables as u64,
        "catalog.observable_count",
    )?;
    require_eq(
        json_u64(catalog_entry.get("rank_H"), "catalog.rank_H")?,
        shape.rank as u64,
        "catalog.rank_H",
    )?;
    let hashes = json_object(catalog_entry.get("hashes"), "catalog.hashes")?;
    require_eq(
        json_string(
            hashes.get("archive_sha256"),
            "catalog.hashes.archive_sha256",
        )?,
        archive_file_sha256,
        "catalog.hashes.archive_sha256",
    )?;
    require_eq(
        json_string(
            hashes.get("manifest_sha256"),
            "catalog.hashes.manifest_sha256",
        )?,
        manifest_sha256,
        "catalog.hashes.manifest_sha256",
    )?;
    Ok(())
}

fn verify_circuit_identity(manifest: &TomlValue, circuit: &[StimInstr]) -> CheckResult<()> {
    let canonical = circuit_to_string(circuit);
    require_eq(
        canonical.len() as i64,
        toml_i64(manifest, &["circuit", "canonical_utf8_bytes"])?,
        "circuit.canonical_utf8_bytes",
    )?;
    let identity_sha256 = hex(&Sha256::digest(canonical.as_bytes()));
    require_eq(
        identity_sha256.as_str(),
        toml_str(manifest, &["hashes", "circuit_identity_sha256"])?,
        "hashes.circuit_identity_sha256",
    )?;
    Ok(())
}

fn verify_transform_identity(
    manifest: &TomlValue,
    shape: &FixtureShape,
    transform: &MeasurementTransform,
) -> CheckResult<()> {
    let identity = transform.identity();
    let identity_sha256 = hex(&identity.circuit_sha256);
    require_eq(
        identity_sha256.as_str(),
        toml_str(manifest, &["hashes", "circuit_identity_sha256"])?,
        "transform.circuit_sha256",
    )?;
    require_eq(
        transform.num_measurements(),
        shape.measurements,
        "transform.measurements",
    )?;
    require_eq(
        transform.num_detectors(),
        shape.detectors,
        "transform.detectors",
    )?;
    require_eq(
        transform.num_observables(),
        shape.observables,
        "transform.observables",
    )?;
    require_eq(transform.rank(), shape.rank, "transform.rank")?;
    require_eq(
        transform.free_columns().len(),
        shape.free_measurements,
        "transform.free_measurements",
    )?;
    require_eq(
        identity.canonicalization_id as i64,
        toml_i64(manifest, &["format", "canonicalization_id"])?,
        "format.canonicalization_id",
    )?;
    require_eq(
        identity.fingerprint_id as i64,
        toml_i64(manifest, &["format", "fingerprint_id"])?,
        "format.fingerprint_id",
    )?;
    require_eq(
        identity.transform_algorithm_id as i64,
        toml_i64(manifest, &["format", "transform_id"])?,
        "format.transform_id",
    )?;
    require_eq(
        identity.reference_strategy_id as i64,
        toml_i64(manifest, &["format", "reference_id"])?,
        "format.reference_id",
    )?;
    Ok(())
}

fn verify_archive_structure(
    manifest: &TomlValue,
    shape: &FixtureShape,
    archive: &[u8],
) -> CheckResult<ArchiveLayout> {
    let format = FormatPins::from_manifest(manifest)?;
    let minimum_len = checked_add_usize(
        format.global_header_len,
        format.trailer_len,
        "minimum archive length",
    )?;
    if archive.len() < minimum_len {
        return Err("archive too short for global header and trailer".to_string());
    }
    require_eq(
        get_u16(archive, 8),
        format.format_major,
        "global.format_major",
    )?;
    require_eq(
        get_u16(archive, 10),
        format.format_minor,
        "global.format_minor",
    )?;
    require_eq(
        get_u32(archive, 12) as usize,
        format.global_header_len,
        "global.header_len",
    )?;
    let header = GlobalHeader::from_bytes(&archive[..format.global_header_len])
        .map_err(|error| format!("global header: {error}"))?;
    let header_digest = hex(&Sha256::digest(&archive[..format.global_header_len - 32]));
    require_eq(
        header_digest.as_str(),
        hex(&header.header_sha256).as_str(),
        "global.header_sha256",
    )?;
    require_eq(
        header.max_shots_per_block,
        shape.block_shots,
        "global.max_shots_per_block",
    )?;
    require_eq(
        header.measurement_count,
        shape.measurements as u64,
        "global.measurement_count",
    )?;
    require_eq(
        header.detector_count,
        shape.detectors as u64,
        "global.detector_count",
    )?;
    require_eq(
        header.observable_count,
        shape.observables as u64,
        "global.observable_count",
    )?;
    require_eq(
        header.detector_rank,
        shape.rank as u64,
        "global.detector_rank",
    )?;
    require_eq(header.total_shots, shape.shots, "global.total_shots")?;
    require_eq(
        header.canonicalization_id,
        format.canonicalization_id,
        "global.canonicalization_id",
    )?;
    require_eq(
        header.fingerprint_id,
        format.fingerprint_id,
        "global.fingerprint_id",
    )?;
    require_eq(
        header.transform_id,
        format.transform_id,
        "global.transform_id",
    )?;
    require_eq(
        header.reference_id,
        format.reference_id,
        "global.reference_id",
    )?;
    require_eq(
        header.codec_suite_id,
        format.codec_suite_id,
        "global.codec_suite_id",
    )?;

    let manifest_blocks = toml_array(manifest, &["blocks"])?;
    require_eq(manifest_blocks.len(), shape.blocks as usize, "blocks.len")?;
    let mut block_layouts = Vec::new();
    let mut offset = format.global_header_len;
    for (block_index, manifest_block) in manifest_blocks.iter().enumerate() {
        let block_end = checked_add_usize(offset, format.block_header_len, "block header end")?;
        if block_end > archive.len() {
            return Err(format!("block {block_index} exceeds archive length"));
        }
        if archive[offset..offset + 8] != BLOCK_MAGIC[..] {
            return Err(format!("block {block_index} magic mismatch"));
        }
        require_eq(
            get_u16(archive, offset + 8),
            format.format_major,
            &format!("block {block_index}.format_major"),
        )?;
        require_eq(
            get_u16(archive, offset + 10),
            format.format_minor,
            &format!("block {block_index}.format_minor"),
        )?;
        let block = BlockHeader::from_bytes(&archive[offset..block_end])
            .map_err(|error| format!("block {block_index}: {error}"))?;
        verify_block_header(block_index, &block, manifest_block, &format)?;
        let syndrome_start = block_end;
        let syndrome_end = checked_add_usize(
            syndrome_start,
            block.syndrome_compressed_len as usize,
            "syndrome stream end",
        )?;
        let free_end = checked_add_usize(
            syndrome_end,
            block.free_compressed_len as usize,
            "free stream end",
        )?;
        if free_end > archive.len() {
            return Err(format!("block {block_index} streams exceed archive length"));
        }
        require_eq(
            hex(&Sha256::digest(&archive[syndrome_start..syndrome_end])).as_str(),
            toml_str_value(manifest_block, &["syndrome_frame_sha256"])?,
            "block.syndrome_frame_sha256",
        )?;
        require_eq(
            hex(&Sha256::digest(&archive[syndrome_end..free_end])).as_str(),
            toml_str_value(manifest_block, &["free_frame_sha256"])?,
            "block.free_frame_sha256",
        )?;
        block_layouts.push(VerifiedBlockLayout { header: block });
        offset = free_end;
    }

    let trailer_end = checked_add_usize(offset, format.trailer_len, "trailer end")?;
    if trailer_end != archive.len() {
        return Err("archive trailer is not at the expected end offset".to_string());
    }
    if archive[offset..offset + 8] != TRAILER_MAGIC[..] {
        return Err("trailer magic mismatch".to_string());
    }
    require_eq(
        get_u16(archive, offset + 8),
        format.format_major,
        "trailer.format_major",
    )?;
    require_eq(
        get_u16(archive, offset + 10),
        format.format_minor,
        "trailer.format_minor",
    )?;
    let trailer = ArchiveTrailer::from_bytes(&archive[offset..trailer_end])
        .map_err(|error| format!("trailer: {error}"))?;
    require_eq(trailer.block_count, shape.blocks, "trailer.block_count")?;
    require_eq(trailer.total_shots, shape.shots, "trailer.total_shots")?;
    let trailer_digest = hex(&Sha256::digest(&archive[..offset + 32]));
    require_eq(
        trailer_digest.as_str(),
        hex(&trailer.archive_sha256).as_str(),
        "trailer.archive_sha256",
    )?;
    require_eq(
        trailer_digest.as_str(),
        toml_str(manifest, &["hashes", "trailer_archive_sha256"])?,
        "hashes.trailer_archive_sha256",
    )?;

    Ok(ArchiveLayout {
        blocks: block_layouts,
    })
}

fn verify_block_header(
    block_index: usize,
    block: &BlockHeader,
    manifest_block: &TomlValue,
    format: &FormatPins,
) -> CheckResult<()> {
    require_eq(block.block_index, block_index as u64, "block.block_index")?;
    for (field, actual) in [
        ("index", block.block_index),
        ("first_shot", block.first_shot),
        ("shot_count", block.shot_count),
        ("syndrome_codec_id", block.syndrome_codec_id as u64),
        ("free_codec_id", block.free_codec_id as u64),
        ("syndrome_uncompressed_len", block.syndrome_uncompressed_len),
        ("syndrome_compressed_len", block.syndrome_compressed_len),
        ("free_uncompressed_len", block.free_uncompressed_len),
        ("free_compressed_len", block.free_compressed_len),
    ] {
        require_eq(
            actual as i64,
            toml_i64_value(manifest_block, &[field])?,
            &format!("block.{field}"),
        )?;
    }
    let expected_syndrome_codec = match block_index {
        0 => ("sparse", format.syndrome_codec_sparse_id),
        1 => ("dense", format.syndrome_codec_dense_id),
        _ => return Err(format!("unexpected block index {block_index}")),
    };
    require_eq(
        toml_str_value(manifest_block, &["syndrome_codec"])?,
        expected_syndrome_codec.0,
        "block.syndrome_codec",
    )?;
    require_eq(
        block.syndrome_codec_id,
        expected_syndrome_codec.1,
        "block.syndrome_codec_id",
    )?;
    require_eq(
        toml_str_value(manifest_block, &["free_codec"])?,
        "dense",
        "block.free_codec",
    )?;
    require_eq(
        block.free_codec_id,
        format.free_codec_dense_id,
        "block.free_codec_id",
    )?;
    require_eq(
        hex(&block.logical_payload_sha256).as_str(),
        toml_str_value(manifest_block, &["logical_payload_sha256"])?,
        "block.logical_payload_sha256",
    )?;
    Ok(())
}

fn decode_archive(
    archive: &[u8],
    circuit: &[StimInstr],
    shape: &FixtureShape,
) -> CheckResult<DecodedTables> {
    let mut decoded = DecodedTables {
        measurements: BitTable::try_new(shape.measurements, shape.shots as usize)
            .map_err(|_| "decoded measurement allocation failed".to_string())?,
        detectors: BitTable::try_new(shape.detectors, shape.shots as usize)
            .map_err(|_| "decoded detector allocation failed".to_string())?,
        observables: BitTable::try_new(shape.observables, shape.shots as usize)
            .map_err(|_| "decoded observable allocation failed".to_string())?,
    };
    let mut reader =
        SampleArchiveReader::open(io::Cursor::new(archive), circuit, ArchiveLimits::default())
            .map_err(|error| format!("reader open: {error}"))?;
    let mut offset = 0usize;
    let mut blocks = 0u64;
    while let Some(block) = reader
        .next_block()
        .map_err(|error| format!("reader next_block: {error}"))?
    {
        let shots = block.measurements.num_minor();
        copy_decoded_block(&block, &mut decoded, offset)?;
        offset += shots;
        blocks += 1;
    }
    require_eq(offset as u64, shape.shots, "decoded shot count")?;
    let summary = reader
        .finish()
        .map_err(|error| format!("reader finish: {error}"))?;
    require_eq(summary.block_count, shape.blocks, "summary.block_count")?;
    require_eq(summary.total_shots, shape.shots, "summary.total_shots")?;
    require_eq(blocks, shape.blocks, "reader block count")?;
    Ok(decoded)
}

fn verify_decoded_logical_payloads(
    manifest: &TomlValue,
    transform: &MeasurementTransform,
    decoded: &DecodedTables,
    archive_layout: &ArchiveLayout,
) -> CheckResult<()> {
    let blocks = toml_array(manifest, &["blocks"])?;
    for (block_index, (manifest_block, layout)) in
        blocks.iter().zip(archive_layout.blocks.iter()).enumerate()
    {
        let first_shot = layout.header.first_shot as usize;
        let shot_count = layout.header.shot_count as usize;
        let selected =
            selected_detector_table(transform, &decoded.detectors, first_shot, shot_count)?;
        let free =
            free_measurement_table(transform, &decoded.measurements, first_shot, shot_count)?;
        let mut hasher = Sha256::new();
        hasher.update(pack_dense_by_shot(&selected));
        hasher.update(pack_dense_by_shot(&free));
        let digest = hex(&hasher.finalize());
        require_eq(
            digest.as_str(),
            toml_str_value(manifest_block, &["logical_payload_sha256"])?,
            &format!("block {block_index} decoded logical payload"),
        )?;
    }
    Ok(())
}

fn selected_detector_table(
    transform: &MeasurementTransform,
    detections: &BitTable,
    first_shot: usize,
    shot_count: usize,
) -> CheckResult<BitTable> {
    let selected_rows = transform.selected_detector_rows();
    let mut table = BitTable::try_new(selected_rows.len(), shot_count)
        .map_err(|_| "selected detector table allocation failed".to_string())?;
    for (row, detector) in selected_rows.iter().copied().enumerate() {
        for shot in 0..shot_count {
            table.set(row, shot, detections.get(detector, first_shot + shot));
        }
    }
    Ok(table)
}

fn free_measurement_table(
    transform: &MeasurementTransform,
    measurements: &BitTable,
    first_shot: usize,
    shot_count: usize,
) -> CheckResult<BitTable> {
    let free_columns = transform.free_columns();
    let reference = transform.reference_bits();
    let mut table = BitTable::try_new(free_columns.len(), shot_count)
        .map_err(|_| "free measurement table allocation failed".to_string())?;
    for (row, measurement) in free_columns.iter().copied().enumerate() {
        for shot in 0..shot_count {
            let flip = measurements.get(measurement, first_shot + shot) ^ reference[measurement];
            table.set(row, shot, flip);
        }
    }
    Ok(table)
}

fn copy_decoded_block(
    block: &DecodedSampleBlock,
    decoded: &mut DecodedTables,
    offset: usize,
) -> CheckResult<()> {
    let shots = block.measurements.num_minor();
    copy_table_columns(
        &block.measurements,
        0,
        &mut decoded.measurements,
        offset,
        shots,
    )?;
    copy_table_columns(&block.detections, 0, &mut decoded.detectors, offset, shots)?;
    copy_table_columns(
        &block.observable_flips,
        0,
        &mut decoded.observables,
        offset,
        shots,
    )?;
    Ok(())
}

fn copy_table_columns(
    source: &BitTable,
    source_offset: usize,
    target: &mut BitTable,
    target_offset: usize,
    shots: usize,
) -> CheckResult<()> {
    require_eq(source.num_major(), target.num_major(), "copy rows")?;
    for row in 0..source.num_major() {
        for shot in 0..shots {
            target.set(
                row,
                target_offset + shot,
                source.get(row, source_offset + shot),
            );
        }
    }
    Ok(())
}

fn read_01_table(path: &Path, bits: usize) -> CheckResult<BitTable> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    read_shots_01(&bytes, bits).map_err(|error| format!("{}: {error}", path.display()))
}

fn assert_tables_eq(left: &BitTable, right: &BitTable, label: &str) -> CheckResult<()> {
    require_eq(
        left.num_major(),
        right.num_major(),
        &format!("{label} rows"),
    )?;
    require_eq(
        left.num_minor(),
        right.num_minor(),
        &format!("{label} shots"),
    )?;
    for row in 0..left.num_major() {
        for shot in 0..left.num_minor() {
            if left.get(row, shot) != right.get(row, shot) {
                return Err(format!("{label} mismatch at row {row}, shot {shot}"));
            }
        }
    }
    Ok(())
}

fn pack_dense_by_shot(table: &BitTable) -> Vec<u8> {
    let bits = table.num_major() * table.num_minor();
    let mut bytes = vec![0u8; bits.div_ceil(8)];
    for shot in 0..table.num_minor() {
        for row in 0..table.num_major() {
            if table.get(row, shot) {
                let bit = shot * table.num_major() + row;
                bytes[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    bytes
}

fn load_catalog(repo_root: &Path) -> CheckResult<JsonValue> {
    let path = checked_repo_path(repo_root, CATALOG_PATH, CATALOG_PATH)?;
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse catalog: {error}"))
}

fn catalog_entry(catalog: &JsonValue) -> CheckResult<&JsonValue> {
    let cases = catalog
        .get("cases")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "catalog.cases must be an array".to_string())?;
    cases
        .iter()
        .find(|case| case.get("id").and_then(JsonValue::as_str) == Some(FIXTURE_ID))
        .ok_or_else(|| format!("catalog missing case {FIXTURE_ID}"))
}

fn verify_catalog_consumers(catalog_entry: &JsonValue) -> CheckResult<()> {
    require_json_string_list(
        catalog_entry.get("consumers"),
        &[
            "compatibility",
            "corruption_corpus",
            "cli_publication_tests",
            "readiness",
        ],
        "catalog.consumers",
    )
}

fn load_manifest(path: &Path) -> CheckResult<TomlValue> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("parse manifest: {error}"))
}

fn checked_repo_path(repo_root: &Path, value: &str, label: &str) -> CheckResult<PathBuf> {
    if value.is_empty() || value.contains('\\') {
        return Err(format!(
            "{label} must be a non-empty POSIX repo-relative path"
        ));
    }
    let relative = Path::new(value);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{label} must be repository-relative without '..'"));
    }
    Ok(repo_root.join(relative))
}

fn sha256_file(path: &Path) -> CheckResult<String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn checked_add_usize(left: usize, right: usize, label: &str) -> CheckResult<usize> {
    left.checked_add(right)
        .ok_or_else(|| format!("{label} overflows usize"))
}

fn json_object<'a>(value: Option<&'a JsonValue>, label: &str) -> CheckResult<&'a JsonValue> {
    value
        .and_then(JsonValue::as_object)
        .map(|_| value.unwrap())
        .ok_or_else(|| format!("{label} must be an object"))
}

fn json_string<'a>(value: Option<&'a JsonValue>, label: &str) -> CheckResult<&'a str> {
    value
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{label} must be a string"))
}

fn json_u64(value: Option<&JsonValue>, label: &str) -> CheckResult<u64> {
    value
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("{label} must be a non-negative integer"))
}

fn require_json_string_list(
    value: Option<&JsonValue>,
    expected: &[&str],
    label: &str,
) -> CheckResult<()> {
    let array = value
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?;
    let actual = array
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| format!("{label} entries must be strings"))
        })
        .collect::<CheckResult<Vec<_>>>()?;
    require_eq(actual, expected.to_vec(), label)
}

fn toml_value_at<'a>(value: &'a TomlValue, path: &[&str]) -> CheckResult<&'a TomlValue> {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .ok_or_else(|| format!("missing TOML field {}", path.join(".")))?;
    }
    Ok(current)
}

fn toml_str<'a>(value: &'a TomlValue, path: &[&str]) -> CheckResult<&'a str> {
    toml_str_value(value, path)
}

fn toml_str_value<'a>(value: &'a TomlValue, path: &[&str]) -> CheckResult<&'a str> {
    toml_value_at(value, path)?
        .as_str()
        .ok_or_else(|| format!("{} must be a string", path.join(".")))
}

fn toml_i64(value: &TomlValue, path: &[&str]) -> CheckResult<i64> {
    toml_i64_value(value, path)
}

fn toml_i64_value(value: &TomlValue, path: &[&str]) -> CheckResult<i64> {
    toml_value_at(value, path)?
        .as_integer()
        .ok_or_else(|| format!("{} must be an integer", path.join(".")))
}

fn toml_u16(value: &TomlValue, path: &[&str]) -> CheckResult<u16> {
    let raw = toml_i64(value, path)?;
    u16::try_from(raw).map_err(|_| format!("{} must fit in u16", path.join(".")))
}

fn toml_usize(value: &TomlValue, path: &[&str]) -> CheckResult<usize> {
    let raw = toml_i64(value, path)?;
    usize::try_from(raw).map_err(|_| format!("{} must fit in usize", path.join(".")))
}

fn toml_array<'a>(value: &'a TomlValue, path: &[&str]) -> CheckResult<&'a Vec<TomlValue>> {
    toml_value_at(value, path)?
        .as_array()
        .ok_or_else(|| format!("{} must be an array", path.join(".")))
}

fn require_toml_string_list(
    value: Option<&TomlValue>,
    expected: &[&str],
    label: &str,
) -> CheckResult<()> {
    let array = value
        .and_then(TomlValue::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?;
    let actual = array
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| format!("{label} entries must be strings"))
        })
        .collect::<CheckResult<Vec<_>>>()?;
    require_eq(actual, expected.to_vec(), label)
}

fn require_eq<T>(actual: T, expected: T, label: &str) -> CheckResult<()>
where
    T: PartialEq + std::fmt::Debug,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{label} mismatch: got {actual:?}, expected {expected:?}"
        ))
    }
}

fn require_sha256(value: &str, label: &str) -> CheckResult<()> {
    let is_hex = value
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte));
    if value.len() == 64 && is_hex {
        Ok(())
    } else {
        Err(format!("{label} must be a lowercase SHA-256 digest"))
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[derive(Clone, Copy)]
struct FixtureShape {
    measurements: usize,
    detectors: usize,
    observables: usize,
    rank: usize,
    free_measurements: usize,
    shots: u64,
    blocks: u64,
    block_shots: u64,
}

impl FixtureShape {
    fn from_manifest(manifest: &TomlValue) -> CheckResult<Self> {
        let shape = Self {
            measurements: toml_i64(manifest, &["shape", "measurements"])? as usize,
            detectors: toml_i64(manifest, &["shape", "detectors"])? as usize,
            observables: toml_i64(manifest, &["shape", "observables"])? as usize,
            rank: toml_i64(manifest, &["shape", "rank"])? as usize,
            free_measurements: toml_i64(manifest, &["shape", "free_measurements"])? as usize,
            shots: toml_i64(manifest, &["shape", "shots"])? as u64,
            blocks: toml_i64(manifest, &["shape", "blocks"])? as u64,
            block_shots: toml_i64(manifest, &["shape", "block_shots"])? as u64,
        };
        require_eq(shape.measurements, 10, "shape.measurements")?;
        require_eq(shape.detectors, 9, "shape.detectors")?;
        require_eq(shape.observables, 1, "shape.observables")?;
        require_eq(shape.rank, 9, "shape.rank")?;
        require_eq(shape.free_measurements, 1, "shape.free_measurements")?;
        require_eq(shape.shots, 4, "shape.shots")?;
        require_eq(shape.blocks, 2, "shape.blocks")?;
        require_eq(shape.block_shots, 2, "shape.block_shots")?;
        Ok(shape)
    }
}

#[derive(Clone, Copy)]
struct FormatPins {
    format_major: u16,
    format_minor: u16,
    global_header_len: usize,
    block_header_len: usize,
    trailer_len: usize,
    canonicalization_id: u16,
    fingerprint_id: u16,
    transform_id: u16,
    reference_id: u16,
    codec_suite_id: u16,
    zstd_level: i64,
    syndrome_codec_sparse_id: u16,
    syndrome_codec_dense_id: u16,
    free_codec_dense_id: u16,
}

impl FormatPins {
    fn from_manifest(manifest: &TomlValue) -> CheckResult<Self> {
        let pins = Self {
            format_major: toml_u16(manifest, &["format", "format_major"])?,
            format_minor: toml_u16(manifest, &["format", "format_minor"])?,
            global_header_len: toml_usize(manifest, &["format", "global_header_len"])?,
            block_header_len: toml_usize(manifest, &["format", "block_header_len"])?,
            trailer_len: toml_usize(manifest, &["format", "trailer_len"])?,
            canonicalization_id: toml_u16(manifest, &["format", "canonicalization_id"])?,
            fingerprint_id: toml_u16(manifest, &["format", "fingerprint_id"])?,
            transform_id: toml_u16(manifest, &["format", "transform_id"])?,
            reference_id: toml_u16(manifest, &["format", "reference_id"])?,
            codec_suite_id: toml_u16(manifest, &["format", "codec_suite_id"])?,
            zstd_level: toml_i64(manifest, &["format", "zstd_level"])?,
            syndrome_codec_sparse_id: toml_u16(manifest, &["format", "syndrome_codec_sparse_id"])?,
            syndrome_codec_dense_id: toml_u16(manifest, &["format", "syndrome_codec_dense_id"])?,
            free_codec_dense_id: toml_u16(manifest, &["format", "free_codec_dense_id"])?,
        };
        require_eq(pins.format_major, FORMAT_MAJOR, "format.format_major")?;
        require_eq(pins.format_minor, FORMAT_MINOR, "format.format_minor")?;
        require_eq(
            pins.global_header_len,
            GLOBAL_HEADER_LEN,
            "format.global_header_len",
        )?;
        require_eq(
            pins.block_header_len,
            BLOCK_HEADER_LEN,
            "format.block_header_len",
        )?;
        require_eq(pins.trailer_len, ARCHIVE_TRAILER_LEN, "format.trailer_len")?;
        require_eq(
            pins.canonicalization_id,
            CANONICALIZATION_RSTIM_CIRCUIT_TEXT_V1,
            "format.canonicalization_id",
        )?;
        require_eq(
            pins.fingerprint_id,
            FINGERPRINT_SHA256_CANONICAL_CIRCUIT,
            "format.fingerprint_id",
        )?;
        require_eq(
            pins.transform_id,
            TRANSFORM_SELECTED_DETECTOR_FREE_MEASUREMENT_V1,
            "format.transform_id",
        )?;
        require_eq(
            pins.reference_id,
            REFERENCE_SIMULATE_NOISELESS,
            "format.reference_id",
        )?;
        require_eq(
            pins.codec_suite_id,
            CODEC_SUITE_ZSTD_FRAMES_V1,
            "format.codec_suite_id",
        )?;
        require_eq(pins.zstd_level, EXPECTED_ZSTD_LEVEL, "format.zstd_level")?;
        require_eq(
            pins.syndrome_codec_sparse_id,
            STREAM_CODEC_SYNDROME_SPARSE_LEB128_V1,
            "format.syndrome_codec_sparse_id",
        )?;
        require_eq(
            pins.syndrome_codec_dense_id,
            STREAM_CODEC_SYNDROME_DENSE_V1,
            "format.syndrome_codec_dense_id",
        )?;
        require_eq(
            pins.free_codec_dense_id,
            STREAM_CODEC_FREE_DENSE_V1,
            "format.free_codec_dense_id",
        )?;
        Ok(pins)
    }
}

struct FixturePaths {
    circuit: PathBuf,
    measurements_01: PathBuf,
    measurements_b8: PathBuf,
    archive: PathBuf,
    expected_detectors_01: PathBuf,
    expected_observables_01: PathBuf,
}

impl FixturePaths {
    fn from_manifest(repo_root: &Path, manifest: &TomlValue) -> CheckResult<Self> {
        Ok(Self {
            circuit: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "circuit"])?,
                "paths.circuit",
            )?,
            measurements_01: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "measurements_01"])?,
                "paths.measurements_01",
            )?,
            measurements_b8: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "measurements_b8"])?,
                "paths.measurements_b8",
            )?,
            archive: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "archive"])?,
                "paths.archive",
            )?,
            expected_detectors_01: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "expected_detectors_01"])?,
                "paths.expected_detectors_01",
            )?,
            expected_observables_01: checked_repo_path(
                repo_root,
                toml_str(manifest, &["paths", "expected_observables_01"])?,
                "paths.expected_observables_01",
            )?,
        })
    }
}

struct ArchiveLayout {
    blocks: Vec<VerifiedBlockLayout>,
}

struct VerifiedBlockLayout {
    header: BlockHeader,
}

struct DecodedTables {
    measurements: BitTable,
    detectors: BitTable,
    observables: BitTable,
}

#[derive(Debug)]
struct CompatReport {
    success_line: String,
}

struct CopiedFixtureTree {
    root: TempDir,
    archive_path: PathBuf,
    manifest_path: PathBuf,
    catalog_path: PathBuf,
}

impl CopiedFixtureTree {
    fn new() -> Self {
        let source_root = repo_root();
        let root = tempfile::tempdir().expect("temporary fixture root");
        copy_repo_file(&source_root, root.path(), "Cargo.lock");
        copy_repo_file(&source_root, root.path(), CATALOG_PATH);
        let source_dir = source_root.join("rstim/tests/fixtures/rsmp/v1");
        for entry in fs::read_dir(&source_dir).expect("read v1 fixture dir") {
            let entry = entry.expect("fixture entry");
            let name = entry.file_name();
            let relative = Path::new("rstim/tests/fixtures/rsmp/v1").join(name);
            copy_repo_file(&source_root, root.path(), relative.to_str().unwrap());
        }
        Self {
            archive_path: root.path().join(ARCHIVE_PATH),
            manifest_path: root.path().join(MANIFEST_PATH),
            catalog_path: root.path().join(CATALOG_PATH),
            root,
        }
    }
}

fn copy_repo_file(source_root: &Path, target_root: &Path, relative: &str) {
    let source = source_root.join(relative);
    let target = target_root.join(relative);
    fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
    fs::copy(&source, &target).unwrap_or_else(|error| {
        panic!("copy {} to {}: {error}", source.display(), target.display())
    });
}

fn flip_first_compressed_stream_byte(archive_path: &Path) {
    let mut bytes = fs::read(archive_path).expect("read archive copy");
    let first_block = GLOBAL_HEADER_LEN;
    let block =
        BlockHeader::from_bytes(&bytes[first_block..first_block + BLOCK_HEADER_LEN]).unwrap();
    assert!(block.syndrome_compressed_len > 8);
    let payload_offset = first_block + BLOCK_HEADER_LEN + 8;
    bytes[payload_offset] ^= 0x01;
    fs::write(archive_path, bytes).expect("write mutated archive copy");
}

fn rewrite_manifest_measurement_hash(manifest_path: &Path, replacement: &str) {
    rewrite_manifest_literal(
        manifest_path,
        "measurements_01_sha256 = \"90efbc9f3f0de6fd6562acba5601d02820f32cbf632410cd01058d1fd4b06c1e\"",
        &format!("measurements_01_sha256 = \"{replacement}\""),
    );
}

fn rewrite_manifest_literal(manifest_path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(manifest_path).expect("read manifest copy");
    let updated = text.replace(from, to);
    assert_ne!(text, updated);
    fs::write(manifest_path, updated).expect("write mutated manifest copy");
}

fn refresh_catalog_manifest_hash(catalog_path: &Path, manifest_path: &Path) {
    let new_hash = sha256_file(manifest_path).expect("hash mutated manifest");
    let text = fs::read_to_string(catalog_path).expect("read catalog copy");
    let updated = text.replace(
        "d5e983ac8261f49fd8a8fdfa3e6d119eddf85d80e23065d041a07797da7b5d8a",
        &new_hash,
    );
    assert_ne!(text, updated);
    fs::write(catalog_path, updated).expect("write catalog copy");
}
