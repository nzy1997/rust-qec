//! Exact accumulator persistence, independent of task identity and random state.
use super::*;
use num_bigint::BigInt;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

#[derive(Clone, Copy, Debug)]
pub struct CheckpointOptions {
    pub max_json_bytes: usize,
}
impl Default for CheckpointOptions {
    fn default() -> Self {
        Self {
            max_json_bytes: 16 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointError {
    pub field: String,
    pub message: String,
}
impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "statistics checkpoint {}: {}", self.field, self.message)
    }
}
impl std::error::Error for CheckpointError {}
fn error(field: &str, message: impl fmt::Display) -> CheckpointError {
    CheckpointError {
        field: field.to_owned(),
        message: message.to_string(),
    }
}
type WireField = [[String; 2]; 4];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateWire {
    schema_version: u32,
    kind: String,
    count: String,
    sums: Vec<WireField>,
    products: Vec<WireField>,
    weight: WireField,
    weight_squared: WireField,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EfficiencyWire {
    schema_version: u32,
    kind: String,
    #[serde(deserialize_with = "object")]
    statistics: StateWire,
    reference: [WireField; 3],
    zero_weights: String,
    missing_error: bool,
}
struct Budget {
    remaining: usize,
}
impl Budget {
    fn integer(&mut self, value: &BigInt) -> Result<String, CheckpointError> {
        // log10(2) > 1/4: reject unambiguously oversized integers before
        // allocating decimal text. Any remaining single conversion is bounded
        // by roughly 1.21 times the total encoded-byte cap.
        let minimum_digits = value.bits().saturating_sub(1) as u128 / 4 + 1;
        if minimum_digits > self.remaining as u128 {
            return Err(error(
                "max_json_bytes",
                "integer text exceeds the remaining byte budget",
            ));
        }
        let text = value.to_string();
        self.remaining = self.remaining.checked_sub(text.len()).ok_or_else(|| {
            error(
                "max_json_bytes",
                "integer text exceeds the remaining byte budget",
            )
        })?;
        Ok(text)
    }
    fn field(&mut self, value: &ExactField) -> Result<WireField, CheckpointError> {
        let values = value
            .coefficients()
            .iter()
            .map(|c| Ok([self.integer(c.numer())?, self.integer(c.denom())?]))
            .collect::<Result<Vec<_>, CheckpointError>>()?;
        Ok(values.try_into().expect("four cyclotomic coefficients"))
    }
}
struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("checkpoint exceeds max_json_bytes"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn encode(
    value: &impl Serialize,
    options: CheckpointOptions,
) -> Result<String, CheckpointError> {
    let mut output = Output {
        bytes: Vec::new(),
        limit: options.max_json_bytes,
    };
    serde_json::to_writer(&mut output, value).map_err(|e| error("max_json_bytes", e))?;
    Ok(String::from_utf8(output.bytes).expect("JSON serialization is UTF-8"))
}
pub(crate) fn object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct ObjectVisitor<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a JSON object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
}
pub(crate) fn decode<'a, T: Deserialize<'a>>(
    text: &'a str,
    options: CheckpointOptions,
) -> Result<T, CheckpointError> {
    if text.len() > options.max_json_bytes {
        return Err(error("max_json_bytes", "input exceeds the byte budget"));
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = object(&mut deserializer).map_err(|e| error("json", e))?;
    deserializer.end().map_err(|e| error("json", e))?;
    Ok(value)
}
fn version(schema: u32, kind: &str, expected: &str) -> Result<(), CheckpointError> {
    if schema != 1 {
        return Err(error("schema_version", "expected schema version 1"));
    }
    if kind != expected {
        return Err(error("kind", format!("expected {expected}")));
    }
    Ok(())
}
fn integer_text(text: &str, signed: bool) -> bool {
    let digits = if signed {
        text.strip_prefix('-').unwrap_or(text)
    } else {
        text
    };
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0"
}
fn count(text: &str, field: &str) -> Result<u64, CheckpointError> {
    if !integer_text(text, false) || text.len() > 20 {
        return Err(error(field, "expected a canonical u64 decimal string"));
    }
    text.parse().map_err(|e| error(field, e))
}
fn value(
    wire: &WireField,
    arithmetic: FieldArithmetic,
    field: &str,
) -> Result<ExactField, CheckpointError> {
    let mut coefficients = Vec::with_capacity(4);
    let digits = u128::from(arithmetic.max_coefficient_bits()) * 30103 / 100000 + 2;
    for [numerator, denominator] in wire {
        if !integer_text(numerator, true) || !integer_text(denominator, false) || denominator == "0"
        {
            return Err(error(
                field,
                "expected canonical decimal numerator and positive denominator strings",
            ));
        }
        if numerator.trim_start_matches('-').len() as u128 > digits
            || denominator.len() as u128 > digits
        {
            return Err(error(
                field,
                "integer text exceeds the arithmetic bit budget",
            ));
        }
        let n = numerator.parse::<BigInt>().map_err(|e| error(field, e))?;
        let d = denominator.parse::<BigInt>().map_err(|e| error(field, e))?;
        if !n.is_zero() && n.bits().max(d.bits()) > arithmetic.max_coefficient_bits() {
            return Err(error(field, "integer exceeds the arithmetic bit budget"));
        }
        let rational = BigRational::new(n.clone(), d.clone());
        if rational.numer() != &n || rational.denom() != &d {
            return Err(error(
                field,
                "rational coefficients must be reduced and zero must have denominator one",
            ));
        }
        coefficients.push(rational);
    }
    let result = arithmetic
        .value(
            coefficients
                .try_into()
                .expect("four cyclotomic coefficients"),
        )
        .map_err(|e| error(field, e))?;
    result.real_sign().map_err(|e| error(field, e))?;
    Ok(result)
}
fn state_wire(state: &ExactStatistics, budget: &mut Budget) -> Result<StateWire, CheckpointError> {
    Ok(StateWire {
        schema_version: 1,
        kind: "exact_statistics".to_owned(),
        count: state.count.to_string(),
        sums: state
            .sums
            .iter()
            .map(|v| budget.field(v))
            .collect::<Result<_, _>>()?,
        products: state
            .products
            .iter()
            .map(|v| budget.field(v))
            .collect::<Result<_, _>>()?,
        weight: budget.field(&state.weight)?,
        weight_squared: budget.field(&state.weight_squared)?,
    })
}
fn restore_state(
    wire: StateWire,
    options: StatisticsOptions,
) -> Result<ExactStatistics, CheckpointError> {
    version(wire.schema_version, &wire.kind, "exact_statistics")?;
    let dimensions = wire.sums.len();
    if dimensions > options.max_dimensions {
        return Err(error(
            "dimensions",
            StatisticsError::DimensionLimit {
                dimensions,
                limit: options.max_dimensions,
            },
        ));
    }
    let products = product_count(dimensions).map_err(|e| error("dimensions", e))?;
    if wire.products.len() != products {
        return Err(error(
            "products",
            "triangular product count does not match the sum dimensions",
        ));
    }
    let mut state =
        ExactStatistics::new(dimensions, options).map_err(|e| error("dimensions", e))?;
    state.count = count(&wire.count, "count")?;
    state.sums = wire
        .sums
        .iter()
        .map(|v| value(v, options.arithmetic, "sums"))
        .collect::<Result<_, _>>()?;
    state.products = wire
        .products
        .iter()
        .map(|v| value(v, options.arithmetic, "products"))
        .collect::<Result<_, _>>()?;
    state.weight = value(&wire.weight, options.arithmetic, "weight")?;
    state.weight_squared = value(&wire.weight_squared, options.arithmetic, "weight_squared")?;
    for (field, v) in [
        ("weight", &state.weight),
        ("weight_squared", &state.weight_squared),
    ] {
        if v.real_sign().map_err(|e| error(field, e))? == Ordering::Less {
            return Err(error(field, "weight moments must be nonnegative"));
        }
    }
    if state.weight.is_zero() != state.weight_squared.is_zero() {
        return Err(error(
            "weight_squared",
            "zero weight and zero squared-weight sums must agree",
        ));
    }
    for j in 0..state.dimensions {
        if state.products[position(j, j)]
            .real_sign()
            .map_err(|e| error("products", e))?
            == Ordering::Less
        {
            return Err(error(
                "products",
                "diagonal second moments must be nonnegative",
            ));
        }
    }
    if state.count == 0
        && state
            .sums
            .iter()
            .chain(&state.products)
            .chain([&state.weight, &state.weight_squared])
            .any(|v| !v.is_zero())
    {
        return Err(error("count", "an empty accumulator must have zero totals"));
    }
    Ok(state)
}
impl ExactStatistics {
    /// Persist raw exact totals, not normalized estimates. This contains no RNG
    /// state, task identity, controller state or authenticity attestation.
    pub fn checkpoint_json(&self, options: CheckpointOptions) -> Result<String, CheckpointError> {
        let mut budget = Budget {
            remaining: options.max_json_bytes,
        };
        encode(&state_wire(self, &mut budget)?, options)
    }
    /// Restore an owned accumulator under the caller's dimension/arithmetic
    /// budgets. Encoding/schema/basic consistency are checked; the original
    /// observations are not reconstructed from their moments.
    pub fn from_checkpoint_json(
        text: &str,
        options: StatisticsOptions,
        checkpoint: CheckpointOptions,
    ) -> Result<Self, CheckpointError> {
        restore_state(decode(text, checkpoint)?, options)
    }
}
impl HistoryEfficiency {
    pub fn checkpoint_json(&self, options: CheckpointOptions) -> Result<String, CheckpointError> {
        let mut budget = Budget {
            remaining: options.max_json_bytes,
        };
        let wire = EfficiencyWire {
            schema_version: 1,
            kind: "history_efficiency".to_owned(),
            statistics: state_wire(&self.statistics, &mut budget)?,
            reference: self
                .reference
                .iter()
                .map(|v| budget.field(v))
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .expect("three reference moments"),
            zero_weights: self.zero_weights.to_string(),
            missing_error: self.missing_error,
        };
        encode(&wire, options)
    }
    /// Restore exact efficiency totals. This does not resume a sampling task.
    pub fn from_checkpoint_json(
        text: &str,
        arithmetic: FieldArithmetic,
        checkpoint: CheckpointOptions,
    ) -> Result<Self, CheckpointError> {
        let wire: EfficiencyWire = decode(text, checkpoint)?;
        version(wire.schema_version, &wire.kind, "history_efficiency")?;
        let statistics = restore_state(
            wire.statistics,
            StatisticsOptions {
                max_dimensions: 2,
                arithmetic,
            },
        )?;
        if statistics.dimensions != 2 {
            return Err(error("statistics", "efficiency requires two quantities"));
        }
        let zero_weights = count(&wire.zero_weights, "zero_weights")?;
        if zero_weights > statistics.count || (statistics.count == 0 && wire.missing_error) {
            return Err(error(
                "zero_weights",
                "counts or missing-error status are inconsistent with an empty/completed stream",
            ));
        }
        let reference: [ExactField; 3] = wire
            .reference
            .iter()
            .map(|v| value(v, arithmetic, "reference"))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("three reference moments");
        if reference
            .iter()
            .any(|v| v.real_sign().expect("validated real value") == Ordering::Less)
        {
            return Err(error(
                "reference",
                "efficiency reference moments must be nonnegative",
            ));
        }
        if statistics.count == 0 && reference.iter().any(|v| !v.is_zero()) {
            return Err(error(
                "reference",
                "an empty accumulator must have zero reference moments",
            ));
        }
        if (zero_weights == statistics.count) != statistics.weight.is_zero() {
            return Err(error(
                "zero_weights",
                "all weights are zero exactly when the weight sum is zero",
            ));
        }
        if statistics.weight.is_zero()
            && statistics
                .sums
                .iter()
                .chain(&statistics.products)
                .chain(&reference)
                .any(|v| !v.is_zero())
        {
            return Err(error(
                "zero_weights",
                "all-zero efficiency weights require zero contributions and reference moments",
            ));
        }
        Ok(Self {
            statistics,
            reference,
            zero_weights,
            missing_error: wire.missing_error,
        })
    }
}
