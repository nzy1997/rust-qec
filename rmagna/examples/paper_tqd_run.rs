//! Paired capture of exact TQD terminal network contributions and fault addresses.
#[path = "support/decoder_worker.rs"]
mod decoder_worker;

use num_rational::BigRational;
use num_traits::{One, ToPrimitive};
use rmagna::catalogue::PhaseSignatureScan;
use rmagna::circuit::{Basis, Circuit, ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::instrument::{FaultSelection, QueryPreparation};
use rmagna::noise::{
    OriginalNoiseAction, OriginalNoiseOptions, RandomSource, SplitMix64, original_noise_categories,
};
use rmagna::phase::NativePhaseOptions;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::scalar::ScalarArithmetic;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::PathBuf, time::Instant};

const PROTOCOL: &str = "splitmix64-addressed-u40-seed-u23-index-role-v1";
const WARMUP: u32 = (1 << 23) - 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    size: usize,
    p: String,
    seed: u64,
    start: u32,
    shots: u32,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
struct Address {
    line: usize,
    wire: Option<usize>,
    record: Option<usize>,
}

#[derive(Deserialize)]
struct EdgeSite {
    kind: String,
    color: String,
    edge: usize,
    #[serde(flatten)]
    address: Address,
}

#[derive(Deserialize)]
struct Family {
    size: usize,
    depth: usize,
    physical_sha256: String,
    entries: Vec<EdgeSite>,
}

