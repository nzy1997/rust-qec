//! Strict raw-token loading; private serde number/object tags are never values.
use super::*;
use num_bigint::BigInt;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
type Raw = Box<RawValue>;
struct Object(BTreeMap<String, Raw>);
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Object;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "an object with unique field names")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Object, M::Error> {
                let mut result = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Raw>()? {
                    if result.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("duplicate field {key}")));
                    }
                }
                Ok(Object(result))
            }
        }
        deserializer.deserialize_map(Fields)
    }
}
fn object(
    raw: &RawValue,
    path: &str,
    allowed: &[&str],
) -> Result<BTreeMap<String, Raw>, CatalogueError> {
    let Object(value) = serde_json::from_str(raw.get()).map_err(|e| error(path, e))?;
    if let Some(key) = value.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(error(format!("{path}.{key}"), "unknown field"));
    }
    Ok(value)
}
fn array(raw: &RawValue, path: &str) -> Result<Vec<Raw>, CatalogueError> {
    serde_json::from_str(raw.get()).map_err(|e| error(path, e))
}
fn required<'a>(
    object: &'a BTreeMap<String, Raw>,
    key: &str,
    path: &str,
) -> Result<&'a RawValue, CatalogueError> {
    object
        .get(key)
        .map(AsRef::as_ref)
        .ok_or_else(|| error(format!("{path}.{key}"), "missing required field"))
}
fn integer(raw: &RawValue, path: &str) -> Result<u64, CatalogueError> {
    serde_json::from_str::<u64>(raw.get())
        .map_err(|_| error(path, "expected an unsigned integer token in range"))
}
fn mask(raw: &RawValue, path: &str, dimension: usize) -> Result<BitVector, CatalogueError> {
    let value = integer(raw, path)?;
    if dimension < 64 && value >> dimension != 0 {
        return Err(error(path, "parity mask is outside entry dimension"));
    }
    BitVector::from_indices(dimension, (0..dimension).filter(|j| value >> j & 1 != 0))
        .map_err(|e| error(path, e))
}
fn pair(raw: &RawValue, path: &str) -> Result<Vec<Raw>, CatalogueError> {
    let values = array(raw, path)?;
    if values.len() != 2 {
        return Err(error(path, "expected a two-element mask/value pair"));
    }
    Ok(values)
}
fn coefficient(
    raw: &RawValue,
    path: &str,
    a: &ScalarArithmetic,
) -> Result<ExactScalar, CatalogueError> {
    let values = array(raw, path)?;
    if values.len() != 4 {
        return Err(error(path, "expected four integer cyclotomic coefficients"));
    }
    let mut coefficients = std::array::from_fn(|_| BigInt::from(0));
    for (j, value) in values.iter().enumerate() {
        let text = value.get();
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return Err(error(
                format!("{path}.{j}"),
                "expected an exact integer token",
            ));
        }
        let lower_bits = (digits.len().saturating_sub(1) as u128) * 3;
        limit(
            &format!("{path}.{j}.coefficient_bits"),
            lower_bits,
            a.max_coefficient_bits() as u128,
        )?;
        coefficients[j] = BigInt::parse_bytes(text.as_bytes(), 10)
            .ok_or_else(|| error(format!("{path}.{j}"), "invalid integer"))?;
    }
    a.scalar(coefficients, 0).map_err(|e| error(path, e))
}
fn entry(
    raw: &RawValue,
    path: &str,
    limits: &CatalogueLimits,
    a: &ScalarArithmetic,
    usage: CatalogueUsage,
) -> Result<PhaseDecompositionInput, CatalogueError> {
    let value = object(
        raw,
        path,
        &[
            "dimension",
            "core_phase_mod8",
            "terms",
            "coordinate_derivation",
        ],
    )?;
    let dimension = usize::try_from(integer(
        required(&value, "dimension", path)?,
        &format!("{path}.dimension"),
    )?)
    .map_err(|_| {
        error(
            format!("{path}.dimension"),
            "dimension exceeds platform index range",
        )
    })?;
    let count = assignments(dimension, limits)?;
    limit(
        "catalogue.max_verification_work",
        usage.verification_work + count as u128,
        limits.max_verification_work,
    )?;
    // Informational native provenance is shape checked; certification below
    // reconstructs the entry itself and does not trust a derivation claim.
    if let Some(raw) = value.get("coordinate_derivation") {
        let field = format!("{path}.coordinate_derivation");
        let metadata = object(raw, &field, &["source_core", "rows", "convention"])?;
        let core = array(
            required(&metadata, "source_core", &field)?,
            &format!("{field}.source_core"),
        )?;
        if core.len() != count {
            return Err(error(
                format!("{field}.source_core"),
                format!("expected {count} phases"),
            ));
        }
        for (j, raw) in core.iter().enumerate() {
            if integer(raw, &format!("{field}.source_core.{j}"))? > 7 {
                return Err(error(
                    format!("{field}.source_core.{j}"),
                    "phase must lie between zero and seven",
                ));
            }
        }
        let rows = array(
            required(&metadata, "rows", &field)?,
            &format!("{field}.rows"),
        )?;
        if rows.len() != dimension {
            return Err(error(
                format!("{field}.rows"),
                "expected one source parity per coordinate",
            ));
        }
        for (j, raw) in rows.iter().enumerate() {
            mask(raw, &format!("{field}.rows.{j}"), dimension)?;
        }
        serde_json::from_str::<String>(required(&metadata, "convention", &field)?.get())
            .map_err(|e| error(format!("{field}.convention"), e))?;
    }

    limit(
        "catalogue.max_table_values",
        usage.table_values as u128 + count as u128,
        limits.max_table_values as u128,
    )?;
    let table = array(
        required(&value, "core_phase_mod8", path)?,
        &format!("{path}.core_phase_mod8"),
    )?;
    if table.len() != count {
        return Err(error(
            format!("{path}.core_phase_mod8"),
            format!("expected {count} phase values, found {}", table.len()),
        ));
    }
    let phase_table = table
        .iter()
        .enumerate()
        .map(|(j, v)| {
            let field = format!("{path}.core_phase_mod8.{j}");
            let value = integer(v, &field)?;
            if value > 7 {
                return Err(error(field, "phase must lie between zero and seven"));
            }
            Ok(value as u8)
        })
        .collect::<Result<Vec<_>, CatalogueError>>()?;
    let terms = array(required(&value, "terms", path)?, &format!("{path}.terms"))?;
    limit(
        "catalogue.max_terms",
        usage.terms as u128 + terms.len() as u128,
        limits.max_terms as u128,
    )?;
    // Before decoding, either proof may be eligible. Admit a lower bound for
    // both; insertion checks the actual normalized rows and exact chosen cost.
    let minimum = verification::plan(dimension, count, terms.len() as u128, 0, 0, true)?;
    let minimum_work = usage
        .verification_work
        .checked_add(minimum.work)
        .ok_or_else(|| {
            error(
                "catalogue.max_verification_work",
                "verification work exceeds u128",
            )
        })?;
    limit(
        "catalogue.max_verification_work",
        minimum_work,
        limits.max_verification_work,
    )?;
    let mut decoded = Vec::with_capacity(terms.len());
    let mut constraints_used = usage.constraint_rows as u128;
    let mut parities_used = usage.parity_rows as u128;
    for (j, raw) in terms.iter().enumerate() {
        let path = format!("{path}.terms.{j}");
        let value = object(
            raw,
            &path,
            &["coefficient", "constraints", "clifford_parities"],
        )?;
        let constraints = array(
            required(&value, "constraints", &path)?,
            &format!("{path}.constraints"),
        )?;
        let parities = array(
            required(&value, "clifford_parities", &path)?,
            &format!("{path}.clifford_parities"),
        )?;
        constraints_used += constraints.len() as u128;
        parities_used += parities.len() as u128;
        limit(
            "catalogue.max_constraint_rows",
            constraints_used,
            limits.max_constraint_rows as u128,
        )?;
        limit(
            "catalogue.max_parity_rows",
            parities_used,
            limits.max_parity_rows as u128,
        )?;
        let constraints = constraints
            .iter()
            .enumerate()
            .map(|(j, v)| {
                let path = format!("{path}.constraints.{j}");
                let pair = pair(v, &path)?;
                let mask = mask(&pair[0], &format!("{path}.0"), dimension)?;
                let bit = integer(&pair[1], &format!("{path}.1"))?;
                if bit > 1 {
                    return Err(error(
                        format!("{path}.1"),
                        "constraint outcome must be zero or one",
                    ));
                }
                Ok((mask, bit == 1))
            })
            .collect::<Result<Vec<_>, CatalogueError>>()?;
        let phases = parities
            .iter()
            .enumerate()
            .map(|(j, v)| {
                let path = format!("{path}.clifford_parities.{j}");
                let pair = pair(v, &path)?;
                let mask = mask(&pair[0], &format!("{path}.0"), dimension)?;
                let turns = serde_json::from_str::<i64>(pair[1].get()).map_err(|_| {
                    error(
                        format!("{path}.1"),
                        "expected a signed integer token in range",
                    )
                })?;
                Ok((mask, 2 * turns.rem_euclid(4) as u8))
            })
            .collect::<Result<Vec<_>, CatalogueError>>()?;
        decoded.push(CliffordIndicator {
            coefficient: coefficient(
                required(&value, "coefficient", &path)?,
                &format!("{path}.coefficient"),
                a,
            )?,
            constraints,
            phase: ParityPhase::new(dimension, 0, phases).map_err(|e| error(&path, e))?,
        });
    }
    Ok(PhaseDecompositionInput {
        dimension,
        phase_table,
        terms: decoded,
    })
}
impl PhaseCatalogue {
    pub fn from_json(
        source: &str,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        let mut catalogue = Self::new();
        catalogue.extend_json(source, limits, arithmetic)?;
        Ok(catalogue)
    }
    /// Parse and certify the complete document on a shared clone. Any parse,
    /// budget, arithmetic, duplicate or reconstruction error leaves self intact.
    pub fn extend_json(
        &mut self,
        source: &str,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(), CatalogueError> {
        self.check_existing(limits)?;
        limit(
            "catalogue.max_json_bytes",
            source.len() as u128,
            limits.max_json_bytes as u128,
        )?;
        let raw: Raw = serde_json::from_str(source).map_err(|e| error("catalogue", e))?;
        let value = object(
            &raw,
            "catalogue",
            &[
                "schema_version",
                "convention",
                "scope",
                "provenance",
                "entries",
            ],
        )?;
        if integer(
            required(&value, "schema_version", "catalogue")?,
            "catalogue.schema_version",
        )? != 1
        {
            return Err(error(
                "catalogue.schema_version",
                "only schema version one is supported",
            ));
        }
        for key in ["convention", "scope", "provenance"] {
            if let Some(raw) = value.get(key) {
                serde_json::from_str::<String>(raw.get())
                    .map_err(|e| error(format!("catalogue.{key}"), e))?;
            }
        }
        let entries = array(
            required(&value, "entries", "catalogue")?,
            "catalogue.entries",
        )?;
        limit(
            "catalogue.max_entries",
            self.usage.entries as u128 + entries.len() as u128,
            limits.max_entries as u128,
        )?;
        let mut staged = self.clone();
        for (j, raw) in entries.iter().enumerate() {
            let input = entry(
                raw,
                &format!("catalogue.entries.{j}"),
                limits,
                arithmetic,
                staged.usage,
            )?;
            staged.insert_at(input, limits, arithmetic, j)?;
        }
        *self = staged;
        Ok(())
    }
}
