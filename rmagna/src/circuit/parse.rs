use super::*;
use crate::probability::ProbabilityParser;
use num_traits::ToPrimitive;
use std::fmt;

#[derive(Clone, Debug)]
pub struct ParseOptions {
    pub source: Option<String>,
    pub max_qubits: usize,
    pub max_instructions: usize,
    pub max_operations: usize,
    pub max_repeat_depth: usize,
    pub max_source_bytes: usize,
    pub probabilities: ProbabilityParser,
    pub controllers: BTreeSet<String>,
}
impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            source: None,
            max_qubits: 65536,
            max_instructions: 1_000_000,
            max_operations: 1_000_000,
            max_repeat_depth: 128,
            max_source_bytes: 64 * 1024 * 1024,
            probabilities: ProbabilityParser::default(),
            controllers: BTreeSet::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub location: SourceLocation,
    pub message: String,
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.location.source.as_deref().unwrap_or("<circuit>"),
            self.location.line,
            self.location.column,
            self.message
        )?;
        if !self.location.repeat.is_empty() {
            write!(f, " (repeat {:?})", self.location.repeat)?;
        }
        Ok(())
    }
}
impl std::error::Error for ParseError {}

#[derive(Clone)]
struct Text {
    text: String,
    line: usize,
    column: usize,
}
enum Token {
    Text(Text),
    Open(Text),
    Close(Text),
}
enum Node {
    Instruction(Text),
    Repeat {
        text: Text,
        count: u64,
        body: Vec<Node>,
    },
}

pub fn parse_circuit(source: &str, options: &ParseOptions) -> Result<Circuit, ParseError> {
    let origin = Text {
        text: String::new(),
        line: 1,
        column: 1,
    };
    let fail = |message: String| error(options, &origin, &[], message);
    if source.len() > options.max_source_bytes {
        return Err(fail(format!(
            "source exceeds max_source_bytes = {}",
            options.max_source_bytes
        )));
    }
    let tokens = lex(source);
    let mut position = 0;
    let nodes = nodes(&tokens, &mut position, 0, false, options)?;
    let num_instructions = expanded_count(&nodes, options)?;
    let mut builder = Builder {
        options,
        circuit: Circuit {
            operations: Vec::new(),
            num_qubits: 0,
            num_records: 0,
            num_instructions,
            detectors: Vec::new(),
            observables: BTreeMap::new(),
            external_calls: 0,
            has_guarded_noise: false,
            declarations_with_targets: BTreeSet::new(),
            source_text: source.to_owned(),
        },
        registers: BTreeMap::new(),
        chain: None,
        next_chain: 0,
    };
    builder.expand(&nodes, &mut Vec::new())?;
    Ok(builder.circuit)
}

