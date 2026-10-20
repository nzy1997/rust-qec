use super::*;
use serde_json::{Map, Value};

pub fn parse_task(
    text: &str,
    base_directory: &Path,
    options: &TaskOptions,
) -> Result<TaskSpec, TaskError> {
    if text.len() > options.max_json_bytes {
        return Err(TaskError::new("<json>", "source exceeds max_json_bytes"));
    }
    let data: Value =
        serde_json::from_str(text).map_err(|e| TaskError::new("<json>", e.to_string()))?;
    let object = object(&data, "<root>")?;
    keys(
        object,
        &[
            "schema_version",
            "outputs",
            "error_observables",
            "ideal_state",
            "recovery",
            "sample_records",
            "observables",
        ],
        "<root>",
    )?;
    if object
        .get("schema_version")
        .is_some_and(|v| v.as_u64() != Some(1))
    {
        return Err(TaskError::new("schema_version", "expected integer 1"));
    }
    let outputs = object
        .get("outputs")
        .map(|v| indices(v, "outputs", options))
        .transpose()?;
    let error_observables = object
        .get("error_observables")
        .map(|v| indices(v, "error_observables", options))
        .transpose()?;
    let ideal = object
        .get("ideal_state")
        .filter(|v| !v.is_null())
        .map(|v| target(v, base_directory, options))
        .transpose()?;
    let recovery = object
        .get("recovery")
        .filter(|v| !v.is_null())
        .map(|v| path(v, base_directory, "recovery"))
        .transpose()?;
    if recovery.is_some() && ideal.is_none() {
        return Err(TaskError::new(
            "recovery",
            "terminal recovery requires ideal_state",
        ));
    }
    let sample_records = object
        .get("sample_records")
        .map(|v| indices(v, "sample_records", options))
        .transpose()?
        .unwrap_or_default();
    let mut observables = Vec::new();
    if let Some(value) = object.get("observables") {
        for (j, value) in array(value, "observables", options)?.iter().enumerate() {
            let field = format!("observables[{j}]");
            let pair = array(value, &field, options)?;
            if pair.len() != 2 {
                return Err(TaskError::new(field, "expected [x, z] masks"));
            }
            let x = mask(&pair[0], &field, options)?;
            let z = mask(&pair[1], &field, options)?;
            observables.push((x, z));
        }
    }
    if error_observables.is_some()
        && (ideal.is_some()
            || recovery.is_some()
            || outputs.as_ref().is_some_and(|v| !v.is_empty())
            || !observables.is_empty())
    {
        return Err(TaskError::new(
            "error_observables",
            "a classical error definition excludes ideal_state, recovery, nonempty outputs and observables",
        ));
    }
    Ok(TaskSpec {
        outputs,
        error_observables,
        ideal,
        recovery,
        sample_records,
        observables,
    })
}
fn target(value: &Value, base: &Path, options: &TaskOptions) -> Result<TargetSpec, TaskError> {
    if value.is_string() {
        return Ok(TargetSpec {
            circuit: path(value, base, "ideal_state")?,
            outputs: None,
            postselection: BTreeMap::new(),
            object_form: false,
        });
    }
    let data = object(value, "ideal_state")?;
    keys(
        data,
        &["circuit", "outputs", "postselection"],
        "ideal_state",
    )?;
    let circuit = path(
        data.get("circuit").ok_or_else(|| {
            TaskError::new("ideal_state.circuit", "missing target preparation path")
        })?,
        base,
        "ideal_state.circuit",
    )?;
    let outputs = data
        .get("outputs")
        .map(|v| indices(v, "ideal_state.outputs", options))
        .transpose()?;
    let mut postselection = BTreeMap::new();
    if let Some(value) = data.get("postselection") {
        let items = object(value, "ideal_state.postselection")?;
        if items.len() > options.max_list_items {
            return Err(TaskError::new(
                "ideal_state.postselection",
                "object exceeds max_list_items",
            ));
        }
        for (key, value) in items {
            if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) {
                return Err(TaskError::new(
                    "ideal_state.postselection",
                    "wire keys must be nonnegative integer strings",
                ));
            }
            let qubit = key.parse::<usize>().map_err(|_| {
                TaskError::new(
                    "ideal_state.postselection",
                    "wire index exceeds platform range",
                )
            })?;
            let bit = match value.as_u64() {
                Some(0) => false,
                Some(1) => true,
                _ => {
                    return Err(TaskError::new(
                        "ideal_state.postselection",
                        "projection values must be integer zero or one",
                    ));
                }
            };
            if postselection.insert(qubit, bit).is_some() {
                return Err(TaskError::new(
                    "ideal_state.postselection",
                    format!("wire {qubit} was specified more than once"),
                ));
            }
        }
    }
    Ok(TargetSpec {
        circuit,
        outputs,
        postselection,
        object_form: true,
    })
}
fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, TaskError> {
    value
        .as_object()
        .ok_or_else(|| TaskError::new(field, "expected an object"))
}
fn keys(data: &Map<String, Value>, allowed: &[&str], field: &str) -> Result<(), TaskError> {
    for key in data.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(TaskError::new(field, format!("unknown field {key:?}")));
        }
    }
    Ok(())
}
fn array<'a>(
    value: &'a Value,
    field: &str,
    options: &TaskOptions,
) -> Result<&'a [Value], TaskError> {
    let values = value
        .as_array()
        .ok_or_else(|| TaskError::new(field, "expected a list"))?;
    if values.len() > options.max_list_items {
        return Err(TaskError::new(field, "list exceeds max_list_items"));
    }
    Ok(values)
}
fn indices(value: &Value, field: &str, options: &TaskOptions) -> Result<Vec<usize>, TaskError> {
    array(value, field, options)?
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| {
                    TaskError::new(
                        field,
                        "indices must be nonnegative integers within the platform range",
                    )
                })
        })
        .collect()
}
fn path(value: &Value, base: &Path, field: &str) -> Result<PathBuf, TaskError> {
    let path = value
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| TaskError::new(field, "expected a nonempty circuit path"))?;
    Ok(base.join(path))
}
fn mask(value: &Value, field: &str, options: &TaskOptions) -> Result<BigUint, TaskError> {
    let value = value
        .as_number()
        .ok_or_else(|| TaskError::new(field, "Pauli masks must be nonnegative integers"))?
        .to_string();
    // A positive integer cannot have more decimal digits than binary bits.
    // Reject huge numeral tokens before expensive arbitrary-integer parsing.
    if value.len() as u128 > u128::from(options.max_mask_bits) + 1 {
        return Err(TaskError::new(field, "Pauli mask exceeds max_mask_bits"));
    }
    let value = value
        .parse::<num_bigint::BigInt>()
        .ok()
        .and_then(|n| n.to_biguint())
        .ok_or_else(|| TaskError::new(field, "Pauli masks must be nonnegative integers"))?;
    if value.bits() > options.max_mask_bits {
        return Err(TaskError::new(field, "Pauli mask exceeds max_mask_bits"));
    }
    Ok(value)
}
