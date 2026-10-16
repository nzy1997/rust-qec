use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::{plan_decoder_point_identity, run_decoder_point};
use crate::decode::RmatchingDemDecoder;

pub struct RmatchingRunner;

impl RustBenchRunner for RmatchingRunner {
    fn name(&self) -> &'static str {
        "rmatching"
    }

    fn plan_point_identity(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<String, String> {
        plan_decoder_point_identity(
            self.name(),
            point,
            ctx,
            &crate::bench::result::ParamMap::new(),
        )
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let decoder = RmatchingDemDecoder;
        let decoder_params = crate::bench::result::ParamMap::new();
        run_decoder_point(self.name(), &decoder, point, ctx, &decoder_params)
    }
}
