use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rstim::dem::DetectorErrorModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(any(
    test,
    feature = "rbposd-runner",
    feature = "rmatching-runner",
    feature = "ilp-runner"
))]
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::decode::Decoder;

#[derive(Debug, Clone)]
pub struct ReplayOptions {
    pub dem: PathBuf,
    pub dets: PathBuf,
    pub decoder: String,
    pub decoder_config: Option<PathBuf>,
    pub predictions_out: PathBuf,
    pub stats_out: PathBuf,
    pub batch_size: usize,
    pub shots: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStats {
    pub schema_version: u32,
    pub decoder: String,
    pub decoder_config: Value,
    pub num_shots: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub batch_size: usize,
    pub batches: usize,
    pub compile_seconds: f64,
    pub decode_seconds: f64,
    pub shots_per_second: f64,
    pub detector_bytes: u64,
    pub prediction_bytes: u64,
    pub dem_sha256: String,
    pub detectors_sha256: String,
    pub predictions_sha256: String,
}

pub fn run_replay(options: &ReplayOptions) -> Result<ReplayStats, String> {
    validate_options(options)?;

    let dem_bytes = fs::read(&options.dem)
        .map_err(|error| format!("failed to read DEM {}: {error}", options.dem.display()))?;
    let dem_text = std::str::from_utf8(&dem_bytes)
        .map_err(|error| format!("DEM {} is not UTF-8: {error}", options.dem.display()))?;
    let dem = DetectorErrorModel::parse(dem_text)
        .map_err(|error| format!("failed to parse DEM {}: {error}", options.dem.display()))?;
    let num_detectors = dem.effective_num_detectors();
    let num_observables = dem.num_observables();
    let detector_row_bytes = num_detectors.div_ceil(8);
    let prediction_row_bytes = num_observables.div_ceil(8);

    let detector_bytes = fs::metadata(&options.dets)
        .map_err(|error| {
            format!(
                "failed to stat detectors {}: {error}",
                options.dets.display()
            )
        })?
        .len();
    let num_shots = infer_num_shots(detector_bytes, detector_row_bytes, options.shots)?;

    let config_text = match &options.decoder_config {
        Some(path) => fs::read_to_string(path).map_err(|error| {
            format!("failed to read decoder config {}: {error}", path.display())
        })?,
        None => String::new(),
    };
    let (decoder, normalized_config) = build_decoder(&options.decoder, &config_text)?;
    let compile_started = Instant::now();
    let compiled = decoder
        .compile_for_dem(&dem)
        .map_err(|error| format!("failed to compile {} decoder: {error}", options.decoder))?;
    let compile_seconds = compile_started.elapsed().as_secs_f64();

    let prediction_parent = parent_dir(&options.predictions_out);
    let stats_parent = parent_dir(&options.stats_out);
    fs::create_dir_all(prediction_parent).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            prediction_parent.display()
        )
    })?;
    fs::create_dir_all(stats_parent).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            stats_parent.display()
        )
    })?;

    let mut prediction_temp = tempfile::NamedTempFile::new_in(prediction_parent)
        .map_err(|error| format!("failed to create prediction temp file: {error}"))?;
    let mut detector_reader = BufReader::new(File::open(&options.dets).map_err(|error| {
        format!(
            "failed to open detectors {}: {error}",
            options.dets.display()
        )
    })?);
    let mut prediction_writer = BufWriter::new(prediction_temp.as_file_mut());
    let mut detector_hasher = Sha256::new();
    let mut prediction_hasher = Sha256::new();
    let mut batches = 0usize;
    let mut shots_completed = 0usize;
    let mut decode_seconds = 0.0f64;

    while shots_completed < num_shots {
        let batch_shots = options.batch_size.min(num_shots - shots_completed);
        let batch_bytes = batch_shots
            .checked_mul(detector_row_bytes)
            .ok_or_else(|| "detector batch size overflow".to_string())?;
        let mut detector_batch = vec![0u8; batch_bytes];
        detector_reader
            .read_exact(&mut detector_batch)
            .map_err(|error| {
                format!("failed to read detector batch at shot {shots_completed}: {error}")
            })?;
        validate_padding(
            &detector_batch,
            batch_shots,
            num_detectors,
            "detector input",
            shots_completed,
        )?;
        detector_hasher.update(&detector_batch);

        let decode_started = Instant::now();
        let predictions = compiled
            .decode_shots_bit_packed(&detector_batch, batch_shots, num_detectors, num_observables)
            .map_err(|error| format!("decoder failed at shot {shots_completed}: {error}"))?;
        decode_seconds += decode_started.elapsed().as_secs_f64();
        let expected = batch_shots
            .checked_mul(prediction_row_bytes)
            .ok_or_else(|| "prediction batch size overflow".to_string())?;
        if predictions.len() != expected {
            return Err(format!(
                "decoder returned {} prediction bytes, expected {expected}",
                predictions.len()
            ));
        }
        validate_padding(
            &predictions,
            batch_shots,
            num_observables,
            "decoder predictions",
            shots_completed,
        )?;
        prediction_hasher.update(&predictions);
        prediction_writer
            .write_all(&predictions)
            .map_err(|error| format!("failed to write predictions: {error}"))?;
        shots_completed += batch_shots;
        batches += 1;
    }
    let mut trailing = [0u8; 1];
    if detector_reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to finish detector input: {error}"))?
        != 0
    {
        return Err("detector input changed while replaying or contains trailing bytes".into());
    }
    prediction_writer
        .flush()
        .map_err(|error| format!("failed to flush predictions: {error}"))?;
    drop(prediction_writer);

    let prediction_bytes = u64::try_from(num_shots)
        .ok()
        .and_then(|shots| shots.checked_mul(prediction_row_bytes as u64))
        .ok_or_else(|| "prediction byte count overflow".to_string())?;
    let stats = ReplayStats {
        schema_version: 1,
        decoder: options.decoder.clone(),
        decoder_config: normalized_config,
        num_shots,
        num_detectors,
        num_observables,
        batch_size: options.batch_size,
        batches,
        compile_seconds,
        decode_seconds,
        shots_per_second: if decode_seconds > 0.0 {
            num_shots as f64 / decode_seconds
        } else {
            0.0
        },
        detector_bytes,
        prediction_bytes,
        dem_sha256: sha256_hex(&dem_bytes),
        detectors_sha256: digest_hex(detector_hasher.finalize()),
        predictions_sha256: digest_hex(prediction_hasher.finalize()),
    };

    let mut stats_temp = tempfile::NamedTempFile::new_in(stats_parent)
        .map_err(|error| format!("failed to create stats temp file: {error}"))?;
    serde_json::to_writer_pretty(stats_temp.as_file_mut(), &stats)
        .map_err(|error| format!("failed to serialize replay stats: {error}"))?;
    stats_temp
        .as_file_mut()
        .write_all(b"\n")
        .map_err(|error| format!("failed to finish replay stats: {error}"))?;
    stats_temp
        .as_file_mut()
        .sync_all()
        .map_err(|error| format!("failed to sync replay stats: {error}"))?;
    prediction_temp
        .as_file()
        .sync_all()
        .map_err(|error| format!("failed to sync predictions: {error}"))?;

    prediction_temp
        .persist(&options.predictions_out)
        .map_err(|error| format!("failed to install predictions: {}", error.error))?;
    stats_temp
        .persist(&options.stats_out)
        .map_err(|error| format!("failed to install replay stats: {}", error.error))?;
    Ok(stats)
}