fn error(
    options: &ParseOptions,
    text: &Text,
    repeat: &[u64],
    message: impl Into<String>,
) -> ParseError {
    ParseError {
        location: SourceLocation {
            source: options.source.clone(),
            line: text.line,
            column: text.column,
            repeat: repeat.to_vec(),
        },
        message: message.into(),
    }
}
fn lex(source: &str) -> Vec<Token> {
    let mut output = Vec::new();
    for (line, raw) in source.lines().enumerate() {
        let raw = raw.split('#').next().unwrap_or("");
        let mut start = 0;
        for (i, c) in raw.char_indices() {
            if c == '{' || c == '}' {
                emit_text(raw, start, i, line + 1, &mut output);
                let text = Text {
                    text: c.to_string(),
                    line: line + 1,
                    column: raw[..i].chars().count() + 1,
                };
                output.push(if c == '{' {
                    Token::Open(text)
                } else {
                    Token::Close(text)
                });
                start = i + 1;
            }
        }
        emit_text(raw, start, raw.len(), line + 1, &mut output);
    }
    output
}
fn emit_text(raw: &str, start: usize, end: usize, line: usize, output: &mut Vec<Token>) {
    let text = raw[start..end].trim();
    if !text.is_empty() {
        let leading = raw[start..end].len() - raw[start..end].trim_start().len();
        output.push(Token::Text(Text {
            text: text.to_owned(),
            line,
            column: raw[..start + leading].chars().count() + 1,
        }));
    }
}
fn nodes(
    tokens: &[Token],
    position: &mut usize,
    depth: usize,
    nested: bool,
    options: &ParseOptions,
) -> Result<Vec<Node>, ParseError> {
    let mut result = Vec::new();
    while let Some(token) = tokens.get(*position) {
        match token {
            Token::Close(text) => {
                if !nested {
                    return Err(error(options, text, &[], "unmatched closing brace"));
                }
                *position += 1;
                return Ok(result);
            }
            Token::Open(text) => {
                return Err(error(
                    options,
                    text,
                    &[],
                    "opening brace requires a REPEAT instruction",
                ));
            }
            Token::Text(text) => {
                *position += 1;
                let words = text.text.split_whitespace().collect::<Vec<_>>();
                if words
                    .first()
                    .is_some_and(|w| w.eq_ignore_ascii_case("REPEAT"))
                {
                    if words.len() != 2 {
                        return Err(error(
                            options,
                            text,
                            &[],
                            "REPEAT requires one nonnegative integer count",
                        ));
                    }
                    let count = words[1].parse::<u64>().map_err(|_| {
                        error(
                            options,
                            text,
                            &[],
                            "REPEAT count exceeds the unsigned 64-bit range or is invalid",
                        )
                    })?;
                    if !matches!(tokens.get(*position), Some(Token::Open(_))) {
                        return Err(error(options, text, &[], "REPEAT requires '{'"));
                    }
                    if depth >= options.max_repeat_depth {
                        return Err(error(
                            options,
                            text,
                            &[],
                            format!(
                                "repeat nesting exceeds max_repeat_depth = {}",
                                options.max_repeat_depth
                            ),
                        ));
                    }
                    *position += 1;
                    let body = nodes(tokens, position, depth + 1, true, options)?;
                    result.push(Node::Repeat {
                        text: text.clone(),
                        count,
                        body,
                    });
                } else {
                    result.push(Node::Instruction(text.clone()));
                }
            }
        }
    }
    if nested {
        let text = tokens
            .last()
            .map(|t| match t {
                Token::Text(t) | Token::Open(t) | Token::Close(t) => t,
            })
            .cloned()
            .unwrap_or(Text {
                text: String::new(),
                line: 1,
                column: 1,
            });
        return Err(error(options, &text, &[], "unclosed REPEAT block"));
    }
    Ok(result)
}
fn expanded_count(nodes: &[Node], options: &ParseOptions) -> Result<usize, ParseError> {
    let mut total = 0_usize;
    for node in nodes {
        let (text, count) = match node {
            Node::Instruction(text) => (text, Some(1)),
            Node::Repeat { text, count, body } => {
                if *count == 0 {
                    continue;
                }
                let length = expanded_count(body, options)?;
                let amount = if length == 0 || *count == 0 {
                    Some(0)
                } else {
                    usize::try_from(*count)
                        .ok()
                        .and_then(|n| n.checked_mul(length))
                };
                (text, amount)
            }
        };
        total = total
            .checked_add(count.unwrap_or(usize::MAX))
            .ok_or_else(|| {
                error(
                    options,
                    text,
                    &[],
                    format!(
                        "expanded instruction count exceeds max_instructions = {}",
                        options.max_instructions
                    ),
                )
            })?;
        if total > options.max_instructions {
            return Err(error(
                options,
                text,
                &[],
                format!(
                    "expanded instruction count {total} exceeds max_instructions = {}",
                    options.max_instructions
                ),
            ));
        }
    }
    Ok(total)
}

