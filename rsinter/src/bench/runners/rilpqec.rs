use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::run_decoder_point;
use crate::decode::IlpDemDecoder;

pub struct RilpqecRunner;

impl RustBenchRunner for RilpqecRunner {
    fn name(&self) -> &'static str {
        "rilpqec"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        let decoder = IlpDemDecoder::default();
        run_decoder_point(self.name(), &decoder, point, ctx)
    }
}
