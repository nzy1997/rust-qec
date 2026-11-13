//! Small exact literal matrix oracle. No compiled Clifford/phase routines.
use super::*;
use crate::circuit::{ParseOptions, parse_circuit};
use crate::runtime::ControllerRegistry;
use crate::unitary::CompilerStrategy;

#[derive(Clone)]
pub(super) struct Dense {
    pub amplitudes: Vec<ExactScalar>,
    arithmetic: ScalarArithmetic,
}
impl Dense {
    pub fn zero(width: usize) -> Self {
        let mut amplitudes = vec![ExactScalar::zero(); 1 << width];
        amplitudes[0] = ExactScalar::one();
        Self {
            amplitudes,
            arithmetic: ScalarArithmetic::default(),
        }
    }
    pub fn h(&mut self, q: usize) {
        let half = self.arithmetic.omega(0, -1).unwrap();
        for j in 0..self.amplitudes.len() {
            if j & (1 << q) == 0 {
                let k = j | (1 << q);
                let (a, b) = (self.amplitudes[j].clone(), self.amplitudes[k].clone());
                self.amplitudes[j] = self
                    .arithmetic
                    .multiply(&half, &self.arithmetic.add(&a, &b).unwrap())
                    .unwrap();
                self.amplitudes[k] = self
                    .arithmetic
                    .multiply(&half, &self.arithmetic.add(&a, &b.negated()).unwrap())
                    .unwrap();
            }
        }
    }
    pub fn phase(&mut self, q: usize, power: i64) {
        let phase = self.arithmetic.omega(power, 0).unwrap();
        for (j, v) in self.amplitudes.iter_mut().enumerate() {
            if j & (1 << q) != 0 {
                *v = self.arithmetic.multiply(v, &phase).unwrap();
            }
        }
    }
    pub fn x(&mut self, q: usize) {
        for j in 0..self.amplitudes.len() {
            if j & (1 << q) == 0 {
                self.amplitudes.swap(j, j | (1 << q));
            }
        }
    }
    pub fn cx(&mut self, control: usize, target: usize) {
        for j in 0..self.amplitudes.len() {
            if j & (1 << control) != 0 && j & (1 << target) == 0 {
                self.amplitudes.swap(j, j | (1 << target));
            }
        }
    }
    pub fn cz(&mut self, q: usize, r: usize) {
        for (j, v) in self.amplitudes.iter_mut().enumerate() {
            if j & (1 << q) != 0 && j & (1 << r) != 0 {
                *v = v.negated();
            }
        }
    }
    pub fn project(&mut self, q: usize, value: bool) {
        for (j, v) in self.amplitudes.iter_mut().enumerate() {
            if (j & (1 << q) != 0) != value {
                *v = ExactScalar::zero();
            }
        }
    }
    /// Hermitian tensor Pauli convention I,X,Z,Y (Y = iXZ).
    pub fn moment(&self, outputs: &[usize], axes: usize) -> ExactScalar {
        let mut sum = ExactScalar::zero();
        for (j, amplitude) in self.amplitudes.iter().enumerate() {
            let mut k = j;
            let mut phase = 0;
            for (n, &q) in outputs.iter().enumerate() {
                let axis = (axes >> (2 * n)) & 3;
                if axis & 2 != 0 && j & (1 << q) != 0 {
                    phase += 4;
                }
                if axis == 3 {
                    phase += 2;
                }
                if axis & 1 != 0 {
                    k ^= 1 << q;
                }
            }
            let v = self
                .arithmetic
                .multiply(&self.amplitudes[k].conjugate(), amplitude)
                .unwrap();
            let v = self
                .arithmetic
                .multiply(&v, &self.arithmetic.omega(phase, 0).unwrap())
                .unwrap();
            sum = self.arithmetic.add(&sum, &v).unwrap();
        }
        sum
    }
}

pub(super) fn configurations() -> impl Iterator<Item = (CompilerStrategy, QueryPreparation)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [
            QueryPreparation::Cold,
            QueryPreparation::FixedFibers(Default::default()),
        ]
        .into_iter()
        .map(move |mode| (compiler, mode))
    })
}
pub(super) fn parse(text: &str) -> Circuit {
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", |bits: &[bool]| Ok(vec![bits[0] & bits[1]]))
        .unwrap();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    parse_circuit(
        text,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap()
}
pub(super) fn template(
    source: &Circuit,
    config: (CompilerStrategy, QueryPreparation),
) -> CompiledInstrument {
    let options = CompileOptions {
        compiler_strategy: config.0,
        moment_preparation: config.1,
        ..Default::default()
    };
    compile_causal_stabilizer(
        source,
        &zero_input(source.num_qubits(), &options).unwrap(),
        &options,
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
pub(super) fn branch(template: &CompiledInstrument, decision: Option<bool>) -> CompiledInstrument {
    let decisions = decision
        .map(|b| BTreeMap::from([((0, 0), b)]))
        .unwrap_or_default();
    let view = template.branch(&decisions, &BTreeSet::new()).unwrap();
    assert!(Arc::ptr_eq(&view.unitary, &template.unitary));
    assert!(Arc::ptr_eq(&view.moment_queries, &template.moment_queries));
    for wire in view.decision_wires.values() {
        assert!(!view.live_wires.iter().flatten().any(|q| q == wire));
    }
    view
}
pub(super) fn pauli(width: usize, axes: usize) -> PauliWord {
    let x =
        BitVector::from_indices(width, (0..width).filter(|q| axes & (1 << (2 * q)) != 0)).unwrap();
    let z =
        BitVector::from_indices(width, (0..width).filter(|q| axes & (2 << (2 * q)) != 0)).unwrap();
    let ys = (0..width).filter(|q| (axes >> (2 * q)) & 3 == 3).count();
    PauliWord::new(x, z, ((2 * ys) % 8) as u8).unwrap()
}
pub(super) fn conditioned(view: &CompiledInstrument, record: usize) -> CompiledInstrument {
    view.with_reported_conditions(
        &(0..2)
            .map(|r| (BitExpr::atom(BitAtom::Record(r)), record & (1 << r) != 0))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
pub(super) fn check(
    view: &CompiledInstrument,
    outputs: &[usize],
    axes: usize,
    expected: ExactScalar,
) {
    assert_eq!(
        view.pauli_moment(
            outputs,
            &pauli(outputs.len(), axes),
            &[],
            256,
            &ScalarArithmetic::default()
        )
        .unwrap()
        .value,
        expected
    );
}
