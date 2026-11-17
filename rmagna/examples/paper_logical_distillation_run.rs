//! Full encoded-input experiment with one physical assignment reused at every p.
#[path = "support/cultivation_rng.rs"]
mod addressed_rng;
#[path = "support/decoder_worker.rs"]
mod decoder_worker;
#[path = "support/logical_residual.rs"]
mod logical_residual;

use logical_residual::{PATCHES, ResidualProposal, WIDTH};
use num_rational::BigRational;
use num_traits::One;
use rmagna::catalogue::PhaseSignatureScan;
use rmagna::circuit::{Basis, Circuit, ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::ExactField;
use rmagna::instrument::{FaultSelection, QueryPreparation};
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
use rmagna::phase::NativePhaseOptions;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::PathBuf, time::Instant};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    scope: String,
    seed: u32,
    start: u32,
    shots: u32,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct FaultSite {
    internal: usize,
    wire: usize,
    alternatives: [usize; 3],
}

fn fault_sites(circuit: &Circuit) -> Result<Vec<FaultSite>, Box<dyn Error>> {
    let categories = original_noise_categories(circuit, &OriginalNoiseOptions::default())?;
    let mut sites = Vec::with_capacity(PATCHES * WIDTH);
    let expected = BigRational::new(1.into(), 300.into());
    for category in categories {
        if category.identity_probability.value().is_one() {
            continue;
        }
        if category.guard.is_some() || category.channels.len() != 3 {
            return Err("encoded inputs require unguarded depolarizing categories".into());
        }
        let mut alternatives = [None; 3];
        let mut wire = None;
        for (index, channel) in category.channels.iter().enumerate() {
            let OriginalNoiseAction::Pauli { factors } = &channel.action else {
                return Err("unexpected noisy readout in encoded-input experiment".into());
            };
            let &[(target, basis)] = factors.as_slice() else {
                return Err("encoded input noise must act on one wire".into());
            };
            if channel.probability.value() != &expected || wire.is_some_and(|old| old != target) {
                return Err("encoded-input depolarizing channel changed".into());
            }
            wire = Some(target);
            let label = match basis {
                Basis::X => 0,
                Basis::Z => 1,
                Basis::Y => 2,
            };
            if alternatives[label].replace(index).is_some() {
                return Err("repeated Pauli channel at encoded-input site".into());
            }
        }
        sites.push(FaultSite {
            internal: category.internal_site,
            wire: wire.ok_or("empty encoded-input noise site")?,
            alternatives: alternatives
                .map(|option| option.expect("three distinct single-qubit Pauli alternatives")),
        });
    }
    if sites.len() != PATCHES * WIDTH
        || sites
            .windows(2)
            .any(|pair| pair[0].internal >= pair[1].internal)
        || sites
            .chunks_exact(WIDTH)
            .any(|patch| patch.iter().map(|site| site.wire).ne(85..94))
    {
        return Err(
            "encoded-input noise must cover all 15 nine-wire blocks in source order".into(),
        );
    }
    Ok(sites)
}

