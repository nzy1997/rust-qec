use crate::sim::bit_table::BitTable;
use rand::Rng;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DemTarget {
    Detector(usize),
    Observable(usize),
    Separator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DemInstruction {
    Error {
        probability: f64,
        targets: Vec<DemTarget>,
    },
    Detector {
        index: usize,
        coords: Vec<f64>,
    },
    LogicalObservable {
        index: usize,
    },
    ShiftDetectors {
        detector_offset: usize,
        coord_offsets: Vec<f64>,
    },
    Repeat {
        count: u64,
        body: DetectorErrorModel,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetectorErrorModel {
    instrs: Vec<DemInstruction>,
    num_detectors: usize,
    num_observables: usize,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instructions(&self) -> &[DemInstruction] {
        &self.instrs
    }

    pub fn num_detectors(&self) -> usize {
        self.num_detectors
    }

    pub fn num_observables(&self) -> usize {
        self.num_observables
    }

    pub fn set_min_counts(&mut self, detectors: usize, observables: usize) {
        self.num_detectors = self.num_detectors.max(detectors);
        self.num_observables = self.num_observables.max(observables);
    }

    pub fn add_error(&mut self, probability: f64, targets: Vec<DemTarget>) {
        self.push(DemInstruction::Error {
            probability,
            targets,
        });
    }

    pub fn add_detector(&mut self, index: usize, coords: Vec<f64>) {
        self.push(DemInstruction::Detector { index, coords });
    }

    pub fn add_observable(&mut self, index: usize) {
        self.push(DemInstruction::LogicalObservable { index });
    }

    pub fn add_shift_detectors(&mut self, detector_offset: usize, coord_offsets: Vec<f64>) {
        self.push(DemInstruction::ShiftDetectors {
            detector_offset,
            coord_offsets,
        });
    }

    pub fn add_repeat(&mut self, count: u64, body: DetectorErrorModel) {
        self.push(DemInstruction::Repeat { count, body });
    }

    pub fn push(&mut self, instr: DemInstruction) {
        match &instr {
            DemInstruction::Error { targets, .. } => self.update_counts_from_targets(targets),
            DemInstruction::Detector { index, .. } => {
                self.num_detectors = self.num_detectors.max(index + 1);
            }
            DemInstruction::LogicalObservable { index } => {
                self.num_observables = self.num_observables.max(index + 1);
            }
            DemInstruction::ShiftDetectors { .. } => {}
            DemInstruction::Repeat { body, .. } => {
                self.num_detectors = self.num_detectors.max(body.num_detectors);
                self.num_observables = self.num_observables.max(body.num_observables);
            }
        }
        self.instrs.push(instr);
    }

    fn update_counts_from_targets(&mut self, targets: &[DemTarget]) {
        for target in targets {
            match target {
                DemTarget::Detector(i) => {
                    self.num_detectors = self.num_detectors.max(i + 1);
                }
                DemTarget::Observable(i) => {
                    self.num_observables = self.num_observables.max(i + 1);
                }
                DemTarget::Separator => {}
            }
        }
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let mut lines: Vec<&str> = input.lines().collect();
        let mut pos = 0;
        parse_block(&mut lines, &mut pos, false)
    }

    pub fn effective_num_detectors(&self) -> usize {
        self.num_detectors.max(effective_det_count(&self.instrs, 0))
    }

    pub fn sample(&self, rng: &mut impl Rng) -> (Vec<bool>, Vec<bool>) {
        let nd = self.effective_num_detectors();
        let mut dets = vec![false; nd];
        let mut obs = vec![false; self.num_observables];
        sample_instrs(&self.instrs, &mut dets, &mut obs, 0, rng);
        (dets, obs)
    }

    pub fn sample_batch(&self, num_shots: usize, rng: &mut impl Rng) -> DemBatchOutput {
        self.try_sample_batch(num_shots, rng)
            .expect("trusted DEM sample dimensions allocate")
    }

    pub fn try_sample_batch(
        &self,
        num_shots: usize,
        rng: &mut impl Rng,
    ) -> Result<DemBatchOutput, String> {
        let nd = self.effective_num_detectors();
        let mut out = DemBatchOutput {
            detections: BitTable::try_new(nd, num_shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?,
            observable_flips: BitTable::try_new(self.num_observables, num_shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?,
        };
        let wpr = out.detections.words_per_row();
        sample_instrs_batch(&self.instrs, &mut out, wpr, 0, rng);
        Ok(out)
    }
}

pub struct DemBatchOutput {
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

fn effective_det_count(instrs: &[DemInstruction], initial_offset: usize) -> usize {
    let mut offset = initial_offset;
    let mut max_det = 0usize;
    for instr in instrs {
        match instr {
            DemInstruction::Error { targets, .. } => {
                for t in targets {
                    if let DemTarget::Detector(i) = t {
                        max_det = max_det.max(i + offset + 1);
                    }
                }
            }
            DemInstruction::Detector { index, .. } => {
                max_det = max_det.max(index + offset + 1);
            }
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                if *count > 0 {
                    let body_shift = total_shift(body.instructions());
                    for i in 0..*count {
                        let iter_offset = offset + (i as usize) * body_shift;
                        let iter_max = effective_det_count(body.instructions(), iter_offset);
                        max_det = max_det.max(iter_max);
                    }
                    offset += (*count as usize) * body_shift;
                }
            }
            DemInstruction::LogicalObservable { .. } => {}
        }
    }
    max_det
}

fn total_shift(instrs: &[DemInstruction]) -> usize {
    let mut shift = 0;
    for instr in instrs {
        match instr {
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                shift += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                shift += (*count as usize) * total_shift(body.instructions());
            }
            _ => {}
        }
    }
    shift
}

fn sample_instrs(
    instrs: &[DemInstruction],
    dets: &mut [bool],
    obs: &mut [bool],
    det_offset: usize,
    rng: &mut impl Rng,
) -> usize {
    let mut offset = det_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                if rng.r#gen::<f64>() < *probability {
                    for t in targets {
                        match t {
                            DemTarget::Detector(i) => {
                                let idx = i + offset;
                                if idx < dets.len() {
                                    dets[idx] ^= true;
                                }
                            }
                            DemTarget::Observable(i) => {
                                if *i < obs.len() {
                                    obs[*i] ^= true;
                                }
                            }
                            DemTarget::Separator => {}
                        }
                    }
                }
            }
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = sample_instrs(body.instructions(), dets, obs, offset, rng);
                }
            }
            DemInstruction::Detector { .. } | DemInstruction::LogicalObservable { .. } => {}
        }
    }
    offset
}

