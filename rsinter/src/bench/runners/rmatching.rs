use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::run_decoder_point;
use crate::decode::RmatchingDemDecoder;

pub struct RmatchingRunner;

impl RustBenchRunner for RmatchingRunner {
    fn name(&self) -> &'static str {
        "rmatching"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let decoder = RmatchingDemDecoder;
        run_decoder_point(self.name(), &decoder, point, ctx)
    }
}
