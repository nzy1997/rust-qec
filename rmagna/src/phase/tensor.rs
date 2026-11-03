//! Raw affine/quadratic/parity path tensors and symbolic bra-boundary traces.
use super::*;
use crate::pauli::PauliWord;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathTensorError {
    DimensionOverflow,
    Algebra(PhaseError),
}
impl fmt::Display for PathTensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionOverflow => {
                write!(f, "phase-path tensor dimensions or geometry overflow")
            }
            Self::Algebra(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for PathTensorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DimensionOverflow => None,
            Self::Algebra(e) => Some(e),
        }
    }
}
impl From<PhaseError> for PathTensorError {
    fn from(e: PhaseError) -> Self {
        Self::Algebra(e)
    }
}
impl From<Gf2Error> for PathTensorError {
    fn from(e: Gf2Error) -> Self {
        Self::Algebra(e.into())
    }
}
impl From<QuadraticError> for PathTensorError {
    fn from(e: QuadraticError) -> Self {
        Self::Algebra(e.into())
    }
}
impl From<ScalarError> for PathTensorError {
    fn from(e: ScalarError) -> Self {
        Self::Algebra(e.into())
    }
}
impl From<StabilizerError> for PathTensorError {
    fn from(e: StabilizerError) -> Self {
        Self::Algebra(e.into())
    }
}

/// Limits for symbolic trace construction, separate from phase decomposition.
#[derive(Clone, Copy, Debug)]
pub struct PathTraceOptions {
    pub max_physical_bits: usize,
    pub max_retained_rows: usize,
    pub max_event_rows: usize,
    /// Twice the input's logical path count, before any cancellation.
    pub max_doubled_path_bits: usize,
    /// Conservative new packed, linear and index words, including joint copies,
    /// equality/auxiliary rows and Gaussian scratch. Excludes caller-owned input,
    /// scalar limbs, allocator overhead and subsequent phase-resource work.
    pub max_geometry_words: usize,
}
impl Default for PathTraceOptions {
    fn default() -> Self {
        Self {
            max_physical_bits: 65_536,
            max_retained_rows: 65_536,
            max_event_rows: 65_536,
            max_doubled_path_bits: 4096,
            max_geometry_words: 16 << 20,
        }
    }
}
/// Ordered physical bra parities, one-sided bra events and a physical Pauli.
/// The trace sums `conj(psi(bra))*[P psi](bra)` over labels satisfying the
/// events. This is `<psi|Pi P|psi>`, not two-sided `Pi P Pi` postselection.
#[derive(Clone, Copy, Debug)]
pub struct PathTraceBoundary<'a> {
    pub retained_rows: &'a [BitVector],
    pub event_rows: &'a [BitVector],
    pub outcomes: &'a BitVector,
    pub word: &'a PauliWord,
}

/// Unnormalized amplitudes `coefficient * i^q(u) * omega^p(u)` at an affine
/// boundary label. Odd parity phases refer to the ket's logical path columns.
/// Columns may be dependent. This raw tensor need not be a quantum state or a
/// positive probability distribution; retained observable traces may be complex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhasePathTensor {
    boundary: AffineKet,
    phase: ParityPhase,
}
fn limit(name: &'static str, required: usize, cap: usize) -> Result<(), PathTensorError> {
    if required > cap {
        return Err(PhaseError::ResourceLimit {
            option: name,
            required: required as u128,
            limit: cap as u128,
        }
        .into());
    }
    Ok(())
}

struct TraceShape {
    half: usize,
    joint_width: usize,
    events: usize,
    kept: usize,
}
fn trace_shape(
    m: usize,
    n: usize,
    labels: usize,
    event_count: usize,
    phase_terms: usize,
    options: PathTraceOptions,
) -> Result<TraceShape, PathTensorError> {
    let add = |x: usize, y: usize| x.checked_add(y).ok_or(PathTensorError::DimensionOverflow);
    let paths = n.checked_mul(2).ok_or(PathTensorError::DimensionOverflow)?;
    limit(
        "path_trace.max_doubled_path_bits",
        paths,
        options.max_doubled_path_bits,
    )?;
    let half = add(m, n)?;
    let joint_width = half
        .checked_mul(2)
        .ok_or(PathTensorError::DimensionOverflow)?;
    let events = add(m, event_count)?;

    let kept = add(labels, paths)?;
    let p = paths as u128;
    let e = events as u128;
    let l = kept as u128;
    let t = phase_terms as u128;
    let products = [
        (16 * (e + l + 1), (joint_width as u128).div_ceil(64)),
        (32 * (e + l + p + 1), p.div_ceil(64)),
        (16 * (p + 1), p + 1),
        (32 * (m as u128 + half as u128 + e + l + p + 1), 1),
        (4 * (t + 1), l.div_ceil(64) + p.div_ceil(64) + 8),
    ];
    let geometry = products.into_iter().try_fold(0_u128, |sum, (x, y)| {
        x.checked_mul(y)
            .and_then(|v| sum.checked_add(v))
            .ok_or(PathTensorError::DimensionOverflow)
    })?;
    if geometry > options.max_geometry_words as u128 {
        return Err(PhaseError::ResourceLimit {
            option: "path_trace.max_geometry_words",
            required: geometry,
            limit: options.max_geometry_words as u128,
        }
        .into());
    }
    Ok(TraceShape {
        half,
        joint_width,
        events,
        kept,
    })
}
impl PhasePathTensor {
    /// Validate the shared logical path dimension without normalizing input.
    /// Trace-specific bounds are admitted separately before joint allocation.
    pub fn new(boundary: AffineKet, phase: ParityPhase) -> Result<Self, PathTensorError> {
        if boundary.columns().len() != phase.num_variables() {
            return Err(PhaseError::DimensionMismatch.into());
        }
        Ok(Self { boundary, phase })
    }
    pub fn boundary(&self) -> &AffineKet {
        &self.boundary
    }
    pub fn phase(&self) -> &ParityPhase {
        &self.phase
    }

