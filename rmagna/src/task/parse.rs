use super::*;
use serde_json::value::RawValue;

type Object = BTreeMap<String, Box<RawValue>>;

pub fn parse_task(
    text: &str,
    base_directory: &Path,
    options: &TaskOptions,
) -> Result<TaskSpec, TaskError> {
    if text.len() > options.max_json_bytes {
        return Err(TaskError::new("<json>", "source exceeds max_json_bytes"));
    }
    // Keep every value raw: Value's arbitrary-precision number visitor can
    // reinterpret an object with serde_json's private number tag as a number.
    // Typed primitives and containers preserve the actual JSON token shapes
    // independently of additive dependency features; only masks need BigInt.
    let object: Object =
        serde_json::from_str(text).map_err(|e| TaskError::new("<json>", e.to_string()))?;
    reject_removed_task_fields(&object)?;
    keys(
        &object,
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
        .is_some_and(|v| integer(v) != Some(1))
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
        .filter(|v| !is_null(v))
        .map(|v| target(v, base_directory, options))
        .transpose()?;
    let recovery = object
        .get("recovery")
        .filter(|v| !is_null(v))
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
        for (j, value) in raw_array(value, "observables", options)?.iter().enumerate() {
            let field = format!("observables[{j}]");
            let pair = raw_array(value, &field, options)?;
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
fn reject_removed_task_fields(data: &Object) -> Result<(), TaskError> {
    for (field, hint) in [
        (
            "detectors",
            "declare each parity with DETECTOR rec[...] and write POSTSELECT det[...] in the circuit",
        ),
        (
            "postselection",
            "write POSTSELECT det[...] after a DETECTOR for each parity in the circuit",
        ),
        (
            "outcomes",
            "write POSTSELECT det[...] for zero or POSTSELECT !det[...] for one in the circuit",
        ),
        (
            "error_checks",
            "write OBSERVABLE_INCLUDE(index) rec[...] in the circuit and select task.error_observables",
        ),
        (
            "decoded_error",
            "write CONTROL[name] det[...] -> ctrl[...] and OBSERVABLE_INCLUDE(index) ctrl[...] in the circuit, then select task.error_observables",
        ),
    ] {
        if data.contains_key(field) {
            return Err(TaskError::new(
                field,
                format!("is no longer supported; {hint}"),
            ));
        }
    }
    Ok(())
}
fn target(value: &RawValue, base: &Path, options: &TaskOptions) -> Result<TargetSpec, TaskError> {
    if value.get().starts_with('"') {
        return Ok(TargetSpec {
            circuit: path(value, base, "ideal_state")?,
            outputs: None,
            postselection: BTreeMap::new(),
            object_form: false,
        });
    }
    let data = object(value, "ideal_state")?;
    keys(
        &data,
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
    let outputs = Some(indices(
        data.get("outputs").ok_or_else(|| {
            TaskError::new(
                "ideal_state.outputs",
                "object targets require an explicit outputs list",
            )
        })?,
        "ideal_state.outputs",
        options,
    )?);
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
            let bit = match integer(&value) {
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
fn object(value: &RawValue, field: &str) -> Result<Object, TaskError> {
    serde_json::from_str(value.get()).map_err(|_| TaskError::new(field, "expected an object"))
}
fn keys(data: &Object, allowed: &[&str], field: &str) -> Result<(), TaskError> {
    for key in data.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(TaskError::new(field, format!("unknown field {key:?}")));
        }
    }
    Ok(())
}
fn integer(value: &RawValue) -> Option<u64> {
    serde_json::from_str::<u64>(value.get()).ok()
}
fn is_null(value: &RawValue) -> bool {
    value.get() == "null"
}
fn indices(value: &RawValue, field: &str, options: &TaskOptions) -> Result<Vec<usize>, TaskError> {
    raw_array(value, field, options)?
        .iter()
        .map(|v| {
            integer(v)
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
fn path(value: &RawValue, base: &Path, field: &str) -> Result<PathBuf, TaskError> {
    let path = serde_json::from_str::<String>(value.get())
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| TaskError::new(field, "expected a nonempty circuit path"))?;
    Ok(base.join(path))
}
fn raw_array(
    value: &RawValue,
    field: &str,
    options: &TaskOptions,
) -> Result<Vec<Box<RawValue>>, TaskError> {
    let values: Vec<Box<RawValue>> =
        serde_json::from_str(value.get()).map_err(|_| TaskError::new(field, "expected a list"))?;
    if values.len() > options.max_list_items {
        return Err(TaskError::new(field, "list exceeds max_list_items"));
    }
    Ok(values)
}
fn mask(value: &RawValue, field: &str, options: &TaskOptions) -> Result<BigUint, TaskError> {
    let value = value.get();
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