fn validate_options(options: &ReplayOptions) -> Result<(), String> {
    if options.batch_size == 0 {
        return Err("batch_size must be positive".into());
    }
    let mut paths = vec![
        (&options.dem, "DEM input"),
        (&options.dets, "detector input"),
        (&options.predictions_out, "prediction output"),
        (&options.stats_out, "stats output"),
    ];
    if let Some(config) = &options.decoder_config {
        paths.push((config, "decoder config input"));
    }
    for left in 0..paths.len() {
        for right in left + 1..paths.len() {
            if same_path(paths[left].0, paths[right].0)? {
                return Err(format!(
                    "{} and {} must use different paths",
                    paths[left].1, paths[right].1
                ));
            }
        }
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> Result<bool, String> {
    if left == right {
        return Ok(true);
    }
    if left.exists() && right.exists() {
        let left = fs::canonicalize(left).map_err(|error| error.to_string())?;
        let right = fs::canonicalize(right).map_err(|error| error.to_string())?;
        return Ok(left == right);
    }
    Ok(lexical_absolute(left)? == lexical_absolute(right)?)
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn infer_num_shots(
    detector_bytes: u64,
    row_bytes: usize,
    requested: Option<usize>,
) -> Result<usize, String> {
    if row_bytes == 0 {
        if detector_bytes != 0 {
            return Err(format!(
                "zero-detector DEM requires an empty detector input, got {detector_bytes} bytes"
            ));
        }
        return requested
            .ok_or_else(|| "--shots is required when the DEM declares zero detectors".to_string());
    }
    let row_bytes = row_bytes as u64;
    if detector_bytes % row_bytes != 0 {
        return Err(format!(
            "detector input has {detector_bytes} bytes, not divisible by row width {row_bytes}"
        ));
    }
    let inferred = usize::try_from(detector_bytes / row_bytes)
        .map_err(|_| "detector shot count exceeds usize".to_string())?;
    if let Some(requested) = requested {
        if requested != inferred {
            return Err(format!(
                "--shots={requested} does not match {inferred} rows in detector input"
            ));
        }
    }
    Ok(inferred)
}

fn validate_padding(
    bytes: &[u8],
    shots: usize,
    bits_per_shot: usize,
    label: &str,
    shot_offset: usize,
) -> Result<(), String> {
    let remainder = bits_per_shot % 8;
    if remainder == 0 || bits_per_shot == 0 {
        return Ok(());
    }
    let row_bytes = bits_per_shot.div_ceil(8);
    let padding_mask = !((1u8 << remainder) - 1);
    for shot in 0..shots {
        let byte = bytes[shot * row_bytes + row_bytes - 1];
        if byte & padding_mask != 0 {
            return Err(format!(
                "{label} has non-zero b8 padding bits at shot {}",
                shot_offset + shot
            ));
        }
    }
    Ok(())
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest_hex(Sha256::digest(bytes))
}

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(feature = "rbposd-runner")]
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RbposdConfigFile {
    bp_method: Option<String>,
    bp_schedule: Option<String>,
    max_bp_iterations: Option<usize>,
    early_stop: Option<bool>,
    osd_method: Option<String>,
    osd_order: Option<usize>,
}

#[cfg(feature = "rbposd-runner")]
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RbplsdConfigFile {
    bp_method: Option<String>,
    bp_schedule: Option<String>,
    max_bp_iterations: Option<usize>,
    early_stop: Option<bool>,
    lsd_method: Option<String>,
    lsd_order: Option<usize>,
}

#[cfg(feature = "rmatching-runner")]
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RmatchingConfigFile {}

#[cfg(feature = "ilp-runner")]
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RilpqecConfigFile {
    backend: Option<String>,
    time_limit_s: Option<f64>,
    mip_gap: Option<f64>,
    threads: Option<u32>,
    verbose: Option<bool>,
}

fn build_decoder(name: &str, config: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    match name {
        "rbposd" => build_rbposd(config),
        "rbplsd" => build_rbplsd(config),
        "rmatching" => build_rmatching(config),
        "rilpqec" => build_rilpqec(config),
        other => Err(format!(
            "unknown decoder {other:?}; expected rbposd, rbplsd, rmatching, or rilpqec"
        )),
    }
}

#[cfg(any(
    feature = "rbposd-runner",
    feature = "rmatching-runner",
    feature = "ilp-runner"
))]
fn parse_config<T>(text: &str, decoder: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if text.trim().is_empty() {
        Ok(T::default())
    } else {
        toml::from_str(text).map_err(|error| format!("invalid {decoder} decoder config: {error}"))
    }
}

