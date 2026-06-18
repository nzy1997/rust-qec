use serde::Serialize;

use crate::parity_schema::{ParityCase, ParityOutcome};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParityReport {
    pub name: String,
    pub expected: Option<ParityOutcome>,
    pub actual: ParityOutcome,
    pub matches_expected: Option<bool>,
    pub tags: Vec<String>,
}

pub fn run_case(case: &ParityCase) -> ParityReport {
    let actual = match case.decode() {
        Ok(result) => ParityOutcome::from_decode_result(result),
        Err(error) => ParityOutcome::from_decode_error(error),
    };

    let matches_expected = case
        .expected
        .as_ref()
        .map(|expected| expected.matches_actual(&actual));

    ParityReport {
        name: case.name.clone(),
        expected: case.expected.clone(),
        actual,
        matches_expected,
        tags: case.tags.clone(),
    }
}
