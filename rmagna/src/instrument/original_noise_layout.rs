//! Deterministic original noise codes, independent of sampling probabilities.
use super::*;
use serde_json::Value;

/// One exact code/action mapping. Codes are arbitrary-width IXZY coordinates,
/// or the code one for a single-channel/report site. The layout owns its table.
#[derive(Clone)]
pub struct OriginalCompactNoiseChannel {
    pub code: BitVector,
    pub faults: Vec<OriginalFault>,
    pub report_flips: Vec<String>,
}
/// Typed selections retain arbitrary-width codes. The JSON adapter separately
/// rejects booleans, negative indices and non-integer numbers at its boundary.
#[derive(Clone)]
pub struct OriginalNoiseCodeSelection {
    pub site: usize,
    pub code: BitVector,
}
#[derive(Clone)]
pub struct OriginalCompactNoiseLayout {
    plan: OriginalAssignmentPlan,
    sites: Arc<Vec<BTreeMap<Vec<u64>, OriginalSharedFaults>>>,
}
fn key(code: &BitVector) -> &[u64] {
    let end = code
        .words()
        .iter()
        .rposition(|v| *v != 0)
        .map_or(0, |i| i + 1);
    &code.words()[..end]
}
impl OriginalCompactNoiseChannel {
    pub fn report(record: String) -> Result<Self, OriginalError> {
        Ok(Self {
            code: BitVector::from_indices(1, [0])?,
            faults: Vec::new(),
            report_flips: vec![record],
        })
    }
    /// Preserve the native IXZY little-endian two-bit convention for multi-
    /// channel sites. A single-channel site always uses code one. Identity
    /// letters contribute no physical faults, but their code is retained.
    pub fn pauli_site(
        after: &str,
        wires: &[OriginalWire],
        alternatives: &[Vec<OriginalPauli>],
    ) -> Result<Vec<Self>, OriginalError> {
        if wires.iter().copied().collect::<BTreeSet<_>>().len() != wires.len() {
            return Err(invalid("repeated original noise wire", after));
        }
        let width = wires
            .len()
            .checked_mul(2)
            .ok_or_else(|| invalid("noise code width overflow", after))?;
        if alternatives.iter().any(|a| a.len() != wires.len()) {
            return Err(invalid("noise channel width does not match wires", after));
        }
        alternatives
            .iter()
            .map(|letters| {
                let mut indices = Vec::new();
                let mut faults = Vec::new();
                for (j, (&wire, &pauli)) in wires.iter().zip(letters).enumerate() {
                    let code = match pauli {
                        OriginalPauli::I => 0,
                        OriginalPauli::X => 1,
                        OriginalPauli::Z => 2,
                        OriginalPauli::Y => 3,
                    };
                    if code & 1 != 0 {
                        indices.push(2 * j);
                    }
                    if code & 2 != 0 {
                        indices.push(2 * j + 1);
                    }
                    if pauli != OriginalPauli::I {
                        faults.push(OriginalFault {
                            after: after.into(),
                            wire,
                            pauli,
                        });
                    }
                }
                let code = if alternatives.len() == 1 {
                    BitVector::from_indices(1, [0])?
                } else {
                    BitVector::from_indices(width, indices)?
                };
                Ok(Self {
                    code,
                    faults,
                    report_flips: Vec::new(),
                })
            })
            .collect()
    }
}
impl OriginalCompactNoiseLayout {
    pub fn new(
        plan: &OriginalAssignmentPlan,
        sites: Vec<Vec<OriginalCompactNoiseChannel>>,
    ) -> Result<Self, OriginalError> {
        let limits = plan.0.instrument.limits;
        admit("max_operations", limits.max_operations, sites.len() as u128)?;
        let metadata = sites
            .iter()
            .flatten()
            .map(|c| {
                64 + 8 * c.code.words().len() as u128
                    + c.faults
                        .iter()
                        .map(|f| 48 + f.after.len() as u128)
                        .sum::<u128>()
                    + c.report_flips
                        .iter()
                        .map(|s| 32 + s.len() as u128)
                        .sum::<u128>()
            })
            .sum::<u128>();
        admit("max_metadata_bytes", limits.max_metadata_bytes, metadata)?;
        let mut owned = Vec::with_capacity(sites.len());
        for channels in sites {
            let mut codes = BTreeMap::<Vec<u64>, OriginalSharedFaults>::new();
            for channel in channels {
                let group = plan.shared_faults(channel.faults, channel.report_flips)?;
                let code = key(&channel.code).to_vec();
                if let Some(previous) = codes.get(&code) {
                    if previous.faults() != group.faults()
                        || previous.report_flips() != group.report_flips()
                    {
                        return Err(invalid("ambiguous original noise code", ""));
                    }
                } else {
                    codes.insert(code, group);
                }
            }
            owned.push(codes);
        }
        Ok(Self {
            plan: plan.clone(),
            sites: Arc::new(owned),
        })
    }
    pub fn decode(
        &self,
        selected: &[OriginalNoiseCodeSelection],
    ) -> Result<OriginalSharedFaults, OriginalError> {
        let limits = self.plan.0.instrument.limits;
        admit("max_faults", limits.max_faults, selected.len() as u128)?;
        // Resolve and admit the combined output before copying action payloads.
        let mut count = 0u128;
        let mut bytes = 0u128;
        for item in selected {
            let group = self.group(item)?;
            count += group.faults().len() as u128;
            bytes += group
                .faults()
                .iter()
                .map(|f| 48 + f.after.len() as u128)
                .sum::<u128>()
                + group
                    .report_flips()
                    .iter()
                    .map(|s| 32 + s.len() as u128)
                    .sum::<u128>();
        }
        admit("max_faults", limits.max_faults, count)?;
        admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
        let mut faults = Vec::with_capacity(count as usize);
        let mut flips = Vec::new();
        for item in selected {
            let group = self.group(item)?;
            faults.extend_from_slice(group.faults());
            flips.extend_from_slice(group.report_flips());
        }
        self.plan.shared_faults(faults, flips)
    }
    fn group(
        &self,
        item: &OriginalNoiseCodeSelection,
    ) -> Result<&OriginalSharedFaults, OriginalError> {
        self.sites
            .get(item.site)
            .and_then(|site| site.get(key(&item.code)))
            .ok_or_else(|| invalid("invalid original noise index/code", ""))
    }
    /// Strict JSON pair adapter for conventional machine-sized integer codes.
    /// The typed decode entry supports arbitrary-width codes without JSON loss.
    pub fn decode_json_pairs(
        &self,
        selected: &[(Value, Value)],
    ) -> Result<OriginalSharedFaults, OriginalError> {
        admit(
            "max_faults",
            self.plan.0.instrument.limits.max_faults,
            selected.len() as u128,
        )?;
        admit(
            "max_metadata_bytes",
            self.plan.0.instrument.limits.max_metadata_bytes,
            64 * selected.len() as u128,
        )?;
        let typed = selected
            .iter()
            .map(|(index, code)| {
                let site = index
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(|| invalid("noise index must be a nonnegative integer", ""))?;
                let code = code
                    .as_u64()
                    .ok_or_else(|| invalid("noise code must be a nonnegative integer", ""))?;
                Ok(OriginalNoiseCodeSelection {
                    site,
                    code: BitVector::from_words(64, vec![code])?,
                })
            })
            .collect::<Result<Vec<_>, OriginalError>>()?;
        self.decode(&typed)
    }
    /// Capture reported words once. Each noise row is decoded once when first
    /// requested; all words from that row share one immutable fault group.
    pub fn assignments<I>(
        &self,
        rows: I,
        reported_words: Vec<BitVector>,
    ) -> Result<OriginalNoiseAssignmentStream<I::IntoIter>, OriginalError>
    where
        I: IntoIterator<Item = Vec<(Value, Value)>>,
    {
        let n = self.plan.records().len();
        for word in &reported_words {
            if word.width() != n {
                return Err(invalid("reported word outside owned record interface", ""));
            }
        }
        admit(
            "max_metadata_bytes",
            self.plan.0.instrument.limits.max_metadata_bytes,
            reported_words
                .iter()
                .map(|w| 8 * w.words().len() as u128 + 32)
                .sum(),
        )?;
        let stopped = reported_words.is_empty();
        Ok(OriginalNoiseAssignmentStream {
            layout: self.clone(),
            rows: rows.into_iter(),
            reported_words,
            group: None,
            index: 0,
            stopped,
        })
    }
}
pub struct OriginalNoiseAssignmentStream<I> {
    layout: OriginalCompactNoiseLayout,
    rows: I,
    reported_words: Vec<BitVector>,
    group: Option<OriginalSharedFaults>,
    index: usize,
    stopped: bool,
}
impl<I: Iterator<Item = Vec<(Value, Value)>>> Iterator for OriginalNoiseAssignmentStream<I> {
    type Item = Result<Arc<OriginalCompactAssignment>, OriginalError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        loop {
            if self.group.is_some() && self.index < self.reported_words.len() {
                let group = self.group.as_ref().unwrap();
                let mut word = self.reported_words[self.index].clone();
                self.index += 1;
                for name in group.report_flips() {
                    let bit = self
                        .layout
                        .plan
                        .records()
                        .iter()
                        .position(|r| r == name)
                        .expect("validated report name");
                    let value = word.get(bit).expect("validated width");
                    word.set(bit, !value).expect("validated width");
                }
                let result = self.layout.plan.compact(word, group.clone());
                self.stopped = result.is_err();
                return Some(result);
            }
            self.group = None;
            self.index = 0;
            let Some(row) = self.rows.next() else {
                self.stopped = true;
                return None;
            };
            match self.layout.decode_json_pairs(&row) {
                Ok(group) => self.group = Some(group),
                Err(e) => {
                    self.stopped = true;
                    return Some(Err(e));
                }
            }
        }
    }
}