#[cfg(feature = "rbposd-runner")]
fn bp_config(
    method: Option<String>,
    schedule: Option<String>,
    iterations: Option<usize>,
    early_stop: Option<bool>,
) -> Result<(rbposd::DecoderConfig, Value), String> {
    use rbposd::{BpVariant, DecoderConfig, Schedule};
    let mut config = DecoderConfig::default();
    let method = method.unwrap_or_else(|| "minimum_sum".into());
    config.bp_variant = match method.as_str() {
        "minimum_sum" => BpVariant::MinimumSum,
        "product_sum" => BpVariant::ProductSum,
        _ => return Err(format!("unknown bp_method {method:?}")),
    };
    let schedule = schedule.unwrap_or_else(|| "parallel".into());
    config.schedule = match schedule.as_str() {
        "parallel" => Schedule::Parallel,
        "serial" => Schedule::Serial,
        _ => return Err(format!("unknown bp_schedule {schedule:?}")),
    };
    config.max_bp_iterations = iterations.unwrap_or(config.max_bp_iterations);
    config.early_stop = early_stop.unwrap_or(config.early_stop);
    Ok((
        config,
        json!({
            "bp_method": method,
            "bp_schedule": schedule,
            "max_bp_iterations": config.max_bp_iterations,
            "early_stop": config.early_stop,
        }),
    ))
}