fn sample_instrs_batch(
    instrs: &[DemInstruction],
    out: &mut DemBatchOutput,
    wpr: usize,
    det_offset: usize,
    rng: &mut impl Rng,
) -> usize {
    let mut offset = det_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                let noise = random_bits_with_prob(wpr, *probability, rng);
                for t in targets {
                    match t {
                        DemTarget::Detector(i) => {
                            let row = i + offset;
                            if row < out.detections.num_major() {
                                let words = out.detections.row_words_mut(row);
                                for w in 0..wpr {
                                    words[w] ^= noise[w];
                                }
                            }
                        }
                        DemTarget::Observable(i) => {
                            if *i < out.observable_flips.num_major() {
                                let words = out.observable_flips.row_words_mut(*i);
                                for w in 0..wpr {
                                    words[w] ^= noise[w];
                                }
                            }
                        }
                        DemTarget::Separator => {}
                    }
                }
            }
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = sample_instrs_batch(body.instructions(), out, wpr, offset, rng);
                }
            }
            DemInstruction::Detector { .. } | DemInstruction::LogicalObservable { .. } => {}
        }
    }
    offset
}

fn random_bits_with_prob(words: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    if p <= 0.0 {
        return result;
    }
    if p >= 1.0 {
        result.fill(!0u64);
        return result;
    }
    for w in &mut result {
        for bit in 0..64u32 {
            if rng.r#gen::<f64>() < p {
                *w |= 1u64 << bit;
            }
        }
    }
    result
}

fn fmt_float(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{}", v);
        s
    }
}

fn write_indented(
    f: &mut std::fmt::Formatter<'_>,
    instrs: &[DemInstruction],
    indent: usize,
) -> std::fmt::Result {
    let pad = "    ".repeat(indent);
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                write!(f, "{}error({}) ", pad, fmt_float(*probability))?;
                let parts: Vec<String> = targets
                    .iter()
                    .map(|t| match t {
                        DemTarget::Detector(i) => format!("D{i}"),
                        DemTarget::Observable(i) => format!("L{i}"),
                        DemTarget::Separator => "^".to_string(),
                    })
                    .collect();
                writeln!(f, "{}", parts.join(" "))?;
            }
            DemInstruction::Detector { index, coords } => {
                let coord_strs: Vec<String> = coords.iter().map(|c| fmt_float(*c)).collect();
                writeln!(f, "{}detector({}) D{}", pad, coord_strs.join(", "), index)?;
            }
            DemInstruction::LogicalObservable { index } => {
                writeln!(f, "{}logical_observable L{}", pad, index)?;
            }
            DemInstruction::ShiftDetectors {
                detector_offset,
                coord_offsets,
            } => {
                if coord_offsets.is_empty() {
                    writeln!(f, "{}shift_detectors {}", pad, detector_offset)?;
                } else {
                    let coord_strs: Vec<String> =
                        coord_offsets.iter().map(|c| fmt_float(*c)).collect();
                    writeln!(
                        f,
                        "{}shift_detectors({}) {}",
                        pad,
                        coord_strs.join(", "),
                        detector_offset
                    )?;
                }
            }
            DemInstruction::Repeat { count, body } => {
                writeln!(f, "{}repeat {} {{", pad, count)?;
                write_indented(f, body.instructions(), indent + 1)?;
                writeln!(f, "{}}}", pad)?;
            }
        }
    }
    Ok(())
}

