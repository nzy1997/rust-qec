use rstim::interactive_shot::{
    EditableShot, ExpansionLimits, NoiseEventId, NoiseOutcome, Pauli, ViewSnapshot,
};

mod wasm_bindings;

pub use wasm_bindings::WasmShotSession;

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

    #[test]
    fn session_lifecycle_forwards_sample_clear_and_restore() {
        let mut session = ShotSessionCore::open(
            "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\n",
            1,
            ExpansionLimits::default(),
        )
        .unwrap();
        let event_id = session.snapshot().unwrap().shot.result.noise_events[0]
            .id
            .clone();

        session.sample(2).unwrap();
        session.clear(3).unwrap();
        session.set_noise(&event_id, "X").unwrap();
        session.restore_noise(&event_id).unwrap();
    }

    #[test]
    fn outcome_parser_accepts_aliases_and_rejects_invalid_paulis() {
        for identity in ["I", "identity", " none "] {
            assert_eq!(parse_outcome(identity).unwrap().label(), "I");
        }
        for (text, label) in [
            ("X", "X"),
            ("Y", "Y"),
            ("Z", "Z"),
            ("lost", "L"),
            ("loss", "L"),
            ("L", "L"),
            ("II", "II"),
            ("XX", "XX"),
            ("YY", "YY"),
            ("ZZ", "ZZ"),
        ] {
            assert_eq!(parse_outcome(text).unwrap().label(), label);
        }
        assert!(parse_outcome("QX").unwrap_err().contains("unknown Pauli"));
        assert!(parse_outcome("invalid").unwrap_err().contains("unknown noise outcome"));
    }
}
