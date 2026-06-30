use crate::code::StabilizerCode;
use crate::css::CssCode;
use crate::distance::LogicalClass;
use crate::error::{QecError, Result};
use crate::gf2;
use crate::Pauli;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceBoundMethod {
    RandomizedUpperBound,
    RandomWindowUpperBound,
    Exact,
}

impl DistanceBoundMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RandomizedUpperBound => "randomized-upper-bound",
            Self::RandomWindowUpperBound => "random-window-upper-bound",
            Self::Exact => "exact",
        }
    }
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

pub trait DistanceBoundOptions {
    fn validate(&self) -> Result<()>;
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
        validate_upper_bound_options(self.iterations, self.restarts, self.target_weight)
    }
}

impl DistanceBoundOptions for RandomizedUpperBoundOptions {
    fn validate(&self) -> Result<()> {
        RandomizedUpperBoundOptions::validate(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomWindowUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

impl RandomWindowUpperBoundOptions {
    pub fn validate(&self) -> Result<()> {
        validate_upper_bound_options(self.iterations, self.restarts, self.target_weight)
    }
}

impl DistanceBoundOptions for RandomWindowUpperBoundOptions {
    fn validate(&self) -> Result<()> {
        RandomWindowUpperBoundOptions::validate(self)
    }
}

fn validate_upper_bound_options(
    iterations: usize,
    restarts: usize,
    target_weight: Option<usize>,
) -> Result<()> {
    if iterations == 0 {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "iterations",
            reason: "must be greater than zero".to_owned(),
        });
    }
    if restarts == 0 {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "restarts",
            reason: "must be greater than zero".to_owned(),
        });
    }
    if target_weight == Some(0) {
        return Err(QecError::InvalidDistanceBoundOption {
            option: "target_weight",
            reason: "must be greater than zero when provided".to_owned(),
        });
    }
    Ok(())
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomWindowSearchStats {
    pub permutations_sampled: usize,
    pub kernel_basis_generations: usize,
    pub component_candidates_generated: usize,
    pub zero_candidates_rejected: usize,
    pub stabilizer_span_candidates_rejected: usize,
    pub witness_validation_candidates_rejected: usize,
    pub valid_witnesses_found: usize,
    pub best_witness_updates: usize,
    pub target_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundResult<Options = RandomizedUpperBoundOptions> {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: Options,
    pub provenance: DistanceBoundProvenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_stats: Option<RandomWindowSearchStats>,
}

impl<Options> DistanceBoundResult<Options> {
    fn completed_with_method(
        method: DistanceBoundMethod,
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: Options,
    ) -> Self {
        Self {
            status: DistanceBoundStatus::Completed,
            method,
            bound_type: BoundType::Upper,
            upper_bound,
            logical_class,
            witness,
            options,
            provenance: DistanceBoundProvenance::current(),
            search_stats: None,
        }
    }
}

impl DistanceBoundResult<RandomizedUpperBoundOptions> {
    pub fn completed(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomizedUpperBoundOptions,
    ) -> Self {
        Self::completed_with_method(
            DistanceBoundMethod::RandomizedUpperBound,
            upper_bound,
            logical_class,
            witness,
            options,
        )
    }
}

impl DistanceBoundResult<RandomWindowUpperBoundOptions> {
    pub fn completed_random_window_upper_bound(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomWindowUpperBoundOptions,
    ) -> Self {
        let mut result = Self::completed_with_method(
            DistanceBoundMethod::RandomWindowUpperBound,
            upper_bound,
            logical_class,
            witness,
            options,
        );
        result.search_stats = Some(RandomWindowSearchStats::default());
        result
    }

    fn completed_random_window_upper_bound_with_stats(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomWindowUpperBoundOptions,
        stats: RandomWindowSearchStats,
    ) -> Self {
        let mut result = Self::completed_with_method(
            DistanceBoundMethod::RandomWindowUpperBound,
            upper_bound,
            logical_class,
            witness,
            options,
        );
        result.search_stats = Some(stats);
        result
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

pub fn random_window_css_upper_bound(
    css: &CssCode,
    options: RandomWindowUpperBoundOptions,
) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>> {
    options.validate()?;

    let code = css.code();
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let width = code.n();
    let stabilizer_span = gf2::try_rref_with_width(&code.stabilizer_rows(), width * 2)?;
    let hx_span = gf2::try_rref_with_width(css.hx(), width)?;
    let hz_span = gf2::try_rref_with_width(css.hz(), width)?;
    let mut rng = SplitMix64::new(options.seed);
    let mut best_witness: Option<Pauli> = None;
    let mut search_stats = RandomWindowSearchStats::default();

    for _restart in 0..options.restarts {
        for _iteration in 0..options.iterations {
            let permutation = shuffled_columns(width, &mut rng);
            search_stats.permutations_sampled += 1;
            consider_component_candidates(
                css.hz(),
                &hx_span,
                ComponentKind::XLike,
                width,
                &permutation,
                code,
                &stabilizer_span,
                &mut best_witness,
                &mut search_stats,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                search_stats.target_reached = true;
                return completed_random_window_upper_bound_result(
                    code,
                    best_witness.unwrap(),
                    options,
                    search_stats,
                );
            }

            consider_component_candidates(
                css.hx(),
                &hz_span,
                ComponentKind::ZLike,
                width,
                &permutation,
                code,
                &stabilizer_span,
                &mut best_witness,
                &mut search_stats,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                search_stats.target_reached = true;
                return completed_random_window_upper_bound_result(
                    code,
                    best_witness.unwrap(),
                    options,
                    search_stats,
                );
            }
        }
    }

    let witness = best_witness.ok_or(QecError::RandomizedUpperBoundWitnessNotFound)?;
    completed_random_window_upper_bound_result(code, witness, options, search_stats)
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

#[derive(Debug, Clone, Copy)]
enum ComponentKind {
    XLike,
    ZLike,
}

fn consider_component_candidates(
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    width: usize,
    permutation: &[usize],
    code: &StabilizerCode,
    stabilizer_span: &gf2::ReducedRows,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.kernel_basis_generations += 1;
    let candidates =
        gf2::try_random_window_kernel_basis_with_width(kernel_checks, width, permutation)?;
    search_stats.component_candidates_generated += candidates.len();

    for candidate in candidates {
        if !candidate.iter().any(|bit| *bit == 1) {
            search_stats.zero_candidates_rejected += 1;
            continue;
        }
        if gf2::try_in_reduced_row_span(stabilizer_component_span, &candidate)? {
            search_stats.stabilizer_span_candidates_rejected += 1;
            continue;
        }

        let witness = component_candidate_to_pauli(component, candidate)?;
        if validate_witness_against_code_with_span(code, stabilizer_span, &witness).is_err() {
            search_stats.witness_validation_candidates_rejected += 1;
            continue;
        }
        search_stats.valid_witnesses_found += 1;
        if best_witness
            .as_ref()
            .is_none_or(|current| witness.weight() < current.weight())
        {
            search_stats.best_witness_updates += 1;
            *best_witness = Some(witness);
        }
    }

    Ok(())
}

fn component_candidate_to_pauli(component: ComponentKind, candidate: Vec<u8>) -> Result<Pauli> {
    let width = candidate.len();
    match component {
        ComponentKind::XLike => Pauli::from_xz_bits(candidate, vec![0; width]),
        ComponentKind::ZLike => Pauli::from_xz_bits(vec![0; width], candidate),
    }
}

fn shuffled_columns(width: usize, rng: &mut SplitMix64) -> Vec<usize> {
    let mut permutation = (0..width).collect::<Vec<_>>();
    for i in (1..width).rev() {
        let j = rng.next_usize(i + 1);
        permutation.swap(i, j);
    }
    permutation
}

fn target_reached(best_witness: &Option<Pauli>, target_weight: Option<usize>) -> bool {
    best_witness
        .as_ref()
        .is_some_and(|witness| target_weight.is_some_and(|target| witness.weight() <= target))
}

fn completed_random_window_upper_bound_result(
    code: &StabilizerCode,
    witness: Pauli,
    options: RandomWindowUpperBoundOptions,
    search_stats: RandomWindowSearchStats,
) -> Result<DistanceBoundResult<RandomWindowUpperBoundOptions>> {
    let result = DistanceBoundResult::completed_random_window_upper_bound_with_stats(
        witness.weight(),
        classify_witness_support(&witness),
        DistanceBoundWitness::from_pauli(&witness),
        options,
        search_stats,
    );
    validate_random_window_upper_bound_result(
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Issue225LadderCase {
    pub case_id: String,
    pub source_issue: u64,
    pub code_id: String,
    pub expected_upper_bound: usize,
    pub target_weight: usize,
    pub tier: String,
    pub run_mode: String,
}

#[derive(Debug, Clone, Copy)]
pub struct MethodAwareBoundValidationContext<'a> {
    pub code: &'a StabilizerCode,
    pub expected_method: DistanceBoundMethod,
    pub known_exact_distance: Option<usize>,
}

pub fn validate_randomized_upper_bound_result(
    result: &DistanceBoundResult,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: context.code,
            expected_method: DistanceBoundMethod::RandomizedUpperBound,
            known_exact_distance: context.known_exact_distance,
        },
    )
}

pub fn validate_random_window_upper_bound_result(
    result: &DistanceBoundResult<RandomWindowUpperBoundOptions>,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: context.code,
            expected_method: DistanceBoundMethod::RandomWindowUpperBound,
            known_exact_distance: context.known_exact_distance,
        },
    )
}

pub fn validate_distance_bound_result<Options: DistanceBoundOptions>(
    result: &DistanceBoundResult<Options>,
    context: MethodAwareBoundValidationContext<'_>,
) -> Result<()> {
    result.options.validate()?;

    if result.method != context.expected_method {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "expected method {}, got {}",
            context.expected_method.label(),
            result.method.label()
        )));
    }
    if result.bound_type != BoundType::Upper {
        return Err(QecError::DistanceBoundValidationFailed(
            "distance bound results must use bound_type upper".to_owned(),
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

pub fn verify_issue_225_ladder_case<Options: DistanceBoundOptions>(
    case: &Issue225LadderCase,
    result: &DistanceBoundResult<Options>,
    css: &CssCode,
    expected_method: DistanceBoundMethod,
) -> Result<()> {
    if result.method != expected_method {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "{} expected method {}, got {}",
            case.case_id,
            expected_method.label(),
            result.method.label()
        )));
    }

    validate_distance_bound_result(
        result,
        MethodAwareBoundValidationContext {
            code: css.code(),
            expected_method,
            known_exact_distance: None,
        },
    )
    .map_err(|error| prefix_ladder_case_error(&case.case_id, error))?;

    if result.upper_bound > case.expected_upper_bound {
        return Err(QecError::DistanceBoundValidationFailed(format!(
            "{} expected upper_bound <= {}, got {}",
            case.case_id, case.expected_upper_bound, result.upper_bound
        )));
    }

    Ok(())
}