impl std::fmt::Display for DetectorErrorModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_indented(f, &self.instrs, 0)
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_block(
    lines: &mut Vec<&str>,
    pos: &mut usize,
    inside_repeat: bool,
) -> Result<DetectorErrorModel, String> {
    let mut dem = DetectorErrorModel::new();
    while *pos < lines.len() {
        let raw = lines[*pos];
        let trimmed = strip_comment(raw).trim();
        if trimmed.is_empty() {
            *pos += 1;
            continue;
        }
        if trimmed == "}" {
            if inside_repeat {
                *pos += 1;
                return Ok(dem);
            } else {
                return Err(format!("unexpected '}}' at line {}", *pos + 1));
            }
        }

        let instr = parse_instruction(trimmed, lines, pos)?;
        dem.push(instr);
    }
    if inside_repeat {
        return Err("unexpected end of input inside repeat block".to_string());
    }
    Ok(dem)
}

fn parse_instruction(
    line: &str,
    lines: &mut Vec<&str>,
    pos: &mut usize,
) -> Result<DemInstruction, String> {
    let lower = line.to_ascii_lowercase();

    if lower.starts_with("error") {
        let (args, rest) = extract_parens_and_rest(&lower, "error")?;
        let probability: f64 = args
            .trim()
            .parse()
            .map_err(|e| format!("bad probability: {e}"))?;
        let targets = parse_targets(rest.trim())?;
        *pos += 1;
        Ok(DemInstruction::Error {
            probability,
            targets,
        })
    } else if lower.starts_with("detector") {
        let (args, rest) = extract_parens_and_rest(&lower, "detector")?;
        let coords = parse_float_list(&args)?;
        let rest = rest.trim();
        let index = parse_d_index(rest)?;
        *pos += 1;
        Ok(DemInstruction::Detector { index, coords })
    } else if lower.starts_with("logical_observable") {
        let rest = lower["logical_observable".len()..].trim().to_string();
        let index = parse_l_index(&rest)?;
        *pos += 1;
        Ok(DemInstruction::LogicalObservable { index })
    } else if lower.starts_with("shift_detectors") {
        let after = lower["shift_detectors".len()..].to_string();
        let (coord_offsets, det_str) = if after.trim_start().starts_with('(') {
            extract_parens_and_rest_raw(&after)?
        } else {
            (String::new(), after)
        };
        let coord_offsets = if coord_offsets.is_empty() {
            vec![]
        } else {
            parse_float_list(&coord_offsets)?
        };
        let detector_offset: usize = det_str
            .trim()
            .parse()
            .map_err(|e| format!("bad detector offset: {e}"))?;
        *pos += 1;
        Ok(DemInstruction::ShiftDetectors {
            detector_offset,
            coord_offsets,
        })
    } else if lower.starts_with("repeat") {
        let after = lower["repeat".len()..].trim().to_string();
        let after = after.trim_end_matches('{').trim().to_string();
        let count: u64 = after
            .parse()
            .map_err(|e| format!("bad repeat count: {e}"))?;
        *pos += 1;
        let body = parse_block(lines, pos, true)?;
        Ok(DemInstruction::Repeat { count, body })
    } else {
        Err(format!("unknown instruction: {line}"))
    }
}

fn extract_parens_and_rest(line: &str, prefix: &str) -> Result<(String, String), String> {
    let after = &line[prefix.len()..];
    extract_parens_and_rest_raw(after)
}

fn extract_parens_and_rest_raw(after: &str) -> Result<(String, String), String> {
    let after = after.trim_start();
    if !after.starts_with('(') {
        return Err(format!("expected '(' after keyword, got: {after}"));
    }
    let close = after
        .find(')')
        .ok_or_else(|| "missing closing ')'".to_string())?;
    let args = after[1..close].to_string();
    let rest = after[close + 1..].to_string();
    Ok((args, rest))
}

fn parse_targets(s: &str) -> Result<Vec<DemTarget>, String> {
    let mut targets = Vec::new();
    for token in s.split_whitespace() {
        if token.starts_with('d') {
            let idx: usize = token[1..]
                .parse()
                .map_err(|e| format!("bad detector index: {e}"))?;
            targets.push(DemTarget::Detector(idx));
        } else if token.starts_with('l') {
            let idx: usize = token[1..]
                .parse()
                .map_err(|e| format!("bad observable index: {e}"))?;
            targets.push(DemTarget::Observable(idx));
        } else if token == "^" {
            targets.push(DemTarget::Separator);
        } else {
            return Err(format!("unknown target: {token}"));
        }
    }
    Ok(targets)
}

fn parse_float_list(s: &str) -> Result<Vec<f64>, String> {
    s.split(',')
        .map(|p| {
            p.trim()
                .parse::<f64>()
                .map_err(|e| format!("bad float: {e}"))
        })
        .collect()
}

fn parse_d_index(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if !s.starts_with('d') {
        return Err(format!("expected D#, got: {s}"));
    }
    s[1..]
        .parse()
        .map_err(|e| format!("bad detector index: {e}"))
}

fn parse_l_index(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if !s.starts_with('l') {
        return Err(format!("expected L#, got: {s}"));
    }
    s[1..]
        .parse()
        .map_err(|e| format!("bad observable index: {e}"))
}
