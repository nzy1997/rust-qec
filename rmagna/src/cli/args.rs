use num_traits::Signed;
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTaskOptions};
use rmagna::instrument::{InsertionPreparation, ProbabilityPreparation, QueryPreparation};
use rmagna::noise::NoiseSampling;
use rmagna::probability::ProbabilityParser;
use rmagna::runtime::process::ProcessOptions;
use rmagna::runtime::{CausalStrategy, HistorySampler};
use rmagna::task::TaskRunOptions;
use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

pub const HELP: &str = "rmagna validate|inspect|run|enumerate CIRCUIT [OPTIONS]

validate parses and fingerprints inputs without quantum queries or callbacks.
inspect additionally compiles the protocol, without draws or decoder callbacks.
run prepares and performs real noisy draws from a fresh SplitMix64 seed.
enumerate integrates bounded physical noise sectors and all positive histories.

Inputs and output:
  --task FILE                         Task JSON; module paths are relative to it
  --controllers FILE                  Explicit Python decoder bindings
  --python PROGRAM                    Decoder Python executable (default python3)
  --module-root DIR                   Decoder sources (default bindings directory)
  --decoder-timeout SECONDS            Positive exchange timeout (default 60)
  --json                              One JSON object on stdout
  --output FILE                       Save the same JSON; refuse existing files
  --help, -h                          Show this help
  --version                           Show the Rust implementation version

Run configuration:
  --shots N                           Positive draw count (default 1; run only)
  --seed N                            u64 seed (default 0; run only)
  --noise-proposal-scale RATIONAL      Exact positive scale; unavailable in enumerate
  --compiler shared|parity_copies      Default shared
  --history-sampler affine_support|exact_marginal   Default affine_support; not enumerate
  --causal-strategy fixed|cold         Default fixed
  --probability-preparation cold|trace|fixed        Default fixed, on demand
  --density-preparation cold|trace|fixed            Default fixed, on demand
  --fiber-search-rank N               Fixed preparation search rank (default 16)
  --max-prepared-probabilities N       Retained query cap (default 64)
  --max-prepared-densities N           Retained query cap (default 16)
  --insertion-preparation direct|prepared           Default direct
  --max-insertion-table-bytes N        Prepared insertion estimate cap (default 67108864)
  --max-insertion-catalog-entries N    Prepared insertion entry cap (default 1000000)

Exact enumeration only:
  --max-weight N                      Nonnegative decimal category cutoff (default 1; unbounded input)
  --max-patterns N                    Positive pattern cap (default 100000)
  --max-fault-entries N               Stored fault entry cap (default 1000000)
  --max-enumeration-sites N           Potential noise category cap (default 100000)
  --max-site-weight-pairs N           Sector probability work cap (default 10000000)
  --max-enumeration-probability-bits N Exact sector integer bit cap (default 1048576)
  --max-history-branches N            Global complete/pending histories (default 4096)
  --max-history-record-bits N         Independent bits per unresolved group (default 64)
  --max-marginal-queries N            Global history marginal quota (default 1000000)
  --max-replay-attempts N             Global complete/interrupted replays (default 1000000)

Limits (zero is allowed for quotas unless marked positive):
  --max-terms N                       Query contraction term budget
  --max-coherent-qubits N             Compiled coherent width budget
  --max-gates N                       Compiled gate budget
  --max-noise-sites N                 Compiled noise site budget
  --max-queries N                     Call-wide query quota, including failures
  --max-seconds SECONDS               Positive deadline checked between operations
  --max-source-bytes N                Positive per-circuit source byte limit
  --max-task-bytes N                  Positive task JSON byte limit
  --max-record-values N               Requested Boolean values per run; not enumerate
  --numeric-work-bits N               Numeric export intermediate integer bit cap
  --fingerprint-bytes N               Positive canonical JSON byte limit