struct Builder<'a> {
    options: &'a ParseOptions,
    circuit: Circuit,
    registers: BTreeMap<usize, RegisterWrite>,
    chain: Option<(usize, usize, Option<BitExpr>)>,
    next_chain: usize,
}
#[derive(Clone, Copy)]
enum ReferenceSet {
    All,
    Records,
    Decisions,
}
impl Builder<'_> {
    fn fail(&self, text: &Text, repeat: &[u64], message: impl Into<String>) -> ParseError {
        error(self.options, text, repeat, message)
    }
    fn push(
        &mut self,
        text: &Text,
        repeat: &[u64],
        operation: Operation,
    ) -> Result<(), ParseError> {
        let id = self.circuit.operations.len();
        if id >= self.options.max_operations {
            return Err(self.fail(
                text,
                repeat,
                format!(
                    "expanded operations exceed max_operations = {}",
                    self.options.max_operations
                ),
            ));
        }
        self.circuit.operations.push(LocatedOperation {
            id,
            location: SourceLocation {
                source: self.options.source.clone(),
                line: text.line,
                column: text.column,
                repeat: repeat.to_vec(),
            },
            operation,
        });
        Ok(())
    }
    fn expand(&mut self, nodes: &[Node], repeat: &mut Vec<u64>) -> Result<(), ParseError> {
        for node in nodes {
            match node {
                Node::Instruction(text) => self.instruction(text, repeat)?,
                Node::Repeat { count, body, .. } => {
                    if *count == 0 {
                        continue;
                    }
                    if expanded_count(body, self.options)? == 0 {
                        continue;
                    }
                    for iteration in 0..*count {
                        repeat.push(iteration);
                        self.expand(body, repeat)?;
                        repeat.pop();
                    }
                }
            }
        }
        Ok(())
    }
    fn qubit(&mut self, token: &str, text: &Text, repeat: &[u64]) -> Result<usize, ParseError> {
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
            return Err(self.fail(
                text,
                repeat,
                format!("expected a nonnegative quantum target; got {token}"),
            ));
        }
        let qubit = token
            .parse::<usize>()
            .map_err(|_| self.fail(text, repeat, "qubit index exceeds the platform range"))?;
        if qubit >= self.options.max_qubits {
            return Err(self.fail(
                text,
                repeat,
                format!(
                    "qubit index {qubit} exceeds max_qubits = {}",
                    self.options.max_qubits
                ),
            ));
        }
        self.circuit.num_qubits = self.circuit.num_qubits.max(qubit + 1);
        Ok(qubit)
    }
    fn reference(
        &self,
        token: &str,
        allowed: ReferenceSet,
        text: &Text,
        repeat: &[u64],
    ) -> Result<BitExpr, ParseError> {
        let (invert, token) = token
            .strip_prefix('!')
            .map_or((false, token), |t| (true, t));
        let mut expression = if let Some(index) =
            token.strip_prefix("rec[").and_then(|t| t.strip_suffix(']'))
        {
            if matches!(allowed, ReferenceSet::Decisions) {
                return Err(self.fail(
                    text,
                    repeat,
                    "external controls and postselection cannot read raw records",
                ));
            }
            let distance = index
                .strip_prefix('-')
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&n| n > 0 && n <= self.circuit.num_records)
                .ok_or_else(|| {
                    self.fail(
                        text,
                        repeat,
                        "record lookback must refer to an earlier measurement",
                    )
                })?;
            BitExpr::atom(BitAtom::Record(self.circuit.num_records - distance))
        } else if let Some(index) = token.strip_prefix("det[").and_then(|t| t.strip_suffix(']')) {
            if matches!(allowed, ReferenceSet::Records) {
                return Err(self.fail(
                    text,
                    repeat,
                    "detectors require measurement-record targets",
                ));
            }
            let index = if let Some(n) = index.strip_prefix('-') {
                let distance = n
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0 && n <= self.circuit.detectors.len())
                    .ok_or_else(|| {
                        self.fail(
                            text,
                            repeat,
                            "detector lookback must refer to an earlier detector",
                        )
                    })?;
                self.circuit.detectors.len() - distance
            } else {
                index
                    .parse::<usize>()
                    .map_err(|_| self.fail(text, repeat, "invalid detector index"))?
            };
            self.circuit.detectors.get(index).cloned().ok_or_else(|| {
                self.fail(
                    text,
                    repeat,
                    "detector target must refer to an earlier detector",
                )
            })?
        } else if let Some(index) = register_index(token) {
            if matches!(allowed, ReferenceSet::Records) {
                return Err(self.fail(
                    text,
                    repeat,
                    "detectors require measurement-record targets",
                ));
            }
            self.registers
                .get(&index)
                .map(|r| r.expression.clone())
                .ok_or_else(|| {
                    self.fail(
                        text,
                        repeat,
                        format!("control register {index} is uninitialized"),
                    )
                })?
        } else {
            return Err(self.fail(text, repeat, format!("invalid classical target {token}")));
        };
        expression.constant ^= invert;
        Ok(expression)
    }
    fn parity(
        &self,
        targets: &[&str],
        allowed: ReferenceSet,
        text: &Text,
        repeat: &[u64],
    ) -> Result<BitExpr, ParseError> {
        let mut result = BitExpr::default();
        for target in targets {
            result.xor_assign(&self.reference(target, allowed, text, repeat)?);
        }
        Ok(result)
    }
    fn distinct(&self, qubits: &[usize], text: &Text, repeat: &[u64]) -> Result<(), ParseError> {
        if qubits.iter().copied().collect::<BTreeSet<_>>().len() != qubits.len() {
            return Err(self.fail(text, repeat, "gate or product repeats a qubit"));
        }
        Ok(())
    }
    fn unitary(
        &mut self,
        gate: Gate,
        targets: &[&str],
        text: &Text,
        repeat: &[u64],
    ) -> Result<(), ParseError> {
        let arity = gate.arity();
        if targets.is_empty() || !targets.len().is_multiple_of(arity) {
            return Err(self.fail(
                text,
                repeat,
                format!("{} requires target groups of {arity}", gate.name()),
            ));
        }
        for raw in targets.chunks(arity) {
            let mut group = raw.to_vec();
            if gate == Gate::Cz && is_reference(group[1]) {
                group.swap(0, 1);
            }
            let (gate, control, start) = if is_reference(group[0]) {
                let target = gate.classical_target().ok_or_else(|| {
                    self.fail(
                        text,
                        repeat,
                        "this gate does not accept a classical control",
                    )
                })?;
                (
                    target,
                    Some(self.reference(group[0], ReferenceSet::All, text, repeat)?),
                    1,
                )
            } else {
                (gate, None, 0)
            };
            let qubits = group[start..]
                .iter()
                .map(|s| self.qubit(s, text, repeat))
                .collect::<Result<Vec<_>, _>>()?;
            self.distinct(&qubits, text, repeat)?;
            self.push(
                text,
                repeat,
                Operation::Unitary {
                    gate,
                    qubits,
                    control,
                },
            )?;
        }
        Ok(())
    }
}
fn is_reference(token: &str) -> bool {
    let token = token.strip_prefix('!').unwrap_or(token);
    token.starts_with("rec[") || token.starts_with("det[") || token.starts_with("ctrl[")
}
fn register_index(token: &str) -> Option<usize> {
    let value = token.strip_prefix("ctrl[")?.strip_suffix(']')?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        None
    } else {
        value.parse().ok()
    }
}

