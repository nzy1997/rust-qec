use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::css::CssCode;
use crate::distance::LogicalClass;
use crate::error::{QecError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceBoundMethod {
    RandomizedUpperBound,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundType {
    Upper,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceBoundStatus {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizedUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

impl RandomizedUpperBoundOptions {
    pub fn validate(&self) -> Result<()> {
        if self.iterations == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "iterations",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.restarts == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "restarts",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.target_weight == Some(0) {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "target_weight",
                reason: "must be greater than zero when provided".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundWitness {
    pub x: Vec<u8>,
    pub z: Vec<u8>,
    pub weight: usize,
}

impl DistanceBoundWitness {
    pub fn from_pauli(pauli: &Pauli) -> Self {
        Self {
            x: pauli.x_bits().to_vec(),
            z: pauli.z_bits().to_vec(),
            weight: pauli.weight(),
        }
    }

    pub fn to_pauli(&self) -> Result<Pauli> {
        Pauli::from_xz_bits(self.x.clone(), self.z.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

impl DistanceBoundProvenance {
    pub fn current() -> Self {
        Self {
            tool: "qec-code".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            method_revision: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundResult {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: RandomizedUpperBoundOptions,
    pub provenance: DistanceBoundProvenance,
}

impl DistanceBoundResult {
    pub fn completed(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomizedUpperBoundOptions,
    ) -> Self {
        Self {
            status: DistanceBoundStatus::Completed,
            method: DistanceBoundMethod::RandomizedUpperBound,
            bound_type: BoundType::Upper,
            upper_bound,
            logical_class,
            witness,
            options,
            provenance: DistanceBoundProvenance::current(),
        }
    }
}

pub fn randomized_css_upper_bound(
    css: &CssCode,
    options: RandomizedUpperBoundOptions,
) -> Result<DistanceBoundResult> {
    options.validate()?;

    let code = css.code();
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let basis = code.canonical_logical_basis()?;
    let logical_rows = basis
        .logical_x
        .iter()
        .chain(&basis.logical_z)
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();
    let stabilizer_rows = code.stabilizer_rows();
    let mut rng = SplitMix64::new(options.seed);
    let mut best_witness: Option<Pauli> = None;

    for _restart in 0..options.restarts {
        for _iteration in 0..options.iterations {
            let candidate_row =
                sampled_logical_plus_stabilizer_row(&logical_rows, &stabilizer_rows, &mut rng);
            let candidate = Pauli::from_symplectic_row(candidate_row)?;

            if validate_witness_against_code(code, &candidate).is_err() {
                continue;
            }

            let replace = match &best_witness {
                Some(current) => candidate.weight() < current.weight(),
                None => true,
            };
            if replace {
                best_witness = Some(candidate);
            }

            if best_witness.as_ref().is_some_and(|witness| {
                options
                    .target_weight
                    .is_some_and(|target| witness.weight() <= target)
            }) {
                return completed_randomized_upper_bound_result(
                    code,
                    best_witness.unwrap(),
                    options,
                );
            }
        }
    }

    let witness = best_witness.ok_or(QecError::RandomizedUpperBoundWitnessNotFound)?;
    completed_randomized_upper_bound_result(code, witness, options)
}

fn completed_randomized_upper_bound_result(
    code: &StabilizerCode,
    witness: Pauli,
    options: RandomizedUpperBoundOptions,
) -> Result<DistanceBoundResult> {
    let result = DistanceBoundResult::completed(
        witness.weight(),
        classify_witness_support(&witness),
        DistanceBoundWitness::from_pauli(&witness),
        options,
    );
    validate_randomized_upper_bound_result(
        &result,
        BoundValidationContext {
            code,
            known_exact_distance: None,
        },
    )?;
    Ok(result)
}

fn sampled_logical_plus_stabilizer_row(
    logical_rows: &[Vec<u8>],
    stabilizer_rows: &[Vec<u8>],
    rng: &mut SplitMix64,
) -> Vec<u8> {
    let width = logical_rows
        .first()
        .or_else(|| stabilizer_rows.first())
        .map(Vec::len)
        .unwrap_or(0);
    let mut row = vec![0; width];
    let mut selected_logical = false;

    for logical in logical_rows {
        if rng.next_bool() {
            xor_assign(&mut row, logical);
            selected_logical = true;
        }
    }
    if !selected_logical {
        let index = rng.next_usize(logical_rows.len());
        xor_assign(&mut row, &logical_rows[index]);
    }

    for stabilizer in stabilizer_rows {
        if rng.next_bool() {
            xor_assign(&mut row, stabilizer);
        }
    }

    row
}

fn xor_assign(target: &mut [u8], source: &[u8]) {
    for (target_bit, source_bit) in target.iter_mut().zip(source) {
        *target_bit ^= *source_bit;
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^ (value >> 31)
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    fn next_usize(&mut self, upper_bound: usize) -> usize {
        debug_assert!(upper_bound > 0);
        (self.next_u64() as usize) % upper_bound
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundValidationContext<'a> {
    pub code: &'a StabilizerCode,
    pub known_exact_distance: Option<usize>,
}

pub fn validate_randomized_upper_bound_result(
    result: &DistanceBoundResult,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    result.options.validate()?;

    if result.method != DistanceBoundMethod::RandomizedUpperBound {
        return Err(QecError::DistanceBoundValidationFailed(
            "distance bound method must be randomized-upper-bound".to_owned(),
        ));
    }
    if result.bound_type != BoundType::Upper {
        return Err(QecError::DistanceBoundValidationFailed(
            "randomized-upper-bound results must use bound_type upper".to_owned(),
        ));
    }
    if result.upper_bound == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "completed upper_bound must be positive".to_owned(),
        ));
    }
    if result.upper_bound != result.witness.weight {
        return Err(QecError::DistanceBoundValidationFailed(
            "upper_bound must equal witness weight".to_owned(),
        ));
    }

    let witness = result.witness.to_pauli()?;
    if witness.n() != context.code.n() {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness width must match code length".to_owned(),
        ));
    }
    if witness.weight() == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ));
    }
    if result.witness.weight != witness.weight() {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness weight field must equal Pauli weight".to_owned(),
        ));
    }
    if result.logical_class != classify_witness_support(&witness) {
        return Err(QecError::DistanceBoundValidationFailed(
            "logical_class must match witness support".to_owned(),
        ));
    }
    validate_witness_against_code(context.code, &witness)?;

    if let Some(known_exact_distance) = context.known_exact_distance {
        if result.upper_bound < known_exact_distance {
            return Err(QecError::DistanceBoundValidationFailed(format!(
                "upper_bound {} is below known exact distance {}",
                result.upper_bound, known_exact_distance
            )));
        }
    }

    Ok(())
}

fn classify_witness_support(witness: &Pauli) -> LogicalClass {
    let has_x = witness.x_bits().contains(&1);
    let has_z = witness.z_bits().contains(&1);

    match (has_x, has_z) {
        (true, false) => LogicalClass::XLike,
        (false, true) => LogicalClass::ZLike,
        (true, true) => LogicalClass::Mixed,
        (false, false) => unreachable!("witness support classification requires non-identity"),
    }
}

fn validate_witness_against_code(code: &StabilizerCode, witness: &Pauli) -> Result<()> {
    if witness.weight() == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ));
    }
    for stabilizer in code.stabilizers() {
        if !witness.try_commutes_with(stabilizer)? {
            return Err(QecError::DistanceBoundValidationFailed(
                "witness does not commute with stabilizers".to_owned(),
            ));
        }
    }
    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ));
    }
    Ok(())
}
