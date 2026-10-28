//! Explicit native policy limits, shared by the selected query families.
use rmagna::phase::{NativePhaseOptions, NativeTransportMode};
use std::collections::BTreeMap;

// Keep the executable and diagnostic example on the same admission path.
struct Flags<'a>(&'a mut BTreeMap<String, String>);
impl Flags<'_> {
    fn number<T: std::str::FromStr>(&mut self, flag: &str, default: T) -> Result<T, String> {
        self.0.remove(flag).map_or(Ok(default), |value| {
            value
                .parse()
                .map_err(|_| format!("{flag} needs a nonnegative integer in range"))
        })
    }
    fn choice(&mut self, flag: &str, default: &str, choices: &[&str]) -> Result<String, String> {
        let value = self.0.remove(flag).unwrap_or_else(|| default.to_owned());
        if choices.contains(&value.as_str()) {
            Ok(value)
        } else {
            Err(format!("{flag} must be {}", choices.join(" or ")))
        }
    }
}
pub(super) const FLAGS: &[&str] = &[
    "--native-transport",
    "--native-transport-max-terms",
    "--native-transport-max-geometry-words",
    "--native-transport-max-construction-work",
    "--native-max-quotient-rank",
    "--native-max-core-table-bytes",
    "--native-max-signature-work",
    "--native-max-transform-work",
    "--native-max-construction-terms",
    "--native-max-construction-entries",
    "--native-max-construction-dimension",
    "--native-max-construction-table-values",
    "--native-max-constraint-rows",
    "--native-max-parity-rows",
    "--native-max-verification-work",
    "--native-max-geometry-words",
    "--native-max-terms",
    "--native-max-core-dimension",
    "--native-max-core-table-values",
    "--native-max-core-work",
    "--injection-max-work",
    "--injection-max-geometry-words",
    "--injection-max-terms",
];
pub(super) fn configure(
    input: &mut BTreeMap<String, String>,
    modes: &[&str],
) -> Result<NativePhaseOptions, String> {
    let mut flags = Flags(input);
    let enabled = modes
        .iter()
        .any(|m| matches!(*m, "native" | "native_injection"));
    let injection = modes.contains(&"native_injection");
    for flag in FLAGS {
        if flags.0.contains_key(*flag)
            && (!enabled || (flag.starts_with("--injection-") && !injection))
        {
            return Err(format!(
                "{flag} requires {} preparation",
                if flag.starts_with("--injection-") {
                    "native_injection"
                } else {
                    "native or native_injection"
                }
            ));
        }
    }
    let mut options = NativePhaseOptions::default();
    let transport = flags.choice("--native-transport", "off", &["off", "prepared"])?;
    if transport == "off"
        && let Some(flag) = [
            "--native-transport-max-terms",
            "--native-transport-max-geometry-words",
            "--native-transport-max-construction-work",
        ]
        .iter()
        .find(|flag| flags.0.contains_key(**flag))
    {
        return Err(format!("{flag} requires --native-transport prepared"));
    }
    options.transport.mode = if transport == "prepared" {
        NativeTransportMode::Prepared
    } else {
        NativeTransportMode::Off
    };
    let t = &mut options.transport.limits;
    t.max_terms = flags.number("--native-transport-max-terms", t.max_terms)?;
    t.max_geometry_words = flags.number(
        "--native-transport-max-geometry-words",
        t.max_geometry_words,
    )?;
    t.max_construction_work = flags.number(
        "--native-transport-max-construction-work",
        t.max_construction_work,
    )?;
    let s = &mut options.selection;
    s.max_quotient_rank = flags.number("--native-max-quotient-rank", s.max_quotient_rank)?;
    s.max_core_table_bytes =
        flags.number("--native-max-core-table-bytes", s.max_core_table_bytes)?;
    s.max_signature_work = flags.number("--native-max-signature-work", s.max_signature_work)?;
    s.max_transform_work = flags.number("--native-max-transform-work", s.max_transform_work)?;
    let c = &mut s.construction;
    c.max_terms = flags.number("--native-max-construction-terms", c.max_terms)?;
    c.max_entries = flags.number("--native-max-construction-entries", c.max_entries)?;
    c.max_dimension = flags.number("--native-max-construction-dimension", c.max_dimension)?;
    c.max_table_values =
        flags.number("--native-max-construction-table-values", c.max_table_values)?;
    c.max_constraint_rows = flags.number("--native-max-constraint-rows", c.max_constraint_rows)?;
    c.max_parity_rows = flags.number("--native-max-parity-rows", c.max_parity_rows)?;
    c.max_verification_work =
        flags.number("--native-max-verification-work", c.max_verification_work)?;
    s.preparation.max_geometry_words = flags.number(
        "--native-max-geometry-words",
        s.preparation.max_geometry_words,
    )?;
    s.preparation.max_terms = flags.number("--native-max-terms", s.preparation.max_terms)?;
    s.preparation.core.max_dimension = flags.number(
        "--native-max-core-dimension",
        s.preparation.core.max_dimension,
    )?;
    s.preparation.core.max_table_values = flags.number(
        "--native-max-core-table-values",
        s.preparation.core.max_table_values,
    )?;
    s.preparation.core.max_work =
        flags.number("--native-max-core-work", s.preparation.core.max_work)?;
    if injection {
        let mut i = rmagna::catalogue::PhaseInjectionOptions::default();
        i.max_work = flags.number("--injection-max-work", i.max_work)?;
        i.max_geometry_words =
            flags.number("--injection-max-geometry-words", i.max_geometry_words)?;
        i.max_terms = flags.number("--injection-max-terms", i.max_terms)?;
        options.injection = Some(i);
    }
    Ok(options)
}