fn prefix_ladder_case_error(case_id: &str, error: QecError) -> QecError {
    match error {
        QecError::DistanceBoundValidationFailed(message) => {
            QecError::DistanceBoundValidationFailed(format!("{case_id} {message}"))
        }
        other => QecError::DistanceBoundValidationFailed(format!("{case_id} {other}")),
    }
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
    let stabilizer_rows = code.stabilizer_rows();
    let stabilizer_span = gf2::try_rref_with_width(&stabilizer_rows, code.n() * 2)?;
    validate_witness_against_code_with_span(code, &stabilizer_span, witness)
}

fn validate_witness_against_code_with_span(
    code: &StabilizerCode,
    stabilizer_span: &gf2::ReducedRows,
    witness: &Pauli,
) -> Result<()> {
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
    if gf2::try_in_reduced_row_span(stabilizer_span, &witness.to_symplectic_row())? {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_validation_rejects_identity_witness() {
        let code = StabilizerCode::from_stabilizers(1, vec![]).unwrap();
        let witness = Pauli::from_xz_bits(vec![0], vec![0]).unwrap();

        assert_eq!(
            validate_witness_against_code(&code, &witness),
            Err(QecError::DistanceBoundValidationFailed(
                "witness must be non-identity".to_owned(),
            ))
        );
    }
}
