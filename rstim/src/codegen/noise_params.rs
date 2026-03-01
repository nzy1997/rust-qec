/// Per-channel noise parameters for circuit generation.
#[derive(Debug, Clone, Copy)]
pub struct NoiseParams {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
}

impl NoiseParams {
    pub fn uniform(noise: f64) -> Self {
        NoiseParams {
            before_round_data_depolarization: noise,
            after_clifford_depolarization: noise,
            before_measure_flip_probability: noise,
            after_reset_flip_probability: noise,
        }
    }
    pub fn none() -> Self { Self::uniform(0.0) }
}

impl Default for NoiseParams {
    fn default() -> Self { Self::none() }
}