Python is launched only with --controllers. Explicit decoder modules may execute
on import, including during validate/inspect. The host cannot import magna.
CLI defaults differ from library defaults. Resolved options are in every success
report. Native option files, compare-samplers and progress are not yet
supported. Use the Rust API for typed exact results if numeric export fails.
";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Validate,
    Inspect,
    Run,
    Enumerate,
}
pub struct Args {
    pub mode: Mode,
    pub circuit: PathBuf,
    pub task: Option<PathBuf>,
    pub controllers: Option<PathBuf>,
    pub python: String,
    pub module_root: Option<PathBuf>,
    pub process: ProcessOptions,
    pub json: bool,
    pub output: Option<PathBuf>,
    pub file: FileTaskOptions,
    pub load: FileLoadOptions,
    pub run: TaskRunOptions,
    pub enumeration: FileEnumerationOptions,
    pub sampling: NoiseSampling,
}
fn duration(text: &str, flag: &str) -> Result<Duration, String> {
    let value = text
        .parse::<f64>()
        .map_err(|_| format!("{flag} needs positive finite seconds"))?;
    let result = Duration::try_from_secs_f64(value)
        .map_err(|_| format!("{flag} needs positive finite representable seconds"))?;
    if result.is_zero() {
        return Err(format!("{flag} must be positive after nanosecond rounding"));
    }
    Ok(result)
}
struct Flags(BTreeMap<String, String>);
const ENUMERATION_FLAGS: [&str; 10] = [
    "--max-weight",
    "--max-patterns",
    "--max-fault-entries",
    "--max-enumeration-sites",
    "--max-site-weight-pairs",
    "--max-enumeration-probability-bits",
    "--max-history-branches",
    "--max-history-record-bits",
    "--max-marginal-queries",
    "--max-replay-attempts",
];
impl Flags {
    fn text(&mut self, flag: &str) -> Option<String> {
        self.0.remove(flag)
    }
    fn number<T: std::str::FromStr>(&mut self, flag: &str, default: T) -> Result<T, String> {
        self.text(flag).map_or(Ok(default), |v| {
            v.parse()
                .map_err(|_| format!("{flag} needs a nonnegative integer in range"))
        })
    }
    fn choice(&mut self, flag: &str, default: &str, choices: &[&str]) -> Result<String, String> {
        let value = self.text(flag).unwrap_or_else(|| default.to_owned());
        if choices.contains(&value.as_str()) {
            Ok(value)
        } else {
            Err(format!("{flag} must be {}", choices.join(" or ")))
        }
    }
}
impl Args {
    pub fn parse(raw: Vec<String>) -> Result<Self, String> {
        let (mode, circuit, mut flags) = parse_tokens(raw)?;
        if mode != Mode::Enumerate {
            if let Some(flag) = ENUMERATION_FLAGS
                .iter()
                .find(|flag| flags.0.contains_key(**flag))
            {
                return Err(format!("{flag} requires enumerate"));
            }
        } else if let Some(flag) = [
            "--noise-proposal-scale",
            "--history-sampler",
            "--max-record-values",
        ]
        .iter()
        .find(|flag| flags.0.contains_key(**flag))
        {
            return Err(format!(
                "{flag} is unavailable in enumerate; exact physical histories do not use a sampling policy"
            ));
        }
        let json = flags.text("--json").is_some();
        let output = flags.text("--output").map(PathBuf::from);
        let task = flags.text("--task").map(PathBuf::from);
        let controllers = flags.text("--controllers").map(PathBuf::from);
        if controllers.is_none()
            && ["--python", "--module-root", "--decoder-timeout"]
                .iter()
                .any(|f| flags.0.contains_key(*f))
        {
            return Err(
                "--python, --module-root and --decoder-timeout require --controllers".to_owned(),
            );
        }
        let python = flags
            .text("--python")
            .unwrap_or_else(|| "python3".to_owned());
        let module_root = flags.text("--module-root").map(PathBuf::from);
        let mut process = ProcessOptions::default();
        if let Some(value) = flags.text("--decoder-timeout") {
            process.timeout = duration(&value, "--decoder-timeout")?;
        }
        let mut file = FileTaskOptions::default();
        let mut load = FileLoadOptions::default();
        let mut run = TaskRunOptions::default();
        if mode != Mode::Run
            && ["--shots", "--seed"]
                .iter()
                .any(|f| flags.0.contains_key(*f))
        {
            return Err("--shots and --seed require run".to_owned());
        }
        configure_limits(&mut flags, &mut file, &mut load, &mut run)?;
        let mut enumeration = configure_enumeration(&mut flags)?;
        enumeration.execution_limits = run.execution_limits;
        configure_runtime(&mut flags, &mut file)?;
        let sampling = if let Some(text) = flags.text("--noise-proposal-scale") {
            let proposal_scale = ProbabilityParser::default()
                .rational(&text)
                .map_err(|e| format!("--noise-proposal-scale: {e}"))?;
            if !proposal_scale.is_positive() {
                return Err("--noise-proposal-scale must be positive".to_owned());
            }
            NoiseSampling::Importance { proposal_scale }
        } else {
            NoiseSampling::Physical
        };
        if let Some((flag, _)) = flags.0.first_key_value() {
            return Err(format!("unknown option {flag}; use --help"));
        }
        Ok(Self {
            mode,
            circuit,
            task,
            controllers,
            python,
            module_root,
            process,
            json,
            output,
            file,
            load,
            run,
            enumeration,
            sampling,
        })
    }
}

