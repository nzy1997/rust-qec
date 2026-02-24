#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PauliBasis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StimTarget {
    Qubit(u32),
    QubitInv(u32),
    Rec(i32),
    Pauli { qubit: u32, basis: PauliBasis, inverted: bool },
    Combiner,
}

impl StimTarget {
    pub fn pauli(qubit: u32, basis: PauliBasis, inverted: bool) -> Self {
        StimTarget::Pauli { qubit, basis, inverted }
    }

    pub fn qubit_index(&self) -> Option<u32> {
        match self {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Some(*q),
            StimTarget::Pauli { qubit, .. } => Some(*qubit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StimInstr {
    Op {
        name: String,
        tag: Option<String>,
        args: Vec<f64>,
        targets: Vec<StimTarget>,
    },
    Repeat {
        count: u64,
        body: Vec<StimInstr>,
    },
}

impl StimInstr {
    pub fn new(name: &str, args: Vec<f64>, targets: Vec<StimTarget>) -> Self {
        StimInstr::Op {
            name: name.to_string(),
            tag: None,
            args,
            targets,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            StimInstr::Op { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    pub fn targets(&self) -> Option<&[StimTarget]> {
        match self {
            StimInstr::Op { targets, .. } => Some(targets.as_slice()),
            _ => None,
        }
    }

    pub fn args(&self) -> Option<&[f64]> {
        match self {
            StimInstr::Op { args, .. } => Some(args.as_slice()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub coords: Vec<f64>,
    pub rec_offsets: Vec<i32>,
    pub observable_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationKind {
    Detector,
    ObservableInclude,
}

pub fn circuit_to_string(instrs: &[StimInstr]) -> String {
    let mut s = String::new();
    write_instrs(&mut s, instrs, 0);
    s
}

fn write_instrs(s: &mut String, instrs: &[StimInstr], indent: usize) {
    use std::fmt::Write;
    let pad = "    ".repeat(indent);
    for instr in instrs {
        match instr {
            StimInstr::Op { name, tag, args, targets } => {
                s.push_str(&pad);
                s.push_str(name);
                if let Some(t) = tag {
                    s.push('[');
                    s.push_str(t);
                    s.push(']');
                }
                if !args.is_empty() {
                    s.push('(');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 { s.push_str(", "); }
                        if *a == (*a as i64) as f64 {
                            write!(s, "{}", *a as i64).unwrap();
                        } else {
                            write!(s, "{}", a).unwrap();
                        }
                    }
                    s.push(')');
                }
                for t in targets {
                    s.push(' ');
                    match t {
                        StimTarget::Qubit(q) => write!(s, "{q}").unwrap(),
                        StimTarget::QubitInv(q) => write!(s, "!{q}").unwrap(),
                        StimTarget::Rec(r) => write!(s, "rec[{r}]").unwrap(),
                        StimTarget::Pauli { qubit, basis, inverted } => {
                            if *inverted { s.push('!'); }
                            s.push(match basis {
                                PauliBasis::X => 'X',
                                PauliBasis::Y => 'Y',
                                PauliBasis::Z => 'Z',
                            });
                            write!(s, "{qubit}").unwrap();
                        }
                        StimTarget::Combiner => s.push('*'),
                    }
                }
                s.push('\n');
            }
            StimInstr::Repeat { count, body } => {
                s.push_str(&pad);
                write!(s, "REPEAT {count} {{\n").unwrap();
                write_instrs(s, body, indent + 1);
                s.push_str(&pad);
                s.push_str("}\n");
            }
        }
    }
}

impl Annotation {
    pub fn detector(coords: Vec<f64>, rec_offsets: Vec<i32>) -> Self {
        Self {
            kind: AnnotationKind::Detector,
            coords,
            rec_offsets,
            observable_index: None,
        }
    }

    pub fn observable_include(index: u32, rec_offsets: Vec<i32>) -> Self {
        Self {
            kind: AnnotationKind::ObservableInclude,
            coords: vec![],
            rec_offsets,
            observable_index: Some(index),
        }
    }
}