fn run(mut args: impl Iterator<Item = String>) -> Result<Value, Box<dyn Error>> {
    let started = Instant::now();
    let physical_path = args
        .next()
        .ok_or("usage: paper_logical_distillation_run PHYSICAL TASK CONFIG CONTROLLERS PYTHON")?;
    let task_path = PathBuf::from(args.next().ok_or("missing task path")?);
    let configuration = args.next().ok_or("missing configuration path")?;
    let controller_path = PathBuf::from(args.next().ok_or("missing controller manifest")?);
    let python = args.next().ok_or("missing decoder Python")?;
    let config: Configuration = serde_json::from_str(&std::fs::read_to_string(configuration)?)?;
    let end = config
        .start
        .checked_add(config.shots)
        .ok_or("draw address overflow")?;
    if config.schema != "rmagna.paper-logical-distillation-configuration.v1"
        || !matches!(
            (config.scope.as_str(), config.seed),
            ("precision", 2026100631) | ("serial_timing", 2026100801)
        )
        || config.shots == 0
        || end > 16384
        || args.next().is_some()
    {
        return Err("invalid logical distillation configuration".into());
    }
    let process = decoder_worker::worker(&controller_path, &python)?;
    let mut registry = ControllerRegistry::default();
    process.register_all(&mut registry)?;
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    let parse = ParseOptions {
        source: Some(physical_path),
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(&physical_text, &parse)?;
    if circuit.num_qubits() != 102 || circuit.num_records() != 451 || circuit.external_calls() != 39
    {
        return Err("logical distillation requires the full compact physical task".into());
    }
    let spec = parse_task(
        &task_text,
        task_path.parent().ok_or("task has no input root")?,
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&circuit, &parse)?;
    if modules.task().outputs().len() != WIDTH
        || modules.recovery().is_some()
        || modules
            .ideal_state()
            .is_none_or(|target| target.outputs().len() != WIDTH)
    {
        return Err("logical distillation requires the full encoded output target".into());
    }
    let sites = fault_sites(&circuit)?;
    let proposal = ResidualProposal::new()?;
    let options = {
        let mut native = NativePhaseOptions::default();
        native.selection.signature_scan = PhaseSignatureScan::OddAxisPivots;
        native.selection.max_quotient_rank = 20;
        native.selection.max_signature_work = 1 << 36;
        native.selection.preparation.max_terms = 65536;
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::ExactMarginal;
        options.runtime.sampling.max_terms = 65536;
        options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
        options.runtime.compile.max_path_bits = 65536;
        options.runtime.compile.max_prepared_probabilities = 512;
        options.runtime.compile.probability_preparation = QueryPreparation::Native(native);
        options.runtime.compile.moment_preparation = QueryPreparation::Native(native);
        options.runtime.compile.density_preparation =
            QueryPreparation::FixedFibers(rmagna::fibers::FiberOptions {
                max_search_rank: 20,
                ..Default::default()
            });
        options
    };
    let compile = Instant::now();
    let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, options)?;
    let compile_seconds = compile.elapsed().as_secs_f64();
    let unit = BigRational::one();
    let born_seed = config.seed + 100000;
    let evaluate = |index,
                    patterns: [u32; PATCHES],
                    physical_weight,
                    nonidentity,
                    meter: &ExecutionMeter|
     -> Result<(Value, Vec<Value>), Box<dyn Error>> {
        let mut born = addressed_rng::draw_stream(born_seed, index);
        let mut faults = Vec::with_capacity(physical_weight);
        for (patch, word) in patterns.iter().enumerate() {
            for (wire, site) in sites[patch * WIDTH..(patch + 1) * WIDTH].iter().enumerate() {
                let label = (word >> (2 * wire)) & 3;
                if label != 0 {
                    faults.push(FaultSelection {
                        site: site.internal,
                        alternative: site.alternatives[(label - 1) as usize],
                    });
                }
            }
        }
        let result =
            evaluator.sample_supplied_noise_with_costs(&faults, &unit, &mut born, meter)?;
        let expected = BTreeMap::from([
            ("gauge", 5),
            ("ideal_input_recovery", 15),
            ("readout_X", 4),
            ("readout_Z", 15),
        ]);
        let mut calls = BTreeMap::new();
        for call in &result.controller_calls {
            *calls.entry(call.name.as_str()).or_insert(0) += 1;
        }
        if calls != expected
            || result.target_normalization.is_none()
            || result.history_weight != ExactField::one()
        {
            return Err("incomplete logical distillation decoder or target evaluation".into());
        }
        let transcript = result
            .controller_calls
            .iter()
            .map(|c| {
                json!({
                    "call":c.call,"name":c.name,"inputs":c.inputs,"outputs":c.outputs
                })
            })
            .collect::<Vec<_>>();
        let row = json!({"index":index,"patterns":patterns,"physical_weight":physical_weight,
                         "nonidentity":nonidentity,
                         "acceptance":result.accepted.coefficients().each_ref().map(ToString::to_string),
                         "error":result.error.coefficients().each_ref().map(ToString::to_string),
                         "resources":{"acceptance":[result.acceptance_resources.quotient_rank,result.acceptance_resources.terms],
                         "target":result.target_resources.map(|r| [r.quotient_rank,r.terms])}});
        Ok((row, transcript))
    };

    let draw = |index, meter: &ExecutionMeter| {
        let mut noise = addressed_rng::draw_stream(config.seed, index);
        let mut physical_weight = 0;
        let mut nonidentity = 0;
        let patterns = std::array::from_fn(|_| {
            let sampled = proposal.draw(&mut noise);
            physical_weight += sampled.weight;
            nonidentity += usize::from(sampled.residual != 0);
            sampled.word
        });
        evaluate(index, patterns, physical_weight, nonidentity, meter)
    };
    draw(u32::MAX, &ExecutionMeter::default())?;
    let preparation_seconds = started.elapsed().as_secs_f64();
    let mut prescribed = Vec::new();
    if config.scope == "precision" && config.start == 0 {
        // The deposit stores separate X and Z masks. Its word 1536 is a Z on
        // each of wires 0 and 1, hence 10 in interleaved IXZY coordinates.
        let pattern = 10;
        for patches in 0..=3 {
            let mut patterns = [0; PATCHES];
            patterns[..patches].fill(pattern);
            let (row, _) = evaluate(
                u32::MAX - 1 - patches as u32,
                patterns,
                2 * patches,
                patches,
                &ExecutionMeter::default(),
            )?;
            let acceptance = if matches!(patches, 0 | 3) { "1" } else { "0" };
            let error = if patches == 3 { "1" } else { "0" };
            if row["acceptance"] != json!([acceptance, "0", "0", "0"])
                || row["error"] != json!([error, "0", "0", "0"])
            {
                return Err(format!(
                    "logical distillation prescribed fault check failed for {patches} patches"
                )
                .into());
            }
            prescribed.push(row);
        }
    }
    let meter = ExecutionMeter::default();
    let mut rows = Vec::with_capacity(config.shots as usize);
    let mut failure = None;
    let mut transcript = Sha256::new();
    let mut first_decoder_calls = None;
    let sampling = Instant::now();
    for index in config.start..end {
        match draw(index, &meter) {
            Ok((row, calls)) => {
                transcript.update(serde_json::to_vec(&calls)?);
                transcript.update(b"\n");
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
    let sampling_seconds = sampling.elapsed().as_secs_f64();
    let costs = meter.snapshot();
    let ideal = spec.ideal_state().ok_or("missing encoded-input target")?;
    Ok(json!({
        "schema":"rmagna.paper-logical-distillation-run.v1","status":if failure.is_none() {"complete"} else {"failed"},
        "failure":failure,"configuration":config,"completed":rows.len(),"rows":rows,"prescribed_fault_checks":prescribed,
        "proposal":{"pattern_encoding":"interleaved_ixzy_low_wire_first","p0":"1/100","nonidentity":"1/5","identity_mass_numerator":proposal.identity_mass.to_string(),
                    "identity_mass_denominator":300_u128.pow(WIDTH as u32).to_string(),"classifier_sha256":proposal.fixture_sha256,
                    "categories":sites.len(),"active_wires_per_patch":WIDTH,"patches":PATCHES},
        "rng_protocol":addressed_rng::PROTOCOL,"born_seed":born_seed,"history_sampler":"exact_marginal","history_weight":"1",
        "excluded_warmup":{"draws":1,"index":u32::MAX},
        "decoder":process.manifest(),"first_decoder_calls":first_decoder_calls,
        "decoder_transcript_sha256":format!("{:x}",transcript.finalize()),
        "timing":{"preparation_seconds":preparation_seconds,"compile_seconds":compile_seconds,"sampling_seconds":sampling_seconds,
                  "decoder_seconds":costs.decoder.seconds,"decoder_completed":costs.decoder.succeeded,
                  "decoder_attempted":costs.decoder.attempted},
        "inputs":{"physical_sha256":digest(physical_text.as_bytes()),"task_sha256":digest(task_text.as_bytes()),
                  "ideal_sha256":digest(&std::fs::read(ideal.circuit_path())?),
                  "controllers_sha256":digest(&std::fs::read(controller_path)?)},
        "build":{"debug_assertions":cfg!(debug_assertions),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH},
        "rust_source_sha256":env!("RMAGNA_PAPER_SOURCE_SHA256"),"executable_sha256":digest(&std::fs::read(std::env::current_exe()?)?)
    }))
}

fn main() {
    let result = run(std::env::args().skip(1)).unwrap_or_else(|error| json!({
        "schema":"rmagna.paper-logical-distillation-run.v1","status":"failed","failure":error.to_string()}));
    println!("{result}");
    if result["status"] != "complete" {
        std::process::exit(1);
    }
}
