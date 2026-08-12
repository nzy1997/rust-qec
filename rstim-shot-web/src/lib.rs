use rstim::interactive_shot::{
    EditableShot, ExpansionLimits, NoiseEventId, NoiseOutcome, Pauli, ViewSnapshot,
};
use wasm_bindgen::prelude::*;

/// Target-independent state holder used by both the browser binding and native contract tests.
pub struct ShotSessionCore {
    shot: EditableShot,
}

impl ShotSessionCore {
    pub fn open(source: &str, seed: u64, limits: ExpansionLimits) -> Result<Self, String> {
        Ok(Self {
            shot: EditableShot::open(source, limits, seed)?,
        })
    }

    pub fn sample(&mut self, seed: u64) -> Result<(), String> {
        self.shot.sample(seed)
    }

    pub fn clear(&mut self, seed: u64) -> Result<(), String> {
        self.shot.clear(seed)
    }

    pub fn set_noise(&mut self, event_id: &str, outcome: &str) -> Result<(), String> {
        let event_id = NoiseEventId::decode(event_id)?;
        self.shot.set_noise(&event_id, parse_outcome(outcome)?)
    }

    pub fn restore_noise(&mut self, event_id: &str) -> Result<(), String> {
        let event_id = NoiseEventId::decode(event_id)?;
        self.shot.restore_noise(&event_id)
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.shot.undo()
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.shot.redo()
    }

    pub fn snapshot(&self) -> Result<ViewSnapshot, String> {
        self.shot.view_snapshot()
    }
}

fn parse_outcome(value: &str) -> Result<NoiseOutcome, String> {
    let normalized = value.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "I" | "IDENTITY" | "NONE" => Ok(NoiseOutcome::Identity),
        "X" => Ok(NoiseOutcome::X),
        "Y" => Ok(NoiseOutcome::Y),
        "Z" => Ok(NoiseOutcome::Z),
        "LOST" | "LOSS" | "L" => Ok(NoiseOutcome::Lost),
        pair if pair.len() == 2 => {
            let mut chars = pair.chars();
            Ok(NoiseOutcome::PauliPair {
                first: parse_pauli(chars.next().unwrap())?,
                second: parse_pauli(chars.next().unwrap())?,
            })
        }
        other => Err(format!("unknown noise outcome {other:?}")),
    }
}

fn parse_pauli(value: char) -> Result<Pauli, String> {
    match value {
        'I' => Ok(Pauli::I),
        'X' => Ok(Pauli::X),
        'Y' => Ok(Pauli::Y),
        'Z' => Ok(Pauli::Z),
        other => Err(format!("unknown Pauli {other:?}")),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_session_contract_round_trips_browser_ids() {
        let mut session = ShotSessionCore::open(
            "REPEAT 2 {\n X_ERROR(0) 0\n M 0\n DETECTOR rec[-1]\n}\n",
            7,
            ExpansionLimits::default(),
        )
        .unwrap();
        let initial = session.snapshot().unwrap();
        let second_id = initial.shot.result.noise_events[1].id.clone();

        session.set_noise(&second_id, "X").unwrap();
        let edited = session.snapshot().unwrap();
        assert!(edited.shot.result.detectors[1].flipped);
        assert!(edited.svg.contains(&second_id));

        assert!(session.undo().unwrap());
        assert!(!session.snapshot().unwrap().shot.result.detectors[1].flipped);
        assert!(session.redo().unwrap());
        assert!(session.snapshot().unwrap().shot.result.detectors[1].flipped);
    }

    #[test]
    fn explicit_limits_reject_oversized_expansion_before_opening() {
        let error = ShotSessionCore::open(
            "REPEAT 3 {\n X_ERROR(0.1) 0\n}\n",
            1,
            ExpansionLimits {
                max_operations: 2,
                ..ExpansionLimits::default()
            },
        )
        .err()
        .unwrap();
        assert!(error.contains("exceeds limit 2"), "{error}");
    }
}
