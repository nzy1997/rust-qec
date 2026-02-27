use crate::ir::{PauliBasis, StimInstr, StimTarget};

pub fn parse_lines(input: &str) -> Result<Vec<StimInstr>, String> {
    let mut stack: Vec<Vec<StimInstr>> = vec![Vec::new()];
    let mut repeat_counts: Vec<u64> = Vec::new();

    for (line_no, raw) in input.lines().enumerate() {
        let line = raw.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            let body = stack.pop().ok_or_else(|| format!("line {}: unmatched }}", line_no + 1))?;
            let count = repeat_counts.pop().ok_or_else(|| format!("line {}: unmatched }}", line_no + 1))?;
            if count == 0 {
                return Err(format!("line {}: REPEAT 0 not allowed", line_no + 1));
            }
            let repeat = StimInstr::Repeat { count, body };
            stack
                .last_mut()
                .ok_or_else(|| format!("line {}: repeat outside program", line_no + 1))?
                .push(repeat);
            continue;
        }

        let mut is_block_start = false;
        let mut line = line.to_string();
        if line.ends_with('{') {
            is_block_start = true;
            line = line[..line.len() - 1].trim().to_string();
        }

        let mut parts = line.split_whitespace();
        let name_token = parts
            .next()
            .ok_or_else(|| format!("line {}: empty", line_no + 1))?;
        let (name, tag, args) = split_name_and_args(name_token)?;
        let name = name.to_ascii_uppercase();

        if is_block_start {
            if name != "REPEAT" {
                return Err(format!("line {}: only REPEAT opens a block", line_no + 1));
            }
            let count_token = parts
                .next()
                .ok_or_else(|| format!("line {}: missing repeat count", line_no + 1))?;
            let count: u64 = count_token
                .parse()
                .map_err(|_| format!("line {}: bad repeat count", line_no + 1))?;
            repeat_counts.push(count);
            stack.push(Vec::new());
            continue;
        }

        let mut instr = StimInstr::Op {
            name: name.clone(),
            tag,
            args,
            targets: vec![],
        };
        if let StimInstr::Op { targets, .. } = &mut instr {
            for token in parts {
                let subs: Vec<&str> = token.split('*').collect();
                if subs.len() > 1 {
                    for (j, sub) in subs.iter().enumerate() {
                        if sub.is_empty() {
                            return Err(format!("line {}: empty target around *", line_no + 1));
                        }
                        if j > 0 {
                            targets.push(StimTarget::Combiner);
                        }
                        if let Some(t) = parse_target(sub)? {
                            targets.push(t);
                        }
                    }
                } else if let Some(t) = parse_target(token)? {
                    targets.push(t);
                }
            }
        }
        stack
            .last_mut()
            .ok_or_else(|| format!("line {}: instruction outside program", line_no + 1))?
            .push(instr);
    }

    if stack.len() != 1 {
        return Err("unterminated REPEAT block".to_string());
    }

    Ok(stack.pop().unwrap())
}

fn parse_target(token: &str) -> Result<Option<StimTarget>, String> {
    if token.starts_with("rec[") && token.ends_with(']') {
        let inner = &token[4..token.len() - 1];
        let val: i32 = inner
            .parse()
            .map_err(|_| format!("bad rec target {token}"))?;
        if val >= 0 {
            return Err("rec must be negative".to_string());
        }
        return Ok(Some(StimTarget::Rec(val)));
    }
    let (negated, raw) = if let Some(rest) = token.strip_prefix('!') {
        (true, rest)
    } else {
        (false, token)
    };
    if raw.len() > 1 {
        let first = raw.as_bytes()[0];
        if matches!(first, b'X' | b'Y' | b'Z') {
            if let Ok(q) = raw[1..].parse::<u32>() {
                let basis = match first {
                    b'X' => PauliBasis::X,
                    b'Y' => PauliBasis::Y,
                    _ => PauliBasis::Z,
                };
                return Ok(Some(StimTarget::pauli(q, basis, negated)));
            }
        }
    }
    if let Ok(q) = raw.parse::<u32>() {
        if negated {
            return Ok(Some(StimTarget::QubitInv(q)));
        }
        return Ok(Some(StimTarget::Qubit(q)));
    }
    // Use `token` (not `raw`) here so that `!sweep[0]` is rejected:
    // negated sweep targets are invalid in Stim, and stripping the `!`
    // prefix before this check would silently accept them.
    if token.starts_with("sweep[") && token.ends_with(']') {
        let inner = &token[6..token.len() - 1];
        let k: u32 = inner.parse().map_err(|_| format!("bad sweep target {token}"))?;
        return Ok(Some(StimTarget::Sweep(k)));
    }
    Err(format!("unsupported target {token}"))
}

fn split_name_and_args(token: &str) -> Result<(&str, Option<String>, Vec<f64>), String> {
    let (name, tag, rest) = if let Some(bi) = token.find('[') {
        let bj = token.find(']').ok_or_else(|| format!("unclosed tag bracket in {token}"))?;
        if bj < bi {
            return Err(format!("bad tag syntax in {token}"));
        }
        let name = &token[..bi];
        let tag = Some(token[bi + 1..bj].to_string());
        (name, tag, &token[bj + 1..])
    } else {
        (token, None, "")
    };

    let args = if let Some(rest) = rest.strip_prefix('(') {
        let rest = rest.strip_suffix(')').ok_or_else(|| format!("bad args {token}"))?;
        let rest = rest.trim();
        if rest.is_empty() {
            vec![]
        } else {
            rest.split(',')
                .map(|s| s.trim().parse::<f64>().map_err(|_| format!("bad arg {s}")))
                .collect::<Result<Vec<_>, _>>()?
        }
    } else if !rest.is_empty() {
        return Err(format!("unexpected characters after tag in {token}"));
    } else if let Some(idx) = name.find('(') {
        if !name.ends_with(')') {
            return Err(format!("bad args {token}"));
        }
        let args_str = name[idx + 1..name.len() - 1].trim();
        let actual_name = &name[..idx];
        let args = if args_str.is_empty() {
            vec![]
        } else {
            args_str
                .split(',')
                .map(|s| s.trim().parse::<f64>().map_err(|_| format!("bad arg {s}")))
                .collect::<Result<Vec<_>, _>>()?
        };
        return Ok((actual_name, tag, args));
    } else {
        vec![]
    };

    Ok((name, tag, args))
}
