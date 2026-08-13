use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::executor::{ExecOutput, Executor, InteractiveExecutionConfig};
use crate::ir::{StimInstr, StimTarget, circuit_to_string};
use crate::parser::parse_lines;
use crate::qp101::{
    Qp101Annotation, Qp101AnnotationStyle, Qp101Operation, export_qp101,
    export_qp101_with_sample_trace,
};
use crate::qp101_svg::render_svg_interactive;
use crate::sample_trace::{MeasurementComponent, SampleTrace};

const SITE_ID_VERSION: &str = "ns1";
const EVENT_ID_VERSION: &str = "ne1";
const KEYED_RANDOM_VERSION: &[u8] = b"rstim-interactive-random-v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CircuitDigest([u8; 32]);

impl CircuitDigest {
    pub fn from_instructions(instructions: &[StimInstr]) -> Self {
        let canonical = circuit_to_string(instructions);
        Self(Sha256::digest(canonical.as_bytes()).into())
    }

    pub fn to_hex(self) -> String {
        hex_bytes(&self.0)
    }

    fn parse_hex(value: &str) -> Result<Self, String> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("circuit digest must contain exactly 64 hexadecimal digits".to_string());
        }
        let mut bytes = [0_u8; 32];
        for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
            let digits = std::str::from_utf8(chunk).expect("hexadecimal input is valid UTF-8");
            bytes[index] = u8::from_str_radix(digits, 16)
                .map_err(|_| "circuit digest contains invalid hexadecimal".to_string())?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for CircuitDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NoiseSiteId {
    pub circuit_digest: CircuitDigest,
    pub op_path: Vec<usize>,
    pub target_slots: Vec<usize>,
}

impl NoiseSiteId {
    pub fn encode(&self) -> String {
        format!(
            "{SITE_ID_VERSION}:{}:{}:{}",
            self.circuit_digest,
            encode_usize_list(&self.op_path),
            encode_usize_list(&self.target_slots)
        )
    }

    pub fn decode(value: &str) -> Result<Self, String> {
        let fields: Vec<_> = value.split(':').collect();
        if fields.len() != 4 || fields[0] != SITE_ID_VERSION {
            return Err(format!("unsupported noise site id: {value}"));
        }
        Ok(Self {
            circuit_digest: CircuitDigest::parse_hex(fields[1])?,
            op_path: decode_usize_list(fields[2], "operation path")?,
            target_slots: decode_usize_list(fields[3], "target slots")?,
        })
    }
}

impl fmt::Display for NoiseSiteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.encode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NoiseEventId {
    pub site: NoiseSiteId,
    pub repeat_iterations: Vec<u64>,
}

impl NoiseEventId {
    pub fn encode(&self) -> String {
        format!(
            "{EVENT_ID_VERSION}:{}:{}:{}:{}",
            self.site.circuit_digest,
            encode_usize_list(&self.site.op_path),
            encode_usize_list(&self.site.target_slots),
            encode_u64_list(&self.repeat_iterations)
        )
    }

    pub fn decode(value: &str) -> Result<Self, String> {
        let fields: Vec<_> = value.split(':').collect();
        if fields.len() != 5 || fields[0] != EVENT_ID_VERSION {
            return Err(format!("unsupported noise event id: {value}"));
        }
        Ok(Self {
            site: NoiseSiteId {
                circuit_digest: CircuitDigest::parse_hex(fields[1])?,
                op_path: decode_usize_list(fields[2], "operation path")?,
                target_slots: decode_usize_list(fields[3], "target slots")?,
            },
            repeat_iterations: decode_u64_list(fields[4], "repeat iterations")?,
        })
    }
}

impl fmt::Display for NoiseEventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.encode())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceKind {
    NoiseOccurrence,
    NoiseBranch,
    IntrinsicMeasurement,
    MeasurementFlip,
    Herald,
    CorrelatedOccurrence,
}

