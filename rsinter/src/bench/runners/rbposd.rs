use std::collections::BTreeMap;

use rbposd::{DecoderConfig, LsdConfig, LsdMethod};
use toml::Value;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, PairMapExt, ParamMap};
use crate::bench::runners::params::{optional_bool, optional_string, optional_usize};
use crate::bench::runners::{DemBuildMode, run_decoder_point_with_dem_mode};
use crate::decode::{RbposdDemDecoder, RbposdLsdDemDecoder};

pub struct RbposdRunner;

struct RbposdRunnerParams {
    bp_config: DecoderConfig,
    decoder: RbposdDecoderFamily,
    normalized: ParamMap,
}

#[allow(dead_code)]
enum RbposdDecoderFamily {
    Osd {
        osd_method: String,
        osd_order: usize,
    },
    Lsd {
        lsd_method: String,
        lsd_order: usize,
        lsd_config: LsdConfig,
    },
}

impl RbposdRunnerParams {
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut bp_config = DecoderConfig::default();
        let bp_algorithm =
            optional_string(params, "bp_algorithm")?.unwrap_or_else(|| "min_sum".to_string());
        if bp_algorithm != "min_sum" {
            return Err(format!(
                "rbposd bp_algorithm must be \"min_sum\", got \"{bp_algorithm}\""
            ));
        }

        let bp_iters = optional_usize(params, "bp_iters")?;
        let max_bp_iterations = optional_usize(params, "max_bp_iterations")?;
        let bp_iters = match (bp_iters, max_bp_iterations) {
            (Some(_), Some(_)) => {
                return Err(
                    "rbposd params must not set both bp_iters and max_bp_iterations".into(),
                );
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => bp_config.max_bp_iterations,
        };
        bp_config.max_bp_iterations = bp_iters;
        bp_config.early_stop = optional_bool(params, "early_stop")?.unwrap_or(bp_config.early_stop);

        let has_lsd_params = params.contains_key("lsd_method") || params.contains_key("lsd_order");
        let has_osd_params = params.contains_key("osd_method") || params.contains_key("osd_order");
        if has_lsd_params && has_osd_params {
            return Err("rbposd params must not mix OSD and LSD decoder params".into());
        }

        if has_lsd_params {
            let lsd_method = optional_string(params, "lsd_method")?
                .unwrap_or_else(|| "localized_statistics".to_string());
            if lsd_method != "localized_statistics" {
                return Err(format!(
                    "rbposd lsd_method must be \"localized_statistics\", got \"{lsd_method}\""
                ));
            }
            let lsd_order =
                optional_usize(params, "lsd_order")?.unwrap_or(LsdConfig::default().lsd_order);
            if lsd_order > 1 {
                return Err(format!("rbposd lsd_order must be <= 1, got {lsd_order}"));
            }
            let lsd_config = LsdConfig {
                method: LsdMethod::LocalizedStatistics,
                lsd_order,
            };

            return Ok(Self {
                bp_config,
                decoder: RbposdDecoderFamily::Lsd {
                    lsd_method: lsd_method.clone(),
                    lsd_order,
                    lsd_config,
                },
                normalized: ParamMap::from_pairs([
                    ("bp_algorithm", serde_json::json!(bp_algorithm)),
                    ("bp_iters", serde_json::json!(bp_config.max_bp_iterations)),
                    ("early_stop", serde_json::json!(bp_config.early_stop)),
                    ("input_type", serde_json::json!("surface")),
                    ("lsd_method", serde_json::json!(lsd_method)),
                    ("lsd_order", serde_json::json!(lsd_order)),
                ]),
            });
        }

        let osd_method = optional_string(params, "osd_method")?
            .unwrap_or_else(|| "combination_sweep".to_string());
        if osd_method != "combination_sweep" {
            return Err(format!(
                "rbposd osd_method must be \"combination_sweep\", got \"{osd_method}\""
            ));
        }
        bp_config.osd_order = optional_usize(params, "osd_order")?.unwrap_or(bp_config.osd_order);

        Ok(Self {
            bp_config,
            decoder: RbposdDecoderFamily::Osd {
                osd_method: osd_method.clone(),
                osd_order: bp_config.osd_order,
            },
            normalized: ParamMap::from_pairs([
                ("bp_algorithm", serde_json::json!(bp_algorithm)),
                ("bp_iters", serde_json::json!(bp_config.max_bp_iterations)),
                ("early_stop", serde_json::json!(bp_config.early_stop)),
                ("osd_method", serde_json::json!(osd_method)),
                ("osd_order", serde_json::json!(bp_config.osd_order)),
            ]),
        })
    }
}

impl RustBenchRunner for RbposdRunner {
    fn name(&self) -> &'static str {
        "rbposd"
    }

    fn preflight_point(&self, point: &BenchCasePoint) -> Result<(), String> {
        RbposdRunnerParams::parse(&point.decoder_params).map(|_| ())
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RbposdRunnerParams::parse(&point.decoder_params)?;
        match &params.decoder {
            RbposdDecoderFamily::Osd { .. } => {
                let decoder = RbposdDemDecoder::new(params.bp_config);
                run_decoder_point_with_dem_mode(
                    self.name(),
                    &decoder,
                    point,
                    ctx,
                    &params.normalized,
                    DemBuildMode::Raw,
                )
            }
            RbposdDecoderFamily::Lsd { lsd_config, .. } => {
                let decoder = RbposdLsdDemDecoder::with_bp_config(*lsd_config, params.bp_config);
                run_decoder_point_with_dem_mode(
                    self.name(),
                    &decoder,
                    point,
                    ctx,
                    &params.normalized,
                    DemBuildMode::Raw,
                )
            }
        }
    }
}