fn parse_tokens(raw: Vec<String>) -> Result<(Mode, PathBuf, Flags), String> {
    let mut it = raw.into_iter();
    let mode = match it.next().as_deref() {
        Some("validate") => Mode::Validate,
        Some("inspect") => Mode::Inspect,
        Some("run") => Mode::Run,
        Some("enumerate") => Mode::Enumerate,
        _ => return Err("expected validate, inspect, run, or enumerate; use --help".to_owned()),
    };
    let mut flags = Flags(BTreeMap::new());
    let mut circuit = None;
    while let Some(arg) = it.next() {
        if !arg.starts_with('-') {
            if circuit.replace(PathBuf::from(arg)).is_some() {
                return Err("expected one CIRCUIT path".to_owned());
            }
            continue;
        }
        if flags.0.contains_key(&arg) {
            return Err(format!("duplicate option {arg}"));
        }
        if !matches!(
            arg.as_str(),
            "--json"
                | "--output"
                | "--task"
                | "--controllers"
                | "--python"
                | "--module-root"
                | "--decoder-timeout"
                | "--shots"
                | "--seed"
                | "--max-queries"
                | "--max-seconds"
                | "--max-record-values"
                | "--numeric-work-bits"
                | "--fingerprint-bytes"
                | "--max-source-bytes"
                | "--max-task-bytes"
                | "--history-sampler"
                | "--causal-strategy"
                | "--max-terms"
                | "--compiler"
                | "--max-coherent-qubits"
                | "--max-gates"
                | "--max-noise-sites"
                | "--max-prepared-probabilities"
                | "--max-prepared-densities"
                | "--probability-preparation"
                | "--density-preparation"
                | "--fiber-search-rank"
                | "--insertion-preparation"
                | "--max-insertion-table-bytes"
                | "--max-insertion-catalog-entries"
                | "--noise-proposal-scale"
        ) && !ENUMERATION_FLAGS.contains(&arg.as_str())
        {
            return Err(format!("unknown option {arg}; use --help"));
        }
        let value = if arg == "--json" {
            String::new()
        } else {
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {arg}"))?;
            if value.starts_with("--") {
                return Err(format!("missing value for {arg}"));
            }
            value
        };
        flags.0.insert(arg, value);
    }
    let circuit = circuit.ok_or("missing CIRCUIT path")?;
    Ok((mode, circuit, flags))
}

fn configure_enumeration(flags: &mut Flags) -> Result<FileEnumerationOptions, String> {
    let mut options = FileEnumerationOptions::default();
    let n = &mut options.enumeration.noise;
    if let Some(text) = flags.text("--max-weight") {
        n.max_weight = weight_cutoff(&text)?;
    }
    n.max_patterns = flags.number("--max-patterns", n.max_patterns)?;
    if n.max_patterns == 0 {
        return Err("--max-patterns must be positive".to_owned());
    }
    n.max_fault_entries = flags.number("--max-fault-entries", n.max_fault_entries)?;
    n.max_sites = flags.number("--max-enumeration-sites", n.max_sites)?;
    n.max_site_weight_pairs = flags.number("--max-site-weight-pairs", n.max_site_weight_pairs)?;
    n.max_probability_bits =
        flags.number("--max-enumeration-probability-bits", n.max_probability_bits)?;
    let h = &mut options.enumeration.histories;
    h.max_branches = flags.number("--max-history-branches", h.max_branches)?;
    h.max_record_bits = flags.number("--max-history-record-bits", h.max_record_bits)?;
    h.max_marginal_queries = flags.number("--max-marginal-queries", h.max_marginal_queries)?;
    h.max_replay_attempts = flags.number("--max-replay-attempts", h.max_replay_attempts)?;
    Ok(options)
}

fn weight_cutoff(text: &str) -> Result<usize, String> {
    let digits = text.strip_prefix('+').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("--max-weight needs a nonnegative decimal integer".to_owned());
    }
    // Every pattern has at most num_sites <= usize::MAX faults. A larger
    // ceiling selects the same finite sector; all allocation/work quotas
    // still apply. Validate the entire token before accepting any overflow.
    Ok(digits.bytes().fold(0usize, |n, b| {
        n.saturating_mul(10).saturating_add(usize::from(b - b'0'))
    }))
}

fn configure_limits(
    flags: &mut Flags,
    file: &mut FileTaskOptions,
    load: &mut FileLoadOptions,
    run: &mut TaskRunOptions,
) -> Result<(), String> {
    run.shots = flags.number("--shots", run.shots)?;
    if run.shots == 0 {
        return Err("--shots must be positive".to_owned());
    }
    run.seed = flags.number("--seed", run.seed)?;
    run.execution_limits.max_queries =
        flags.number("--max-queries", run.execution_limits.max_queries)?;
    if let Some(value) = flags.text("--max-seconds") {
        run.execution_limits.max_duration = Some(duration(&value, "--max-seconds")?);
    }
    run.max_retained_record_values =
        flags.number("--max-record-values", run.max_retained_record_values)?;
    file.numeric.max_work_bits = flags.number("--numeric-work-bits", file.numeric.max_work_bits)?;
    file.fingerprints.max_json_bytes =
        flags.number("--fingerprint-bytes", file.fingerprints.max_json_bytes)?;
    load.circuit.max_source_bytes =
        flags.number("--max-source-bytes", load.circuit.max_source_bytes)?;
    load.task.max_json_bytes = flags.number("--max-task-bytes", load.task.max_json_bytes)?;
    for (flag, value) in [
        ("--fingerprint-bytes", file.fingerprints.max_json_bytes),
        ("--max-source-bytes", load.circuit.max_source_bytes),
        ("--max-task-bytes", load.task.max_json_bytes),
    ] {
        if value == 0 {
            return Err(format!("{flag} must be positive"));
        }
    }
    Ok(())
}

