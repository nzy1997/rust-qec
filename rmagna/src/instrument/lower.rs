use super::*;
use crate::circuit::{Basis, Gate, NoiseKind, Operation};
use crate::clifford::CliffordGate as C;
use crate::unitary::{UnitaryCircuit, UnitaryGate as U};
use std::collections::BTreeMap;

struct PendingSite {
    source_operations: Vec<usize>,
    before_gate: usize,
    alternatives: Vec<(Probability, Vec<(usize, Basis)>)>,
    identity: Probability,
}
struct Lowering<'a> {
    options: &'a CompileOptions,
    program: Vec<U>,
    live: Vec<usize>,
    records: Vec<usize>,
    width: usize,
    work: Option<usize>,
    sites: Vec<PendingSite>,
    chains: BTreeMap<usize, usize>,
    selections: Vec<BitExpr>,
    decisions: BTreeMap<(usize, usize), usize>,
}
impl Lowering<'_> {
    fn fresh(&mut self) -> Result<usize, InstrumentError> {
        let wire = self.width;
        if wire >= self.options.max_coherent_qubits {
            return Err(InstrumentError::ResourceLimit {
                option: "max_coherent_qubits",
                limit: self.options.max_coherent_qubits,
                required: wire.saturating_add(1),
            });
        }
        self.width += 1;
        Ok(wire)
    }
    fn extend(&mut self, gates: impl IntoIterator<Item = U>) -> Result<(), InstrumentError> {
        for gate in gates {
            if self.program.len() >= self.options.max_gates {
                return Err(InstrumentError::ResourceLimit {
                    option: "max_gates",
                    limit: self.options.max_gates,
                    required: self.program.len().saturating_add(1),
                });
            }
            self.program.push(gate);
        }
        Ok(())
    }
    fn prepare(&mut self, wire: usize, basis: Basis) -> Result<(), InstrumentError> {
        if basis != Basis::Z {
            self.extend([U::Clifford(C::H(wire))])?;
        }
        if basis == Basis::Y {
            self.extend([U::Clifford(C::S {
                qubit: wire,
                turns: 1,
            })])?;
        }
        Ok(())
    }
    fn reset(&mut self, qubit: usize, basis: Basis) -> Result<(), InstrumentError> {
        let wire = self.fresh()?;
        self.live[qubit] = wire;
        self.prepare(wire, basis)
    }
    fn unitary(
        &mut self,
        gate: Gate,
        qubits: &[usize],
        control: &Option<BitExpr>,
        source: usize,
    ) -> Result<(), InstrumentError> {
        let targets = qubits.iter().map(|&q| self.live[q]).collect::<Vec<_>>();
        let Some(control) = control else {
            return self.extend(gates::lower(gate, &targets));
        };
        if !control.is_affine() && self.decisions.is_empty() {
            return Err(InstrumentError::NeedsHistory { operation: source });
        }
        if control.terms.is_empty() {
            return if control.constant {
                self.extend(gates::lower(gate, &targets))
            } else {
                Ok(())
            };
        }
        let work = if let Some(work) = self.work {
            work
        } else {
            let work = self.fresh()?;
            self.work = Some(work);
            work
        };
        let mut controls = Vec::new();
        for atom in &control.terms {
            controls.push(match atom {
                BitAtom::Record(record) => self.records[*record],
                BitAtom::Decision { call, bit } => {
                    *self
                        .decisions
                        .get(&(*call, *bit))
                        .ok_or(InstrumentError::UnknownDecision {
                            call: *call,
                            bit: *bit,
                        })?
                }
            });
        }
        for &record in &controls {
            self.extend([U::Clifford(C::Cx {
                control: record,
                target: work,
            })])?;
        }
        if control.constant {
            self.extend([U::Clifford(C::X(work))])?;
        }
        let controlled = gates::controlled(gate).ok_or(InstrumentError::Unsupported {
            operation: source,
            reason: "gate has no supported coherent control",
        })?;
        let mut endpoints = vec![work];
        endpoints.extend(targets);
        self.extend(gates::lower(controlled, &endpoints))?;
        if control.constant {
            self.extend([U::Clifford(C::X(work))])?;
        }
        for &record in controls.iter().rev() {
            self.extend([U::Clifford(C::Cx {
                control: record,
                target: work,
            })])?;
        }
        Ok(())
    }
    fn site(
        &mut self,
        source: usize,
        alternatives: Vec<(Probability, Vec<(usize, Basis)>)>,
    ) -> Result<usize, InstrumentError> {
        if self.sites.len() >= self.options.max_noise_sites {
            return Err(InstrumentError::ResourceLimit {
                option: "max_noise_sites",
                limit: self.options.max_noise_sites,
                required: self.sites.len().saturating_add(1),
            });
        }
        let identity =
            Probability::checked_sum(alternatives.iter().map(|(p, _)| p.clone()))?.complement();
        let index = self.sites.len();
        self.sites.push(PendingSite {
            source_operations: vec![source],
            before_gate: self.program.len(),
            alternatives,
            identity,
        });
        Ok(index)
    }
    fn measure(
        &mut self,
        product: &crate::circuit::PauliProduct,
        reset: bool,
        flip: &Probability,
        source: usize,
    ) -> Result<(), InstrumentError> {
        let record = self.fresh()?;
        for &(q, basis) in &product.factors {
            let wire = self.live[q];
            if basis == Basis::Y {
                self.extend([U::Clifford(C::S {
                    qubit: wire,
                    turns: 3,
                })])?;
            }
            if basis != Basis::Z {
                self.extend([U::Clifford(C::H(wire))])?;
            }
            self.extend([U::Clifford(C::Cx {
                control: wire,
                target: record,
            })])?;
            if basis != Basis::Z {
                self.extend([U::Clifford(C::H(wire))])?;
            }
            if basis == Basis::Y {
                self.extend([U::Clifford(C::S {
                    qubit: wire,
                    turns: 1,
                })])?;
            }
        }
        if product.inverted {
            self.extend([U::Clifford(C::X(record))])?;
        }
        self.records.push(record);
        self.site(source, vec![(flip.clone(), vec![(record, Basis::X)])])?;
        if reset {
            let (q, basis) = product.factors[0];
            self.reset(q, basis)?;
        }
        Ok(())
    }
    fn noise(
        &mut self,
        kind: &NoiseKind,
        probabilities: &[Probability],
        qubits: &[usize],
        product: &Option<crate::circuit::PauliProduct>,
        source: usize,
    ) -> Result<(), InstrumentError> {
        if let NoiseKind::Correlated { chain, alternative } = kind {
            let factors = product
                .as_ref()
                .expect("correlated product was validated by parser")
                .factors
                .iter()
                .map(|&(q, b)| (self.live[q], b))
                .collect::<Vec<_>>();
            if *alternative == 0 {
                let index = self.site(source, vec![(probabilities[0].clone(), factors)])?;
                self.chains.insert(*chain, index);
            } else {
                let index = self.chains[chain];
                let site = &mut self.sites[index];
                let probability =
                    Probability::from_rational(site.identity.value() * probabilities[0].value())?;
                site.identity = Probability::from_rational(
                    site.identity.value() * probabilities[0].complement().value(),
                )?;
                site.source_operations.push(source);
                site.alternatives.push((probability, factors));
            }
            return Ok(());
        }
        let wire = |index| self.live[qubits[index]];
        let alternatives = match kind {
            NoiseKind::X => vec![(probabilities[0].clone(), vec![(wire(0), Basis::X)])],
            NoiseKind::Y => vec![(probabilities[0].clone(), vec![(wire(0), Basis::Y)])],
            NoiseKind::Z => vec![(probabilities[0].clone(), vec![(wire(0), Basis::Z)])],
            NoiseKind::Depolarize1 | NoiseKind::Pauli1 => {
                let mut alternatives = Vec::new();
                for (j, basis) in [Basis::X, Basis::Y, Basis::Z].into_iter().enumerate() {
                    let probability = if matches!(kind, NoiseKind::Depolarize1) {
                        Probability::from_rational(
                            probabilities[0].value() / BigRational::from_integer(3.into()),
                        )?
                    } else {
                        probabilities[j].clone()
                    };
                    alternatives.push((probability, vec![(wire(0), basis)]));
                }
                alternatives
            }
            NoiseKind::Depolarize2 | NoiseKind::Pauli2 => {
                let bases = [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)];
                let mut alternatives = Vec::new();
                for index in 1..16 {
                    let probability = if matches!(kind, NoiseKind::Depolarize2) {
                        Probability::from_rational(
                            probabilities[0].value() / BigRational::from_integer(15.into()),
                        )?
                    } else {
                        probabilities[index - 1].clone()
                    };
                    let mut factors = Vec::new();
                    if let Some(basis) = bases[index / 4] {
                        factors.push((wire(0), basis));
                    }
                    if let Some(basis) = bases[index % 4] {
                        factors.push((wire(1), basis));
                    }
                    alternatives.push((probability, factors));
                }
                alternatives
            }
            NoiseKind::Correlated { .. } => unreachable!(),
        };
        self.site(source, alternatives)?;
        Ok(())
    }
}

