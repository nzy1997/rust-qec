use super::*;

pub(super) fn expression(
    original: &BitExpr,
    decisions: &BTreeMap<(usize, usize), bool>,
    defaults: bool,
) -> Result<BitExpr, RuntimeError> {
    let mut expression = BitExpr {
        constant: original.constant,
        ..Default::default()
    };
    for atom in &original.terms {
        match atom {
            BitAtom::Record(_) => {
                expression.terms.insert(atom.clone());
            }
            BitAtom::Decision { call, bit } => {
                let value = decisions
                    .get(&(*call, *bit))
                    .copied()
                    .or(defaults.then_some(false))
                    .ok_or(RuntimeError::UnresolvedDecision {
                        call: *call,
                        bit: *bit,
                    })?;
                expression.constant ^= value;
            }
        }
    }
    Ok(expression)
}
pub(super) fn potential(circuit: &Circuit) -> Circuit {
    let operations = circuit
        .operations()
        .iter()
        .filter_map(|op| {
            let mut op = op.clone();
            match &mut op.operation {
                Operation::Control { .. } => return None,
                Operation::Noise { guard, .. } => *guard = None,
                Operation::Unitary {
                    control: Some(control),
                    ..
                } => {
                    *control = expression(control, &BTreeMap::new(), true)
                        .expect("default decisions resolve")
                }
                Operation::Detector {
                    expression: expr, ..
                }
                | Operation::Postselect { expression: expr }
                | Operation::Observable {
                    expression: expr, ..
                } => {
                    *expr =
                        expression(expr, &BTreeMap::new(), true).expect("default decisions resolve")
                }
                _ => {}
            }
            Some(op)
        })
        .collect();
    circuit.runtime_view(operations)
}
pub(super) fn prefix(
    circuit: &Circuit,
    end: usize,
    decisions: &BTreeMap<(usize, usize), bool>,
    guards: &BTreeMap<usize, bool>,
    source_sites: &BTreeMap<usize, usize>,
) -> Result<Vec<LocatedOperation>, RuntimeError> {
    let mut operations = Vec::new();
    for op in &circuit.operations()[..end] {
        let mut op = op.clone();
        match &mut op.operation {
            Operation::Control { .. } => continue,
            Operation::Noise { guard, .. } => {
                if guard.is_some() {
                    let site = source_sites
                        .get(&op.id)
                        .ok_or(RuntimeError::InternalSite { operation: op.id })?;
                    if !guards
                        .get(site)
                        .copied()
                        .ok_or(RuntimeError::InternalSite { operation: op.id })?
                    {
                        continue;
                    }
                    *guard = None;
                }
            }
            Operation::Unitary {
                control: Some(control),
                ..
            } => *control = expression(control, decisions, false)?,
            Operation::Detector {
                expression: expr, ..
            }
            | Operation::Postselect { expression: expr }
            | Operation::Observable {
                expression: expr, ..
            } => *expr = expression(expr, decisions, false)?,
            _ => {}
        }
        operations.push(op);
    }
    Ok(operations)
}
