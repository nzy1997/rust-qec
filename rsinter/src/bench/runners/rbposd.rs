use rbposd::DecoderConfig;

use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::run_decoder_point;
use crate::decode::RbposdDemDecoder;

pub struct RbposdRunner;

impl RustBenchRunner for RbposdRunner {
    fn name(&self) -> &'static str {
        "rbposd"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let decoder = RbposdDemDecoder::new(DecoderConfig::default());
        run_decoder_point(self.name(), &decoder, point, ctx)
    }
}
