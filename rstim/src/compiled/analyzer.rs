use crate::compiled::{choose_analyzer_path, CompiledBlock, CompiledCircuit, CompiledPathDecision};
use crate::dem::DetectorErrorModel;
use crate::error_analyzer::{AnalyzeBackend, AnalyzeOptions, ErrorAnalyzer};

pub fn analyze_compiled_circuit(
    compiled: &CompiledCircuit,
    options: AnalyzeOptions,
    decompose_channel_errors: bool,
) -> Result<DetectorErrorModel, String> {
    match choose_analyzer_path(compiled) {
        CompiledPathDecision::FastPath => {}
        CompiledPathDecision::Fallback(reason) => return Err(reason.to_string()),
    }

    let [CompiledBlock::Repeat(region)] = compiled.blocks.as_slice() else {
        return Err(
            "compiled analyzer currently supports only a single top-level repeat region"
                .to_string(),
        );
    };

    let flattened_options = AnalyzeOptions {
        backend: AnalyzeBackend::Flattened,
        ..options
    };
    let mut body_dem = if decompose_channel_errors {
        ErrorAnalyzer::circuit_to_dem_with_options_decomposed(&region.body_source, flattened_options)
    } else {
        ErrorAnalyzer::circuit_to_dem_with_options(&region.body_source, flattened_options)
    }?;
    if region.detector_span > 0 {
        body_dem.add_shift_detectors(region.detector_span, Vec::new());
    }

    let mut dem = DetectorErrorModel::new();
    dem.set_min_counts(
        region.detector_span * (region.count as usize),
        body_dem.num_observables(),
    );
    dem.add_repeat(region.count, body_dem);
    Ok(dem)
}
