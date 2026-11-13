//! Bounded sampling declarations shared by library callers and file-oriented CLI.
use super::NoiseSampling;
use crate::probability::ProbabilityParser;
use num_traits::Signed;
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt, io::Read, path::Path};

/// Resource limits, not a precision setting. JSON integer and rational-string
/// values stay exact. JSON floating numbers follow decimal-string binary64
/// semantics (`Fraction(str(float))` in the pinned Python interface).
#[derive(Clone, Copy, Debug)]
pub struct SamplingDeclarationOptions {
    pub max_json_bytes: usize,
    pub numbers: ProbabilityParser,
}
impl Default for SamplingDeclarationOptions {
    fn default() -> Self {
        Self {
            max_json_bytes: 16 << 10,
            numbers: ProbabilityParser::new(4096, 4096).expect("positive token budget"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingDeclarationError {
    pub field: String,
    pub message: String,
}
impl fmt::Display for SamplingDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}
impl std::error::Error for SamplingDeclarationError {}
fn error(field: &str, message: impl fmt::Display) -> SamplingDeclarationError {
    SamplingDeclarationError {
        field: field.into(),
        message: message.to_string(),
    }
}
fn byte_limit(options: &SamplingDeclarationOptions) -> Result<(), SamplingDeclarationError> {
    if options.max_json_bytes == 0 || options.max_json_bytes == usize::MAX {
        return Err(error(
            "sampling",
            "max_json_bytes must be positive and below usize::MAX",
        ));
    }
    Ok(())
}
/// Parse a strict object. Empty means physical; physical accepts only `method`;
/// importance additionally requires `proposal_scale`. Booleans and nonfinite,
/// zero or negative scales fail before any task preparation or callback work.
/// Repeated keys use the last value, as in the pinned JSON file interface.
///
/// ```
/// use rmagna::noise::{parse_sampling_declaration, SamplingDeclarationOptions, NoiseSampling};
/// let policy = parse_sampling_declaration(r#"{"method":"importance","proposal_scale":"500/77"}"#, &SamplingDeclarationOptions::default()).unwrap();
/// assert!(matches!(policy, NoiseSampling::Importance { .. }));
/// ```
pub fn parse_sampling_declaration(
    text: &str,
    options: &SamplingDeclarationOptions,
) -> Result<NoiseSampling, SamplingDeclarationError> {
    byte_limit(options)?;
    if text.len() > options.max_json_bytes {
        return Err(error(
            "sampling",
            format!(
                "JSON has {} bytes; configured limit is {}",
                text.len(),
                options.max_json_bytes
            ),
        ));
    }
    // Raw tokens preserve integer precision even without serde_json's optional
    // arbitrary_precision feature. Number kind remains visible before parsing.
    let object: BTreeMap<String, Box<RawValue>> = serde_json::from_str(text)
        .map_err(|e| error("sampling", format!("expected a JSON object: {e}")))?;
    let method = match object.get("method") {
        None => "physical".to_owned(),
        Some(raw) => serde_json::from_str::<String>(raw.get())
            .map_err(|_| error("sampling.method", "must be 'physical' or 'importance'"))?,
    };
    let allowed: &[&str] = match method.as_str() {
        "physical" => &["method"],
        "importance" => &["method", "proposal_scale"],
        _ => {
            return Err(error(
                "sampling.method",
                "must be 'physical' or 'importance'",
            ));
        }
    };
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(error(
            "sampling",
            format!(
                "unknown {method} field {key:?}; allowed fields: {}",
                allowed.join(", ")
            ),
        ));
    }
    if method == "physical" {
        return Ok(NoiseSampling::Physical);
    }
    let raw = object
        .get("proposal_scale")
        .ok_or_else(|| {
            error(
                "sampling.proposal_scale",
                "importance sampling requires an explicit finite positive rational",
            )
        })?
        .get();
    let value = if raw.starts_with('"') {
        let text: String = serde_json::from_str(raw).map_err(|e| error("sampling.proposal_scale",e))?;
        options.numbers.rational(&text)
    } else {
        if !raw.starts_with(|c: char| c == '-' || c.is_ascii_digit()) { return Err(error("sampling.proposal_scale","must be an integer, finite decimal number, or rational string; booleans are not numbers")); }
        // Enforce original token/exponent budgets before float conversion can
        // shrink the declaration, and before expensive rational construction.
        let exact = options.numbers.rational(raw).map_err(|e| error("sampling.proposal_scale",e))?;
        if raw.contains(['.','e','E']) {
            let float: f64 = serde_json::from_str(raw).map_err(|e| error("sampling.proposal_scale",format!("must be a finite positive rational: {e}")))?;
            if !float.is_finite() { return Err(error("sampling.proposal_scale","must be finite")); }
            options.numbers.rational(&float.to_string())
        } else { Ok(exact) }
    }.map_err(|e| error("sampling.proposal_scale",e))?;
    if !value.is_positive() {
        return Err(error("sampling.proposal_scale", "must be positive"));
    }
    Ok(NoiseSampling::Importance {
        proposal_scale: value,
    })
}
/// Read at most the configured byte limit plus one before parsing. The returned
/// immutable policy joins the usual metadata and semantic fingerprint path.
pub fn load_sampling_declaration(
    path: &Path,
    options: &SamplingDeclarationOptions,
) -> Result<NoiseSampling, SamplingDeclarationError> {
    byte_limit(options)?;
    let file = std::fs::File::open(path).map_err(|e| {
        error(
            "sampling.path",
            format!("cannot open {}: {e}", path.display()),
        )
    })?;
    let mut bytes = Vec::new();
    file.take((options.max_json_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| {
            error(
                "sampling.path",
                format!("cannot read {}: {e}", path.display()),
            )
        })?;
    if bytes.len() > options.max_json_bytes {
        return Err(error(
            "sampling",
            format!(
                "JSON exceeds configured limit of {} bytes",
                options.max_json_bytes
            ),
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| error("sampling", format!("expected UTF-8 JSON: {e}")))?;
    parse_sampling_declaration(text, options)
}
