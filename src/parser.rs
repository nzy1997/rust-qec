use crate::ir::{StimInstr, StimTarget};

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
        let (name, args) = split_name_and_args(name_token)?;
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

        let mut instr = StimInstr::new(&name, args, vec![]);
        if let StimInstr::Op { targets, .. } = &mut instr {
            for token in parts {
                if let Some(t) = parse_target(token)? {
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
    if let Ok(q) = raw.parse::<u32>() {
        if negated {
            return Ok(Some(StimTarget::QubitInv(q)));
        }
        return Ok(Some(StimTarget::Qubit(q)));
    }
    Err(format!("unsupported target {token}"))
}

fn split_name_and_args(token: &str) -> Result<(&str, Vec<f64>), String> {
    if let Some(idx) = token.find('(') {
        if !token.ends_with(')') {
            return Err(format!("bad args {token}"));
        }
        let name = &token[..idx];
        let args_str = token[idx + 1..token.len() - 1].trim();
        let args = if args_str.is_empty() {
            vec![]
        } else {
            args_str
                .split(',')
                .map(|s| s.trim().parse::<f64>().map_err(|_| format!("bad arg {s}")))
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok((name, args))
    } else {
        Ok((token, vec![]))
    }
}
