#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSampleTree {
    pub prefix_bits: Vec<bool>,
    pub suffix_children: Vec<ReferenceSampleTree>,
    pub repetitions: u64,
}

impl Default for ReferenceSampleTree {
    fn default() -> Self {
        Self {
            prefix_bits: Vec::new(),
            suffix_children: Vec::new(),
            repetitions: 0,
        }
    }
}

impl ReferenceSampleTree {
    pub fn empty(&self) -> bool {
        if self.repetitions == 0 {
            return true;
        }
        if !self.prefix_bits.is_empty() {
            return false;
        }
        self.suffix_children.iter().all(Self::empty)
    }

    pub fn size(&self) -> usize {
        let mut body_size = self.prefix_bits.len();
        for child in &self.suffix_children {
            body_size = body_size
                .checked_add(child.size())
                .expect("reference sample tree size overflow");
        }
        let repetitions = usize::try_from(self.repetitions)
            .expect("reference sample tree repetitions do not fit in usize");
        body_size
            .checked_mul(repetitions)
            .expect("reference sample tree size overflow")
    }

    pub fn decompress_into(&self, output: &mut Vec<bool>) {
        for _ in 0..self.repetitions {
            output.extend(self.prefix_bits.iter().copied());
            for child in &self.suffix_children {
                child.decompress_into(output);
            }
        }
    }

    pub fn simplified(&self) -> Self {
        let mut flat = Vec::new();
        self.flatten_and_simplify_into(&mut flat);
        match flat.len() {
            0 => Self::default(),
            1 => flat.pop().expect("single simplified tree"),
            _ => {
                let first_is_payload =
                    flat[0].repetitions == 1 && flat[0].suffix_children.is_empty();
                if first_is_payload {
                    let mut iter = flat.into_iter();
                    let mut result = iter.next().expect("first simplified tree");
                    result.suffix_children = iter.collect();
                    result
                } else {
                    Self {
                        prefix_bits: Vec::new(),
                        suffix_children: flat,
                        repetitions: 1,
                    }
                }
            }
        }
    }

    pub fn try_factorize(&mut self, period_factor: usize) {
        if period_factor == 0
            || !self.prefix_bits.is_empty()
            || self.suffix_children.len() % period_factor != 0
        {
            return;
        }

        let period_len = self.suffix_children.len() / period_factor;
        for k in period_len..self.suffix_children.len() {
            if self.suffix_children[k - period_len] != self.suffix_children[k] {
                return;
            }
        }

        self.suffix_children.truncate(period_len);
        self.repetitions = self
            .repetitions
            .checked_mul(period_factor as u64)
            .expect("reference sample tree repetitions overflow");
    }

    fn flatten_and_simplify_into(&self, out: &mut Vec<Self>) {
        if self.repetitions == 0 {
            return;
        }

        let mut flattened = Vec::new();
        if !self.prefix_bits.is_empty() {
            flattened.push(Self {
                prefix_bits: self.prefix_bits.clone(),
                suffix_children: Vec::new(),
                repetitions: 1,
            });
        }
        for child in &self.suffix_children {
            child.flatten_and_simplify_into(&mut flattened);
        }

        let mut fused: Vec<Self> = Vec::new();
        for src in flattened {
            if let Some(dst) = fused.last_mut() {
                if dst.prefix_bits == src.prefix_bits
                    && dst.suffix_children == src.suffix_children
                {
                    dst.repetitions = dst
                        .repetitions
                        .checked_add(src.repetitions)
                        .expect("reference sample tree repetitions overflow");
                    continue;
                }
                if src.repetitions == 1 && dst.repetitions == 1 && dst.suffix_children.is_empty()
                {
                    dst.prefix_bits.extend(src.prefix_bits);
                    dst.suffix_children = src.suffix_children;
                    continue;
                }
            }
            fused.push(src);
        }

        if self.repetitions == 1 {
            out.extend(fused);
        } else if fused.len() == 1 {
            let mut only = fused.pop().expect("single fused tree");
            only.repetitions = only
                .repetitions
                .checked_mul(self.repetitions)
                .expect("reference sample tree repetitions overflow");
            out.push(only);
        } else if fused.is_empty() {
        } else if fused[0].suffix_children.is_empty() && fused[0].repetitions == 1 {
            let mut iter = fused.into_iter();
            let mut result = iter.next().expect("first fused tree");
            result.repetitions = self.repetitions;
            result.suffix_children = iter.collect();
            out.push(result);
        } else {
            out.push(Self {
                prefix_bits: Vec::new(),
                suffix_children: fused,
                repetitions: self.repetitions,
            });
        }
    }
}