#[cfg(feature = "rbposd-runner")]
fn build_rbposd(text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    use crate::decode::RbposdDemDecoder;
    use rbposd::OsdVariant;
    let file: RbposdConfigFile = parse_config(text, "rbposd")?;
    let (mut config, mut normalized) = bp_config(
        file.bp_method,
        file.bp_schedule,
        file.max_bp_iterations,
        file.early_stop,
    )?;
    let requested_method = file.osd_method.unwrap_or_else(|| "osd0".into());
    let (variant, method) = match requested_method.as_str() {
        "osd0" => (OsdVariant::Osd0, "osd0"),
        "combination_sweep" | "legacy_combination_sweep" => (
            OsdVariant::LegacyCombinationSweep,
            "legacy_combination_sweep",
        ),
        "ldpc_osd_cs" | "osd_cs" => (OsdVariant::LdpcCombinationSweep, "ldpc_osd_cs"),
        _ => return Err(format!("unknown osd_method {requested_method:?}")),
    };
    config.osd_variant = variant;
    config.osd_order = file.osd_order.unwrap_or(config.osd_order);
    if config.osd_variant == OsdVariant::Osd0 && config.osd_order != 0 {
        return Err("osd_method=\"osd0\" requires osd_order=0".into());
    }
    normalized["osd_method"] = json!(method);
    normalized["osd_order"] = json!(config.osd_order);
    Ok((Box::new(RbposdDemDecoder::new(config)), normalized))
}

#[cfg(not(feature = "rbposd-runner"))]
fn build_rbposd(_text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    Err("decoder rbposd requires Cargo feature 'rbposd-runner'".into())
}

#[cfg(feature = "rbposd-runner")]
fn build_rbplsd(text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    use crate::decode::RbposdLsdDemDecoder;
    use rbposd::{LsdConfig, LsdMethod};
    let file: RbplsdConfigFile = parse_config(text, "rbplsd")?;
    let (bp, mut normalized) = bp_config(
        file.bp_method,
        file.bp_schedule,
        file.max_bp_iterations,
        file.early_stop,
    )?;
    let method = file
        .lsd_method
        .unwrap_or_else(|| "localized_statistics".into());
    if method != "localized_statistics" {
        return Err(format!("unknown lsd_method {method:?}"));
    }
    let order = file.lsd_order.unwrap_or(0);
    if order > 1 {
        return Err(format!("lsd_order must be 0 or 1, got {order}"));
    }
    normalized["lsd_method"] = json!(method);
    normalized["lsd_order"] = json!(order);
    Ok((
        Box::new(RbposdLsdDemDecoder::with_bp_config(
            LsdConfig {
                method: LsdMethod::LocalizedStatistics,
                lsd_order: order,
            },
            bp,
        )),
        normalized,
    ))
}

