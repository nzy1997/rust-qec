use crate::code::StabilizerCode;
use crate::css::CssCode;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steane {
    code: StabilizerCode,
}

impl Steane {
    pub fn new() -> Result<Self> {
        let h = vec![
            vec![1, 0, 0, 1, 0, 1, 1],
            vec![0, 1, 0, 1, 1, 0, 1],
            vec![0, 0, 1, 0, 1, 1, 1],
        ];
        let css = CssCode::from_hx_hz(h.clone(), h)?;

        Ok(Self {
            code: css.code().clone(),
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}
