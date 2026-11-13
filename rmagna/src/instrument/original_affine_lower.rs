//! Lower authoritative affine plans over the true coherent record legs.
use super::*;
impl BranchLowering {
    fn physical_affine_phase(&mut self, wires: &[usize], turns: u8) -> Result<(), OriginalError> {
        if turns & 7 == 0 || wires.is_empty() {
            return Ok(());
        }
        let (&pivot, rest) = wires.split_first().expect("nonempty parity");
        for &q in rest {
            self.c(C::Cx {
                control: q,
                target: pivot,
            })?;
        }
        self.push(U::T {
            qubit: pivot,
            turns: turns & 7,
        })?;
        for &q in rest.iter().rev() {
            self.c(C::Cx {
                control: q,
                target: pivot,
            })?;
        }
        Ok(())
    }
    pub(super) fn affine_control(
        &mut self,
        rule: &control_rules::AffineRulePlan,
        records: &[String],
        origin: &OriginalOrigin,
    ) -> Result<(), OriginalError> {
        for gate in &rule.gates {
            self.affine_gate(gate, records, origin)?;
        }
        Ok(())
    }
    fn affine_gate(
        &mut self,
        gate: &control_rules::AffineGate,
        indices: &[String],
        origin: &OriginalOrigin,
    ) -> Result<(), OriginalError> {
        let mut inverted = gate.guard.inverted;
        let records = gate
            .guard
            .names
            .iter()
            .map(|name| {
                let index = indices
                    .iter()
                    .position(|r| r == name)
                    .ok_or_else(|| invalid("feedback names unknown record", name))?;
                let &(slot, negative) = self
                    .record_slots
                    .get(index)
                    .ok_or_else(|| invalid("feedback requires an earlier record", name))?;
                inverted ^= negative;
                Ok(slot)
            })
            .collect::<Result<Vec<_>, OriginalError>>()?;
        if gate.kind == ControlGateKind::I {
            return Ok(());
        }
        if records.is_empty() {
            if inverted {
                let actual = ControlEmission {
                    kind: gate.kind,
                    wires: gate.wires.clone(),
                    exponent: gate.exponent.map(i32::from),
                }
                .into_original_gate();
                self.gate(&actual, origin, None)?;
            }
            return Ok(());
        }
        let wires = gate
            .wires
            .iter()
            .map(|&q| self.live(q))
            .collect::<Result<Vec<_>, _>>()?;
        if records.iter().any(|q| wires.contains(q)) {
            return Err(invalid("feedback quantum target aliases a record", ""));
        }
        let (&pivot, other) = records.split_first().expect("nonempty affine guard");
        if inverted {
            self.c(C::X(pivot))?;
        }
        for &q in other {
            self.c(C::Cx {
                control: q,
                target: pivot,
            })?;
        }
        if let Some(turns) = gate.exponent {
            if turns & 1 != 0 {
                return Err(invalid("controlled odd phase cannot use affine trace", ""));
            }
            let half = turns / 2;
            self.physical_affine_phase(&[pivot], half)?;
            self.physical_affine_phase(&wires, half)?;
            let mut axis = Vec::with_capacity(wires.len() + 1);
            axis.push(pivot);
            axis.extend_from_slice(&wires);
            self.physical_affine_phase(&axis, (8 - half) & 7)?;
        } else {
            use crate::circuit::Gate;
            let kind = match gate.kind {
                ControlGateKind::X => Gate::X,
                ControlGateKind::Y => Gate::Y,
                ControlGateKind::Z => Gate::Z,
                ControlGateKind::H => Gate::H,
                ControlGateKind::Cx => Gate::Cx,
                ControlGateKind::Cz => Gate::Cz,
                _ => return Err(invalid("invalid affine controlled gate", "")),
            };
            let controlled =
                crate::instrument::gates::controlled(kind).expect("supported Clifford control");
            let mut targets = Vec::with_capacity(wires.len() + 1);
            targets.push(pivot);
            targets.extend_from_slice(&wires);
            for literal in crate::instrument::gates::lower(controlled, &targets) {
                self.push(literal)?;
            }
        }
        for &q in other.iter().rev() {
            self.c(C::Cx {
                control: q,
                target: pivot,
            })?;
        }
        if inverted {
            self.c(C::X(pivot))?;
        }
        Ok(())
    }
}
