use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct CoordState {
    pub offset: Vec<f64>,
    pub qubit_coords: HashMap<u32, Vec<f64>>,
    pub tick: i64,
}

impl CoordState {
    pub fn shift(&mut self, delta: &[f64]) {
        if self.offset.len() < delta.len() {
            self.offset.resize(delta.len(), 0.0);
        }
        for (i, d) in delta.iter().enumerate() {
            self.offset[i] += d;
        }
    }

    pub fn apply_offset(&self, coords: &[f64]) -> Vec<f64> {
        let mut out = coords.to_vec();
        if out.len() < self.offset.len() {
            out.resize(self.offset.len(), 0.0);
        }
        for i in 0..self.offset.len() {
            out[i] += self.offset[i];
        }
        out
    }
}