    /// Trace before any phase-resource construction or path enumeration.
    /// All query dimensions and conservative geometry are admitted before the
    /// extended/joint tensors are allocated. Input and query rows stay borrowed.
    pub fn trace(
        &self,
        query: PathTraceBoundary<'_>,
        options: PathTraceOptions,
        a: &ScalarArithmetic,
    ) -> Result<Self, PathTensorError> {
        let m = self.boundary.width();
        let n = self.boundary.columns().len();
        limit("path_trace.max_physical_bits", m, options.max_physical_bits)?;
        limit(
            "path_trace.max_retained_rows",
            query.retained_rows.len(),
            options.max_retained_rows,
        )?;
        limit(
            "path_trace.max_event_rows",
            query.event_rows.len(),
            options.max_event_rows,
        )?;
        if query.word.width() != m
            || query.outcomes.width() != query.event_rows.len()
            || query
                .retained_rows
                .iter()
                .chain(query.event_rows)
                .any(|r| r.width() != m)
        {
            return Err(PhaseError::DimensionMismatch.into());
        }
        let labels = query.retained_rows.len();
        let TraceShape {
            half,
            joint_width,
            events,
            kept,
        } = trace_shape(
            m,
            n,
            labels,
            query.event_rows.len(),
            self.phase.terms().len(),
            options,
        )?;
        // Retain a unique auxiliary copy of every logical path coordinate so
        // Gaussian projection cannot sum a direction carrying an odd phase.
        let extend = |v: &BitVector| BitVector::from_indices(half, v.ones());
        let columns = self
            .boundary
            .columns()
            .iter()
            .enumerate()
            .map(|(j, col)| BitVector::from_indices(half, col.ones().chain([m + j])))
            .collect::<Result<Vec<_>, _>>()?;
        let extended = AffineKet::new(
            extend(self.boundary.offset())?,
            columns,
            self.boundary.phase().clone(),
            self.boundary.coefficient().clone(),
        )?;
        let ket = extended.pauli(&extend(query.word.x())?, &extend(query.word.z())?)?;
        let ket = AffineKet::new(
            ket.offset().clone(),
            ket.columns().to_vec(),
            ket.phase().clone(),
            a.multiply(
                ket.coefficient(),
                &a.omega(i64::from(query.word.phase()), 0)?,
            )?,
        )?;
        let joint = extended.conjugate().tensor(&ket, a)?;
        let mut constraints = (0..m)
            .map(|j| BitVector::from_indices(joint_width, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        constraints.extend(
            query
                .event_rows
                .iter()
                .map(|r| BitVector::from_indices(joint_width, r.ones()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let rhs = BitVector::from_indices(events, query.outcomes.ones().map(|j| m + j))?;
        let projection = crate::stabilizer::PreparedZProjection::new(&joint, &constraints)?;
        let Some(projected) = projection.project(&joint, &rhs)? else {
            return Self::new(
                AffineKet::new(
                    BitVector::zeros(labels)?,
                    vec![],
                    QuadraticForm::new(0, vec![], vec![])?,
                    ExactScalar::zero(),
                )?,
                ParityPhase::new(0, 0, [])?,
            );
        };
        let mut rows = query
            .retained_rows
            .iter()
            .map(|r| BitVector::from_indices(joint_width, r.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        rows.extend(
            (m..half)
                .chain(half + m..joint_width)
                .map(|j| BitVector::from_indices(joint_width, [j]))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let retained = projected.linear_boundary(&rows)?;
        let mut terms = Vec::new();
        for (conjugate, start) in [(true, labels), (false, labels + n)] {
            for (axis, c) in self.phase.terms() {
                terms.push((
                    BitVector::from_indices(kept, axis.ones().map(|j| start + j))?,
                    if conjugate { (8 - c) & 7 } else { *c },
                ));
            }
        }
        // Equal input global parity constants cancel between bra and ket.
        let phase =
            ParityPhase::new(kept, 0, terms)?.restrict(retained.offset(), retained.columns())?;
        Self::new(
            retained.retain_boundary(&(0..labels).collect::<Vec<_>>())?,
            phase,
        )
    }

    /// Contract only a completely Clifford residual phase. A remaining odd
    /// resource returns `NotClifford`; no decomposition rescues the operation.
    /// `None` means the exact contracted tensor is zero. This is not universal
    /// absorption of partially Clifford directions in a non-Clifford tensor.
    pub fn contract_clifford(
        &self,
        a: &ScalarArithmetic,
    ) -> Result<Option<AffineKet>, PathTensorError> {
        let (global, phase) = self.phase.clifford()?;
        let phase = add_quadratic(self.boundary.phase(), &phase)?;
        let coefficient =
            a.multiply(self.boundary.coefficient(), &a.omega(i64::from(global), 0)?)?;
        Ok(AffineKet::new(
            self.boundary.offset().clone(),
            self.boundary.columns().to_vec(),
            phase,
            coefficient,
        )?
        .contract(a)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn symbolic_trace_geometry_rejects_unrepresentable_shapes_before_allocation() {
        let options = PathTraceOptions {
            max_physical_bits: usize::MAX,
            max_retained_rows: usize::MAX,
            max_event_rows: usize::MAX,
            max_doubled_path_bits: usize::MAX,
            max_geometry_words: usize::MAX,
        };
        for (m, n, l, e, t) in [
            (usize::MAX, 0, 0, 0, 0),
            (0, usize::MAX, 0, 0, 0),
            (0, 1, usize::MAX, 0, 0),
            (
                usize::MAX / 4,
                usize::MAX / 4,
                usize::MAX / 4,
                usize::MAX / 4,
                usize::MAX,
            ),
        ] {
            assert!(matches!(
                trace_shape(m, n, l, e, t, options),
                Err(PathTensorError::DimensionOverflow)
            ));
        }
        assert!(matches!(
            trace_shape(1, 8, 1, 0, usize::MAX / 8, PathTraceOptions::default()),
            Err(PathTensorError::Algebra(PhaseError::ResourceLimit {
                option: "path_trace.max_geometry_words",
                ..
            }))
        ));
    }

    #[test]
    fn trace_admission_precedes_factorization_and_scalar_work_and_preserves_norm() {
        use crate::gf2::FACTORIZATION_CALLS;
        let a = ScalarArithmetic::default();
        let zero = BitVector::zeros(1).unwrap();
        let one = BitVector::from_indices(1, [0]).unwrap();
        let boundary = AffineKet::new(
            zero.clone(),
            vec![one.clone()],
            QuadraticForm::new(0, vec![0], vec![zero.clone()]).unwrap(),
            a.integer(3).unwrap(),
        )
        .unwrap();
        let tensor =
            PhasePathTensor::new(boundary, ParityPhase::new(1, 0, [(one, 1)]).unwrap()).unwrap();
        let before = tensor.clone();
        let word = PauliWord::identity(1).unwrap();
        let outcomes = BitVector::zeros(0).unwrap();
        let query = PathTraceBoundary {
            retained_rows: &[],
            event_rows: &[],
            outcomes: &outcomes,
            word: &word,
        };
        let tight = ScalarArithmetic::new(1).unwrap();
        for (options, expected) in [
            (
                PathTraceOptions {
                    max_physical_bits: 0,
                    ..Default::default()
                },
                "path_trace.max_physical_bits",
            ),
            (
                PathTraceOptions {
                    max_doubled_path_bits: 1,
                    ..Default::default()
                },
                "path_trace.max_doubled_path_bits",
            ),
            (
                PathTraceOptions {
                    max_geometry_words: 0,
                    ..Default::default()
                },
                "path_trace.max_geometry_words",
            ),
        ] {
            FACTORIZATION_CALLS.with(|v| v.set(0));
            assert!(matches!(tensor.trace(query, options, &tight),
                Err(PathTensorError::Algebra(PhaseError::ResourceLimit { option, .. }))
                if option == expected));
            assert_eq!(FACTORIZATION_CALLS.with(|v| v.get()), 0);
        }
        assert!(matches!(
            tensor.trace(query, Default::default(), &tight),
            Err(PathTensorError::Algebra(PhaseError::Scalar(_)))
        ));
        let traced = tensor.trace(query, Default::default(), &a).unwrap();
        let scalar = traced.contract_clifford(&a).unwrap().unwrap();
        assert_eq!(scalar.width(), 0);
        assert!(scalar.columns().is_empty());
        assert_eq!(scalar.coefficient(), &a.integer(18).unwrap());
        assert_eq!(tensor, before);
    }
}
