use std::collections::BTreeMap;

use rilpqec::{BackendKind, IlpDecoderConfig};
use toml::Value;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, ParamMap};
use crate::bench::runners::params::{
    optional_bool, optional_f64, optional_positive_u32, optional_string,
};
use crate::bench::runners::{plan_decoder_point_identity, run_decoder_point};
use crate::decode::IlpDemDecoder;

pub struct RilpqecRunner;

struct RilpqecRunnerParams {
    config: IlpDecoderConfig,
    normalized: ParamMap,
}

impl RilpqecRunnerParams {
    fn parse(params: &BTreeMap<String, Value>) -> Result<Self, String> {
        let mut config = IlpDecoderConfig::default();
        let backend_name = optional_string(params, "backend")?.unwrap_or_else(|| "auto".into());
        config.backend.kind = match backend_name.as_str() {
            "auto" => BackendKind::Auto,
            "highs" => BackendKind::Highs,
            "gurobi" => BackendKind::Gurobi,
            other => return Err(format!("unknown rilpqec backend: {other}")),
        };

        config.backend.time_limit_seconds = optional_f64(params, "time_limit_s")?;
        if let Some(limit) = config.backend.time_limit_seconds {
            if !limit.is_finite() || limit <= 0.0 {
                return Err("time_limit_s must be positive".into());
            }
        }

        config.backend.mip_gap = optional_f64(params, "mip_gap")?;
        if let Some(gap) = config.backend.mip_gap {
            if !gap.is_finite() || !(0.0..1.0).contains(&gap) {
                return Err("mip_gap must be in [0, 1)".into());
            }
        }

        config.backend.threads = optional_positive_u32(params, "threads")?;
        config.backend.verbose = optional_bool(params, "verbose")?.unwrap_or(false);

        let mut normalized = ParamMap::new();
        normalized.insert("backend".into(), serde_json::json!(backend_name));
        if let Some(limit) = config.backend.time_limit_seconds {
            normalized.insert("time_limit_s".into(), serde_json::json!(limit));
        }
        if let Some(gap) = config.backend.mip_gap {
            normalized.insert("mip_gap".into(), serde_json::json!(gap));
        }
        if let Some(threads) = config.backend.threads {
            normalized.insert("threads".into(), serde_json::json!(threads));
        }
        normalized.insert("verbose".into(), serde_json::json!(config.backend.verbose));

        Ok(Self { config, normalized })
    }
}

impl RustBenchRunner for RilpqecRunner {
    fn name(&self) -> &'static str {
        "rilpqec"
    }

    fn preflight_point(&self, point: &BenchCasePoint) -> Result<(), String> {
        RilpqecRunnerParams::parse(&point.decoder_params).map(|_| ())
    }

    fn plan_point_identity(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<String, String> {
        let params = RilpqecRunnerParams::parse(&point.decoder_params)?;
        plan_decoder_point_identity(self.name(), point, ctx, &params.normalized)
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let params = RilpqecRunnerParams::parse(&point.decoder_params)?;
        let decoder = IlpDemDecoder::new(params.config);
        run_decoder_point(self.name(), &decoder, point, ctx, &params.normalized)
    }
}
