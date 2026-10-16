use std::collections::BTreeMap;

use toml::Value;

pub(crate) fn optional_bool(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a boolean")),
    }
}

#[allow(dead_code)]
pub(crate) fn optional_f64(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<f64>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => {
            if let Some(value) = value.as_float() {
                Ok(Some(value))
            } else if let Some(value) = value.as_integer() {
                Ok(Some(value as f64))
            } else {
                Err(format!("{key} must be numeric"))
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn optional_string(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(str::to_string)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string")),
    }
}

pub(crate) fn optional_usize(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<usize>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => {
            let integer = value
                .as_integer()
                .ok_or_else(|| format!("{key} must be an integer"))?;
            usize::try_from(integer)
                .map(Some)
                .map_err(|_| format!("{key} must be non-negative"))
        }
    }
}

#[allow(dead_code)]
pub(crate) fn optional_positive_u32(
    params: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<u32>, String> {
    match optional_usize(params, key)? {
        None => Ok(None),
        Some(0) => Err(format!("{key} must be positive")),
        Some(value) => u32::try_from(value)
            .map(Some)
            .map_err(|_| format!("{key} exceeds supported u32 range")),
    }
}
