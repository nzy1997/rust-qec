use crate::Pauli;
use crate::code::StabilizerCode;
use crate::css::CssCode;
use crate::distance::LogicalClass;
use crate::error::{QecError, Result};
use crate::gf2;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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
    pub weight_pruned_candidates: usize,
    pub stabilizer_span_candidates_rejected: usize,
    pub witness_validation_candidates_rejected: usize,
    pub valid_witnesses_found: usize,
    pub best_witness_updates: usize,
    pub target_reached: bool,
    pub permutation_time_ns: u64,
    pub kernel_basis_time_ns: u64,
    pub span_filter_time_ns: u64,
    pub witness_validation_time_ns: u64,
    pub best_update_time_ns: u64,
    pub total_search_time_ns: u64,
}

fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}

fn add_elapsed_ns(total: &mut u64, started: Instant) {
    *total = total.saturating_add(duration_ns(started.elapsed()));
}

fn finish_search_timing(search_stats: &mut RandomWindowSearchStats, started: Instant) {
    search_stats.total_search_time_ns = duration_ns(started.elapsed()).max(1);
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
    let hx_span = gf2::try_rref_with_width(css.hx(), width)?;
    let hz_span = gf2::try_rref_with_width(css.hz(), width)?;
    let mut rng = SplitMix64::new(options.seed);
    let mut best_witness: Option<Pauli> = None;
    let mut search_stats = RandomWindowSearchStats::default();
    let mut kernel_workspace = gf2::RandomWindowKernelWorkspace::new();
    let search_started = Instant::now();

    for _restart in 0..options.restarts {
        for _iteration in 0..options.iterations {
            let permutation_started = Instant::now();
            let permutation = shuffled_columns(width, &mut rng);
            add_elapsed_ns(&mut search_stats.permutation_time_ns, permutation_started);
            search_stats.permutations_sampled += 1;
            consider_component_candidates(
                css.hz(),
                &hx_span,
                ComponentKind::XLike,
                width,
                &permutation,
                &mut kernel_workspace,
                &mut best_witness,
                &mut search_stats,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                search_stats.target_reached = true;
                finish_search_timing(&mut search_stats, search_started);
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
                &mut kernel_workspace,
                &mut best_witness,
                &mut search_stats,
            )?;
            if target_reached(&best_witness, options.target_weight) {
                search_stats.target_reached = true;
                finish_search_timing(&mut search_stats, search_started);
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
    finish_search_timing(&mut search_stats, search_started);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssComponentCandidateVerdict {
    Accepted,
    Zero,
    NonKernel,
    StabilizerSpan,
}

fn css_component_candidate_verdict(
    opposite_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    candidate: &[u8],
) -> Result<CssComponentCandidateVerdict> {
    let width = stabilizer_component_span.width;
    gf2::validate_rows_with_width(opposite_checks, width)?;
    gf2::validate_target(candidate)?;
    if candidate.len() != width {
        return Err(QecError::RowWidthMismatch {
            expected: width,
            actual: candidate.len(),
        });
    }
    if !candidate.iter().any(|bit| *bit == 1) {
        return Ok(CssComponentCandidateVerdict::Zero);
    }

    for check in opposite_checks {
        let parity = check
            .iter()
            .zip(candidate)
            .fold(0, |acc, (&check_bit, &candidate_bit)| {
                acc ^ (check_bit & candidate_bit)
            });
        if parity != 0 {
            return Ok(CssComponentCandidateVerdict::NonKernel);
        }
    }

    if gf2::try_in_reduced_row_span(stabilizer_component_span, candidate)? {
        return Ok(CssComponentCandidateVerdict::StabilizerSpan);
    }

    Ok(CssComponentCandidateVerdict::Accepted)
}

fn consider_component_candidates(
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    width: usize,
    permutation: &[usize],
    kernel_workspace: &mut gf2::RandomWindowKernelWorkspace,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.kernel_basis_generations += 1;
    let kernel_started = Instant::now();
    let candidates =
        kernel_workspace.try_kernel_basis_with_width(kernel_checks, width, permutation);
    add_elapsed_ns(&mut search_stats.kernel_basis_time_ns, kernel_started);
    let candidates = candidates?;

    consider_component_candidate_rows(
        candidates,
        kernel_checks,
        stabilizer_component_span,
        component,
        best_witness,
        search_stats,
    )
}

fn consider_component_candidate_rows(
    candidates: &[Vec<u8>],
    kernel_checks: &[Vec<u8>],
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.component_candidates_generated += candidates.len();

    for candidate in candidates {
        let span_started = Instant::now();
        let candidate_weight = candidate.iter().filter(|&&bit| bit == 1).count();
        if candidate_weight == 0 {
            add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
            search_stats.zero_candidates_rejected += 1;
            continue;
        }
        if best_witness
            .as_ref()
            .is_some_and(|current| candidate_weight >= current.weight())
        {
            add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
            search_stats.weight_pruned_candidates += 1;
            continue;
        }
        let component_verdict =
            css_component_candidate_verdict(kernel_checks, stabilizer_component_span, &candidate)?;
        add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
        match component_verdict {
            CssComponentCandidateVerdict::Accepted => {}
            CssComponentCandidateVerdict::Zero => {
                search_stats.zero_candidates_rejected += 1;
                continue;
            }
            CssComponentCandidateVerdict::NonKernel => {
                search_stats.witness_validation_candidates_rejected += 1;
                continue;
            }
            CssComponentCandidateVerdict::StabilizerSpan => {
                search_stats.stabilizer_span_candidates_rejected += 1;
                continue;
            }
        }

        let validation_started = Instant::now();
        let witness = component_candidate_to_pauli(component, candidate)?;
        add_elapsed_ns(
            &mut search_stats.witness_validation_time_ns,
            validation_started,
        );
        search_stats.valid_witnesses_found += 1;
        let best_update_started = Instant::now();
        let should_update = best_witness
            .as_ref()
            .is_none_or(|current| witness.weight() < current.weight());
        if should_update {
            search_stats.best_witness_updates += 1;
            *best_witness = Some(witness);
        }
        add_elapsed_ns(&mut search_stats.best_update_time_ns, best_update_started);
    }

    Ok(())
}

fn component_candidate_to_pauli(component: ComponentKind, candidate: &[u8]) -> Result<Pauli> {
    let width = candidate.len();
    match component {
        ComponentKind::XLike => Pauli::from_xz_bits(candidate.to_vec(), vec![0; width]),
        ComponentKind::ZLike => Pauli::from_xz_bits(vec![0; width], candidate.to_vec()),
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
    use crate::codes::built_in_css::built_in_css_checks;
    use crate::css::{CssCode, SparseRowsMatrix};

    fn empty_reduced_rows(width: usize) -> gf2::ReducedRows {
        gf2::try_rref_with_width(&[], width).unwrap()
    }

    fn css_from_sparse_rows(num_cols: usize, hx: Vec<Vec<usize>>, hz: Vec<Vec<usize>>) -> CssCode {
        let hx = SparseRowsMatrix::new(num_cols, hx).unwrap().to_dense_rows();
        let hz = SparseRowsMatrix::new(num_cols, hz).unwrap().to_dense_rows();
        CssCode::from_hx_hz(hx, hz).unwrap()
    }

    fn css_from_built_in_code_id(code_id: &str) -> CssCode {
        let checks = built_in_css_checks(code_id).unwrap();
        css_from_sparse_rows(checks.num_cols, checks.hx, checks.hz)
    }

    fn first_non_kernel_candidate(checks: &[Vec<u8>], width: usize) -> Vec<u8> {
        let column = (0..width)
            .find(|&column| checks.iter().any(|row| row[column] == 1))
            .expect("expected at least one nonzero check column");
        let mut candidate = vec![0; width];
        candidate[column] = 1;
        candidate
    }

    fn full_validator_component_verdict(
        code: &StabilizerCode,
        stabilizer_span: &gf2::ReducedRows,
        component: ComponentKind,
        candidate: &[u8],
    ) -> Result<CssComponentCandidateVerdict> {
        let witness = component_candidate_to_pauli(component, candidate)?;
        match validate_witness_against_code_with_span(code, stabilizer_span, &witness) {
            Ok(()) => Ok(CssComponentCandidateVerdict::Accepted),
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness must be non-identity" =>
            {
                Ok(CssComponentCandidateVerdict::Zero)
            }
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness does not commute with stabilizers" =>
            {
                Ok(CssComponentCandidateVerdict::NonKernel)
            }
            Err(QecError::DistanceBoundValidationFailed(message))
                if message == "witness lies in stabilizer span" =>
            {
                Ok(CssComponentCandidateVerdict::StabilizerSpan)
            }
            Err(error) => Err(error),
        }
    }

    fn x_pauli(width: usize, support: &[usize]) -> Pauli {
        let mut x = vec![0; width];
        for &index in support {
            x[index] = 1;
        }
        Pauli::from_xz_bits(x, vec![0; width]).unwrap()
    }

    #[test]
    fn random_window_prunes_candidates_that_cannot_improve_best() {
        let width = 3;
        let component_span = empty_reduced_rows(width);
        let mut best_witness = Some(x_pauli(width, &[0, 1]));
        let mut search_stats = RandomWindowSearchStats::default();

        consider_component_candidate_rows(
            &[vec![0, 0, 0], vec![1, 1, 0], vec![1, 1, 1], vec![0, 0, 1]],
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();

        let best = best_witness.expect("strictly lighter candidate should replace current best");
        assert_eq!(best.weight(), 1);
        assert_eq!(search_stats.component_candidates_generated, 4);
        assert_eq!(search_stats.zero_candidates_rejected, 1);
        assert_eq!(search_stats.weight_pruned_candidates, 2);
        assert_eq!(search_stats.valid_witnesses_found, 1);
        assert_eq!(search_stats.best_witness_updates, 1);
        assert_eq!(search_stats.stabilizer_span_candidates_rejected, 0);
        assert_eq!(search_stats.witness_validation_candidates_rejected, 0);

        let stats_json = serde_json::to_value(search_stats).unwrap();
        assert_eq!(stats_json["weight_pruned_candidates"], 2);
    }

    #[test]
    fn random_window_pruning_does_not_skip_strictly_better_candidate() {
        let width = 5;
        let component_span = empty_reduced_rows(width);
        let mut best_witness = Some(x_pauli(width, &[0, 1, 2, 3, 4]));
        let mut search_stats = RandomWindowSearchStats::default();

        consider_component_candidate_rows(
            &[vec![1, 1, 1, 0, 0]],
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();

        let best = best_witness.expect("weight-3 candidate should replace weight-5 best");
        assert_eq!(best.weight(), 3);
        assert_eq!(search_stats.component_candidates_generated, 1);
        assert_eq!(search_stats.weight_pruned_candidates, 0);
        assert_eq!(search_stats.valid_witnesses_found, 1);
        assert_eq!(search_stats.best_witness_updates, 1);
    }

    #[test]
    fn random_window_candidate_rows_accepts_workspace_output_without_stale_rows() {
        let width = 3;
        let component_span = empty_reduced_rows(width);
        let mut workspace = gf2::RandomWindowKernelWorkspace::new();
        let permutation = vec![2, 0, 1];
        let candidates = workspace
            .try_kernel_basis_with_width(&[], width, &permutation)
            .unwrap();
        assert_eq!(candidates, &[vec![0, 0, 1], vec![1, 0, 0], vec![0, 1, 0],]);

        let mut best_witness = Some(x_pauli(width, &[0, 1]));
        let mut search_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            candidates,
            &[],
            &component_span,
            ComponentKind::XLike,
            &mut best_witness,
            &mut search_stats,
        )
        .unwrap();

        let best = best_witness.expect("workspace candidate should update the best witness");
        assert_eq!(best.weight(), 1);
        assert_eq!(search_stats.component_candidates_generated, 3);
        assert_eq!(search_stats.weight_pruned_candidates, 2);
        assert_eq!(search_stats.valid_witnesses_found, 1);
        assert_eq!(search_stats.best_witness_updates, 1);
    }

    #[test]
    fn random_window_component_filter_matches_full_witness_validation() {
        for code_id in ["surface_rotated:d=3", "bb72"] {
            let css = css_from_built_in_code_id(code_id);
            let width = css.code().n();
            let stabilizer_span =
                gf2::try_rref_with_width(&css.code().stabilizer_rows(), width * 2).unwrap();
            let identity_permutation = (0..width).collect::<Vec<_>>();

            for (component, kernel_checks, component_span_rows) in [
                (ComponentKind::XLike, css.hz(), css.hx()),
                (ComponentKind::ZLike, css.hx(), css.hz()),
            ] {
                let component_span = gf2::try_rref_with_width(component_span_rows, width).unwrap();
                let mut candidates = Vec::new();
                candidates.push(vec![0; width]);
                candidates.push(first_non_kernel_candidate(kernel_checks, width));
                if let Some(span_row) = component_span_rows.first() {
                    candidates.push(span_row.clone());
                }
                candidates.extend(
                    gf2::try_random_window_kernel_basis_with_width(
                        kernel_checks,
                        width,
                        &identity_permutation,
                    )
                    .unwrap(),
                );

                let mut accepted = 0;
                let mut non_kernel_rejected = 0;
                let mut stabilizer_span_rejected = 0;
                for candidate in candidates {
                    let component_verdict =
                        css_component_candidate_verdict(kernel_checks, &component_span, &candidate)
                            .unwrap();
                    let full_verdict = full_validator_component_verdict(
                        css.code(),
                        &stabilizer_span,
                        component,
                        &candidate,
                    )
                    .unwrap();

                    assert_eq!(
                        component_verdict, full_verdict,
                        "{code_id} {component:?} candidate {candidate:?}"
                    );
                    match component_verdict {
                        CssComponentCandidateVerdict::Accepted => accepted += 1,
                        CssComponentCandidateVerdict::NonKernel => non_kernel_rejected += 1,
                        CssComponentCandidateVerdict::StabilizerSpan => {
                            stabilizer_span_rejected += 1
                        }
                        CssComponentCandidateVerdict::Zero => {}
                    }
                }

                assert!(
                    accepted > 0,
                    "{code_id} {component:?} should have accepted rows"
                );
                assert!(
                    non_kernel_rejected > 0,
                    "{code_id} {component:?} should exercise non-kernel rejection"
                );
                assert!(
                    stabilizer_span_rejected > 0,
                    "{code_id} {component:?} should exercise stabilizer-span rejection"
                );
            }
        }
    }

    #[test]
    fn random_window_component_filter_rejects_non_kernel_and_stabilizer_span_candidates() {
        let css = css_from_sparse_rows(3, vec![vec![0, 1]], vec![vec![2]]);
        let width = css.code().n();

        let hx_span = gf2::try_rref_with_width(css.hx(), width).unwrap();
        let mut x_best = None;
        let mut x_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            &[vec![0, 0, 1], vec![1, 1, 0]],
            css.hz(),
            &hx_span,
            ComponentKind::XLike,
            &mut x_best,
            &mut x_stats,
        )
        .unwrap();
        assert!(x_best.is_none());
        assert_eq!(x_stats.component_candidates_generated, 2);
        assert_eq!(x_stats.witness_validation_candidates_rejected, 1);
        assert_eq!(x_stats.stabilizer_span_candidates_rejected, 1);
        assert_eq!(x_stats.valid_witnesses_found, 0);
        assert_eq!(x_stats.best_witness_updates, 0);

        let hz_span = gf2::try_rref_with_width(css.hz(), width).unwrap();
        let mut z_best = None;
        let mut z_stats = RandomWindowSearchStats::default();
        consider_component_candidate_rows(
            &[vec![1, 0, 0], vec![0, 0, 1]],
            css.hx(),
            &hz_span,
            ComponentKind::ZLike,
            &mut z_best,
            &mut z_stats,
        )
        .unwrap();
        assert!(z_best.is_none());
        assert_eq!(z_stats.component_candidates_generated, 2);
        assert_eq!(z_stats.witness_validation_candidates_rejected, 1);
        assert_eq!(z_stats.stabilizer_span_candidates_rejected, 1);
        assert_eq!(z_stats.valid_witnesses_found, 0);
        assert_eq!(z_stats.best_witness_updates, 0);
    }

    #[test]
    fn random_window_component_filter_reports_validation_errors() {
        let span = empty_reduced_rows(3);

        assert_eq!(
            css_component_candidate_verdict(&[], &span, &[1, 0]).unwrap_err(),
            QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            }
        );
        assert_eq!(
            css_component_candidate_verdict(&[], &span, &[1, 2, 0]).unwrap_err(),
            QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            }
        );
        assert_eq!(
            css_component_candidate_verdict(&[vec![1, 0]], &span, &[1, 0, 0]).unwrap_err(),
            QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            }
        );
        assert_eq!(
            css_component_candidate_verdict(&[vec![1, 2, 0]], &span, &[1, 0, 0]).unwrap_err(),
            QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            }
        );
    }

    #[test]
    fn full_validator_component_verdict_propagates_unexpected_errors() {
        let code = StabilizerCode::from_stabilizers(2, vec![]).unwrap();
        let stabilizer_span = empty_reduced_rows(4);

        assert_eq!(
            full_validator_component_verdict(
                &code,
                &stabilizer_span,
                ComponentKind::XLike,
                &[1, 0, 0],
            )
            .unwrap_err(),
            QecError::RowWidthMismatch {
                expected: 4,
                actual: 6,
            }
        );
    }

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