pub(super) fn compile(
    circuit: &Circuit,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
    causal: bool,
) -> Result<CompiledInstrument, InstrumentError> {
    if circuit.num_qubits() > options.max_coherent_qubits {
        return Err(InstrumentError::ResourceLimit {
            option: "max_coherent_qubits",
            limit: options.max_coherent_qubits,
            required: circuit.num_qubits(),
        });
    }
    let mut state = Lowering {
        options,
        program: Vec::new(),
        live: (0..circuit.num_qubits()).collect(),
        records: Vec::new(),
        width: circuit.num_qubits(),
        work: None,
        sites: Vec::new(),
        chains: BTreeMap::new(),
        selections: Vec::new(),
        decisions: BTreeMap::new(),
    };
    if causal {
        for op in circuit.operations() {
            if let Operation::Control {
                call: Some(call),
                outputs,
                ..
            } = &op.operation
            {
                for bit in 0..outputs.len() {
                    let wire = state.fresh()?;
                    state.decisions.insert((*call, bit), wire);
                }
            }
        }
    }
    for op in circuit.operations() {
        match &op.operation {
            Operation::Unitary {
                gate,
                qubits,
                control,
            } => state.unitary(*gate, qubits, control, op.id)?,
            Operation::Reset { qubit, basis } => state.reset(*qubit, *basis)?,
            Operation::Measure {
                product,
                reset,
                report_flip,
                ..
            } => state.measure(product, *reset, report_flip, op.id)?,
            Operation::Noise {
                kind,
                probabilities,
                qubits,
                product,
                guard,
            } => {
                if guard.is_some() && !causal {
                    return Err(InstrumentError::NeedsHistory { operation: op.id });
                }
                state.noise(kind, probabilities, qubits, product, op.id)?;
            }
            Operation::Control { call: Some(_), .. } => {
                if !causal {
                    return Err(InstrumentError::NeedsHistory { operation: op.id });
                }
            }
            Operation::Postselect { expression } => {
                if !expression.is_affine() && !causal {
                    return Err(InstrumentError::NeedsHistory { operation: op.id });
                }
                state.selections.push(expression.clone());
            }
            Operation::Control { .. }
            | Operation::Detector { .. }
            | Operation::Observable { .. }
            | Operation::Annotation { .. } => {}
        }
    }
    let unitary = UnitaryCircuit::new(state.width, state.program)?.compile_with_strategy(
        &BitVector::zeros(state.width)?,
        arithmetic,
        options.compiler_strategy,
    )?;
    let mut rows = Vec::new();
    let mut outcomes = BitVector::zeros(state.selections.len())?;
    for (j, expression) in state.selections.iter().enumerate() {
        let mut row = record_row(
            &BitExpr {
                constant: expression.constant,
                terms: expression
                    .terms
                    .iter()
                    .filter(|a| matches!(a, BitAtom::Record(_)))
                    .cloned()
                    .collect(),
            },
            &state.records,
            state.width,
        )?;
        for atom in &expression.terms {
            if let BitAtom::Decision { call, bit } = atom {
                let &wire = state.decisions.get(&(*call, *bit)).ok_or(
                    InstrumentError::UnknownDecision {
                        call: *call,
                        bit: *bit,
                    },
                )?;
                row.set(wire, !row.get(wire)?)?;
            }
        }
        rows.push(row);
        outcomes.set(j, expression.constant)?;
    }
    let mut sites = Vec::new();
    for site in state.sites {
        let mut alternatives = Vec::new();
        for (probability, factors) in site.alternatives {
            let mut x = BitVector::zeros(state.width)?;
            let mut z = BitVector::zeros(state.width)?;
            let mut phase = 0;
            for (wire, basis) in factors {
                if basis != Basis::Z {
                    x.set(wire, true)?;
                }
                if basis != Basis::X {
                    z.set(wire, true)?;
                }
                if basis == Basis::Y {
                    phase = (phase + 2) & 7;
                }
            }
            alternatives.push(FaultAlternative {
                probability,
                word: PauliWord::new(x, z, phase)?,
            });
        }
        sites.push(NoiseSite {
            source_operations: site.source_operations,
            before_gate: site.before_gate,
            alternatives: Arc::new(alternatives),
            identity: site.identity,
        });
    }
    Ok(CompiledInstrument {
        unitary: Arc::new(unitary),
        probability_queries: Arc::new(probabilities::ProbabilityQueries::new(
            options.probability_preparation,
            options.max_prepared_probabilities,
        )?),
        density_queries: Arc::new(densities::DensityQueries::new(
            options.density_preparation,
            options.max_prepared_densities,
        )?),
        live_wires: Arc::new(state.live),
        record_wires: Arc::new(state.records),
        event_rows: Arc::new(rows),
        outcomes,
        sites: Arc::new(sites),
        decision_wires: Arc::new(state.decisions),
        initial_insertions: vec![],
    })
}