#[cfg(not(feature = "rbposd-runner"))]
fn build_rbplsd(_text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    Err("decoder rbplsd requires Cargo feature 'rbposd-runner'".into())
}

#[cfg(feature = "rmatching-runner")]
fn build_rmatching(text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    use crate::decode::RmatchingDemDecoder;
    let _: RmatchingConfigFile = parse_config(text, "rmatching")?;
    Ok((Box::new(RmatchingDemDecoder), json!({})))
}

#[cfg(not(feature = "rmatching-runner"))]
fn build_rmatching(_text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    Err("decoder rmatching requires Cargo feature 'rmatching-runner'".into())
}

#[cfg(feature = "ilp-runner")]
fn build_rilpqec(text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    use crate::decode::IlpDemDecoder;
    use qec_ilp_core::BackendKind;
    use rilpqec::IlpDecoderConfig;
    let file: RilpqecConfigFile = parse_config(text, "rilpqec")?;
    let backend = file.backend.unwrap_or_else(|| "auto".into());
    let mut config = IlpDecoderConfig::default();
    config.backend.kind = match backend.as_str() {
        "auto" => BackendKind::Auto,
        "highs" => BackendKind::Highs,
        "gurobi" => BackendKind::Gurobi,
        _ => return Err(format!("unknown rilpqec backend {backend:?}")),
    };
    if file
        .time_limit_s
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err("time_limit_s must be positive and finite".into());
    }
    if file
        .mip_gap
        .is_some_and(|value| !value.is_finite() || !(0.0..1.0).contains(&value))
    {
        return Err("mip_gap must be finite and in [0, 1)".into());
    }
    if file.threads == Some(0) {
        return Err("threads must be positive".into());
    }
    config.backend.time_limit_seconds = file.time_limit_s;
    config.backend.mip_gap = file.mip_gap;
    config.backend.threads = file.threads;
    config.backend.verbose = file.verbose.unwrap_or(false);
    let normalized = json!({
        "backend": backend,
        "time_limit_s": config.backend.time_limit_seconds,
        "mip_gap": config.backend.mip_gap,
        "threads": config.backend.threads,
        "verbose": config.backend.verbose,
    });
    Ok((Box::new(IlpDemDecoder::new(config)), normalized))
}

#[cfg(not(feature = "ilp-runner"))]
fn build_rilpqec(_text: &str) -> Result<(Box<dyn Decoder>, Value), String> {
    Err("decoder rilpqec requires Cargo feature 'ilp-runner'".into())
}

#[cfg(test)]
mod tests {
    use super::build_decoder;

    #[test]
    fn rbposd_and_rbplsd_configs_are_normalized() {
        let (_, osd) = build_decoder(
            "rbposd",
            "bp_method = \"product_sum\"\nbp_schedule = \"serial\"\nosd_method = \"ldpc_osd_cs\"\nosd_order = 3\n",
        )
        .unwrap();
        assert_eq!(osd["bp_method"], "product_sum");
        assert_eq!(osd["bp_schedule"], "serial");
        assert_eq!(osd["osd_method"], "ldpc_osd_cs");
        assert_eq!(osd["osd_order"], 3);

        let (_, lsd) = build_decoder("rbplsd", "lsd_order = 1\n").unwrap();
        assert_eq!(lsd["lsd_method"], "localized_statistics");
        assert_eq!(lsd["lsd_order"], 1);
    }

    #[test]
    fn decoder_configs_reject_invalid_values_and_unknown_names() {
        assert!(
            build_decoder("rbplsd", "lsd_order = 2\n")
                .err()
                .unwrap()
                .contains("0 or 1")
        );
        assert!(
            build_decoder("rbposd", "osd_method = \"osd0\"\nosd_order = 1\n")
                .err()
                .unwrap()
                .contains("requires osd_order=0")
        );
        assert!(
            build_decoder("rilpqec", "mip_gap = 1.0\n")
                .err()
                .unwrap()
                .contains("[0, 1)")
        );
        assert!(
            build_decoder("mystery", "")
                .err()
                .unwrap()
                .contains("unknown decoder")
        );
    }
}