impl Builder<'_> {
    fn instruction(&mut self, text: &Text, repeat: &[u64]) -> Result<(), ParseError> {
        let raw = text.text.as_str();
        let end = raw
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(raw.len());
        let head = &raw[..end];
        let mut tail = raw[end..].trim();
        let mut args = Vec::new();
        if let Some(after) = tail.strip_prefix('(') {
            let end = after
                .find(')')
                .ok_or_else(|| self.fail(text, repeat, "unclosed instruction arguments"))?;
            if !after[..end].trim().is_empty() {
                args = after[..end].split(',').map(str::trim).collect();
            }
            tail = after[end + 1..].trim();
        }
        let targets = tail.split_whitespace().collect::<Vec<_>>();
        let name = head.to_ascii_uppercase();
        if matches!(name.as_str(), "DETECTOR" | "OBSERVABLE_INCLUDE") && !targets.is_empty() {
            self.circuit
                .declarations_with_targets
                .insert(self.circuit.operations.len());
        }
        let continuing = name == "ELSE_CORRELATED_ERROR";
        if !continuing && name != "CORRELATED_ERROR" && name != "E" {
            self.chain = None;
        }
        if let Some(gate) = Gate::from_name(&name) {
            self.no_args(&args, text, repeat)?;
            return self.unitary(gate, &targets, text, repeat);
        }
        if name.starts_with("CONTROL[") && name.ends_with(']') {
            let function = &head[8..head.len() - 1];
            return self.control(function, &args, &targets, text, repeat);
        }
        match name.as_str() {
            "R" | "RZ" | "RX" | "RY" => {
                self.no_args(&args, text, repeat)?;
                if targets.is_empty() {
                    return Err(self.fail(text, repeat, "reset requires targets"));
                }
                let basis = match name.as_str() {
                    "RX" => Basis::X,
                    "RY" => Basis::Y,
                    _ => Basis::Z,
                };
                for target in targets {
                    let qubit = self.qubit(target, text, repeat)?;
                    self.push(text, repeat, Operation::Reset { qubit, basis })?;
                }
            }
            "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "MXX" | "MYY" | "MZZ"
            | "MPP" => self.measure(&name, &args, &targets, text, repeat)?,
            "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "CORRELATED_ERROR"
            | "ELSE_CORRELATED_ERROR"
            | "E" => self.noise(&name, &args, &targets, text, repeat)?,
            "DETECTOR" => {
                let coordinates = self.coordinates(&args, text, repeat)?;
                let expression = self.parity(&targets, ReferenceSet::Records, text, repeat)?;
                let index = self.circuit.detectors.len();
                self.circuit.detectors.push(expression.clone());
                self.push(
                    text,
                    repeat,
                    Operation::Detector {
                        index,
                        expression,
                        coordinates,
                    },
                )?;
            }
            "POSTSELECT" => {
                self.no_args(&args, text, repeat)?;
                if targets.is_empty() {
                    return Err(self.fail(
                        text,
                        repeat,
                        "POSTSELECT requires detector or register targets",
                    ));
                }
                for target in targets {
                    let expression =
                        self.reference(target, ReferenceSet::Decisions, text, repeat)?;
                    self.push(text, repeat, Operation::Postselect { expression })?;
                }
            }
            "OBSERVABLE_INCLUDE" => {
                if args.len() != 1 {
                    return Err(self.fail(
                        text,
                        repeat,
                        "OBSERVABLE_INCLUDE requires one nonnegative integer index",
                    ));
                }
                let value = self
                    .options
                    .probabilities
                    .rational(args[0])
                    .map_err(|_| self.fail(text, repeat, "invalid observable index"))?;
                let index = value
                    .is_integer()
                    .then(|| value.numer().to_usize())
                    .flatten()
                    .ok_or_else(|| {
                        self.fail(
                            text,
                            repeat,
                            "observable index must be a nonnegative platform-sized integer",
                        )
                    })?;
                let expression = self.parity(&targets, ReferenceSet::All, text, repeat)?;
                self.circuit
                    .observables
                    .entry(index)
                    .or_default()
                    .xor_assign(&expression);
                self.push(text, repeat, Operation::Observable { index, expression })?;
            }
            "TICK" => {
                self.no_args(&args, text, repeat)?;
                if !targets.is_empty() {
                    return Err(self.fail(text, repeat, "TICK does not accept targets"));
                }
                self.push(
                    text,
                    repeat,
                    Operation::Annotation {
                        name,
                        coordinates: Vec::new(),
                        qubits: Vec::new(),
                    },
                )?;
            }
            "QUBIT_COORDS" | "SHIFT_COORDS" => {
                let coordinates = self.coordinates(&args, text, repeat)?;
                if name == "SHIFT_COORDS" && !targets.is_empty() {
                    return Err(self.fail(text, repeat, "SHIFT_COORDS does not accept targets"));
                }
                if name == "QUBIT_COORDS" && targets.is_empty() {
                    return Err(self.fail(text, repeat, "QUBIT_COORDS requires targets"));
                }
                let qubits = targets
                    .iter()
                    .map(|s| self.qubit(s, text, repeat))
                    .collect::<Result<Vec<_>, _>>()?;
                self.push(
                    text,
                    repeat,
                    Operation::Annotation {
                        name,
                        coordinates,
                        qubits,
                    },
                )?;
            }
            _ => return Err(self.fail(text, repeat, format!("unsupported instruction {head}"))),
        }
        Ok(())
    }
    fn no_args(&self, args: &[&str], text: &Text, repeat: &[u64]) -> Result<(), ParseError> {
        if !args.is_empty() {
            Err(self.fail(text, repeat, "instruction does not take arguments"))
        } else {
            Ok(())
        }
    }
    fn coordinates(
        &self,
        args: &[&str],
        text: &Text,
        repeat: &[u64],
    ) -> Result<Vec<f64>, ParseError> {
        args.iter()
            .map(|s| {
                self.options
                    .probabilities
                    .rational(s)
                    .ok()
                    .and_then(|x| x.to_f64())
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| {
                        self.fail(text, repeat, "coordinates must be finite decimal numbers")
                    })
            })
            .collect()
    }
    fn probability(
        &self,
        arg: &str,
        text: &Text,
        repeat: &[u64],
    ) -> Result<Probability, ParseError> {
        self.options
            .probabilities
            .parse(arg)
            .map_err(|e| self.fail(text, repeat, e.to_string()))
    }

    fn control(
        &mut self,
        function: &str,
        args: &[&str],
        targets: &[&str],
        text: &Text,
        repeat: &[u64],
    ) -> Result<(), ParseError> {
        if function.is_empty() {
            return Err(self.fail(text, repeat, "CONTROL requires a nonempty function name"));
        }
        let arrow = targets.iter().position(|&t| t == "->").ok_or_else(|| {
            self.fail(
                text,
                repeat,
                "CONTROL requires '->' before output registers",
            )
        })?;
        if targets[arrow + 1..].contains(&"->") {
            return Err(self.fail(text, repeat, "CONTROL has more than one output arrow"));
        }
        let builtin = function == "XOR" || function == "NOT";
        let allowed = if builtin {
            ReferenceSet::All
        } else {
            ReferenceSet::Decisions
        };
        let inputs = targets[..arrow]
            .iter()
            .map(|s| self.reference(s, allowed, text, repeat))
            .collect::<Result<Vec<_>, _>>()?;
        let registers = targets[arrow + 1..]
            .iter()
            .map(|s| {
                register_index(s).ok_or_else(|| {
                    self.fail(
                        text,
                        repeat,
                        "CONTROL outputs must be non-inverted control registers",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if registers.is_empty()
            || registers.iter().collect::<BTreeSet<_>>().len() != registers.len()
        {
            return Err(self.fail(text, repeat, "CONTROL requires distinct output registers"));
        }
        let call = if builtin {
            if registers.len() != 1 {
                return Err(self.fail(text, repeat, "builtin CONTROL requires exactly one output"));
            }
            if function == "NOT" && (inputs.len() != 1 || !args.is_empty()) {
                return Err(self.fail(
                    text,
                    repeat,
                    "CONTROL[NOT] requires exactly one input and no arguments",
                ));
            }
            if function == "XOR" && args.len() > 1 {
                return Err(self.fail(
                    text,
                    repeat,
                    "CONTROL[XOR] takes an optional constant zero or one",
                ));
            }
            None
        } else {
            self.no_args(args, text, repeat)?;
            if !self.options.controllers.contains(function) {
                return Err(self.fail(
                    text,
                    repeat,
                    format!("missing controller declaration for {function}"),
                ));
            }
            let call = self.circuit.external_calls;
            self.circuit.external_calls += 1;
            Some(call)
        };
        let builtin_constant = if function == "NOT" {
            true
        } else if builtin {
            if let Some(arg) = args.first() {
                let p = self.probability(arg, text, repeat)?;
                if p != Probability::zero() && p != Probability::one() {
                    return Err(self.fail(
                        text,
                        repeat,
                        "CONTROL[XOR] constant must be zero or one",
                    ));
                }
                p == Probability::one()
            } else {
                false
            }
        } else {
            false
        };
        let mut outputs = Vec::new();
        for (bit, register) in registers.into_iter().enumerate() {
            let expression = if let Some(call) = call {
                BitExpr::atom(BitAtom::Decision { call, bit })
            } else {
                let mut expr = BitExpr {
                    constant: builtin_constant,
                    terms: BTreeSet::new(),
                };
                for input in &inputs {
                    expr.xor_assign(input);
                }
                expr
            };
            let version = self.registers.get(&register).map_or(0, |r| r.version + 1);
            outputs.push(RegisterWrite {
                register,
                version,
                expression,
            });
        }
        // Every input was resolved before any output is installed.
        for output in &outputs {
            self.registers.insert(output.register, output.clone());
        }
        self.push(
            text,
            repeat,
            Operation::Control {
                name: function.to_owned(),
                call,
                inputs,
                outputs,
            },
        )
    }

    fn pauli_product(
        &mut self,
        tokens: &[&str],
        inversions: bool,
        text: &Text,
        repeat: &[u64],
    ) -> Result<PauliProduct, ParseError> {
        let mut factors = Vec::new();
        let mut inverted = false;
        for token in tokens {
            let token = if let Some(t) = token.strip_prefix('!') {
                if !inversions {
                    return Err(self.fail(text, repeat, "noise Pauli products cannot be inverted"));
                }
                inverted ^= true;
                t
            } else {
                token
            };
            let (basis, index) = match token.as_bytes().first() {
                Some(b'X') => (Basis::X, &token[1..]),
                Some(b'Y') => (Basis::Y, &token[1..]),
                Some(b'Z') => (Basis::Z, &token[1..]),
                _ => {
                    return Err(self.fail(
                        text,
                        repeat,
                        "product requires X, Y or Z Pauli targets",
                    ));
                }
            };
            factors.push((self.qubit(index, text, repeat)?, basis));
        }
        if factors.is_empty() {
            return Err(self.fail(text, repeat, "Pauli product requires targets"));
        }
        self.distinct(
            &factors.iter().map(|(q, _)| *q).collect::<Vec<_>>(),
            text,
            repeat,
        )?;
        factors.sort_by_key(|(q, _)| *q);
        Ok(PauliProduct { factors, inverted })
    }
    fn measure(
        &mut self,
        name: &str,
        args: &[&str],
        targets: &[&str],
        text: &Text,
        repeat: &[u64],
    ) -> Result<(), ParseError> {
        if args.len() > 1 {
            return Err(self.fail(
                text,
                repeat,
                "measurement takes at most one report-flip probability",
            ));
        }
        let flip = if let Some(arg) = args.first() {
            self.probability(arg, text, repeat)?
        } else {
            Probability::zero()
        };
        if targets.is_empty() {
            return Err(self.fail(text, repeat, "measurement requires targets"));
        }
        let mut products = Vec::new();
        if name == "MPP" {
            let mut joined = Vec::<String>::new();
            for &token in targets {
                if token.starts_with('*') || joined.last().is_some_and(|s| s.ends_with('*')) {
                    let previous = joined.last_mut().ok_or_else(|| {
                        self.fail(text, repeat, "MPP product cannot begin with '*'")
                    })?;
                    previous.push_str(token);
                } else {
                    joined.push(token.to_owned());
                }
            }
            for product in joined {
                products.push(self.pauli_product(
                    &product.split('*').collect::<Vec<_>>(),
                    true,
                    text,
                    repeat,
                )?);
            }
        } else {
            let basis = if name.ends_with('X') {
                Basis::X
            } else if name.ends_with('Y') {
                Basis::Y
            } else {
                Basis::Z
            };
            let arity = if matches!(name, "MXX" | "MYY" | "MZZ") {
                2
            } else {
                1
            };
            if !targets.len().is_multiple_of(arity) {
                return Err(self.fail(
                    text,
                    repeat,
                    format!("{name} requires target groups of {arity}"),
                ));
            }
            for group in targets.chunks(arity) {
                let mut inverted = false;
                let mut factors = Vec::new();
                for &token in group {
                    let token = if let Some(t) = token.strip_prefix('!') {
                        inverted ^= true;
                        t
                    } else {
                        token
                    };
                    factors.push((self.qubit(token, text, repeat)?, basis));
                }
                self.distinct(
                    &factors.iter().map(|(q, _)| *q).collect::<Vec<_>>(),
                    text,
                    repeat,
                )?;
                products.push(PauliProduct { factors, inverted });
            }
        }
        for product in products {
            let record = self.circuit.num_records;
            self.circuit.num_records += 1;
            self.push(
                text,
                repeat,
                Operation::Measure {
                    product,
                    reset: name.starts_with("MR"),
                    report_flip: flip.clone(),
                    record,
                },
            )?;
        }
        Ok(())
    }

    fn noise(
        &mut self,
        name: &str,
        args: &[&str],
        targets: &[&str],
        text: &Text,
        repeat: &[u64],
    ) -> Result<(), ParseError> {
        let expected = match name {
            "PAULI_CHANNEL_1" => 3,
            "PAULI_CHANNEL_2" => 15,
            _ => 1,
        };
        if args.len() != expected {
            return Err(self.fail(
                text,
                repeat,
                format!("{name} requires {expected} probability arguments"),
            ));
        }
        let probabilities = args
            .iter()
            .map(|s| self.probability(s, text, repeat))
            .collect::<Result<Vec<_>, _>>()?;
        Probability::checked_sum(probabilities.clone())
            .map_err(|_| self.fail(text, repeat, "Pauli channel probability sum exceeds one"))?;
        let (guard, targets) = if targets.first().is_some_and(|s| is_reference(s)) {
            (
                Some(self.reference(targets[0], ReferenceSet::All, text, repeat)?),
                &targets[1..],
            )
        } else {
            (None, targets)
        };
        if guard.is_some() {
            self.circuit.has_guarded_noise = true;
        }
        if targets.is_empty() {
            return Err(self.fail(text, repeat, "noise instruction requires targets"));
        }
        if matches!(name, "CORRELATED_ERROR" | "ELSE_CORRELATED_ERROR" | "E") {
            let (chain, alternative) = if name == "ELSE_CORRELATED_ERROR" {
                let (chain, previous, old_guard) = self.chain.clone().ok_or_else(|| {
                    self.fail(
                        text,
                        repeat,
                        "ELSE_CORRELATED_ERROR must immediately follow a correlated alternative",
                    )
                })?;
                if old_guard != guard {
                    return Err(self.fail(
                        text,
                        repeat,
                        "correlated alternatives require the same control guard",
                    ));
                }
                (chain, previous + 1)
            } else {
                let chain = self.next_chain;
                self.next_chain += 1;
                (chain, 0)
            };
            let product = self.pauli_product(targets, false, text, repeat)?;
            self.chain = Some((chain, alternative, guard.clone()));
            self.push(
                text,
                repeat,
                Operation::Noise {
                    kind: NoiseKind::Correlated { chain, alternative },
                    probabilities,
                    qubits: Vec::new(),
                    product: Some(product),
                    guard,
                },
            )?;
        } else {
            let (kind, arity) = match name {
                "X_ERROR" => (NoiseKind::X, 1),
                "Y_ERROR" => (NoiseKind::Y, 1),
                "Z_ERROR" => (NoiseKind::Z, 1),
                "DEPOLARIZE1" => (NoiseKind::Depolarize1, 1),
                "DEPOLARIZE2" => (NoiseKind::Depolarize2, 2),
                "PAULI_CHANNEL_1" => (NoiseKind::Pauli1, 1),
                "PAULI_CHANNEL_2" => (NoiseKind::Pauli2, 2),
                _ => unreachable!(),
            };
            if !targets.len().is_multiple_of(arity) {
                return Err(self.fail(
                    text,
                    repeat,
                    format!("{name} requires target groups of {arity}"),
                ));
            }
            for group in targets.chunks(arity) {
                let qubits = group
                    .iter()
                    .map(|s| self.qubit(s, text, repeat))
                    .collect::<Result<Vec<_>, _>>()?;
                self.distinct(&qubits, text, repeat)?;
                self.push(
                    text,
                    repeat,
                    Operation::Noise {
                        kind: kind.clone(),
                        probabilities: probabilities.clone(),
                        qubits,
                        product: None,
                        guard: guard.clone(),
                    },
                )?;
            }
        }
        Ok(())
    }
}
