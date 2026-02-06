#[derive(Debug, Clone, PartialEq)]
pub enum StimTarget {
    Qubit(u32),
    QubitInv(u32),
    Rec(i32),
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
