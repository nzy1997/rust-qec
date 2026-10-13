#[derive(Default, Debug, Clone)]
pub struct Recorder {
    bits: Vec<bool>,
}

impl Recorder {
    pub fn push(&mut self, bit: bool) {
        self.bits.push(bit);
    }

    pub(crate) fn flip_last(&mut self) {
        let bit = self.bits.last_mut().expect("measurement was recorded");
        *bit = !*bit;
    }

    pub fn len(&self) -> usize {
        self.bits.len()
    }

    pub fn rec(&self, offset: i32) -> Option<bool> {
        if offset >= 0 {
            return None;
        }
        let idx = (self.bits.len() as i32) + offset;
        if idx < 0 {
            return None;
        }
        self.bits.get(idx as usize).copied()
    }

    pub fn extend(&mut self, bits: Vec<bool>) {
        self.bits.extend(bits);
    }
}
