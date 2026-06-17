use crate::bench::registry::{BenchCasePoint, BenchRunContext, RustBenchRunner};
use crate::bench::result::{BenchmarkResultRow, ParamMap};
use crate::bench::runners::run_decoder_point;
use crate::decode::VacuousDecoder;

pub struct PredictZeroRunner;

impl RustBenchRunner for PredictZeroRunner {
    fn name(&self) -> &'static str {
        "predict-zero"
    }

    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String> {
        run_decoder_point(self.name(), &VacuousDecoder, point, ctx, &ParamMap::new())
    }
}
