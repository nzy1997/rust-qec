use super::*;
use crate::circuit::{BitAtom, BitExpr, LocatedOperation, PauliProduct};

fn shifted(index: usize, offset: usize, field: &str) -> Result<usize, TaskError> {
    index
        .checked_add(offset)
        .ok_or_else(|| TaskError::new(field, "combined module index exceeds platform range"))
}
fn expression(
    value: &BitExpr,
    record_offset: usize,
    call_offset: usize,
) -> Result<BitExpr, TaskError> {
    let mut result = BitExpr {
        constant: value.constant,
        ..Default::default()
    };
    for atom in &value.terms {
        result.terms.insert(match atom {
            BitAtom::Record(record) => {
                BitAtom::Record(shifted(*record, record_offset, "recovery.records")?)
            }
            BitAtom::Decision { call, bit } => BitAtom::Decision {
                call: shifted(*call, call_offset, "recovery.controllers")?,
                bit: *bit,
            },
        });
    }
    Ok(result)
}
/// Compose already validated noiseless recovery, preserving its file-local
/// register snapshots while separating physical records and external calls.
pub(super) fn recovery(
    physical: &Circuit,
    outputs: &[usize],
    recovery: Option<&Circuit>,
) -> Result<Circuit, TaskError> {
    let Some(recovery) = recovery else {
        return Ok(physical.clone());
    };
    let width = shifted(
        physical.num_qubits(),
        recovery.num_qubits().saturating_sub(outputs.len()),
        "recovery.qubits",
    )?;
    let wire = |q: usize| {
        if q < outputs.len() {
            Ok(outputs[q])
        } else {
            shifted(physical.num_qubits(), q - outputs.len(), "recovery.qubits")
        }
    };
    let expr =
        |value: &BitExpr| expression(value, physical.num_records(), physical.external_calls());
    let product = |value: &PauliProduct| -> Result<PauliProduct, TaskError> {
        Ok(PauliProduct {
            factors: value
                .factors
                .iter()
                .map(|&(q, b)| Ok((wire(q)?, b)))
                .collect::<Result<Vec<_>, TaskError>>()?,
            inverted: value.inverted,
        })
    };
    let mut operations = physical.operations().to_vec();
    for original in recovery.operations() {
        let operation = match &original.operation {
            Operation::Unitary {
                gate,
                qubits,
                control,
            } => Operation::Unitary {
                gate: *gate,
                qubits: qubits
                    .iter()
                    .map(|&q| wire(q))
                    .collect::<Result<Vec<_>, _>>()?,
                control: control.as_ref().map(expr).transpose()?,
            },
            Operation::Reset { qubit, basis } => Operation::Reset {
                qubit: wire(*qubit)?,
                basis: *basis,
            },
            Operation::Measure {
                product: value,
                reset,
                report_flip,
                record,
            } => Operation::Measure {
                product: product(value)?,
                reset: *reset,
                report_flip: report_flip.clone(),
                record: shifted(*record, physical.num_records(), "recovery.records")?,
            },
            Operation::Control {
                name,
                call,
                inputs,
                outputs: writes,
            } => Operation::Control {
                name: name.clone(),
                call: call
                    .map(|call| shifted(call, physical.external_calls(), "recovery.controllers"))
                    .transpose()?,
                inputs: inputs.iter().map(expr).collect::<Result<Vec<_>, _>>()?,
                outputs: writes
                    .iter()
                    .map(|write| {
                        let mut write = write.clone();
                        write.expression = expr(&write.expression)?;
                        Ok(write)
                    })
                    .collect::<Result<Vec<_>, TaskError>>()?,
            },
            Operation::Detector {
                index,
                expression,
                coordinates,
            } => Operation::Detector {
                index: shifted(*index, physical.detectors().len(), "recovery.detectors")?,
                expression: expr(expression)?,
                coordinates: coordinates.clone(),
            },
            Operation::Observable { .. } => continue,
            Operation::Annotation {
                name,
                coordinates,
                qubits,
            } => Operation::Annotation {
                name: name.clone(),
                coordinates: coordinates.clone(),
                qubits: qubits
                    .iter()
                    .map(|&q| wire(q))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Operation::Noise { .. } | Operation::Postselect { .. } => {
                return Err(TaskError::new(
                    "recovery",
                    "terminal recovery must be noiseless and preserve trace",
                ));
            }
        };
        operations.push(LocatedOperation {
            id: operations.len(),
            location: original.location.clone(),
            operation,
        });
    }
    Ok(physical.runtime_view(operations).minimum_width(width))
}
