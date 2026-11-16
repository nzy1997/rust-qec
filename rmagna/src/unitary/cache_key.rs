//! Bounded exact keys for the actual physical wavefunction, not pointer hashes.
use super::*;

pub(crate) struct ScalarStateKey {
    bytes: Vec<u8>,
    limit: usize,
}
impl ScalarStateKey {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    pub(crate) fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return None;
        }
        self.bytes.extend_from_slice(bytes);
        Some(())
    }
    pub(crate) fn number(&mut self, number: usize) -> Option<()> {
        self.bytes(&(number as u128).to_le_bytes())
    }
    fn bits(&mut self, bits: &BitVector) -> Option<()> {
        self.number(bits.width())?;
        for word in bits.words() {
            self.bytes(&word.to_le_bytes())?;
        }
        Some(())
    }
    pub(crate) fn scalar(&mut self, value: &ExactScalar) -> Option<()> {
        self.bytes(&value.power().to_le_bytes())?;
        for coefficient in value.coefficients() {
            // Bound the temporary integer encoding before allocating it.
            let required = coefficient.bits().div_ceil(8).saturating_add(1);
            if u128::from(required) > self.limit.saturating_sub(self.bytes.len()) as u128 {
                return None;
            }
            let encoded = coefficient.to_signed_bytes_le();
            self.number(encoded.len())?;
            self.bytes(&encoded)?;
        }
        Some(())
    }
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
impl CompiledUnitary {
    /// Encode exactly the amplitude() inputs. A byte limit refuses key
    /// retention without changing physical evaluation or resource admission.
    pub(crate) fn write_scalar_state_key(&self, key: &mut ScalarStateKey) -> Option<()> {
        key.bytes(b"rmagna.scalar-state.v1")?;
        key.number(self.physical_width)?;
        key.bits(self.boundary.offset())?;
        key.number(self.boundary.columns().len())?;
        for column in self.boundary.columns() {
            key.bits(column)?;
        }
        let phase = self.boundary.phase();
        key.bytes(&[phase.constant()])?;
        key.number(phase.linear().len())?;
        key.bytes(phase.linear())?;
        for row in phase.upper() {
            key.bits(row)?;
        }
        key.scalar(self.boundary.coefficient())?;
        key.number(self.phase.num_variables())?;
        key.bytes(&[self.phase.constant()])?;
        key.number(self.phase.terms().len())?;
        for (axis, coefficient) in self.phase.terms() {
            key.bits(axis)?;
            key.bytes(&[*coefficient])?;
        }
        Some(())
    }
}
