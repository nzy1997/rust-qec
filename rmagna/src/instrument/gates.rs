use crate::circuit::Gate;
use crate::clifford::CliffordGate as C;
use crate::unitary::UnitaryGate as U;

pub(super) fn lower(gate: Gate, qubits: &[usize]) -> Vec<U> {
    let q = qubits[0];
    let s = |turns| U::Clifford(C::S { qubit: q, turns });
    let h = U::Clifford(C::H(q));
    match gate {
        Gate::I => vec![],
        Gate::X => vec![U::Clifford(C::X(q))],
        Gate::Z => vec![U::Clifford(C::Z(q))],
        Gate::Y => vec![s(3), U::Clifford(C::X(q)), s(1)],
        Gate::H => vec![h],
        Gate::S => vec![s(1)],
        Gate::Sdag => vec![s(3)],
        Gate::T => vec![U::T { qubit: q, turns: 1 }],
        Gate::Tdag => vec![U::T { qubit: q, turns: 7 }],
        Gate::SqrtX => vec![h, s(1), h],
        Gate::SqrtXdag => vec![h, s(3), h],
        Gate::SqrtY => vec![s(3), h, s(1), h, s(1)],
        Gate::SqrtYdag => vec![s(3), h, s(3), h, s(1)],
        Gate::Hxy => vec![h, s(1), h, s(3), h],
        Gate::Hyz => vec![s(3), h, s(1)],
        Gate::Cxyz => vec![s(2), h, s(3), h, s(3)],
        Gate::Czyx => vec![s(1), h, s(1), h, s(2)],
        Gate::Cx => vec![U::Clifford(C::Cx {
            control: q,
            target: qubits[1],
        })],
        Gate::Cz => vec![U::Clifford(C::Cz { a: q, b: qubits[1] })],
        Gate::Swap => vec![U::Clifford(C::Swap { a: q, b: qubits[1] })],
        Gate::Cy => {
            let t = qubits[1];
            vec![
                U::Clifford(C::S { qubit: t, turns: 3 }),
                U::Clifford(C::Cx {
                    control: q,
                    target: t,
                }),
                U::Clifford(C::S { qubit: t, turns: 1 }),
            ]
        }
        Gate::Iswap | Gate::IswapDag => {
            let t = qubits[1];
            let turns = if gate == Gate::Iswap { 1 } else { 3 };
            vec![
                s(turns),
                U::Clifford(C::S { qubit: t, turns }),
                U::Clifford(C::Cz { a: q, b: t }),
                U::Clifford(C::Swap { a: q, b: t }),
            ]
        }
        Gate::Cs | Gate::CsDag => vec![U::Cs {
            a: q,
            b: qubits[1],
            turns: if gate == Gate::Cs { 1 } else { 3 },
        }],
        Gate::Ccz => vec![U::Ccz {
            a: q,
            b: qubits[1],
            c: qubits[2],
        }],
        Gate::Ccx => {
            let t = qubits[2];
            vec![
                U::Clifford(C::H(t)),
                U::Ccz {
                    a: q,
                    b: qubits[1],
                    c: t,
                },
                U::Clifford(C::H(t)),
            ]
        }
        Gate::Ch => {
            let t = qubits[1];
            let h = U::Clifford(C::H(t));
            vec![
                U::Clifford(C::S { qubit: t, turns: 3 }),
                h,
                U::T { qubit: t, turns: 7 },
                h,
                U::Clifford(C::Cz { a: q, b: t }),
                h,
                U::T { qubit: t, turns: 1 },
                h,
                U::Clifford(C::S { qubit: t, turns: 1 }),
            ]
        }
    }
}

pub(super) fn controlled(gate: Gate) -> Option<Gate> {
    Some(match gate {
        Gate::X => Gate::Cx,
        Gate::Y => Gate::Cy,
        Gate::Z => Gate::Cz,
        Gate::S => Gate::Cs,
        Gate::Sdag => Gate::CsDag,
        Gate::H => Gate::Ch,
        Gate::Cx => Gate::Ccx,
        Gate::Cz => Gate::Ccz,
        _ => return None,
    })
}
