//! Explicit import of already weighted floating contribution rows.
use super::{TaskError, TaskRunStatistics};
use crate::field::ExactField;
use crate::statistics::{ExactStatistics, StatisticsOptions};
use num_rational::BigRational;

/// One external contribution row, already weighted by its sampling likelihood.
/// This import has no separate weight/ESS input. Its statistical uncertainty
/// follows the same independent-draw assumptions as exact task statistics.
#[derive(Clone, Debug)]
pub struct FloatingTaskDraw {
    pub acceptance: f64,
    pub error: Option<f64>,
    pub moments: Vec<f64>,
}
impl TaskRunStatistics {
    /// Summarize finite, consistently shaped external floating contributions.
    /// Each finite f64 is converted to its exact binary rational value; this
    /// cannot recover precision already lost by the external producer. Error
    /// presence and moment count must be identical in every row. Empty input
    /// returns None. No partial summary is returned on malformed input.
    ///
    /// Rows are consumed lazily with bounded accumulator dimensions and field
    /// arithmetic. Diagnostic weight is one per row, so reported ESS is only
    /// the row count, not the producer's importance-weight diagnostic.
    pub fn from_floating_draws(
        rows: impl IntoIterator<Item = FloatingTaskDraw>,
        options: StatisticsOptions,
    ) -> Result<Option<Self>, TaskError> {
        let mut rows = rows.into_iter().peekable();
        let Some(first) = rows.peek() else {
            return Ok(None);
        };
        let moments = first.moments.len();
        let has_error = first.error.is_some();
        let dimensions = moments
            .checked_add(2)
            .ok_or_else(|| invalid("shape", "dimensions overflow"))?;
        let mut statistics =
            ExactStatistics::new(dimensions, options).map_err(|error| invalid("shape", error))?;
        for (index, row) in rows.enumerate() {
            if row.error.is_some() != has_error || row.moments.len() != moments {
                return Err(invalid(
                    &format!("rows[{index}]"),
                    "error presence and moment count must match the first row",
                ));
            }
            let field = |name: &str, value: f64| -> Result<ExactField, TaskError> {
                let rational = BigRational::from_float(value)
                    .ok_or_else(|| invalid(&format!("rows[{index}].{name}"), "must be finite"))?;
                options
                    .arithmetic
                    .from_rational(rational)
                    .map_err(|error| invalid(&format!("rows[{index}].{name}"), error))
            };
            let mut values = Vec::with_capacity(dimensions);
            values.push(field("acceptance", row.acceptance)?);
            values.push(field("error", row.error.unwrap_or(0.0))?);
            for (j, value) in row.moments.into_iter().enumerate() {
                values.push(field(&format!("moments[{j}]"), value)?);
            }
            statistics
                .push(&values, &ExactField::one())
                .map_err(|error| invalid(&format!("rows[{index}]"), error))?;
        }
        Self::from_statistics(&statistics, has_error)
    }
}
fn invalid(field: &str, message: impl ToString) -> TaskError {
    TaskError {
        field: format!("floating_draws.{field}"),
        message: message.to_string(),
    }
}
