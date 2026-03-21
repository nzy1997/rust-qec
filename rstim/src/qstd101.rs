use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qstd101Document {
    pub standard: String,
    pub version: String,
    pub num_qubits: usize,
    pub operations: Vec<Qstd101Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Qstd101Operation {
    #[serde(rename = "gate")]
    Gate {
        gate: String,
        targets: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        controls: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        control_configs: Option<Vec<bool>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        params: Vec<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_targets: Option<Vec<Qstd101TargetRef>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<Qstd101Display>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
    },
    #[serde(rename = "tick")]
    Tick,
    #[serde(rename = "repeat")]
    Repeat {
        count: u64,
        body: Vec<Qstd101Operation>,
    },
    #[serde(rename = "qubit_coords")]
    QubitCoords {
        coords: Vec<f64>,
        targets: Vec<u32>,
    },
    #[serde(rename = "shift_coords")]
    ShiftCoords {
        delta: Vec<f64>,
    },
    #[serde(rename = "detector")]
    Detector {
        coords: Vec<f64>,
        sources: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "observable_include")]
    ObservableInclude {
        index: u32,
        sources: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "noise")]
    Noise {
        gate: String,
        params: Vec<f64>,
        raw_targets: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "annotation")]
    Annotation {
        kind: String,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Qstd101TargetRef {
    #[serde(rename = "qubit")]
    Qubit {
        index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        inverted: Option<bool>,
    },
    #[serde(rename = "rec")]
    Rec {
        offset: i32,
    },
    #[serde(rename = "pauli")]
    Pauli {
        basis: Qstd101PauliBasis,
        qubit: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        inverted: Option<bool>,
    },
    #[serde(rename = "combiner")]
    Combiner,
    #[serde(rename = "sweep")]
    Sweep {
        index: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qstd101Display {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Qstd101PauliBasis {
    X,
    Y,
    Z,
}