#[derive(Deserialize)]
struct FaultMap {
    schema: String,
    families: BTreeMap<String, Family>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fault_sites(circuit: &Circuit, family: &Family) -> Result<Vec<usize>, Box<dyn Error>> {
    let raw = original_noise_categories(circuit, &OriginalNoiseOptions::default())?;
    let mut observed = BTreeMap::new();
    let expected_rate = BigRational::new(1.into(), 100.into());
    for category in &raw {
        if category.identity_probability.value().is_one() {
            continue;
        }
        let [channel] = category.channels.as_slice() else {
            return Err("TQD requires binary fault categories".into());
        };
        if category.guard.is_some() || channel.probability.value() != &expected_rate {
            return Err("TQD original fault law changed".into());
        }
        let line = channel.source.location.line;
        let (wire, record, basis) = match &channel.action {
            OriginalNoiseAction::ReportFlip { record } => (None, Some(*record), None),
            OriginalNoiseAction::Pauli { factors } => match factors.as_slice() {
                [(wire, basis @ (Basis::X | Basis::Z))] => (Some(*wire), None, Some(*basis)),
                _ => return Err("TQD requires one-wire X or Z faults".into()),
            },
        };
        let address = Address { line, wire, record };
        if observed
            .insert(address, (category.internal_site, basis))
            .is_some()
        {
            return Err("duplicate TQD original noise address".into());
        }
    }
    if observed.len() != family.entries.len() {
        return Err("TQD physical and author fault counts differ".into());
    }
    let mut sites = Vec::with_capacity(observed.len());
    let mut lattice = std::collections::BTreeSet::new();
    for entry in &family.entries {
        let (site, basis) = observed
            .remove(&entry.address)
            .ok_or("fault map does not address an original fault")?;
        let kind = match entry.kind.as_str() {
            "X" => Basis::X,
            "Z" => Basis::Z,
            _ => return Err("invalid TQD fault kind".into()),
        };
        let vertices = family.size * family.size;
        if basis.is_some_and(|actual| actual != kind)
            || !matches!(entry.color.as_str(), "b" | "g" | "r")
            || entry.edge >= 3 * vertices * family.depth
            || (entry.edge % 3 == 2) != entry.address.record.is_some()
            || (entry.edge % 3 == 2 && entry.edge >= 3 * vertices * (family.depth - 1))
            || !lattice.insert((entry.kind.as_str(), entry.color.as_str(), entry.edge))
        {
            return Err("invalid TQD author edge mapping".into());
        }
        sites.push(site);
    }
    if !observed.is_empty() {
        return Err("TQD author map omitted original faults".into());
    }
    Ok(sites)
}

fn stream(seed: u64, index: u32, history: bool) -> SplitMix64 {
    let mut address = SplitMix64::new((seed << 24) | (u64::from(index) << 1) | u64::from(history));
    SplitMix64::new(address.next_u64())
}

fn run(mut args: impl Iterator<Item = String>) -> Result<Value, Box<dyn Error>> {
    let started = Instant::now();
    let physical_path = args
        .next()
        .ok_or("usage: paper_tqd_run PHYSICAL TASK CONFIG CONTROLLERS PYTHON FAULT_MAP")?;
    let task_path = PathBuf::from(args.next().ok_or("missing task path")?);
    let config_path = args.next().ok_or("missing TQD configuration")?;
    let controller_path = PathBuf::from(args.next().ok_or("missing TQD controller manifest")?);
    let python = args.next().ok_or("missing TQD decoder Python")?;
    let map_path = args.next().ok_or("missing TQD fault map")?;
    let config: Configuration = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    let end = config
        .start
        .checked_add(config.shots)
        .ok_or("TQD draw address overflow")?;
    let depth = match config.size {
        3 => 5,
        4 => 6,
        5 => 8,
        _ => return Err("unsupported TQD scale".into()),
    };
    if config.schema != "rmagna.paper-tqd-configuration.v1"
        || config.seed >= 1 << 40
        || config.shots == 0
        || end > 1536
        || args.next().is_some()
    {
        return Err("invalid bounded TQD configuration".into());
    }
    let rate: BigRational = config.p.parse()?;
    if ![(3, 1000), (1, 200), (1, 100), (3, 200), (1, 50)]
        .into_iter()
        .any(|(n, d)| rate == BigRational::new(n.into(), d.into()))
    {
        return Err("unsupported TQD physical rate".into());
    }
    let numerator = rate.numer().to_u64().ok_or("invalid TQD rate numerator")?;
    let denominator = rate
        .denom()
        .to_u64()
        .ok_or("invalid TQD rate denominator")?;
    let cutoff = u64::MAX - u64::MAX % denominator;
    let map_bytes = std::fs::read(map_path)?;
    let map: FaultMap = serde_json::from_slice(&map_bytes)?;
    let family = map
        .families
        .get(&format!("tqd{}", config.size))
        .ok_or("missing TQD scale in fault map")?;
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    if map.schema != "rmagna.paper-tqd-fault-map.v1"
        || family.size != config.size
        || family.depth != depth
        || family.physical_sha256 != digest(physical_text.as_bytes())
    {
        return Err("TQD map does not bind the full physical source".into());
    }
    let process = decoder_worker::worker(&controller_path, &python)?;
    let mut registry = ControllerRegistry::default();
    process.register_all(&mut registry)?;
    let parse = ParseOptions {
        source: Some(physical_path),
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(&physical_text, &parse)?;
    let sites = fault_sites(&physical, family)?;
    let spec = parse_task(
        &task_text,
        task_path.parent().ok_or("task has no input root")?,
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&physical, &parse)?;
    let width = config.size * config.size;
    let expected_calls = 3 * depth + 4;
    if physical.num_qubits() != 6 * width + 1
        || physical.external_calls() != expected_calls
        || modules.task().outputs().len() != 4 * width
        || modules.recovery().is_some()
        || modules
            .ideal_state()
            .is_none_or(|target| target.outputs().len() != 4 * width)
    {
        return Err(
            "TQD requires the complete four-logical-qubit task and author callbacks".into(),
        );
    }
    let mut native = NativePhaseOptions::default();
    native.selection.signature_scan = PhaseSignatureScan::OddAxisPivots;
    native.selection.max_quotient_rank = 12;
    native.selection.max_signature_work = 1 << 37;
    native.selection.preparation.max_terms = 4096;
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.sampling.max_terms = 4096;
    options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.runtime.compile.probability_preparation = QueryPreparation::Native(native);
    options.runtime.compile.moment_preparation = QueryPreparation::Native(native);
    options.runtime.compile.density_preparation = QueryPreparation::FixedFibers(Default::default());
    let compile = Instant::now();
    let evaluator = TaskEvaluator::new(&physical, &modules, &registry, options)?;
    let compile_seconds = compile.elapsed().as_secs_f64();
    let unit = BigRational::one();
    let expected_norm =
        ScalarArithmetic::default().scalar([5.into(), 0.into(), 0.into(), 0.into()], -3)?;
    let evaluate = |index,
                    selected: Vec<usize>,
                    meter: &ExecutionMeter|
     -> Result<(Value, Vec<Value>), Box<dyn Error>> {
        let mut born = stream(config.seed, index, true);
        let faults = selected
            .iter()
            .map(|&ordinal| FaultSelection {
                site: sites[ordinal],
                alternative: 0,
            })
            .collect::<Vec<_>>();
        let result =
            evaluator.sample_supplied_noise_with_costs(&faults, &unit, &mut born, meter)?;
        let mut charge = BTreeMap::new();
        for call in &result.controller_calls {
            if let Some(color) = call.name.strip_prefix("charge_") {
                if call.inputs.len() != width * (depth + 1)
                    || call.outputs.len() != 2 * width
                    || charge
                        .insert(
                            color,
                            call.inputs.iter().filter(|&&bit| bit).count() % 2 != 0,
                        )
                        .is_some()
                {
                    return Err("TQD terminal reported charge transcript changed".into());
                }
            }
        }
        if result.controller_calls.len() != expected_calls
            || charge.keys().copied().ne(["b", "g", "r"])
            || result.target_normalization.as_ref() != Some(&expected_norm)
        {
            return Err("TQD target normalization or actual decoder coverage differs".into());
        }
        let calls = result.controller_calls.iter().map(|call| {
            json!({"call":call.call,"name":call.name,"inputs":call.inputs,"outputs":call.outputs})
        }).collect();
        let fields = |value: &rmagna::field::ExactField| {
            value.coefficients().each_ref().map(ToString::to_string)
        };
        let row = json!({"index":index,"faults":selected,"accepted":fields(&result.accepted),"error":fields(&result.error),
                         "history_weight":fields(&result.history_weight),"odd_terminal_charge":charge,
                         "resources":{"acceptance":[result.acceptance_resources.quotient_rank,result.acceptance_resources.terms],
                           "target":result.target_resources.map(|resource| [resource.quotient_rank,resource.terms])}});
        Ok((row, calls))
    };
    let draw = |index, meter: &ExecutionMeter| {
        let mut noise = stream(config.seed, index, false);
        let mut selected = Vec::new();
        for ordinal in 0..sites.len() {
            let value = loop {
                let value = noise.next_u64();
                if value < cutoff {
                    break value;
                }
            };
            if value % denominator < numerator {
                selected.push(ordinal);
            }
        }
        evaluate(index, selected, meter)
    };
    draw(WARMUP, &ExecutionMeter::default())?;
    let preparation = started.elapsed().as_secs_f64();
    let mut readout_witness = None;
    let first_precision_seed = 202610080000 + config.size as u64 * 100000;
    if config.seed == first_precision_seed && config.start == 0 {
        let ordinal = family
            .entries
            .iter()
            .position(|entry| {
                entry.kind == "X"
                    && entry.color == "b"
                    && entry.edge == 2
                    && entry.address.record.is_some()
            })
            .ok_or("missing first TQD reported-B fault")?;
        let probe_index = WARMUP - 1;
        let (_, ideal) = evaluate(probe_index, vec![], &ExecutionMeter::default())?;
        let (_, flipped) = evaluate(probe_index, vec![ordinal], &ExecutionMeter::default())?;
        let first = |calls: Vec<Value>| {
            calls
                .into_iter()
                .next()
                .ok_or("missing TQD reported syndrome trace")
        };
        let ideal = first(ideal)?;
        let flipped = first(flipped)?;
        let ideal_inputs = ideal["inputs"]
            .as_array()
            .ok_or("invalid TQD syndrome witness")?;
        let flipped_inputs = flipped["inputs"]
            .as_array()
            .ok_or("invalid TQD syndrome witness")?;
        if ideal["name"] != "jit_b_0"
            || flipped["name"] != "jit_b_0"
            || ideal_inputs.len() != flipped_inputs.len()
            || ideal_inputs.is_empty()
            || ideal_inputs[0] == flipped_inputs[0]
            || ideal_inputs[1..] != flipped_inputs[1..]
        {
            return Err("actual TQD decoder did not receive the flipped reported readout".into());
        }
        readout_witness =
            Some(json!({"index":probe_index,"fault":ordinal,"unflipped":ideal,"flipped":flipped}));
    }
    let meter = ExecutionMeter::default();
    let sampling = Instant::now();
    let mut rows = Vec::with_capacity(config.shots as usize);
    let mut first_decoder_calls = None;
    let mut decoder_sha = Sha256::new();
    let mut failure = None;
    for index in config.start..end {
        match draw(index, &meter) {
            Ok((row, calls)) => {
                decoder_sha.update(serde_json::to_vec(&calls)?);
                decoder_sha.update(b"\n");
                if first_decoder_calls.is_none() {
                    first_decoder_calls = Some(calls);
                }
                rows.push(row);
            }
            Err(error) => {
                failure = Some(json!({"index":index,"error":error.to_string()}));
                break;
            }
        }
    }
    let elapsed = sampling.elapsed().as_secs_f64();
    let costs = meter.snapshot();
    let target = spec.ideal_state().ok_or("missing TQD full target")?;
    Ok(
        json!({"schema":"rmagna.paper-tqd-physical-run.v1","status":if failure.is_none() {"complete"} else {"failed"},
                 "configuration":config,"completed":rows.len(),"failure":failure,"rows":rows,
                 "active_sites":sites.len(),"fault_map_sha256":digest(&map_bytes),"rng_protocol":PROTOCOL,
                 "branch_target_norm":"5/8","outputs":4*width,"charge_depth":depth+1,
                 "excluded_warmup":{"draws":1,"index":WARMUP},"timing":{"compile_seconds":compile_seconds,
                    "preparation_seconds":preparation,"sampling_seconds":elapsed,"decoder_seconds":costs.decoder.seconds,
                    "decoder_attempted":costs.decoder.attempted,"decoder_completed":costs.decoder.succeeded},
                 "decoder":process.manifest(),"first_decoder_calls":first_decoder_calls,"readout_witness":readout_witness,
                 "decoder_transcript_sha256":format!("{:x}",decoder_sha.finalize()),
                 "inputs":{"physical_sha256":digest(physical_text.as_bytes()),"task_sha256":digest(task_text.as_bytes()),
                           "target_sha256":digest(&std::fs::read(target.circuit_path())?),
                           "controllers_sha256":digest(&std::fs::read(controller_path)?)},
                 "build":{"debug_assertions":cfg!(debug_assertions),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH},
                 "rust_source_sha256":env!("RMAGNA_PAPER_SOURCE_SHA256"),"executable_sha256":digest(&std::fs::read(std::env::current_exe()?)?)
        }),
    )
}

fn main() {
    let result = run(std::env::args().skip(1)).unwrap_or_else(|error| {
        json!({
        "schema":"rmagna.paper-tqd-physical-run.v1","status":"failed","failure":error.to_string()})
    });
    println!("{result}");
    if result["status"] != "complete" {
        std::process::exit(1);
    }
}
