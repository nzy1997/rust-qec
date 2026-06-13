use rstim::codegen::NoiseParams;
use rstim::codegen::css::{
    CssCheckMatrices, CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis, css_memory,
};

#[test]
fn css_memory_rejects_zero_rounds() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0]],
            hz: vec![],
            num_data_qubits: 1,
        },
        rounds: 0,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();
    assert!(err.contains("rounds must be >= 1"), "error was: {err}");
}