pub(super) fn unitary(
    circuit: &Circuit,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<CompiledUnitary, InstrumentError> {
    let mut gates = Vec::new();
    if circuit.num_qubits() > options.max_coherent_qubits {
        return Err(InstrumentError::ResourceLimit {
            option: "max_coherent_qubits",
            limit: options.max_coherent_qubits,
            required: circuit.num_qubits(),
        });
    }
    for op in circuit.operations() {
        match &op.operation {
            Operation::Unitary {
                gate,
                qubits,
                control: None,
            } => gates.extend(gates::lower(*gate, qubits)),
            Operation::Annotation { .. } => {}
            Operation::Detector { .. } | Operation::Observable { .. }
                if !circuit.declaration_has_targets(op.id) => {}
            _ => {
                return Err(InstrumentError::Unsupported {
                    operation: op.id,
                    reason: "pure target requires measurement-free unitary gates",
                });
            }
        }
        if gates.len() > options.max_gates {
            return Err(InstrumentError::ResourceLimit {
                option: "max_gates",
                limit: options.max_gates,
                required: gates.len(),
            });
        }
    }
    Ok(
        UnitaryCircuit::new(circuit.num_qubits(), gates)?.compile_with_strategy(
            &BitVector::zeros(circuit.num_qubits())?,
            arithmetic,
            options.compiler_strategy,
        )?,
    )
}