impl ChoiceKind {
    fn encoding(self) -> u8 {
        match self {
            Self::NoiseOccurrence => 0,
            Self::NoiseBranch => 1,
            Self::IntrinsicMeasurement => 2,
            Self::MeasurementFlip => 3,
            Self::Herald => 4,
            Self::CorrelatedOccurrence => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RandomKey {
    pub circuit_digest: CircuitDigest,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slots: Vec<usize>,
    pub choice_kind: ChoiceKind,
    pub subchoice: u16,
}

impl RandomKey {
    fn update_hasher(&self, hasher: &mut Sha256) {
        hasher.update(self.circuit_digest.0);
        update_usize_list(hasher, &self.op_path);
        update_u64_list(hasher, &self.repeat_iterations);
        update_usize_list(hasher, &self.target_slots);
        hasher.update([self.choice_kind.encoding()]);
        hasher.update(self.subchoice.to_le_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct KeyedRng {
    seed: u64,
    key: RandomKey,
    block_index: u64,
    block: [u8; 32],
    offset: usize,
}

impl KeyedRng {
    pub fn new(seed: u64, key: RandomKey) -> Self {
        let mut rng = Self {
            seed,
            key,
            block_index: 0,
            block: [0; 32],
            offset: 32,
        };
        rng.refill();
        rng
    }

    pub fn set_key(&mut self, key: RandomKey) {
        self.key = key;
        self.block_index = 0;
        self.offset = 32;
        self.refill();
    }

    fn refill(&mut self) {
        let mut hasher = Sha256::new();
        hasher.update(KEYED_RANDOM_VERSION);
        hasher.update(self.seed.to_le_bytes());
        self.key.update_hasher(&mut hasher);
        hasher.update(self.block_index.to_le_bytes());
        self.block = hasher.finalize().into();
        self.block_index += 1;
        self.offset = 0;
    }
}

impl RngCore for KeyedRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut written = 0;
        while written < destination.len() {
            if self.offset == self.block.len() {
                self.refill();
            }
            let available = self.block.len() - self.offset;
            let count = available.min(destination.len() - written);
            destination[written..written + count]
                .copy_from_slice(&self.block[self.offset..self.offset + count]);
            self.offset += count;
            written += count;
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(destination);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pauli {
    I,
    X,
    Y,
    Z,
}

impl Pauli {
    pub fn label(self) -> char {
        match self {
            Self::I => 'I',
            Self::X => 'X',
            Self::Y => 'Y',
            Self::Z => 'Z',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoiseOutcome {
    Identity,
    X,
    Y,
    Z,
    PauliPair { first: Pauli, second: Pauli },
    Lost,
    Correlated,
}

impl NoiseOutcome {
    pub fn label(self) -> String {
        match self {
            Self::Identity => "I".to_string(),
            Self::X => "X".to_string(),
            Self::Y => "Y".to_string(),
            Self::Z => "Z".to_string(),
            Self::PauliPair { first, second } => {
                format!("{}{}", first.label(), second.label())
            }
            Self::Lost => "L".to_string(),
            Self::Correlated => "correlated".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseSiteKind {
    XError,
    YError,
    ZError,
    Depolarize1,
    Depolarize2,
    Loss,
    PauliChannel1,
    PauliChannel2,
    HeraldedErase,
    HeraldedPauliChannel1,
    CorrelatedError,
    ElseCorrelatedError,
}

impl NoiseSiteKind {
    pub fn editable(self) -> bool {
        matches!(
            self,
            Self::XError
                | Self::YError
                | Self::ZError
                | Self::Depolarize1
                | Self::Depolarize2
                | Self::Loss
        )
    }

    pub fn allowed_outcomes(self) -> Vec<NoiseOutcome> {
        match self {
            Self::XError => vec![NoiseOutcome::Identity, NoiseOutcome::X],
            Self::YError => vec![NoiseOutcome::Identity, NoiseOutcome::Y],
            Self::ZError => vec![NoiseOutcome::Identity, NoiseOutcome::Z],
            Self::Depolarize1 => vec![
                NoiseOutcome::Identity,
                NoiseOutcome::X,
                NoiseOutcome::Y,
                NoiseOutcome::Z,
            ],
            Self::Depolarize2 => {
                let paulis = [Pauli::I, Pauli::X, Pauli::Y, Pauli::Z];
                let mut outcomes = Vec::with_capacity(16);
                for first in paulis {
                    for second in paulis {
                        outcomes.push(if first == Pauli::I && second == Pauli::I {
                            NoiseOutcome::Identity
                        } else {
                            NoiseOutcome::PauliPair { first, second }
                        });
                    }
                }
                outcomes
            }
            Self::Loss => vec![NoiseOutcome::Identity, NoiseOutcome::Lost],
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseSite {
    pub id: NoiseSiteId,
    pub instruction: String,
    pub kind: NoiseSiteKind,
    pub parameters: Vec<f64>,
    pub probability: Option<f64>,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub editable: bool,
    pub allowed_outcomes: Vec<NoiseOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoiseEvent {
    pub id: NoiseEventId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseEventCatalog {
    sites: Vec<NoiseSite>,
    events: Vec<NoiseEvent>,
}

impl NoiseEventCatalog {
    pub fn sites(&self) -> &[NoiseSite] {
        &self.sites
    }

    pub fn events(&self) -> &[NoiseEvent] {
        &self.events
    }

    pub fn site(&self, id: &NoiseSiteId) -> Option<&NoiseSite> {
        self.sites.iter().find(|site| &site.id == id)
    }

    pub fn event(&self, id: &NoiseEventId) -> Option<&NoiseEvent> {
        self.events.iter().find(|event| &event.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionLimits {
    pub max_operations: u64,
    pub max_noise_events: u64,
    pub max_measurements: u64,
    pub max_svg_nodes: u64,
    pub max_qubits: u64,
}

impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_operations: 5_000,
            max_noise_events: 5_000,
            max_measurements: 5_000,
            max_svg_nodes: 100_000,
            max_qubits: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionSummary {
    pub operations: u64,
    pub noise_events: u64,
    pub measurements: u64,
    pub estimated_svg_nodes: u64,
}

impl ExpansionSummary {
    fn checked_add(self, other: Self) -> Result<Self, String> {
        Ok(Self {
            operations: checked_add(self.operations, other.operations, "operation")?,
            noise_events: checked_add(self.noise_events, other.noise_events, "noise event")?,
            measurements: checked_add(self.measurements, other.measurements, "measurement")?,
            estimated_svg_nodes: checked_add(
                self.estimated_svg_nodes,
                other.estimated_svg_nodes,
                "estimated SVG node",
            )?,
        })
    }

    fn checked_mul(self, factor: u64) -> Result<Self, String> {
        Ok(Self {
            operations: checked_mul(self.operations, factor, "operation")?,
            noise_events: checked_mul(self.noise_events, factor, "noise event")?,
            measurements: checked_mul(self.measurements, factor, "measurement")?,
            estimated_svg_nodes: checked_mul(
                self.estimated_svg_nodes,
                factor,
                "estimated SVG node",
            )?,
        })
    }

    fn check_limits(self, limits: ExpansionLimits) -> Result<Self, String> {
        for (label, actual, limit) in [
            (
                "expanded operations",
                self.operations,
                limits.max_operations,
            ),
            (
                "dynamic noise events",
                self.noise_events,
                limits.max_noise_events,
            ),
            ("measurements", self.measurements, limits.max_measurements),
            (
                "estimated SVG nodes",
                self.estimated_svg_nodes,
                limits.max_svg_nodes,
            ),
        ] {
            if actual > limit {
                return Err(format!(
                    "circuit too large: {label} estimate {actual} exceeds limit {limit}; \
                     interactive repeat collapsing is not supported"
                ));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitSession {
    source: String,
    source_sha256: [u8; 32],
    circuit_digest: CircuitDigest,
    instructions: Vec<StimInstr>,
    catalog: NoiseEventCatalog,
    expansion: ExpansionSummary,
}

impl CircuitSession {
    pub fn open(source: &str, limits: ExpansionLimits) -> Result<Self, String> {
        let instructions = parse_lines(source)?;
        let qubits = required_qubits(&instructions)?;
        if qubits > limits.max_qubits {
            return Err(format!(
                "circuit too large: required qubits {qubits} exceeds limit {}; \
                 large tableau allocation is not supported by the interactive viewer",
                limits.max_qubits
            ));
        }
        let circuit_digest = CircuitDigest::from_instructions(&instructions);
        let expansion = estimate_instructions(&instructions)?.check_limits(limits)?;
        let catalog = build_catalog(&instructions, circuit_digest);
        debug_assert_eq!(catalog.events.len() as u64, expansion.noise_events);
        Ok(Self {
            source: source.to_string(),
            source_sha256: Sha256::digest(source.as_bytes()).into(),
            circuit_digest,
            instructions,
            catalog,
            expansion,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_sha256_hex(&self) -> String {
        hex_bytes(&self.source_sha256)
    }

    pub fn circuit_digest(&self) -> CircuitDigest {
        self.circuit_digest
    }

    pub fn instructions(&self) -> &[StimInstr] {
        &self.instructions
    }

    pub fn catalog(&self) -> &NoiseEventCatalog {
        &self.catalog
    }

    pub fn expansion(&self) -> ExpansionSummary {
        self.expansion
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShotBase {
    Sampled {
        #[serde(with = "u64_as_string")]
        seed: u64,
    },
    Noiseless {
        #[serde(with = "u64_as_string")]
        seed: u64,
    },
}

impl ShotBase {
    pub fn seed(self) -> u64 {
        match self {
            Self::Sampled { seed } | Self::Noiseless { seed } => seed,
        }
    }

    pub fn is_noiseless(self) -> bool {
        matches!(self, Self::Noiseless { .. })
    }
}

mod u64_as_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoiseEventState {
    pub id: String,
    pub site_id: String,
    pub instruction: String,
    pub target_qubits: Vec<u32>,
    pub base_outcome: NoiseOutcome,
    pub override_outcome: Option<NoiseOutcome>,
    pub effective_outcome: NoiseOutcome,
    pub applicable: bool,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementResult {
    pub id: String,
    pub index: usize,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slot: usize,
    pub target_qubit: u32,
    pub instruction: String,
    pub bit: bool,
    pub loss_cause: bool,
    pub component: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorResult {
    pub id: String,
    pub index: usize,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub flipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservableResult {
    pub id: String,
    pub sequence_index: usize,
    pub observable_index: u32,
    pub bit: bool,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSet {
    pub measurements: Vec<String>,
    pub detectors: Vec<String>,
    pub observables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotResult {
    pub noise_events: Vec<NoiseEventState>,
    pub measurements: Vec<MeasurementResult>,
    pub detectors: Vec<DetectorResult>,
    pub observables: Vec<ObservableResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotSummary {
    pub revision: u64,
    pub base: ShotBase,
    pub result: ShotResult,
    pub changed_by_last_action: ChangedSet,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseSiteSummary {
    pub id: String,
    pub instruction: String,
    pub kind: NoiseSiteKind,
    pub parameters: Vec<f64>,
    pub probability: Option<f64>,
    pub target_qubits: Vec<u32>,
    pub editable: bool,
    pub allowed_outcomes: Vec<NoiseOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceOverride {
    pub event_id: String,
    pub outcome: NoiseOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub format_version: String,
    pub rstim_version: String,
    pub source_sha256: String,
    pub circuit_digest: String,
    pub base: ShotBase,
    pub overrides: Vec<ProvenanceOverride>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub format_version: String,
    pub revision: u64,
    pub svg: String,
    pub shot: ShotSummary,
    pub noise_sites: Vec<NoiseSiteSummary>,
    pub provenance: Provenance,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditCommand {
    event_id: NoiseEventId,
    before: Option<NoiseOutcome>,
    after: Option<NoiseOutcome>,
}

struct RunArtifacts {
    output: ExecOutput,
    trace: SampleTrace,
    outcomes: BTreeMap<NoiseEventId, NoiseOutcome>,
    inapplicable: BTreeSet<NoiseEventId>,
}

pub struct EditableShot {
    session: CircuitSession,
    base: ShotBase,
    base_run: RunArtifacts,
    current_run: RunArtifacts,
    overrides: BTreeMap<NoiseEventId, NoiseOutcome>,
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
    revision: u64,
    changed_by_last_action: ChangedSet,
}

impl EditableShot {
    pub fn open(source: &str, limits: ExpansionLimits, initial_seed: u64) -> Result<Self, String> {
        Self::new(CircuitSession::open(source, limits)?, initial_seed)
    }

    pub fn new(session: CircuitSession, initial_seed: u64) -> Result<Self, String> {
        let base = ShotBase::Noiseless { seed: initial_seed };
        let overrides = BTreeMap::new();
        let base_run = execute_session(&session, base, &overrides)?;
        let current_run = execute_session(&session, base, &overrides)?;
        Ok(Self {
            session,
            base,
            base_run,
            current_run,
            overrides,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            changed_by_last_action: ChangedSet::default(),
        })
    }

    pub fn session(&self) -> &CircuitSession {
        &self.session
    }

    pub fn base(&self) -> ShotBase {
        self.base
    }

    pub fn overrides(&self) -> &BTreeMap<NoiseEventId, NoiseOutcome> {
        &self.overrides
    }

    pub fn sample(&mut self, seed: u64) -> Result<(), String> {
        self.replace_base(ShotBase::Sampled { seed })
    }

    pub fn clear(&mut self, seed: u64) -> Result<(), String> {
        self.replace_base(ShotBase::Noiseless { seed })
    }

    pub fn set_noise(
        &mut self,
        event_id: &NoiseEventId,
        outcome: NoiseOutcome,
    ) -> Result<(), String> {
        let site = self.validate_edit(event_id, outcome)?;
        if !site.allowed_outcomes.contains(&outcome) {
            return Err(format!(
                "outcome {} is not allowed for {}",
                outcome.label(),
                site.instruction
            ));
        }
        self.apply_edit(event_id.clone(), Some(outcome), true)
    }

    pub fn restore_noise(&mut self, event_id: &NoiseEventId) -> Result<(), String> {
        self.validate_event(event_id)?;
        self.apply_edit(event_id.clone(), None, true)
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        let Some(command) = self.undo.last().cloned() else {
            return Ok(false);
        };
        self.apply_edit(command.event_id.clone(), command.before, false)?;
        self.undo.pop();
        self.redo.push(command);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        let Some(command) = self.redo.last().cloned() else {
            return Ok(false);
        };
        self.apply_edit(command.event_id.clone(), command.after, false)?;
        self.redo.pop();
        self.undo.push(command);
        Ok(true)
    }

    pub fn summary(&self) -> ShotSummary {
        ShotSummary {
            revision: self.revision,
            base: self.base,
            result: build_shot_result(
                &self.session,
                &self.base_run,
                &self.current_run,
                &self.overrides,
            ),
            changed_by_last_action: self.changed_by_last_action.clone(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    pub fn current_trace(&self) -> &SampleTrace {
        &self.current_run.trace
    }

    pub fn view_snapshot(&self) -> Result<ViewSnapshot, String> {
        let (mut document, render_warning) = match export_qp101_with_sample_trace(
            &self.session.instructions,
            &self.current_run.trace,
        ) {
            Ok(document) => (document, None),
            Err(error) => (
                export_qp101(&self.session.instructions)?,
                Some(format!(
                    "sample annotations are unavailable for part of this circuit: {error}"
                )),
            ),
        };
        decorate_manual_overrides(
            &mut document.operations,
            &self.overrides,
            &self.current_run.outcomes,
        )?;
        decorate_observables(
            &mut document.operations,
            &self.current_run.output.observable_events,
        )?;
        let svg = render_svg_interactive(&document, self.session.circuit_digest)?;
        let shot = self.summary();
        let mut warnings = self
            .session
            .catalog
            .sites
            .iter()
            .filter(|site| !site.editable)
            .map(|site| {
                format!(
                    "{} is sampled and cleared correctly but is read-only in this version",
                    site.instruction
                )
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if let Some(warning) = render_warning {
            warnings.push(warning);
        }
        Ok(ViewSnapshot {
            format_version: "rstim-shot-view-v1".to_string(),
            revision: self.revision,
            svg,
            shot,
            noise_sites: self
                .session
                .catalog
                .sites
                .iter()
                .map(|site| NoiseSiteSummary {
                    id: site.id.encode(),
                    instruction: site.instruction.clone(),
                    kind: site.kind,
                    parameters: site.parameters.clone(),
                    probability: site.probability,
                    target_qubits: site.target_qubits.clone(),
                    editable: site.editable,
                    allowed_outcomes: site.allowed_outcomes.clone(),
                })
                .collect(),
            provenance: Provenance {
                format_version: "rstim-shot-provenance-v1".to_string(),
                rstim_version: crate::version().to_string(),
                source_sha256: self.session.source_sha256_hex(),
                circuit_digest: self.session.circuit_digest.to_hex(),
                base: self.base,
                overrides: self
                    .overrides
                    .iter()
                    .map(|(event_id, outcome)| ProvenanceOverride {
                        event_id: event_id.encode(),
                        outcome: *outcome,
                    })
                    .collect(),
            },
            warnings,
        })
    }

    fn replace_base(&mut self, base: ShotBase) -> Result<(), String> {
        let overrides = BTreeMap::new();
        let next_base = execute_session(&self.session, base, &overrides)?;
        let next_current = execute_session(&self.session, base, &overrides)?;
        let previous = build_shot_result(
            &self.session,
            &self.base_run,
            &self.current_run,
            &self.overrides,
        );
        let next = build_shot_result(&self.session, &next_base, &next_current, &overrides);
        self.base = base;
        self.base_run = next_base;
        self.current_run = next_current;
        self.overrides = overrides;
        self.undo.clear();
        self.redo.clear();
        self.revision = self.revision.saturating_add(1);
        self.changed_by_last_action = diff_results(&previous, &next);
        Ok(())
    }

    fn apply_edit(
        &mut self,
        event_id: NoiseEventId,
        after: Option<NoiseOutcome>,
        record_history: bool,
    ) -> Result<(), String> {
        self.validate_event(&event_id)?;
        let before = self.overrides.get(&event_id).copied();
        if before == after {
            return Ok(());
        }
        let mut next_overrides = self.overrides.clone();
        match after {
            Some(outcome) => {
                next_overrides.insert(event_id.clone(), outcome);
            }
            None => {
                next_overrides.remove(&event_id);
            }
        }
        let next_run = execute_session(&self.session, self.base, &next_overrides)?;
        let previous = build_shot_result(
            &self.session,
            &self.base_run,
            &self.current_run,
            &self.overrides,
        );
        let next = build_shot_result(&self.session, &self.base_run, &next_run, &next_overrides);
        self.current_run = next_run;
        self.overrides = next_overrides;
        self.revision = self.revision.saturating_add(1);
        self.changed_by_last_action = diff_results(&previous, &next);
        if record_history {
            self.undo.push(EditCommand {
                event_id,
                before,
                after,
            });
            self.redo.clear();
        }
        Ok(())
    }

    fn validate_event(&self, event_id: &NoiseEventId) -> Result<&NoiseSite, String> {
        if event_id.site.circuit_digest != self.session.circuit_digest {
            return Err("noise event belongs to a different circuit".to_string());
        }
        if self.session.catalog.event(event_id).is_none() {
            return Err(format!("unknown noise event id: {event_id}"));
        }
        self.session
            .catalog
            .site(&event_id.site)
            .ok_or_else(|| format!("unknown noise site id: {}", event_id.site))
    }

    fn validate_edit(
        &self,
        event_id: &NoiseEventId,
        _outcome: NoiseOutcome,
    ) -> Result<&NoiseSite, String> {
        let site = self.validate_event(event_id)?;
        if !site.editable {
            return Err(format!(
                "{} is sampled but read-only in the first interactive version",
                site.instruction
            ));
        }
        Ok(site)
    }
}

fn decorate_manual_overrides(
    operations: &mut [Qp101Operation],
    overrides: &BTreeMap<NoiseEventId, NoiseOutcome>,
    effective_outcomes: &BTreeMap<NoiseEventId, NoiseOutcome>,
) -> Result<(), String> {
    for (event_id, requested_outcome) in overrides {
        let annotations = annotations_at_path_mut(operations, &event_id.site.op_path)?;
        annotations
            .retain(|annotation| !annotation_context_matches_event(annotation, event_id, "noise"));
        let effective = effective_outcomes
            .get(event_id)
            .copied()
            .unwrap_or(NoiseOutcome::Identity);
        let active =
            *requested_outcome == NoiseOutcome::Identity || effective != NoiseOutcome::Identity;
        annotations.push(Qp101Annotation {
            kind: "marker".to_string(),
            target_slots: event_id.site.target_slots.clone(),
            label: Some(requested_outcome.label()),
            text: Some(if active {
                "manual".to_string()
            } else {
                "manual · inactive".to_string()
            }),
            style: Some(Qp101AnnotationStyle {
                preset: Some(if effective == NoiseOutcome::Identity {
                    "info".to_string()
                } else {
                    "danger".to_string()
                }),
                color: Some(if effective == NoiseOutcome::Identity {
                    "blue".to_string()
                } else {
                    "red".to_string()
                }),
                highlight: Some(true),
            }),
            tags: vec![
                "sample-trace".to_string(),
                "query-result".to_string(),
                "manual-override".to_string(),
            ],
            context: Some(json!({
                "query_kind": "sample_trace",
                "annotation_kind": "noise",
                "op_path": event_id.site.op_path,
                "repeat_iterations": event_id.repeat_iterations,
                "target_slots": event_id.site.target_slots,
                "noise_event_id": event_id.encode(),
                "requested_outcome": requested_outcome.label(),
                "effective_outcome": effective.label(),
                "active": active,
            })),
        });
    }
    Ok(())
}

fn decorate_observables(
    operations: &mut [Qp101Operation],
    events: &[crate::executor::ObservableEvent],
) -> Result<(), String> {
    for event in events {
        let annotations = annotations_at_path_mut(operations, &event.op_path)?;
        annotations.push(Qp101Annotation {
            kind: "marker".to_string(),
            target_slots: Vec::new(),
            label: Some(format!(
                "L{}={}",
                event.observable_index,
                u8::from(event.bit)
            )),
            text: None,
            style: event.bit.then(|| Qp101AnnotationStyle {
                preset: Some("info".to_string()),
                color: Some("blue".to_string()),
                highlight: Some(true),
            }),
            tags: vec!["sample-trace".to_string(), "query-result".to_string()],
            context: Some(json!({
                "query_kind": "sample_trace",
                "annotation_kind": "observable",
                "op_path": event.op_path,
                "repeat_iterations": event.repeat_iterations,
                "sequence_index": event.sequence_index,
                "observable_index": event.observable_index,
                "bit": event.bit,
            })),
        });
    }
    Ok(())
}

fn annotations_at_path_mut<'a>(
    operations: &'a mut [Qp101Operation],
    op_path: &[usize],
) -> Result<&'a mut Vec<Qp101Annotation>, String> {
    let Some((&head, tail)) = op_path.split_first() else {
        return Err("interactive annotation operation path is empty".to_string());
    };
    let operation = operations
        .get_mut(head)
        .ok_or_else(|| format!("interactive annotation op_path index {head} is out of range"))?;
    if !tail.is_empty() {
        return match operation {
            Qp101Operation::Repeat { body, .. } => annotations_at_path_mut(body, tail),
            _ => Err(format!(
                "interactive annotation op_path descends into non-repeat operation at {head}"
            )),
        };
    }
    match operation {
        Qp101Operation::QubitCoords { annotations, .. }
        | Qp101Operation::ShiftCoords { annotations, .. }
        | Qp101Operation::Gate { annotations, .. }
        | Qp101Operation::Noise { annotations, .. }
        | Qp101Operation::Tick { annotations }
        | Qp101Operation::Repeat { annotations, .. }
        | Qp101Operation::Detector { annotations, .. }
        | Qp101Operation::ObservableInclude { annotations, .. }
        | Qp101Operation::Annotation { annotations, .. } => Ok(annotations),
    }
}

fn annotation_context_matches_event(
    annotation: &Qp101Annotation,
    event_id: &NoiseEventId,
    annotation_kind: &str,
) -> bool {
    let Some(context) = annotation.context.as_ref() else {
        return false;
    };
    context
        .get("query_kind")
        .and_then(serde_json::Value::as_str)
        == Some("sample_trace")
        && annotation.target_slots == event_id.site.target_slots
        && context
            .get("annotation_kind")
            .and_then(serde_json::Value::as_str)
            == Some(annotation_kind)
        && context.get("op_path") == Some(&json!(event_id.site.op_path))
        && context.get("repeat_iterations") == Some(&json!(event_id.repeat_iterations))
        && context
            .get("target_slots")
            .is_none_or(|value| value == &json!(event_id.site.target_slots))
}

fn execute_session(
    session: &CircuitSession,
    base: ShotBase,
    overrides: &BTreeMap<NoiseEventId, NoiseOutcome>,
) -> Result<RunArtifacts, String> {
    let executor = Executor::from_instrs(session.instructions.clone())?;
    let (output, trace) = executor.run_with_choices(InteractiveExecutionConfig {
        circuit_digest: session.circuit_digest,
        seed: base.seed(),
        force_noiseless: base.is_noiseless(),
        overrides,
    })?;
    let outcomes = trace_outcomes(session, &trace)?;
    let inapplicable = output
        .inapplicable_noise_events
        .iter()
        .map(|event| NoiseEventId {
            site: NoiseSiteId {
                circuit_digest: session.circuit_digest,
                op_path: event.op_path.clone(),
                target_slots: event.target_slots.clone(),
            },
            repeat_iterations: event.repeat_iterations.clone(),
        })
        .collect();
    Ok(RunArtifacts {
        output,
        trace,
        outcomes,
        inapplicable,
    })
}

fn trace_outcomes(
    session: &CircuitSession,
    trace: &SampleTrace,
) -> Result<BTreeMap<NoiseEventId, NoiseOutcome>, String> {
    let mut outcomes = BTreeMap::new();
    for event in &trace.noise_events {
        if !event.occurred {
            continue;
        }
        let event_id = NoiseEventId {
            site: NoiseSiteId {
                circuit_digest: session.circuit_digest,
                op_path: event.op_path.clone(),
                target_slots: event.target_slots.clone(),
            },
            repeat_iterations: event.repeat_iterations.clone(),
        };
        let site = session
            .catalog
            .site(&event_id.site)
            .ok_or_else(|| format!("trace contains unknown noise site {}", event_id.site))?;
        let label = event
            .branch_label
            .as_deref()
            .ok_or_else(|| format!("occurred noise event {event_id} has no branch label"))?;
        outcomes.insert(event_id, parse_trace_outcome(site.kind, label)?);
    }
    Ok(outcomes)
}

fn parse_trace_outcome(kind: NoiseSiteKind, label: &str) -> Result<NoiseOutcome, String> {
    match label {
        "X" => Ok(NoiseOutcome::X),
        "Y" => Ok(NoiseOutcome::Y),
        "Z" => Ok(NoiseOutcome::Z),
        "L" => Ok(NoiseOutcome::Lost),
        _ if label.len() == 2
            && matches!(
                kind,
                NoiseSiteKind::Depolarize2 | NoiseSiteKind::PauliChannel2
            ) =>
        {
            let bytes = label.as_bytes();
            Ok(NoiseOutcome::PauliPair {
                first: parse_pauli_label(bytes[0])?,
                second: parse_pauli_label(bytes[1])?,
            })
        }
        _ if matches!(
            kind,
            NoiseSiteKind::CorrelatedError | NoiseSiteKind::ElseCorrelatedError
        ) =>
        {
            Ok(NoiseOutcome::Correlated)
        }
        _ => Err(format!(
            "unsupported trace branch label {label:?} for {kind:?}"
        )),
    }
}

fn parse_pauli_label(label: u8) -> Result<Pauli, String> {
    match label {
        b'I' => Ok(Pauli::I),
        b'X' => Ok(Pauli::X),
        b'Y' => Ok(Pauli::Y),
        b'Z' => Ok(Pauli::Z),
        _ => Err(format!("invalid Pauli branch label byte {label}")),
    }
}

fn build_shot_result(
    session: &CircuitSession,
    base_run: &RunArtifacts,
    current_run: &RunArtifacts,
    overrides: &BTreeMap<NoiseEventId, NoiseOutcome>,
) -> ShotResult {
    let noise_events = session
        .catalog
        .events
        .iter()
        .map(|event| {
            let site = session
                .catalog
                .site(&event.id.site)
                .expect("catalog event must reference its catalog site");
            let base_outcome = base_run
                .outcomes
                .get(&event.id)
                .copied()
                .unwrap_or(NoiseOutcome::Identity);
            let override_outcome = overrides.get(&event.id).copied();
            let effective_outcome = current_run
                .outcomes
                .get(&event.id)
                .copied()
                .unwrap_or(NoiseOutcome::Identity);
            let applicable = !current_run.inapplicable.contains(&event.id);
            NoiseEventState {
                id: event.id.encode(),
                site_id: event.id.site.encode(),
                instruction: site.instruction.clone(),
                target_qubits: site.target_qubits.clone(),
                base_outcome,
                override_outcome,
                effective_outcome,
                applicable,
                editable: site.editable,
            }
        })
        .collect();

    let measurements = current_run
        .trace
        .measurement_events
        .iter()
        .map(|event| MeasurementResult {
            id: format!("m{}", event.measurement_index),
            index: event.measurement_index,
            op_path: event.op_path.clone(),
            repeat_iterations: event.repeat_iterations.clone(),
            target_slot: event.target_slot,
            target_qubit: event.target_qubit,
            instruction: event.instr_name.clone(),
            bit: event.bit,
            loss_cause: event.loss_cause,
            component: match event.component {
                MeasurementComponent::Value => "value",
                MeasurementComponent::LossFlag => "loss_flag",
            }
            .to_string(),
        })
        .collect();

    let detectors = current_run
        .trace
        .detector_events
        .iter()
        .map(|event| DetectorResult {
            id: format!("d{}", event.detector_index),
            index: event.detector_index,
            op_path: event.op_path.clone(),
            repeat_iterations: event.repeat_iterations.clone(),
            flipped: event.flipped,
        })
        .collect();

    let observables = current_run
        .output
        .observable_events
        .iter()
        .map(|event| ObservableResult {
            id: format!("l{}-{}", event.observable_index, event.sequence_index),
            sequence_index: event.sequence_index,
            observable_index: event.observable_index,
            bit: event.bit,
            op_path: event.op_path.clone(),
            repeat_iterations: event.repeat_iterations.clone(),
        })
        .collect();

    ShotResult {
        noise_events,
        measurements,
        detectors,
        observables,
    }
}

fn diff_results(previous: &ShotResult, next: &ShotResult) -> ChangedSet {
    ChangedSet {
        measurements: changed_ids(&previous.measurements, &next.measurements, |value| {
            (&value.id, value.bit)
        }),
        detectors: changed_ids(&previous.detectors, &next.detectors, |value| {
            (&value.id, value.flipped)
        }),
        observables: changed_ids(&previous.observables, &next.observables, |value| {
            (&value.id, value.bit)
        }),
    }
}

fn changed_ids<'a, T>(
    previous: &'a [T],
    next: &'a [T],
    key: impl Fn(&'a T) -> (&'a String, bool),
) -> Vec<String> {
    let previous_values: BTreeMap<_, _> = previous
        .iter()
        .map(|value| {
            let (id, bit) = key(value);
            (id.as_str(), bit)
        })
        .collect();
    next.iter()
        .filter_map(|value| {
            let (id, bit) = key(value);
            (previous_values.get(id.as_str()).copied() != Some(bit)).then(|| id.clone())
        })
        .collect()
}

fn estimate_instructions(instructions: &[StimInstr]) -> Result<ExpansionSummary, String> {
    let mut total = ExpansionSummary {
        operations: 0,
        noise_events: 0,
        measurements: 0,
        estimated_svg_nodes: 0,
    };
    for instruction in instructions {
        let summary = match instruction {
            StimInstr::Op {
                name,
                args: _,
                targets,
                ..
            } => {
                let noise_events = noise_sites_for_op(name, targets).len() as u64;
                let measurements = measurement_result_count(name, targets);
                ExpansionSummary {
                    operations: 1,
                    noise_events,
                    measurements,
                    estimated_svg_nodes: 12_u64
                        .checked_add((targets.len() as u64).saturating_mul(3))
                        .and_then(|value| value.checked_add(noise_events.saturating_mul(3)))
                        .and_then(|value| value.checked_add(measurements.saturating_mul(2)))
                        .ok_or_else(|| "estimated SVG node count overflow".to_string())?,
                }
            }
            StimInstr::Repeat { count, body } => {
                estimate_instructions(body)?.checked_mul(*count)?
            }
        };
        total = total.checked_add(summary)?;
    }
    Ok(total)
}

fn build_catalog(instructions: &[StimInstr], digest: CircuitDigest) -> NoiseEventCatalog {
    let mut sites = BTreeMap::<NoiseSiteId, NoiseSite>::new();
    let mut events = Vec::new();
    collect_catalog(instructions, digest, &[], &[], &mut sites, &mut events);
    NoiseEventCatalog {
        sites: sites.into_values().collect(),
        events,
    }
}

fn collect_catalog(
    instructions: &[StimInstr],
    digest: CircuitDigest,
    op_prefix: &[usize],
    repeat_iterations: &[u64],
    sites: &mut BTreeMap<NoiseSiteId, NoiseSite>,
    events: &mut Vec<NoiseEvent>,
) {
    for (op_index, instruction) in instructions.iter().enumerate() {
        let mut op_path = op_prefix.to_vec();
        op_path.push(op_index);
        match instruction {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                for descriptor in noise_sites_for_op(name, targets) {
                    let id = NoiseSiteId {
                        circuit_digest: digest,
                        op_path: op_path.clone(),
                        target_slots: descriptor.target_slots.clone(),
                    };
                    sites.entry(id.clone()).or_insert_with(|| NoiseSite {
                        id: id.clone(),
                        instruction: name.clone(),
                        kind: descriptor.kind,
                        parameters: args.clone(),
                        probability: noise_total_probability(descriptor.kind, args),
                        target_slots: descriptor.target_slots,
                        target_qubits: descriptor.target_qubits,
                        editable: descriptor.kind.editable(),
                        allowed_outcomes: descriptor.kind.allowed_outcomes(),
                    });
                    events.push(NoiseEvent {
                        id: NoiseEventId {
                            site: id,
                            repeat_iterations: repeat_iterations.to_vec(),
                        },
                    });
                }
            }
            StimInstr::Repeat { count, body } => {
                for iteration in 0..*count {
                    let mut nested_iterations = repeat_iterations.to_vec();
                    nested_iterations.push(iteration);
                    collect_catalog(body, digest, &op_path, &nested_iterations, sites, events);
                }
            }
        }
    }
}

#[derive(Debug)]
struct SiteDescriptor {
    kind: NoiseSiteKind,
    target_slots: Vec<usize>,
    target_qubits: Vec<u32>,
}

fn noise_sites_for_op(name: &str, targets: &[StimTarget]) -> Vec<SiteDescriptor> {
    let Some(kind) = noise_kind(name) else {
        return Vec::new();
    };
    let target_entries: Vec<_> = targets
        .iter()
        .enumerate()
        .filter_map(|(slot, target)| target.qubit_index().map(|qubit| (slot, qubit)))
        .collect();
    match kind {
        NoiseSiteKind::Depolarize2 | NoiseSiteKind::PauliChannel2 => target_entries
            .chunks_exact(2)
            .map(|pair| SiteDescriptor {
                kind,
                target_slots: vec![pair[0].0, pair[1].0],
                target_qubits: vec![pair[0].1, pair[1].1],
            })
            .collect(),
        NoiseSiteKind::CorrelatedError | NoiseSiteKind::ElseCorrelatedError => (!target_entries
            .is_empty())
        .then(|| SiteDescriptor {
            kind,
            target_slots: target_entries.iter().map(|entry| entry.0).collect(),
            target_qubits: target_entries.iter().map(|entry| entry.1).collect(),
        })
        .into_iter()
        .collect(),
        _ => target_entries
            .into_iter()
            .map(|(slot, qubit)| SiteDescriptor {
                kind,
                target_slots: vec![slot],
                target_qubits: vec![qubit],
            })
            .collect(),
    }
}

fn noise_kind(name: &str) -> Option<NoiseSiteKind> {
    Some(match name {
        "X_ERROR" => NoiseSiteKind::XError,
        "Y_ERROR" => NoiseSiteKind::YError,
        "Z_ERROR" => NoiseSiteKind::ZError,
        "DEPOLARIZE1" => NoiseSiteKind::Depolarize1,
        "DEPOLARIZE2" => NoiseSiteKind::Depolarize2,
        "LOSS" => NoiseSiteKind::Loss,
        "PAULI_CHANNEL_1" => NoiseSiteKind::PauliChannel1,
        "PAULI_CHANNEL_2" => NoiseSiteKind::PauliChannel2,
        "HERALDED_ERASE" => NoiseSiteKind::HeraldedErase,
        "HERALDED_PAULI_CHANNEL_1" => NoiseSiteKind::HeraldedPauliChannel1,
        "CORRELATED_ERROR" | "E" => NoiseSiteKind::CorrelatedError,
        "ELSE_CORRELATED_ERROR" => NoiseSiteKind::ElseCorrelatedError,
        _ => return None,
    })
}

fn noise_total_probability(kind: NoiseSiteKind, args: &[f64]) -> Option<f64> {
    match kind {
        NoiseSiteKind::PauliChannel1
        | NoiseSiteKind::PauliChannel2
        | NoiseSiteKind::HeraldedPauliChannel1 => (!args.is_empty()).then(|| args.iter().sum()),
        _ => args.first().copied(),
    }
}

fn measurement_result_count(name: &str, targets: &[StimTarget]) -> u64 {
    let target_count = targets
        .iter()
        .filter(|target| target.qubit_index().is_some())
        .count() as u64;
    match name {
        "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" => target_count,
        "ML" | "MZL" | "MXL" | "MYL" | "MRL" | "MRZL" | "MRXL" | "MRYL" => {
            target_count.saturating_mul(2)
        }
        "MXX" | "MYY" | "MZZ" => target_count / 2,
        "MPP" => {
            let terms = targets
                .iter()
                .filter(|target| !matches!(target, StimTarget::Combiner))
                .count() as u64;
            let combiners = targets
                .iter()
                .filter(|target| matches!(target, StimTarget::Combiner))
                .count() as u64;
            terms.saturating_sub(combiners)
        }
        "MPAD" | "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => targets.len() as u64,
        _ => 0,
    }
}

fn required_qubits(instructions: &[StimInstr]) -> Result<u64, String> {
    fn visit(instructions: &[StimInstr], max_index: &mut Option<u32>) {
        for instruction in instructions {
            match instruction {
                StimInstr::Op { targets, .. } => {
                    for target in targets {
                        let qubit = match target {
                            StimTarget::Qubit(qubit) | StimTarget::QubitInv(qubit) => Some(*qubit),
                            StimTarget::Pauli { qubit, .. } => Some(*qubit),
                            StimTarget::Combiner | StimTarget::Rec(_) | StimTarget::Sweep(_) => {
                                None
                            }
                        };
                        if let Some(qubit) = qubit {
                            *max_index =
                                Some(max_index.map_or(qubit, |current| current.max(qubit)));
                        }
                    }
                }
                StimInstr::Repeat { body, .. } => visit(body, max_index),
            }
        }
    }

    let mut max_index = None;
    visit(instructions, &mut max_index);
    Ok(max_index.map_or(0, |index| u64::from(index) + 1))
}

fn checked_add(left: u64, right: u64, label: &str) -> Result<u64, String> {
    left.checked_add(right)
        .ok_or_else(|| format!("expanded {label} count overflow"))
}

fn checked_mul(value: u64, factor: u64, label: &str) -> Result<u64, String> {
    value
        .checked_mul(factor)
        .ok_or_else(|| format!("expanded {label} count overflow"))
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn update_usize_list(hasher: &mut Sha256, values: &[usize]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update((*value as u64).to_le_bytes());
    }
}

fn update_u64_list(hasher: &mut Sha256, values: &[u64]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_usize_list(values: &[usize]) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn encode_u64_list(values: &[u64]) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_usize_list(value: &str, label: &str) -> Result<Vec<usize>, String> {
    decode_number_list(value, label, str::parse)
}

fn decode_u64_list(value: &str, label: &str) -> Result<Vec<u64>, String> {
    decode_number_list(value, label, str::parse)
}

fn decode_number_list<T>(
    value: &str,
    label: &str,
    parse: impl Fn(&str) -> Result<T, std::num::ParseIntError>,
) -> Result<Vec<T>, String> {
    if value == "-" {
        return Ok(Vec::new());
    }
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    value
        .split(',')
        .map(|part| {
            if part.is_empty() {
                return Err(format!("{label} contains an empty component"));
            }
            parse(part).map_err(|_| format!("{label} contains invalid integer {part:?}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn canonical_digest_ignores_source_formatting() {
        let compact = parse_lines("H 0\nX_ERROR(0.1) 0\n").unwrap();
        let spaced = parse_lines("  H   0  # comment\n\nX_ERROR(0.1)   0\n").unwrap();
        assert_eq!(
            CircuitDigest::from_instructions(&compact),
            CircuitDigest::from_instructions(&spaced)
        );
    }

    #[test]
    fn site_and_event_ids_round_trip() {
        let session = CircuitSession::open(
            "REPEAT 2 {\n  DEPOLARIZE1(0.1) 3\n}\n",
            ExpansionLimits::default(),
        )
        .unwrap();
        let site = &session.catalog().sites()[0].id;
        let event = &session.catalog().events()[1].id;
        assert_eq!(NoiseSiteId::decode(&site.encode()).unwrap(), *site);
        assert_eq!(NoiseEventId::decode(&event.encode()).unwrap(), *event);
        assert_eq!(event.repeat_iterations, vec![1]);
    }

    #[test]
    fn nested_repeats_produce_unique_dynamic_event_ids() {
        let session = CircuitSession::open(
            "REPEAT 2 {\n  REPEAT 3 {\n    X_ERROR(0.25) 0 1\n  }\n}\n",
            ExpansionLimits::default(),
        )
        .unwrap();
        assert_eq!(session.catalog().sites().len(), 2);
        assert_eq!(session.catalog().events().len(), 12);
        let unique = session
            .catalog()
            .events()
            .iter()
            .map(|event| event.id.encode())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 12);
        assert_eq!(session.expansion().operations, 6);
    }

    #[test]
    fn depolarize_two_uses_target_pairs_and_sixteen_choices() {
        let session =
            CircuitSession::open("DEPOLARIZE2(0.1) 0 1 2 3\n", ExpansionLimits::default()).unwrap();
        assert_eq!(session.catalog().sites().len(), 2);
        assert_eq!(session.catalog().sites()[0].target_slots, vec![0, 1]);
        assert_eq!(session.catalog().sites()[1].target_slots, vec![2, 3]);
        assert_eq!(session.catalog().sites()[0].allowed_outcomes.len(), 16);
    }

    #[test]
    fn expansion_limit_rejects_before_catalog_enumeration() {
        let error = CircuitSession::open(
            "REPEAT 1000000000 {\n  X_ERROR(0.1) 0\n}\n",
            ExpansionLimits::default(),
        )
        .unwrap_err();
        assert!(error.contains("expanded operations estimate 1000000000 exceeds limit 5000"));
        assert!(error.contains("repeat collapsing is not supported"));
    }

    #[test]
    fn exact_source_and_canonical_digests_have_distinct_roles() {
        let first = CircuitSession::open("H 0\n", ExpansionLimits::default()).unwrap();
        let second = CircuitSession::open("  H 0 # comment\n", ExpansionLimits::default()).unwrap();
        assert_eq!(first.circuit_digest(), second.circuit_digest());
        assert_ne!(first.source_sha256_hex(), second.source_sha256_hex());
    }

    #[test]
    fn keyed_rng_is_repeatable_and_context_separated() {
        let digest = CircuitDigest::from_instructions(&parse_lines("H 0\nM 0\n").unwrap());
        let key = RandomKey {
            circuit_digest: digest,
            op_path: vec![1],
            repeat_iterations: vec![],
            target_slots: vec![0],
            choice_kind: ChoiceKind::IntrinsicMeasurement,
            subchoice: 0,
        };
        let mut first = KeyedRng::new(42, key.clone());
        let expected = [first.next_u64(), first.next_u64(), first.next_u64()];
        let mut second = KeyedRng::new(42, key.clone());
        assert_eq!(
            expected,
            [second.next_u64(), second.next_u64(), second.next_u64()]
        );

        let mut different = key;
        different.target_slots = vec![1];
        let mut third = KeyedRng::new(42, different);
        assert_ne!(expected[0], third.next_u64());
    }

    #[test]
    fn keyed_rng_resetting_a_key_replays_the_same_bytes() {
        let digest = CircuitDigest::from_instructions(&[]);
        let first_key = RandomKey {
            circuit_digest: digest,
            op_path: vec![0],
            repeat_iterations: vec![],
            target_slots: vec![0],
            choice_kind: ChoiceKind::NoiseOccurrence,
            subchoice: 0,
        };
        let second_key = RandomKey {
            target_slots: vec![1],
            ..first_key.clone()
        };
        let mut rng = KeyedRng::new(7, first_key.clone());
        let value = rng.next_u64();
        rng.set_key(second_key);
        assert_ne!(value, rng.next_u64());
        rng.set_key(first_key);
        assert_eq!(value, rng.next_u64());
    }

    #[test]
    fn noiseless_shot_can_add_restore_undo_and_redo_a_physical_error() {
        let mut shot = EditableShot::open(
            "R 0\nX_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            ExpansionLimits::default(),
            11,
        )
        .unwrap();
        let event_id = shot.session().catalog().events()[0].id.clone();

        let initial = shot.summary();
        assert_eq!(initial.base, ShotBase::Noiseless { seed: 11 });
        assert!(!initial.result.measurements[0].bit);
        assert!(!initial.result.detectors[0].flipped);
        assert!(!initial.result.observables[0].bit);

        shot.set_noise(&event_id, NoiseOutcome::X).unwrap();
        let edited = shot.summary();
        assert!(edited.result.measurements[0].bit);
        assert!(edited.result.detectors[0].flipped);
        assert!(edited.result.observables[0].bit);
        assert_eq!(edited.changed_by_last_action.measurements, vec!["m1"]);
        assert_eq!(edited.changed_by_last_action.detectors, vec!["d0"]);
        assert_eq!(edited.changed_by_last_action.observables, vec!["l0-0"]);
        assert!(edited.can_undo);

        assert!(shot.undo().unwrap());
        assert!(!shot.summary().result.measurements[0].bit);
        assert!(shot.summary().can_redo);
        assert!(shot.redo().unwrap());
        assert!(shot.summary().result.measurements[0].bit);

        shot.restore_noise(&event_id).unwrap();
        assert_eq!(shot.summary().result.noise_events[0].override_outcome, None);
        assert!(!shot.summary().result.measurements[0].bit);
    }

    #[test]
    fn sample_and_clear_create_history_boundaries() {
        let mut shot =
            EditableShot::open("X_ERROR(1) 0\nM 0\n", ExpansionLimits::default(), 1).unwrap();
        let event_id = shot.session().catalog().events()[0].id.clone();
        shot.set_noise(&event_id, NoiseOutcome::X).unwrap();
        assert!(shot.summary().can_undo);

        shot.sample(9).unwrap();
        let sampled = shot.summary();
        assert_eq!(sampled.base, ShotBase::Sampled { seed: 9 });
        assert!(sampled.result.measurements[0].bit);
        assert!(!sampled.can_undo);
        assert!(!sampled.can_redo);
        assert!(!shot.undo().unwrap());

        shot.clear(10).unwrap();
        let cleared = shot.summary();
        assert_eq!(cleared.base, ShotBase::Noiseless { seed: 10 });
        assert!(!cleared.result.measurements[0].bit);
        assert!(!cleared.can_undo);
    }

    #[test]
    fn editing_one_event_preserves_unrelated_intrinsic_measurement_choice() {
        let mut shot = EditableShot::open(
            "H 0\nH 1\nX_ERROR(0.5) 0\nM 0 1\n",
            ExpansionLimits::default(),
            22,
        )
        .unwrap();
        shot.sample(1234).unwrap();
        let before = shot.summary();
        let unrelated = before.result.measurements[1].bit;
        let event_id = shot.session().catalog().events()[0].id.clone();
        shot.set_noise(&event_id, NoiseOutcome::X).unwrap();
        let after = shot.summary();
        assert_eq!(after.result.measurements[1].bit, unrelated);
    }

    #[test]
    fn clear_suppresses_read_only_declared_noise() {
        let shot = EditableShot::open(
            "PAULI_CHANNEL_1(1,0,0) 0\nM 0\n",
            ExpansionLimits::default(),
            7,
        )
        .unwrap();
        let summary = shot.summary();
        assert!(!summary.result.measurements[0].bit);
        assert!(!summary.result.noise_events[0].editable);
        assert_eq!(
            summary.result.noise_events[0].effective_outcome,
            NoiseOutcome::Identity
        );
    }

    #[test]
    fn read_only_noise_reports_realized_outcomes_and_dynamic_applicability() {
        let mut lost = EditableShot::open(
            "LOSS(0) 0\nPAULI_CHANNEL_1(1,0,0) 0\nM 0\n",
            ExpansionLimits::default(),
            7,
        )
        .unwrap();
        let loss_id = lost.session().catalog().events()[0].id.clone();
        lost.set_noise(&loss_id, NoiseOutcome::Lost).unwrap();
        assert!(!lost.summary().result.noise_events[1].applicable);

        let mut heralded = EditableShot::open(
            "HERALDED_PAULI_CHANNEL_1(0,1,0,0) 0\n",
            ExpansionLimits::default(),
            1,
        )
        .unwrap();
        heralded.sample(9).unwrap();
        assert_eq!(
            heralded.summary().result.noise_events[0].effective_outcome,
            NoiseOutcome::X
        );
        assert!(heralded.summary().result.measurements[0].bit);

        let mut correlated = EditableShot::open(
            "CORRELATED_ERROR(1) X0 Y1 Z2\n",
            ExpansionLimits::default(),
            1,
        )
        .unwrap();
        correlated.sample(3).unwrap();
        assert_eq!(
            correlated.summary().result.noise_events[0].effective_outcome,
            NoiseOutcome::Correlated
        );
    }

    #[test]
    fn read_only_noise_sites_keep_stable_interactions_and_channel_parameters() {
        let source = "PAULI_CHANNEL_1(0.1,0.2,0.3) 0 1\n\
                      PAULI_CHANNEL_2(0.01,0.02,0.03,0.04,0.05,0.06,0.07,0.08,0.09,0.1,0.11,0.12,0.13,0.14,0.15) 2 3\n\
                      HERALDED_ERASE(0.4) 4\n\
                      HERALDED_PAULI_CHANNEL_1(0.1,0.2,0.3,0.4) 5\n\
                      CORRELATED_ERROR(0.25) X6 Y7\n";
        let mut shot = EditableShot::open(source, ExpansionLimits::default(), 7).unwrap();
        let first = shot.view_snapshot().unwrap();

        assert_eq!(first.noise_sites.len(), 6);
        assert_eq!(first.shot.result.noise_events.len(), 6);
        assert_eq!(first.svg.matches("class=\"noise-site\"").count(), 6);
        for event in &first.shot.result.noise_events {
            assert!(
                first
                    .svg
                    .contains(&format!("data-noise-event-id=\"{}\"", event.id))
            );
            assert!(!event.editable);
        }

        let channel = first
            .noise_sites
            .iter()
            .find(|site| site.kind == NoiseSiteKind::PauliChannel1)
            .unwrap();
        assert_eq!(channel.parameters, vec![0.1, 0.2, 0.3]);
        assert!((channel.probability.unwrap() - 0.6).abs() < f64::EPSILON);
        assert_eq!(
            serde_json::to_value(channel.kind).unwrap(),
            json!("pauli_channel1")
        );
        let heralded = first
            .noise_sites
            .iter()
            .find(|site| site.kind == NoiseSiteKind::HeraldedPauliChannel1)
            .unwrap();
        assert_eq!(heralded.parameters, vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(heralded.probability, Some(1.0));

        let ids = first
            .shot
            .result
            .noise_events
            .iter()
            .map(|event| event.id.clone())
            .collect::<Vec<_>>();
        shot.sample(11).unwrap();
        assert_eq!(
            shot.summary()
                .result
                .noise_events
                .iter()
                .map(|event| event.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
    }

    #[test]
    fn depolarize2_keyed_branch_has_a_fixed_width_golden_value() {
        let mut shot =
            EditableShot::open("DEPOLARIZE2(1) 0 1\n", ExpansionLimits::default(), 1).unwrap();
        shot.sample(0x0123_4567_89ab_cdef).unwrap();
        assert_eq!(
            shot.summary().result.noise_events[0].effective_outcome,
            NoiseOutcome::PauliPair {
                first: Pauli::Y,
                second: Pauli::X,
            }
        );
    }

    #[test]
    fn preflight_rejects_qubit_and_target_width_before_execution() {
        let qubit_error =
            CircuitSession::open("H 1000000\n", ExpansionLimits::default()).unwrap_err();
        assert!(qubit_error.contains("required qubits 1000001 exceeds limit 256"));

        let many_targets = format!("H {}\n", vec!["0"; 40_000].join(" "));
        let target_error =
            CircuitSession::open(&many_targets, ExpansionLimits::default()).unwrap_err();
        assert!(
            target_error.contains("estimated SVG nodes"),
            "{target_error}"
        );
    }

    #[test]
    fn all_supported_measurement_shapes_enter_the_interactive_result() {
        let shot = EditableShot::open(
            "R 0 1\nMXX 0 1\nMPP X0*X1\nMPAD 0 1\n\
             HERALDED_ERASE(0) 0\nHERALDED_PAULI_CHANNEL_1(0,0,0,0) 1\n",
            ExpansionLimits::default(),
            17,
        )
        .unwrap();
        let measurements = &shot.summary().result.measurements;
        assert_eq!(measurements.len(), 6);
        assert_eq!(
            measurements
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            vec!["m1", "m2", "m3", "m4", "m5", "m6"]
        );
        assert_eq!(
            measurements
                .iter()
                .map(|event| event.instruction.as_str())
                .collect::<Vec<_>>(),
            vec![
                "MXX",
                "MPP",
                "MPAD",
                "MPAD",
                "HERALDED_ERASE",
                "HERALDED_PAULI_CHANNEL_1"
            ]
        );
        let snapshot = shot.view_snapshot().unwrap();
        assert!(
            snapshot
                .warnings
                .iter()
                .all(|warning| !warning.contains("sample annotations are unavailable")),
            "{:?}",
            snapshot.warnings
        );
        let svg = snapshot.svg;
        assert!(svg.contains("sample-trace"));
        for id in ["m1", "m2", "m3", "m4", "m5", "m6"] {
            assert!(svg.contains(id), "interactive SVG is missing {id}");
        }
    }

    #[test]
    fn invalid_edit_is_transactional() {
        let mut shot =
            EditableShot::open("X_ERROR(0.5) 0\nM 0\n", ExpansionLimits::default(), 3).unwrap();
        let event_id = shot.session().catalog().events()[0].id.clone();
        let before = shot.summary();
        let error = shot.set_noise(&event_id, NoiseOutcome::Y).unwrap_err();
        assert!(error.contains("not allowed"));
        assert_eq!(shot.summary(), before);
    }

    #[test]
    fn override_on_lost_qubit_is_preserved_but_ineffective() {
        let mut shot = EditableShot::open(
            "LOSS(0) 0\nX_ERROR(0) 0\nM 0\n",
            ExpansionLimits::default(),
            3,
        )
        .unwrap();
        let loss = shot.session().catalog().events()[0].id.clone();
        let x_error = shot.session().catalog().events()[1].id.clone();
        shot.set_noise(&loss, NoiseOutcome::Lost).unwrap();
        shot.set_noise(&x_error, NoiseOutcome::X).unwrap();
        let unavailable = shot.summary();
        assert!(!unavailable.result.noise_events[1].applicable);
        assert_eq!(
            unavailable.result.noise_events[1].override_outcome,
            Some(NoiseOutcome::X)
        );

        shot.set_noise(&loss, NoiseOutcome::Identity).unwrap();
        let reactivated = shot.summary();
        assert!(reactivated.result.noise_events[1].applicable);
        assert_eq!(
            reactivated.result.noise_events[1].effective_outcome,
            NoiseOutcome::X
        );
    }

    #[test]
    fn view_snapshot_connects_sidecar_ids_to_svg_and_filters_repeat_annotations() {
        let mut shot = EditableShot::open(
            "REPEAT 2 {\n  X_ERROR(0) 0\n  M 0\n  DETECTOR rec[-1]\n  OBSERVABLE_INCLUDE(0) rec[-1]\n}\n",
            ExpansionLimits::default(),
            5,
        )
        .unwrap();
        let second_event = shot.session().catalog().events()[1].id.clone();
        shot.set_noise(&second_event, NoiseOutcome::X).unwrap();
        let snapshot = shot.view_snapshot().unwrap();

        assert_eq!(snapshot.format_version, "rstim-shot-view-v1");
        assert_eq!(
            snapshot.shot.result.noise_events[1].id,
            second_event.encode()
        );
        assert_eq!(snapshot.provenance.overrides.len(), 1);
        assert_eq!(
            snapshot.provenance.overrides[0].event_id,
            second_event.encode()
        );
        assert_eq!(snapshot.svg.matches("class=\"noise-site\"").count(), 2);
        assert_eq!(snapshot.svg.matches("manual-override").count(), 1);
        assert!(snapshot.svg.contains("data-observable-id=\"l0-1\""));
        assert!(snapshot.shot.result.observables[1].bit, "{snapshot:?}");
        assert_eq!(
            snapshot.shot.result.observables[1].repeat_iterations,
            vec![1]
        );
        assert_eq!(snapshot.svg.matches("L0=0").count(), 1);
        let second_observable = snapshot
            .svg
            .split("data-observable-id=\"l0-1\"")
            .nth(1)
            .unwrap();
        assert!(
            second_observable
                .split("</g>")
                .next()
                .unwrap()
                .contains("stroke=\"#2563eb\"")
        );

        let json = serde_json::to_value(&snapshot).unwrap();
        assert!(json["shot"]["result"]["noise_events"][1]["id"].is_string());
        assert_eq!(json["shot"]["base"]["seed"], "5");
    }
}
