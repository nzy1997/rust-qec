//! Exact sector reports with captured identities and call-local failure costs.
use super::enumeration_categories::category_table;
use super::*;
use crate::field::ExactField;
use crate::instrument::FaultSelection;
use crate::noise::OriginalNoiseOptions;
use crate::probability::Probability;
use crate::task::{
    ExactInterval, NumericEncoder, TaskEnumeration, TaskEnumerationOptions, TaskSectorMasses,
    fingerprint_with_configuration,
};

#[derive(Clone, Copy, Debug)]
pub struct FileEnumerationOptions {
    pub enumeration: TaskEnumerationOptions,
    pub execution_limits: ExecutionLimits,
    /// Physical-source description budgets, used only with retained patterns.
    pub original_noise: OriginalNoiseOptions,
    /// Maximum numerator/denominator bits in each exported category/channel
    /// probability. This does not bound intermediate exact arithmetic.
    pub max_original_probability_bits: u64,
}
impl Default for FileEnumerationOptions {
    fn default() -> Self {
        Self {
            enumeration: TaskEnumerationOptions::default(),
            execution_limits: ExecutionLimits::default(),
            original_noise: OriginalNoiseOptions::default(),
            max_original_probability_bits: 1 << 20,
        }
    }
}
impl FileEnumerationOptions {
    fn to_json(self) -> Value {
        let n = self.enumeration.noise;
        let h = self.enumeration.histories;
        json!({"noise":{"max_weight":n.max_weight,"max_patterns":n.max_patterns,
            "max_fault_entries":n.max_fault_entries,"max_sites":n.max_sites,
            "max_site_weight_pairs":n.max_site_weight_pairs,"max_probability_bits":n.max_probability_bits},
            "histories":{"max_branches":h.max_branches,"max_record_bits":h.max_record_bits,
                "max_marginal_queries":h.max_marginal_queries,"max_replay_attempts":h.max_replay_attempts},
            "include_patterns":self.enumeration.include_patterns,
            "max_retained_response_values":self.enumeration.max_retained_response_values,
            "original_noise":{"max_categories":self.original_noise.max_categories,
                "max_channels":self.original_noise.max_channels,
                "max_probability_bits":self.max_original_probability_bits},
            "execution_limits":self.execution_limits.to_json()})
    }
}
struct EnumerationDescriptor {
    metadata: Value,
    original_sites: Option<BTreeMap<usize, usize>>,
}
impl FileTask {
    fn enumeration_descriptor(
        &self,
        options: &FileEnumerationOptions,
    ) -> Result<EnumerationDescriptor, FileApiError> {
        let i = &self.inputs;
        if !matches!(i.sampling, NoiseSampling::Physical) {
            return Err(input_error(
                "enumeration.sampling",
                "exact physical enumeration requires physical sampling configuration",
            ));
        }
        if !i.modules.task().sample_records().is_empty() {
            return Err(input_error(
                "enumeration.sample_records",
                "exact task enumeration requires an empty sample_records list",
            ));
        }
        // Share the captured circuit/module/controller description, replacing
        // the sampling configuration before it becomes observable to callers.
        let run = TaskRunOptions::default();
        let mut configured = resolved_options(&self.fingerprint_input(&run));
        let object = configured.as_object_mut().expect("options object");
        object.remove("run");
        object.remove("history_sampler");
        object.insert("enumeration".to_owned(), options.to_json());
        let sampling = json!({"method":"exact_physical_sectors","histories":"all_positive_causal_histories","entropy":"none"});
        let fingerprints = fingerprint_with_configuration(
            &self.fingerprint_input(&run),
            &configured,
            &sampling,
            i.options.fingerprints,
        )?;
        let mut value = self.descriptor_value(configured, fingerprints);
        value["kind"] = json!("enumeration");
        value["method"] = json!("exact_physical_noise_sectors");
        value
            .as_object_mut()
            .expect("descriptor object")
            .remove("history_sampler");
        value["sampling"] = sampling;
        let original_sites = if options.enumeration.include_patterns {
            let table = category_table(
                &i.physical,
                &options.original_noise,
                options.max_original_probability_bits,
            )?;
            value["original_noise_categories"] = table.value;
            Some(table.internal_to_original)
        } else {
            value["original_noise_categories"] = Value::Null;
            None
        };
        Ok(EnumerationDescriptor {
            metadata: value,
            original_sites,
        })
    }
    /// Fresh exact integration shares preparation and traversal quotas. Invalid
    /// sampling/record requests fail before quantum preparation or callbacks.
    pub fn enumerate(
        &self,
        options: &FileEnumerationOptions,
    ) -> Result<FileEnumerationReport, FileApiError> {
        let EnumerationDescriptor {
            metadata,
            original_sites,
        } = self.enumeration_descriptor(options)?;
        let started = Instant::now();
        let meter = ExecutionMeter::new(options.execution_limits);
        let i = &self.inputs;
        let evaluator = TaskEvaluator::new_with_costs(
            &i.physical,
            &i.modules,
            &i.registry,
            i.options.evaluation,
            &i.sampling,
            &meter,
        );
        let preparation = Preparation {
            seconds: started.elapsed().as_secs_f64(),
            costs: meter.snapshot(),
            limits: options.execution_limits,
        };
        let report = match evaluator {
            Ok(evaluator) => CompiledFileTask {
                task: self.clone(),
                evaluator,
                preparation,
            }
            .enumerate_report(metadata, original_sites, options, &meter, started, true),
            Err(error) => {
                let mut metadata = metadata;
                metadata["route"] = json!("unavailable");
                metadata["compilation"] = json!({"status":"preparation_failed","preparation_included_in_this_call":true,"preparation_seconds":preparation.seconds,"preparation_costs":costs_json(preparation.costs),"preparation_limits":preparation.limits.to_json()});
                FileEnumerationReport {
                    result: Err(error),
                    metadata,
                    original_sites,
                    numeric: i.options.numeric,
                    execution_costs: meter.snapshot(),
                    seconds: started.elapsed().as_secs_f64(),
                    failure_phase: Some("preparation"),
                    metadata_failure: None,
                }
            }
        };
        Ok(report)
    }
}
impl CompiledFileTask {
    /// Reuse captured plans with fresh per-call quotas and operation counters.
    pub fn enumerate(
        &self,
        options: &FileEnumerationOptions,
    ) -> Result<FileEnumerationReport, FileApiError> {
        let EnumerationDescriptor {
            metadata,
            original_sites,
        } = self.task.enumeration_descriptor(options)?;
        let started = Instant::now();
        let meter = ExecutionMeter::new(options.execution_limits);
        Ok(self.enumerate_report(metadata, original_sites, options, &meter, started, false))
    }
    fn enumerate_report(
        &self,
        mut metadata: Value,
        original_sites: Option<BTreeMap<usize, usize>>,
        options: &FileEnumerationOptions,
        meter: &ExecutionMeter,
        started: Instant,
        included: bool,
    ) -> FileEnumerationReport {
        let result = self
            .evaluator
            .enumerate_physical_with_costs(&options.enumeration, meter);
        let metadata_failure =
            match compilation_snapshot(&self.evaluator, self.preparation, included) {
                Ok(snapshot) => {
                    add_snapshot(&mut metadata, snapshot);
                    None
                }
                Err(error) => Some(error),
            };
        let failure_phase = if result.is_err() {
            Some("enumeration")
        } else if metadata_failure.is_some() {
            Some("metadata")
        } else {
            None
        };
        FileEnumerationReport {
            result,
            metadata,
            original_sites,
            numeric: self.task.inputs.options.numeric,
            execution_costs: meter.snapshot(),
            seconds: started.elapsed().as_secs_f64(),
            failure_phase,
            metadata_failure,
        }
    }
}
/// An owned exact result (or execution error) and the work performed to obtain
/// it. Export errors leave raw_result accessible. Failed traversals have no
/// partial sector estimate; metadata failures retain an already completed sum.
#[derive(Debug)]
pub struct FileEnumerationReport {
    result: Result<TaskEnumeration, TaskError>,
    metadata: Value,
    original_sites: Option<BTreeMap<usize, usize>>,
    numeric: NumericOptions,
    execution_costs: ExecutionCosts,
    seconds: f64,
    failure_phase: Option<&'static str>,
    metadata_failure: Option<TaskError>,
}
impl FileEnumerationReport {
    fn choice_json(&self, choice: &FaultSelection) -> Result<Value, BatchExportError> {
        let inconsistent = || {
            BatchExportError::InvalidState(format!(
                "pattern choice at internal category {}, channel {} has no original physical category mapping",
                choice.site, choice.alternative,
            ))
        };
        let original = self
            .original_sites
            .as_ref()
            .and_then(|sites| sites.get(&choice.site))
            .copied()
            .ok_or_else(inconsistent)?;
        let category = &self.metadata["original_noise_categories"][original];
        if category["internal_site"].as_u64() != Some(choice.site as u64)
            || category["original_category"].as_u64() != Some(original as u64)
            || category["channels"]
                .as_array()
                .and_then(|c| c.get(choice.alternative))
                .is_none()
        {
            return Err(inconsistent());
        }
        Ok(
            json!({"category":choice.site,"channel":choice.alternative,"original_category":original}),
        )
    }
    pub fn metadata(&self) -> &Value {
        &self.metadata
    }
    /// Borrow the completed sum, or return an owned copy of its execution error.
    pub fn raw_result(&self) -> Result<&TaskEnumeration, TaskError> {
        self.result.as_ref().map_err(Clone::clone)
    }
    pub fn into_raw_result(self) -> Result<TaskEnumeration, TaskError> {
        self.result
    }
    pub fn execution_costs(&self) -> ExecutionCosts {
        self.execution_costs
    }
    pub fn total_seconds(&self) -> f64 {
        self.seconds
    }
    /// Execution and metadata succeeded, independently of numerical export.
    pub fn is_complete(&self) -> bool {
        self.failure_phase.is_none()
    }
    pub fn metadata_failure(&self) -> Option<&TaskError> {
        self.metadata_failure.as_ref()
    }
    pub fn to_json(&self) -> Result<Value, BatchExportError> {
        let mut value = self.metadata.clone();
        value["status"] = json!(if self.is_complete() {
            "complete"
        } else {
            "failed"
        });
        value["failure_phase"] = json!(self.failure_phase);
        let error_json = |e: &TaskError| json!({"field":e.field,"message":e.message});
        value["failure"] = self.result.as_ref().err().map_or(Value::Null, error_json);
        value["metadata_failure"] = self
            .metadata_failure
            .as_ref()
            .map_or(Value::Null, error_json);
        value["execution_costs"] = costs_json(self.execution_costs);
        value["execution_costs"]["scope"] = json!("this_call_including_failed_attempts");
        value["timings"] = json!({"total_seconds":self.seconds,"components_are_additive":false});
        value["result"] = Value::Null;
        let mut encoder = NumericEncoder::new(self.numeric);
        if let Ok(result) = &self.result {
            let error_defined = !self.metadata["evaluation"]["error_observables"].is_null()
                || !self.metadata["evaluation"]["ideal_state"].is_null();
            let mut sectors = Vec::with_capacity(result.by_weight.len());
            for (j, sector) in result.by_weight.iter().enumerate() {
                sectors.push(json!({"weight":sector.weight,"probability":probability(&sector.probability),
                    "patterns":sector.patterns,"histories":sector.histories,
                    "masses":masses(&sector.masses, error_defined, &format!("result.by_weight.{j}.masses"), &mut encoder)?}));
            }
            let patterns = result.pattern_responses.as_ref().map(|patterns| patterns.iter().enumerate().map(|(j, p)| {
                Ok(json!({"weight":p.choices.len(),"choices":p.choices.iter().map(|c|self.choice_json(c)).collect::<Result<Vec<_>,_>>()?,
                    "probability":probability(&p.probability),"histories":p.histories,
                    "response":masses(&p.response,error_defined,&format!("result.pattern_responses.{j}.response"),&mut encoder)?}))
            }).collect::<Result<Vec<_>,BatchExportError>>()).transpose()?;
            value["result"] = json!({
                "semantics":"unconditional covered masses; no covered-probability or acceptance renormalization",
                "error_defined":error_defined,"covered_probability":probability(&result.covered_probability),
                "omitted_probability":probability(&result.omitted_probability),
                "masses":masses(&result.masses,error_defined,"result.masses",&mut encoder)?,
                "target_normalization":result.target_normalization.as_ref().map(|norm|json!({
                    "coefficients":norm.coefficients().iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "power":norm.power(),"scale":"2**power",
                    "basis":"1,omega,omega^2,omega^3; omega=exp(i*pi/4)"})),
                "by_weight":sectors,"patterns":result.patterns,"histories":result.histories,
                "pattern_responses":patterns,"pattern_response_semantics":"quantum history integrated; excludes physical pattern probability",
                "marginal_queries":result.marginal_queries,"replay_attempts":result.replay_attempts,
                "network_compilations":result.network_compilations,
                "full_acceptance":interval(Some(&result.full_acceptance),"result.full_acceptance",&mut encoder)?,
                "full_error":interval(error_defined.then_some(&result.full_error),"result.full_error",&mut encoder)?,
                "conditional_error":interval(result.conditional_error.as_ref().filter(|_|error_defined),"result.conditional_error",&mut encoder)?,
            });
        }
        value["numerics"] = encoder.numerics();
        Ok(value)
    }
}
fn probability(p: &Probability) -> Value {
    json!({"numerator":p.value().numer().to_string(),"denominator":p.value().denom().to_string()})
}
fn number(
    value: Option<&ExactField>,
    path: &str,
    encoder: &mut NumericEncoder,
) -> Result<Value, BatchExportError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    let (ordinary, scientific) = encoder.number(path, Some(value), false)?;
    let coefficients = value
        .coefficients()
        .iter()
        .map(|v| json!({"numerator":v.numer().to_string(),"denominator":v.denom().to_string()}))
        .collect::<Vec<_>>();
    Ok(
        json!({"value":ordinary,"scientific":scientific,"exact":{"basis":"1,omega,omega^2,omega^3; omega=exp(i*pi/4)","coefficients":coefficients}}),
    )
}
fn masses(
    m: &TaskSectorMasses,
    error_defined: bool,
    path: &str,
    encoder: &mut NumericEncoder,
) -> Result<Value, BatchExportError> {
    let moments = m
        .moments
        .iter()
        .enumerate()
        .map(|(j, v)| number(Some(v), &format!("{path}.moments.{j}"), encoder))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"accepted":number(Some(&m.accepted),&format!("{path}.accepted"),encoder)?,
        "error":number(error_defined.then_some(&m.error),&format!("{path}.error"),encoder)?,
        "target_success":number(m.target_success.as_ref(),&format!("{path}.target_success"),encoder)?,"moments":moments}),
    )
}
fn interval(
    i: Option<&ExactInterval>,
    path: &str,
    encoder: &mut NumericEncoder,
) -> Result<Value, BatchExportError> {
    match i {
        None => Ok(Value::Null),
        Some(i) => Ok(
            json!({"lower":number(Some(&i.lower),&format!("{path}.lower"),encoder)?,"upper":number(Some(&i.upper),&format!("{path}.upper"),encoder)?}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_inconsistent_physical_addresses_cannot_be_exported() {
        let parse = ParseOptions::default();
        let circuit = parse_circuit("M 0\nX_ERROR(1/3) 0", &parse).unwrap();
        let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
        let file = FileTask::from_loaded(
            circuit,
            modules,
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        let mut options = FileEnumerationOptions::default();
        options.enumeration.include_patterns = true;
        let mut report = file.enumerate(&options).unwrap();
        assert!(report.to_json().is_ok());
        // A lost join must be rejected, never guessed from a site number.
        report.original_sites.as_mut().unwrap().remove(&1);
        assert!(matches!(
            report.to_json(),
            Err(BatchExportError::InvalidState(_))
        ));
        report.original_sites.as_mut().unwrap().insert(1, 1);
        assert!(matches!(
            report.to_json(),
            Err(BatchExportError::InvalidState(_))
        ));
        report.original_sites.as_mut().unwrap().insert(1, 0);
        report.metadata["original_noise_categories"][0]["internal_site"] = json!(0);
        assert!(matches!(
            report.to_json(),
            Err(BatchExportError::InvalidState(_))
        ));
        report.metadata["original_noise_categories"][0]["internal_site"] = json!(1);
        report.metadata["original_noise_categories"][0]["channels"] = json!([]);
        assert!(matches!(
            report.to_json(),
            Err(BatchExportError::InvalidState(_))
        ));
        assert!(report.raw_result().is_ok());
    }
}
