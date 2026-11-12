use super::*;
use crate::circuit::Operation;

/// Private to vacuum task execution. The parsed circuit, public physical width,
/// source addresses and record expressions are never relabeled. A missing wire
/// is a factorized |0> that no operation or captured output can observe.
#[derive(Clone)]
pub(crate) struct VacuumLayout {
    pub(super) live: Vec<Option<usize>>,
    width: usize,
}

impl VacuumLayout {
    pub(crate) fn for_task(circuit: &Circuit, outputs: &[usize]) -> Result<Self, InstrumentError> {
        let mut retained: BTreeSet<_> = outputs.iter().copied().collect();
        for located in circuit.operations() {
            match &located.operation {
                Operation::Unitary { qubits, .. } | Operation::Noise { qubits, .. } => {
                    retained.extend(qubits);
                }
                Operation::Reset { qubit, .. } => {
                    retained.insert(*qubit);
                }
                _ => {}
            }
            match &located.operation {
                Operation::Measure { product, .. }
                | Operation::Noise {
                    product: Some(product),
                    ..
                } => {
                    retained.extend(product.factors.iter().map(|&(q, _)| q));
                }
                _ => {}
            }
        }
        let mut live = vec![None; circuit.num_qubits()];
        for (wire, &qubit) in retained.iter().enumerate() {
            let entry = live
                .get_mut(qubit)
                .ok_or(InstrumentError::QubitOutOfBounds {
                    qubit,
                    width: circuit.num_qubits(),
                })?;
            *entry = Some(wire);
        }
        Ok(Self {
            live,
            width: retained.len(),
        })
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

pub(crate) fn compile_vacuum_task(
    circuit: &Circuit,
    layout: &VacuumLayout,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
    causal: bool,
) -> Result<CompiledInstrument, InstrumentError> {
    let input = zero_input(layout.width, options)?;
    lower::compile(circuit, &input, options, arithmetic, causal, Some(layout))
}
