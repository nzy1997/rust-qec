use std::collections::BTreeMap;

use rbposd::DecoderConfig;
use toml::Value;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, PairMapExt, ParamMap};
use crate::bench::runners::params::{optional_bool, optional_usize};
use crate::bench::runners::run_decoder_point;
use crate::decode::RbposdDemDecoder;

pub struct RbposdRunner;

struct RbposdRunnerParams {
    config: DecoderConfig,
    normalized: ParamMap,
}

impl RbposdRunnerParams {
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut config = DecoderConfig::default();
        let bp_iters = optional_usize(params, "bp_iters")?;
        let max_bp_iterations = optional_usize(params, "max_bp_iterations")?;
        let bp_iters = match (bp_iters, max_bp_iterations) {
            (Some(_), Some(_)) => {
                return Err(
                    "rbposd params must not set both bp_iters and max_bp_iterations".into(),
                );
            }
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => config.max_bp_iterations,
        };
        config.max_bp_iterations = bp_iters;
        config.early_stop = optional_bool(params, "early_stop")?.unwrap_or(config.early_stop);
        config.osd_order = optional_usize(params, "osd_order")?.unwrap_or(config.osd_order);

        Ok(Self {
            config,
            normalized: ParamMap::from_pairs([
                ("bp_iters", serde_json::json!(config.max_bp_iterations)),
                ("early_stop", serde_json::json!(config.early_stop)),
                ("osd_order", serde_json::json!(config.osd_order)),
            ]),
        })
    }
}

impl RustBenchRunner for RbposdRunner {
    fn name(&self) -> &'static str {
        "rbposd"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RbposdRunnerParams::parse(&point.decoder_params)?;
        let decoder = RbposdDemDecoder::new(params.config);
        run_decoder_point(self.name(), &decoder, point, ctx, &params.normalized)
    }
}