fn configure_runtime(flags: &mut Flags, file: &mut FileTaskOptions) -> Result<(), String> {
    let runtime = &mut file.evaluation.runtime;
    runtime.history_sampler = match flags
        .choice(
            "--history-sampler",
            "affine_support",
            &["affine_support", "exact_marginal"],
        )?
        .as_str()
    {
        "affine_support" => HistorySampler::AffineSupport,
        _ => HistorySampler::ExactMarginal,
    };
    runtime.strategy = match flags
        .choice("--causal-strategy", "fixed", &["fixed", "cold"])?
        .as_str()
    {
        "fixed" => CausalStrategy::FixedTemplate,
        _ => CausalStrategy::ColdPrefix,
    };
    runtime.sampling.max_terms = flags.number("--max-terms", runtime.sampling.max_terms)?;
    let compile = &mut runtime.compile;
    compile.compiler_strategy = match flags
        .choice("--compiler", "shared", &["shared", "parity_copies"])?
        .as_str()
    {
        "shared" => CompilerStrategy::SharedCommutingLayer,
        _ => CompilerStrategy::ParityCopies,
    };
    compile.max_coherent_qubits =
        flags.number("--max-coherent-qubits", compile.max_coherent_qubits)?;
    compile.max_gates = flags.number("--max-gates", compile.max_gates)?;
    compile.max_noise_sites = flags.number("--max-noise-sites", compile.max_noise_sites)?;
    compile.max_prepared_probabilities = flags.number(
        "--max-prepared-probabilities",
        compile.max_prepared_probabilities,
    )?;
    compile.max_prepared_densities =
        flags.number("--max-prepared-densities", compile.max_prepared_densities)?;
    configure_queries(flags, compile)?;
    configure_insertions(flags, compile)?;
    Ok(())
}

fn configure_queries(
    flags: &mut Flags,
    compile: &mut rmagna::instrument::CompileOptions,
) -> Result<(), String> {
    let probability = flags.choice(
        "--probability-preparation",
        "fixed",
        &["cold", "trace", "fixed"],
    )?;
    let density = flags.choice(
        "--density-preparation",
        "fixed",
        &["cold", "trace", "fixed"],
    )?;
    if flags.0.contains_key("--fiber-search-rank") && probability != "fixed" && density != "fixed" {
        return Err(
            "--fiber-search-rank requires fixed probability or density preparation".to_owned(),
        );
    }
    let mut fiber = rmagna::fibers::FiberOptions::default();
    fiber.max_search_rank = flags.number("--fiber-search-rank", fiber.max_search_rank)?;
    compile.probability_preparation = match probability.as_str() {
        "cold" => ProbabilityPreparation::Cold,
        "trace" => ProbabilityPreparation::Trace,
        _ => ProbabilityPreparation::FixedFibers(fiber),
    };
    compile.density_preparation = match density.as_str() {
        "cold" => QueryPreparation::Cold,
        "trace" => QueryPreparation::Trace,
        _ => QueryPreparation::FixedFibers(fiber),
    };
    Ok(())
}

fn configure_insertions(
    flags: &mut Flags,
    compile: &mut rmagna::instrument::CompileOptions,
) -> Result<(), String> {
    let insertion = flags.choice("--insertion-preparation", "direct", &["direct", "prepared"])?;
    if insertion != "prepared"
        && [
            "--max-insertion-table-bytes",
            "--max-insertion-catalog-entries",
        ]
        .iter()
        .any(|f| flags.0.contains_key(*f))
    {
        return Err("insertion limits require --insertion-preparation prepared".to_owned());
    }
    let mut prepared = InsertionPreparationOptions::default();
    prepared.max_table_bytes =
        flags.number("--max-insertion-table-bytes", prepared.max_table_bytes)?;
    prepared.max_catalog_entries = flags.number(
        "--max-insertion-catalog-entries",
        prepared.max_catalog_entries,
    )?;
    compile.insertion_preparation = if insertion == "prepared" {
        InsertionPreparation::Prepared(prepared)
    } else {
        InsertionPreparation::Direct
    };
    Ok(())
}
