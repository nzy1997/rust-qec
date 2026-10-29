//! Captured physical-source descriptions for retained enumeration responses.
use super::{FileApiError, input_error};
use crate::circuit::{Basis, BitAtom, BitExpr, Circuit};
use crate::noise::{
    OriginalNoiseAction, OriginalNoiseAnchor, OriginalNoiseOptions, original_noise_categories,
};
use crate::probability::Probability;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) struct CategoryTable {
    pub value: Value,
    pub internal_to_original: BTreeMap<usize, usize>,
}

fn anchor(a: &OriginalNoiseAnchor) -> Value {
    let p = &a.location;
    json!({"operation":a.operation,"source":p.source,"line":p.line,
        "column":p.column,"repeat":p.repeat})
}
fn guard(expression: &BitExpr) -> Value {
    let terms = expression
        .terms
        .iter()
        .map(|atom| match atom {
            BitAtom::Record(record) => json!({"kind":"record","record":record}),
            BitAtom::Decision { call, bit } => {
                json!({"kind":"decision","call":call,"bit":bit})
            }
        })
        .collect::<Vec<_>>();
    json!({"constant":expression.constant,"terms":terms})
}
fn probability(p: &Probability) -> Value {
    json!({"numerator":p.value().numer().to_string(),"denominator":p.value().denom().to_string()})
}
fn action(a: &OriginalNoiseAction) -> Value {
    match a {
        OriginalNoiseAction::ReportFlip { record } => {
            json!({"kind":"report_flip","record":record})
        }
        OriginalNoiseAction::Pauli { factors } => {
            let factors = factors
                .iter()
                .map(|(qubit, basis)| {
                    let basis = match basis {
                        Basis::X => "X",
                        Basis::Y => "Y",
                        Basis::Z => "Z",
                    };
                    json!({"qubit":qubit,"basis":basis})
                })
                .collect::<Vec<_>>();
            json!({"kind":"pauli","factors":factors})
        }
    }
}

/// Called only for retained patterns, before fresh quantum preparation. The
/// physical snapshot excludes appended recovery and projected-target circuits.
pub(super) fn category_table(
    circuit: &Circuit,
    options: &OriginalNoiseOptions,
    max_probability_bits: u64,
) -> Result<CategoryTable, FileApiError> {
    let categories = original_noise_categories(circuit, options)
        .map_err(|e| input_error("enumeration.original_noise", e))?;
    // Bound exact exported integers before converting them to decimal strings.
    // This is a final-value bit bound, not an intermediate arithmetic quota.
    for p in categories.iter().flat_map(|category| {
        std::iter::once(&category.identity_probability)
            .chain(category.channels.iter().map(|c| &c.probability))
    }) {
        let required = p.value().numer().bits().max(p.value().denom().bits());
        if required > max_probability_bits {
            return Err(input_error(
                "enumeration.original_noise.max_probability_bits",
                format!(
                    "max_probability_bits requires {required}, limit is {max_probability_bits}"
                ),
            ));
        }
    }
    let mut internal_to_original = BTreeMap::new();
    let mut values = Vec::with_capacity(categories.len());
    for (original, category) in categories.into_iter().enumerate() {
        if internal_to_original
            .insert(category.internal_site, original)
            .is_some()
        {
            return Err(input_error(
                "enumeration.original_noise",
                "duplicate internal physical category address",
            ));
        }
        let channels = category
            .channels
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                json!({"channel":index,"source":anchor(&channel.source),
                    "probability":probability(&channel.probability),"action":action(&channel.action)})
            })
            .collect::<Vec<_>>();
        values.push(json!({"original_category":original,"internal_site":category.internal_site,
            "source_operations":category.source_operations.iter().map(anchor).collect::<Vec<_>>(),
            "guard":category.guard.as_ref().map(guard),
            "identity_probability":probability(&category.identity_probability),"channels":channels}));
    }
    Ok(CategoryTable {
        value: Value::Array(values),
        internal_to_original,
    })
}
