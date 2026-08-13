use crate::ShotSessionCore;
use rstim::interactive_shot::ExpansionLimits;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = ShotSession)]
pub struct WasmShotSession {
    core: ShotSessionCore,
}

#[wasm_bindgen(js_class = ShotSession)]
impl WasmShotSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, seed_low: u32, seed_high: u32) -> Result<WasmShotSession, JsValue> {
        let seed = seed_from_halves(seed_low, seed_high);
        let core =
            ShotSessionCore::open(source, seed, ExpansionLimits::default()).map_err(js_error)?;
        Ok(Self { core })
    }

    #[wasm_bindgen(js_name = withLimits)]
    pub fn with_limits(
        source: &str,
        seed_low: u32,
        seed_high: u32,
        max_expanded_operations: u32,
        max_noise_events: u32,
        max_measurements: u32,
        max_svg_nodes: u32,
    ) -> Result<WasmShotSession, JsValue> {
        let limits = ExpansionLimits {
            max_operations: u64::from(max_expanded_operations),
            max_noise_events: u64::from(max_noise_events),
            max_measurements: u64::from(max_measurements),
            max_svg_nodes: u64::from(max_svg_nodes),
            max_qubits: ExpansionLimits::default().max_qubits,
        };
        let core = ShotSessionCore::open(source, seed_from_halves(seed_low, seed_high), limits)
            .map_err(js_error)?;
        Ok(Self { core })
    }

    pub fn sample(&mut self, seed_low: u32, seed_high: u32) -> Result<String, JsValue> {
        self.core
            .sample(seed_from_halves(seed_low, seed_high))
            .map_err(js_error)?;
        self.snapshot()
    }

    pub fn clear(&mut self, seed_low: u32, seed_high: u32) -> Result<String, JsValue> {
        self.core
            .clear(seed_from_halves(seed_low, seed_high))
            .map_err(js_error)?;
        self.snapshot()
    }

    #[wasm_bindgen(js_name = setNoise)]
    pub fn set_noise(&mut self, event_id: &str, outcome: &str) -> Result<String, JsValue> {
        self.core.set_noise(event_id, outcome).map_err(js_error)?;
        self.snapshot()
    }

    #[wasm_bindgen(js_name = restoreNoise)]
    pub fn restore_noise(&mut self, event_id: &str) -> Result<String, JsValue> {
        self.core.restore_noise(event_id).map_err(js_error)?;
        self.snapshot()
    }

    pub fn undo(&mut self) -> Result<String, JsValue> {
        self.core.undo().map_err(js_error)?;
        self.snapshot()
    }

    pub fn redo(&mut self) -> Result<String, JsValue> {
        self.core.redo().map_err(js_error)?;
        self.snapshot()
    }

    pub fn snapshot(&self) -> Result<String, JsValue> {
        let snapshot = self.core.snapshot().map_err(js_error)?;
        serde_json::to_string(&snapshot).map_err(js_error)
    }
}

fn seed_from_halves(low: u32, high: u32) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}
