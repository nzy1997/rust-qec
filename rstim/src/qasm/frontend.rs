//! OpenQASM 2 frontend with explicit syntax, symbol resolution and bounded expansion.
use super::expr::Expr;
use super::{
    Angle, ErrorCode, Gate, IncludeResolver, Operation, OperationKind, Pauli, QasmCircuit,
    QasmError, QasmLimits, Register, SourceSpan,
};
use std::collections::{HashMap, HashSet};

const QELIB: &str = include_str!("qelib1.inc");
const QELIB_SOURCE: &str = "rstim:bundled/qelib1.inc";

fn error(code: ErrorCode, message: impl Into<String>, span: &SourceSpan) -> QasmError {
    QasmError::new(code, message).at(span.clone())
}
#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Name(String),
    Number(String),
    String(String),
    Symbol(char),
    Arrow,
    Equal,
    End,
}
#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
}
fn lex(text: &str, name: &str, budget: usize) -> Result<Vec<Token>, QasmError> {
    let bytes = text.as_bytes();
    let (mut i, mut line, mut col) = (0, 1, 1);
    let mut out = Vec::new();
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            if bytes[i] == b'\n' {
                line += 1;
                col = 1
            } else {
                col += 1
            };
            i += 1;
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        let start = i;
        let start_col = col;
        let span = |end: usize| SourceSpan {
            source: name.to_owned(),
            line,
            column: start_col,
            offset: start,
            length: end - start,
            expansion: vec![],
        };
        let kind = if bytes[i].is_ascii_alphabetic() {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let n = &text[start..i];
            if !bytes[start].is_ascii_lowercase() && !matches!(n, "U" | "CX" | "OPENQASM") {
                return Err(error(
                    ErrorCode::Parse,
                    "identifiers must begin with a lowercase ASCII letter",
                    &span(i),
                ));
            }
            TokenKind::Name(n.to_owned())
        } else if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if bytes.get(i) == Some(&b'.') {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if matches!(bytes.get(i), Some(b'e' | b'E')) {
                i += 1;
                if matches!(bytes.get(i), Some(b'+' | b'-')) {
                    i += 1;
                }
                let e = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if e == i {
                    return Err(error(
                        ErrorCode::Parse,
                        "exponent requires digits",
                        &span(i),
                    ));
                }
            }
            TokenKind::Number(text[start..i].to_owned())
        } else if bytes[i] == b'"' {
            i += 1;
            let a = i;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] < 32 || bytes[i] == b'\\' {
                    return Err(error(
                        ErrorCode::Parse,
                        "invalid include filename",
                        &span(i + 1),
                    ));
                }
                i += 1;
            }
            if i == bytes.len() {
                return Err(error(
                    ErrorCode::Parse,
                    "unterminated include filename",
                    &span(i),
                ));
            }
            let value = text[a..i].to_owned();
            i += 1;
            TokenKind::String(value)
        } else if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'>') {
            i += 2;
            TokenKind::Arrow
        } else if bytes[i] == b'=' && bytes.get(i + 1) == Some(&b'=') {
            i += 2;
            TokenKind::Equal
        } else if b";()[]{},+-*/^".contains(&bytes[i]) {
            let c = bytes[i] as char;
            i += 1;
            TokenKind::Symbol(c)
        } else {
            return Err(error(
                ErrorCode::Parse,
                "unexpected character",
                &span(i + 1),
            ));
        };
        if out.len() >= budget {
            return Err(error(ErrorCode::Limit, "token limit exceeded", &span(i)));
        }
        out.push(Token {
            kind,
            span: span(i),
        });
        col += i - start;
    }
    out.push(Token {
        kind: TokenKind::End,
        span: SourceSpan {
            source: name.to_owned(),
            line,
            column: col,
            offset: i,
            length: 0,
            expansion: vec![],
        },
    });
    Ok(out)
}
#[derive(Clone, Debug)]
struct Operand {
    name: String,
    index: Option<usize>,
    span: SourceSpan,
}
#[derive(Clone, Debug)]
struct Call {
    name: String,
    args: Vec<Expr>,
    operands: Vec<Operand>,
    span: SourceSpan,
}
#[derive(Clone, Debug)]
enum Statement {
    Call(Call),
    Measure(Operand, Operand, SourceSpan),
    Reset(Operand, SourceSpan),
    Barrier(Vec<Operand>, SourceSpan),
    Conditional(String, usize, Box<Statement>, SourceSpan),
}
#[derive(Clone, Debug)]
struct Definition {
    name: String,
    params: Vec<String>,
    qubits: Vec<String>,
    body: Option<Vec<Statement>>,
    span: SourceSpan,
    standard: bool,
}
#[derive(Clone, Debug)]
enum Item {
    Include(String, SourceSpan),
    Register(bool, String, usize, SourceSpan),
    Definition(Definition),
    Statement(Statement),
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    max_expr_depth: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn take(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if t.kind != TokenKind::End {
            self.pos += 1;
        }
        t
    }
    fn fail(&self, message: impl Into<String>) -> QasmError {
        error(ErrorCode::Parse, message, &self.token().span)
    }
    fn word(&mut self, w: &str) -> bool {
        if self.token().kind == TokenKind::Name(w.to_owned()) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn symbol(&mut self, c: char) -> bool {
        if self.token().kind == TokenKind::Symbol(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> Result<(), QasmError> {
        if self.symbol(c) {
            Ok(())
        } else {
            Err(self.fail(format!("expected '{c}'")))
        }
    }
    fn name(&mut self) -> Result<String, QasmError> {
        let t = self.take();
        match t.kind {
            TokenKind::Name(n) if !reserved(&n) => Ok(n),
            _ => Err(error(ErrorCode::Parse, "expected identifier", &t.span)),
        }
    }
    fn integer(&mut self) -> Result<usize, QasmError> {
        let t = self.take();
        match t.kind {
            TokenKind::Number(n) if n.bytes().all(|b| b.is_ascii_digit()) => {
                n.parse().map_err(|_| {
                    error(
                        ErrorCode::Limit,
                        "integer exceeds addressable range",
                        &t.span,
                    )
                })
            }
            _ => Err(error(
                ErrorCode::Parse,
                "expected non-negative integer",
                &t.span,
            )),
        }
    }
    fn names(&mut self) -> Result<Vec<String>, QasmError> {
        let mut v = vec![self.name()?];
        while self.symbol(',') {
            v.push(self.name()?);
        }
        Ok(v)
    }
    fn operand(&mut self) -> Result<Operand, QasmError> {
        let span = self.token().span.clone();
        let name = self.name()?;
        let index = if self.symbol('[') {
            let i = self.integer()?;
            self.expect(']')?;
            Some(i)
        } else {
            None
        };
        Ok(Operand { name, index, span })
    }
    fn operands(&mut self) -> Result<Vec<Operand>, QasmError> {
        let mut v = vec![self.operand()?];
        while self.symbol(',') {
            v.push(self.operand()?);
        }
        Ok(v)
    }
    fn expression(&mut self, min: u8, depth: usize) -> Result<Expr, QasmError> {
        if depth >= self.max_expr_depth {
            return Err(error(
                ErrorCode::Limit,
                "expression depth limit exceeded",
                &self.token().span,
            ));
        }
        let mut left = if self.symbol('-') {
            Expr::Neg(Box::new(self.expression(3, depth + 1)?))
        } else if self.symbol('(') {
            let e = self.expression(0, depth + 1)?;
            self.expect(')')?;
            e
        } else {
            let t = self.take();
            match t.kind {
                TokenKind::Number(n) => Expr::Number(n),
                TokenKind::Name(n) if n == "pi" => Expr::Pi,
                TokenKind::Name(n)
                    if matches!(n.as_str(), "sin" | "cos" | "tan" | "exp" | "ln" | "sqrt") =>
                {
                    self.expect('(')?;
                    let e = self.expression(0, depth + 1)?;
                    self.expect(')')?;
                    Expr::Function(n, Box::new(e))
                }
                TokenKind::Name(n) if !reserved(&n) => Expr::Name(n),
                _ => return Err(error(ErrorCode::Parse, "expected expression", &t.span)),
            }
        };
        loop {
            let (op, precedence) = match self.token().kind {
                TokenKind::Symbol(c @ ('+' | '-')) => (c, 1),
                TokenKind::Symbol(c @ ('*' | '/')) => (c, 2),
                TokenKind::Symbol('^') => ('^', 4),
                _ => break,
            };
            if precedence < min {
                break;
            }
            self.pos += 1;
            let right = self.expression(
                if op == '^' {
                    precedence
                } else {
                    precedence + 1
                },
                depth + 1,
            )?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
            left.ensure_depth(self.max_expr_depth)
                .map_err(|e| e.at(self.token().span.clone()))?;
        }
        left.ensure_depth(self.max_expr_depth)
            .map_err(|e| e.at(self.token().span.clone()))?;
        Ok(left)
    }
    fn statement(&mut self, body: bool) -> Result<Statement, QasmError> {
        let span = self.token().span.clone();
        if self.word("measure") {
            if body {
                return Err(error(
                    ErrorCode::Type,
                    "measurement is not legal in a gate body",
                    &span,
                ));
            }
            let q = self.operand()?;
            if self.take().kind != TokenKind::Arrow {
                return Err(self.fail("expected '->'"));
            }
            let c = self.operand()?;
            self.expect(';')?;
            return Ok(Statement::Measure(q, c, span));
        }
        if self.word("reset") {
            if body {
                return Err(error(
                    ErrorCode::Type,
                    "reset is not legal in a gate body",
                    &span,
                ));
            }
            let q = self.operand()?;
            self.expect(';')?;
            return Ok(Statement::Reset(q, span));
        }
        if self.word("barrier") {
            let qs = self.operands()?;
            self.expect(';')?;
            return Ok(Statement::Barrier(qs, span));
        }
        if self.word("if") {
            if body {
                return Err(error(
                    ErrorCode::Type,
                    "condition is not legal in a gate body",
                    &span,
                ));
            }
            self.expect('(')?;
            let c = self.name()?;
            if self.take().kind != TokenKind::Equal {
                return Err(self.fail("expected '=='"));
            }
            let value = self.integer()?;
            self.expect(')')?;
            if self.token().kind == TokenKind::Name("if".to_owned())
                || self.token().kind == TokenKind::Name("barrier".to_owned())
            {
                return Err(self.fail("if requires a quantum operation"));
            }
            let op = self.statement(false)?;
            return Ok(Statement::Conditional(c, value, Box::new(op), span));
        }
        let name = if self.word("U") {
            "U".to_owned()
        } else if self.word("CX") {
            "CX".to_owned()
        } else {
            self.name()?
        };
        let mut args = Vec::new();
        if self.symbol('(') {
            if !self.symbol(')') {
                args.push(self.expression(0, 0)?);
                while self.symbol(',') {
                    args.push(self.expression(0, 0)?);
                }
                self.expect(')')?;
            }
        }
        let operands = self.operands()?;
        self.expect(';')?;
        Ok(Statement::Call(Call {
            name,
            args,
            operands,
            span,
        }))
    }
    fn items(&mut self, root: bool, standard: bool) -> Result<Vec<Item>, QasmError> {
        if root {
            if !self.word("OPENQASM") {
                return Err(self.fail("required OPENQASM 2.0 header"));
            }
            if self.take().kind != TokenKind::Number("2.0".to_owned()) {
                return Err(self.fail("only OPENQASM 2.0 is supported"));
            }
            self.expect(';')?;
        }
        let mut items = vec![];
        while self.token().kind != TokenKind::End {
            let span = self.token().span.clone();
            if self.word("include") {
                let t = self.take();
                let TokenKind::String(n) = t.kind else {
                    return Err(self.fail("expected include filename"));
                };
                self.expect(';')?;
                items.push(Item::Include(n, span));
                continue;
            }
            let quantum = if self.word("qreg") {
                Some(true)
            } else if self.word("creg") {
                Some(false)
            } else {
                None
            };
            if let Some(q) = quantum {
                let n = self.name()?;
                self.expect('[')?;
                let size = self.integer()?;
                self.expect(']')?;
                self.expect(';')?;
                items.push(Item::Register(q, n, size, span));
                continue;
            }
            let opaque = if self.word("gate") {
                Some(false)
            } else if self.word("opaque") {
                Some(true)
            } else {
                None
            };
            if let Some(opaque) = opaque {
                let name = self.name()?;
                let mut params = vec![];
                if self.symbol('(') {
                    if !self.symbol(')') {
                        params = self.names()?;
                        self.expect(')')?;
                    }
                }
                let qubits = self.names()?;
                let body = if opaque {
                    self.expect(';')?;
                    None
                } else {
                    self.expect('{')?;
                    let mut body = vec![];
                    while !self.symbol('}') {
                        if self.token().kind == TokenKind::End {
                            return Err(self.fail("unterminated gate definition"));
                        }
                        body.push(self.statement(true)?);
                    }
                    Some(body)
                };
                items.push(Item::Definition(Definition {
                    name,
                    params,
                    qubits,
                    body,
                    span,
                    standard,
                }));
                continue;
            }
            items.push(Item::Statement(self.statement(false)?));
        }
        Ok(items)
    }
}
fn reserved(n: &str) -> bool {
    matches!(
        n,
        "OPENQASM"
            | "U"
            | "CX"
            | "qreg"
            | "creg"
            | "gate"
            | "opaque"
            | "include"
            | "measure"
            | "reset"
            | "barrier"
            | "if"
            | "pi"
            | "sin"
            | "cos"
            | "tan"
            | "exp"
            | "ln"
            | "sqrt"
    )
}

struct Frontend<'a> {
    resolver: &'a dyn IncludeResolver,
    limits: &'a QasmLimits,
    bytes: usize,
    files: usize,
    tokens: usize,
    expansion_steps: usize,
    retained_targets: usize,
    includes: Vec<String>,
    defs: HashMap<String, Definition>,
    symbols: HashSet<String>,
    circuit: QasmCircuit,
    statements: Vec<Statement>,
}
impl Frontend<'_> {
    fn load(
        &mut self,
        text: &str,
        name: &str,
        root: bool,
        standard: bool,
    ) -> Result<(), QasmError> {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .ok_or_else(|| QasmError::new(ErrorCode::Limit, "source byte limit exceeded"))?;
        if self.bytes > self.limits.max_source_bytes {
            return Err(QasmError::new(
                ErrorCode::Limit,
                "source byte limit exceeded",
            ));
        }
        if self.includes.len() > self.limits.max_include_depth {
            return Err(QasmError::new(
                ErrorCode::Limit,
                "include depth limit exceeded",
            ));
        }
        if self.includes.iter().any(|n| n == name) {
            return Err(QasmError::new(
                ErrorCode::Type,
                format!("include cycle: {} -> {name}", self.includes.join(" -> ")),
            ));
        }
        self.includes.push(name.to_owned());
        let mut tokens = lex(
            text,
            name,
            self.limits.max_tokens.saturating_sub(self.tokens),
        )?;
        self.tokens += tokens.len() - 1;
        for token in &mut tokens {
            token.span.expansion = self.includes[..self.includes.len() - 1]
                .iter()
                .map(|n| format!("included from {n}"))
                .collect();
        }
        let mut parser = Parser {
            tokens,
            pos: 0,
            max_expr_depth: self.limits.max_expr_depth,
        };
        let items = parser.items(root, standard)?;
        for item in items {
            match item {
                Item::Include(request, span) => {
                    if self.includes.len() > self.limits.max_include_depth {
                        return Err(error(
                            ErrorCode::Limit,
                            "include depth limit exceeded",
                            &span,
                        ));
                    }
                    self.files += 1;
                    if self.files > self.limits.max_include_files {
                        return Err(error(
                            ErrorCode::Limit,
                            "include file limit exceeded",
                            &span,
                        ));
                    }
                    let result = if request == "qelib1.inc" {
                        self.load(QELIB, QELIB_SOURCE, false, true)
                    } else {
                        let src = self
                            .resolver
                            .resolve(&request, name)
                            .map_err(|e| e.at(span.clone()))?;
                        self.load(&src.text, &src.name, false, false)
                    };
                    result.map_err(|mut e| {
                        // Preserve an included source's own span when available.
                        // The call/include chain is also useful when the failing file has no tokens.
                        if e.origin.is_none() {
                            e = e.at(span.clone());
                        }
                        if let Some(origin) = &mut e.origin {
                            origin.expansion.push(format!(
                                "include {} at {}:{}:{}",
                                request, span.source, span.line, span.column
                            ));
                        }
                        e
                    })?;
                }
                Item::Register(q, name, size, span) => {
                    if size == 0 {
                        return Err(error(
                            ErrorCode::Type,
                            "registers must have positive width",
                            &span,
                        ));
                    }
                    if !self.symbols.insert(name.clone()) {
                        return Err(error(
                            ErrorCode::Type,
                            format!("duplicate declaration {name}"),
                            &span,
                        ));
                    }
                    let regs = if q {
                        &mut self.circuit.quantum_registers
                    } else {
                        &mut self.circuit.classical_registers
                    };
                    let offset = regs.last().map_or(0, |r| r.offset + r.size);
                    let total = offset
                        .checked_add(size)
                        .ok_or_else(|| error(ErrorCode::Limit, "register size overflow", &span))?;
                    if total
                        > if q {
                            self.limits.max_qubits.min(u32::MAX as usize)
                        } else {
                            self.limits.max_classical_bits
                        }
                    {
                        return Err(error(
                            ErrorCode::Limit,
                            "register width limit exceeded",
                            &span,
                        ));
                    }
                    regs.push(Register { name, size, offset });
                }
                Item::Definition(d) => {
                    if let Some(body) = &d.body {
                        for statement in body {
                            if let Statement::Call(call) = statement {
                                self.call_check(call, &d.params, Some(&d.qubits))?;
                            }
                        }
                    }
                    if !self.symbols.insert(d.name.clone()) {
                        return Err(error(
                            ErrorCode::Type,
                            format!("duplicate declaration {}", d.name),
                            &d.span,
                        ));
                    }
                    self.defs.insert(d.name.clone(), d);
                }
                Item::Statement(s) => {
                    self.check_visible(&s)?;
                    self.statements.push(s)
                }
            }
        }
        self.includes.pop();
        Ok(())
    }
    fn check_visible(&self, s: &Statement) -> Result<(), QasmError> {
        match s {
            Statement::Call(c) => {
                self.call_check(c, &[], None)?;
                for o in &c.operands {
                    self.operand(o, true, None)?;
                }
                Ok(())
            }
            Statement::Measure(q, c, _) => {
                self.operand(q, true, None)?;
                self.operand(c, false, None)?;
                Ok(())
            }
            Statement::Reset(q, _) => {
                self.operand(q, true, None)?;
                Ok(())
            }
            Statement::Barrier(qs, _) => {
                for q in qs {
                    self.operand(q, true, None)?;
                }
                Ok(())
            }
            Statement::Conditional(name, _, s, span) => {
                if !self
                    .circuit
                    .classical_registers
                    .iter()
                    .any(|r| r.name == *name)
                {
                    return Err(error(
                        ErrorCode::Type,
                        "condition requires a declared classical register",
                        span,
                    ));
                }
                self.check_visible(s)
            }
        }
    }
    fn signature(&self, name: &str, span: &SourceSpan) -> Result<(usize, usize), QasmError> {
        match name {
            "U" => Ok((3, 1)),
            "CX" => Ok((0, 2)),
            _ => self
                .defs
                .get(name)
                .map(|d| (d.params.len(), d.qubits.len()))
                .ok_or_else(|| error(ErrorCode::Type, format!("undeclared gate {name}"), span)),
        }
    }
    fn call_check(
        &self,
        c: &Call,
        params: &[String],
        qubits: Option<&[String]>,
    ) -> Result<(), QasmError> {
        let (p, q) = self.signature(&c.name, &c.span)?;
        if p != c.args.len() || q != c.operands.len() {
            return Err(error(
                ErrorCode::Type,
                format!(
                    "gate {} requires {p} parameters and {q} qubit operands",
                    c.name
                ),
                &c.span,
            ));
        }
        for e in &c.args {
            e.check(params).map_err(|e| e.at(c.span.clone()))?;
        }
        if let Some(formals) = qubits {
            for o in &c.operands {
                if o.index.is_some() || !formals.contains(&o.name) {
                    return Err(error(
                        ErrorCode::Type,
                        "gate bodies may use only unindexed formal qubits",
                        &o.span,
                    ));
                }
            }
            let mut seen = HashSet::new();
            for o in &c.operands {
                if !seen.insert(&o.name) {
                    return Err(error(ErrorCode::Type, "aliased qubit operands", &o.span));
                }
            }
        }
        Ok(())
    }
    fn check_defs(&self) -> Result<(), QasmError> {
        let mut ordered: Vec<_> = self.defs.values().collect();
        ordered.sort_by(|a, b| a.name.cmp(&b.name));
        for d in ordered {
            let mut names = HashSet::new();
            for n in d.params.iter().chain(d.qubits.iter()) {
                if !names.insert(n) {
                    return Err(error(
                        ErrorCode::Type,
                        "duplicate gate formal parameter",
                        &d.span,
                    ));
                }
            }
            if let Some(body) = &d.body {
                for s in body {
                    match s {
                        Statement::Call(c) => self.call_check(c, &d.params, Some(&d.qubits))?,
                        Statement::Barrier(qs, _) => {
                            for q in qs {
                                if q.index.is_some() || !d.qubits.contains(&q.name) {
                                    return Err(error(
                                        ErrorCode::Type,
                                        "invalid barrier formal qubit",
                                        &q.span,
                                    ));
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        fn visit(
            front: &Frontend<'_>,
            name: &str,
            path: &mut Vec<String>,
            done: &mut HashSet<String>,
        ) -> Result<(), QasmError> {
            if done.contains(name) {
                return Ok(());
            }
            let d = &front.defs[name];
            if path.iter().any(|n| n == name) {
                return Err(error(
                    ErrorCode::Type,
                    format!("recursive gate definition: {} -> {name}", path.join(" -> ")),
                    &d.span,
                ));
            }
            if path.len() >= front.limits.max_gate_depth {
                return Err(error(
                    ErrorCode::Limit,
                    "gate depth limit exceeded",
                    &d.span,
                ));
            }
            path.push(name.to_owned());
            if let Some(body) = &d.body {
                for s in body {
                    if let Statement::Call(c) = s {
                        if front.defs.contains_key(&c.name) {
                            visit(front, &c.name, path, done)?;
                        }
                    }
                }
            }
            path.pop();
            done.insert(name.to_owned());
            Ok(())
        }
        let mut done = HashSet::new();
        let mut names: Vec<_> = self.defs.keys().collect();
        names.sort();
        for name in names {
            visit(self, name, &mut vec![], &mut done)?;
        }
        Ok(())
    }
    fn operand(
        &self,
        o: &Operand,
        quantum: bool,
        locals: Option<&HashMap<String, u32>>,
    ) -> Result<(Vec<usize>, bool), QasmError> {
        if let Some(locals) = locals {
            if o.index.is_none() {
                if let Some(q) = locals.get(&o.name) {
                    return Ok((vec![*q as usize], false));
                }
            }
            return Err(error(
                ErrorCode::Type,
                "invalid gate formal operand",
                &o.span,
            ));
        }
        let regs = if quantum {
            &self.circuit.quantum_registers
        } else {
            &self.circuit.classical_registers
        };
        let reg = regs.iter().find(|r| r.name == o.name).ok_or_else(|| {
            error(
                ErrorCode::Type,
                format!(
                    "expected {} register {}",
                    if quantum { "quantum" } else { "classical" },
                    o.name
                ),
                &o.span,
            )
        })?;
        if let Some(i) = o.index {
            if i >= reg.size {
                return Err(error(
                    ErrorCode::Type,
                    "register index out of bounds",
                    &o.span,
                ));
            }
            Ok((vec![reg.offset + i], false))
        } else {
            Ok(((reg.offset..reg.offset + reg.size).collect(), true))
        }
    }
    fn push(
        &mut self,
        kind: OperationKind,
        span: &SourceSpan,
        stack: &[String],
    ) -> Result<(), QasmError> {
        if self.circuit.operations.len() >= self.limits.max_operations {
            return Err(error(
                ErrorCode::Limit,
                "operation expansion limit exceeded",
                span,
            ));
        }
        let targets = self
            .retained_targets
            .checked_add(super::lower::target_work(&kind))
            .ok_or_else(|| {
                error(
                    ErrorCode::Limit,
                    "expanded operand work overflows the supported range",
                    span,
                )
            })?;
        if targets > self.limits.max_operations {
            return Err(error(
                ErrorCode::Limit,
                "expanded operand work exceeds limit",
                span,
            ));
        }
        self.retained_targets = targets;
        let mut origin = span.clone();
        origin.expansion.extend_from_slice(stack);
        self.circuit.operations.push(Operation {
            kind,
            origin: Some(origin),
        });
        Ok(())
    }
    fn call(
        &mut self,
        c: &Call,
        env: &HashMap<String, Angle>,
        locals: Option<&HashMap<String, u32>>,
        stack: &mut Vec<String>,
    ) -> Result<(), QasmError> {
        self.expansion_steps = self.expansion_steps.checked_add(1).ok_or_else(|| {
            error(
                ErrorCode::Limit,
                "gate expansion work limit exceeded",
                &c.span,
            )
        })?;
        if self.expansion_steps
            > self
                .limits
                .max_operations
                .saturating_mul(self.limits.max_gate_depth.max(1))
        {
            return Err(error(
                ErrorCode::Limit,
                "gate expansion work limit exceeded",
                &c.span,
            ));
        }
        self.call_check(c, &env.keys().cloned().collect::<Vec<_>>(), None)?;
        if stack.len() >= self.limits.max_gate_depth {
            return Err(error(
                ErrorCode::Limit,
                "gate expansion depth exceeded",
                &c.span,
            ));
        }
        let args = c
            .args
            .iter()
            .map(|e| e.eval(env).map_err(|e| e.at(c.span.clone())))
            .collect::<Result<Vec<_>, _>>()?;
        let operands = c
            .operands
            .iter()
            .map(|o| self.operand(o, true, locals))
            .collect::<Result<Vec<_>, _>>()?;
        let mut width = None;
        for (values, whole) in &operands {
            if *whole {
                if width.is_some_and(|n| n != values.len()) {
                    return Err(error(
                        ErrorCode::Type,
                        "register broadcast widths differ",
                        &c.span,
                    ));
                }
                width = Some(values.len());
            }
        }
        for i in 0..width.unwrap_or(1) {
            let targets: Vec<u32> = operands
                .iter()
                .map(|(v, whole)| v[if *whole { i } else { 0 }] as u32)
                .collect();
            let mut seen = HashSet::new();
            if targets.iter().any(|q| !seen.insert(*q)) {
                return Err(error(
                    ErrorCode::Type,
                    "aliased qubit operands after register expansion",
                    &c.span,
                ));
            }
            if c.name == "U" {
                self.push(
                    OperationKind::Gate {
                        gate: Gate::U {
                            theta: args[0].clone(),
                            phi: args[1].clone(),
                            lambda: args[2].clone(),
                        },
                        targets,
                    },
                    &c.span,
                    stack,
                )?;
                continue;
            }
            if c.name == "CX" {
                self.push(
                    OperationKind::Gate {
                        gate: Gate::CX,
                        targets,
                    },
                    &c.span,
                    stack,
                )?;
                continue;
            }
            let d = self.defs[&c.name].clone();
            if d.standard {
                if let Some(gate) = standard_gate(&d.name, &args) {
                    self.push(OperationKind::Gate { gate, targets }, &c.span, stack)?;
                    continue;
                }
            }
            let body = d.body.ok_or_else(|| {
                error(
                    ErrorCode::Unsupported,
                    format!("opaque gate {} has no implementation", d.name),
                    &c.span,
                )
            })?;
            let env = d.params.iter().cloned().zip(args.iter().cloned()).collect();
            let locals = d
                .qubits
                .iter()
                .cloned()
                .zip(targets.iter().copied())
                .collect();
            stack.push(format!(
                "{} at {}:{}:{}",
                c.name, c.span.source, c.span.line, c.span.column
            ));
            for s in &body {
                self.statement(s, &env, Some(&locals), stack)
                    .map_err(|mut e| {
                        if let Some(origin) = &mut e.origin {
                            origin.expansion.extend_from_slice(stack);
                        }
                        e
                    })?;
            }
            stack.pop();
        }
        Ok(())
    }
    fn statement(
        &mut self,
        s: &Statement,
        env: &HashMap<String, Angle>,
        locals: Option<&HashMap<String, u32>>,
        stack: &mut Vec<String>,
    ) -> Result<(), QasmError> {
        match s {
            Statement::Call(c) => self.call(c, env, locals, stack),
            Statement::Measure(q, c, span) => {
                let (qs, qwhole) = self.operand(q, true, locals)?;
                let (cs, cwhole) = self.operand(c, false, None)?;
                if qwhole != cwhole || qs.len() != cs.len() {
                    return Err(error(
                        ErrorCode::Type,
                        "measurement operands must be matching scalars or equally sized registers",
                        span,
                    ));
                }
                for (q, b) in qs.into_iter().zip(cs) {
                    self.push(
                        OperationKind::Measure {
                            qubit: q as u32,
                            bit: b,
                        },
                        span,
                        stack,
                    )?;
                }
                Ok(())
            }
            Statement::Reset(q, span) => {
                for q in self.operand(q, true, locals)?.0 {
                    self.push(OperationKind::Reset { qubit: q as u32 }, span, stack)?;
                }
                Ok(())
            }
            Statement::Barrier(qs, span) => {
                let mut qubits = Vec::new();
                let mut seen = HashSet::new();
                for q in qs {
                    for index in self.operand(q, true, locals)?.0 {
                        let index = index as u32;
                        if !seen.insert(index) {
                            return Err(error(ErrorCode::Type, "aliased barrier operands", span));
                        }
                        qubits.push(index);
                    }
                }
                self.push(OperationKind::Barrier { qubits }, span, stack)
            }
            Statement::Conditional(name, value, body, span) => {
                let reg = self
                    .circuit
                    .classical_registers
                    .iter()
                    .find(|r| r.name == *name)
                    .ok_or_else(|| {
                        error(
                            ErrorCode::Type,
                            "condition requires a declared classical register",
                            span,
                        )
                    })?;
                let (size, bit) = (reg.size, reg.offset);
                // First validate the actual operation even if the condition is unsupported.
                let start = self.circuit.operations.len();
                self.statement(body, env, locals, stack)?;
                if size < usize::BITS as usize && *value >= (1usize << size) {
                    return Err(error(
                        ErrorCode::Type,
                        "condition value does not fit classical register",
                        span,
                    ));
                }
                if size != 1 {
                    return Err(error(
                        ErrorCode::Unsupported,
                        "only one-bit classical conditions are supported",
                        span,
                    ));
                }
                let expanded = self.circuit.operations.split_off(start);
                // Replace the temporary expanded gates with semantic conditions;
                // only retained operations consume this target-storage budget.
                for operation in &expanded {
                    self.retained_targets -= super::lower::target_work(&operation.kind);
                }
                for op in expanded {
                    let OperationKind::Gate { gate, targets } = op.kind else {
                        return Err(error(
                            ErrorCode::Unsupported,
                            "only conditional Pauli gates are supported",
                            span,
                        ));
                    };
                    let gate = conditional_pauli(&gate).ok_or_else(|| {
                        error(
                            ErrorCode::Unsupported,
                            "only conditional Pauli gates are supported",
                            span,
                        )
                    })?;
                    self.push(
                        OperationKind::Conditional {
                            bit,
                            value: *value == 1,
                            gate,
                            targets,
                        },
                        span,
                        stack,
                    )?;
                }
                Ok(())
            }
        }
    }
}
// Primitive U operations can represent a Pauli up to one global phase.
// For classical control that phase is unobservable; no approximate angles are snapped.
fn conditional_pauli(gate: &Gate) -> Option<Pauli> {
    match gate {
        Gate::X => Some(Pauli::X),
        Gate::Y => Some(Pauli::Y),
        Gate::Z => Some(Pauli::Z),
        Gate::U { theta, phi, lambda } => {
            let theta = theta.clifford_quarters()?;
            let phi = phi.clifford_quarters()?;
            let lambda = lambda.clifford_quarters()?;
            match (
                theta,
                (phi - lambda).rem_euclid(4),
                (phi + lambda).rem_euclid(4),
            ) {
                (0, _, 2) => Some(Pauli::Z),
                (2, 2, _) => Some(Pauli::X),
                (2, 0, _) => Some(Pauli::Y),
                _ => None,
            }
        }
        _ => None,
    }
}
fn standard_gate(name: &str, args: &[Angle]) -> Option<Gate> {
    Some(match name {
        "id" => Gate::I,
        "h" => Gate::H,
        "x" => Gate::X,
        "y" => Gate::Y,
        "z" => Gate::Z,
        "s" => Gate::S,
        "sdg" => Gate::SDag,
        "t" => Gate::T,
        "tdg" => Gate::TDag,
        "cx" => Gate::CX,
        "cy" => Gate::CY,
        "cz" => Gate::CZ,
        "rx" => Gate::Rx(args[0].clone()),
        "ry" => Gate::Ry(args[0].clone()), // Original rz is u1, equal up to one overall phase at this arity.
        "rz" => Gate::Rz(args[0].clone()),
        "u1" => Gate::U {
            theta: Angle::pi_ratio(0, 1).ok()?,
            phi: Angle::pi_ratio(0, 1).ok()?,
            lambda: args[0].clone(),
        },
        "u2" => Gate::U {
            theta: Angle::pi_ratio(1, 2).ok()?,
            phi: args[0].clone(),
            lambda: args[1].clone(),
        },
        "u3" => Gate::U {
            theta: args[0].clone(),
            phi: args[1].clone(),
            lambda: args[2].clone(),
        },
        _ => return None,
    })
}
pub(super) fn parse(
    source: &str,
    name: &str,
    resolver: &dyn IncludeResolver,
    limits: &QasmLimits,
) -> Result<QasmCircuit, QasmError> {
    let mut f = Frontend {
        resolver,
        limits,
        bytes: 0,
        files: 0,
        tokens: 0,
        expansion_steps: 0,
        retained_targets: 0,
        includes: vec![],
        defs: HashMap::new(),
        symbols: HashSet::new(),
        circuit: QasmCircuit {
            quantum_registers: vec![],
            classical_registers: vec![],
            operations: vec![],
        },
        statements: vec![],
    };
    f.load(source, name, true, false)?;
    f.check_defs()?;
    let statements = std::mem::take(&mut f.statements);
    for s in statements {
        f.statement(&s, &HashMap::new(), None, &mut vec![])?;
    }
    Ok(f.circuit)
}
