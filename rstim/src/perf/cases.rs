use serde::{Deserialize, Serialize};

use crate::compiled::{
    CompiledPathDecision, SamplerPathDecision, choose_analyzer_path, choose_sampler_path,
    compile_circuit,
};
use crate::ir::StimInstr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfCaseTier {
    Gating,
    ReportOnly,
}

impl PerfCaseTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            PerfCaseTier::Gating => "gating",
            PerfCaseTier::ReportOnly => "report_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfComparisonKind {
    SamplerCompiledVsInterpreted,
    SamplerAtomLossVsInterpreted,
    AnalyzerCompiledVsFlattened,
}

impl PerfComparisonKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PerfComparisonKind::SamplerCompiledVsInterpreted => "sampler_compiled_vs_interpreted",
            PerfComparisonKind::SamplerAtomLossVsInterpreted => "sampler_atom_loss_vs_interpreted",
            PerfComparisonKind::AnalyzerCompiledVsFlattened => "analyzer_compiled_vs_flattened",
        }
    }
}

pub fn comparison_variant_labels(kind: PerfComparisonKind) -> (&'static str, &'static str) {
    match kind {
        PerfComparisonKind::SamplerCompiledVsInterpreted => ("rstim-compiled", "rstim-interpreted"),
        PerfComparisonKind::SamplerAtomLossVsInterpreted => (
            PerfVariant::RstimInterpretedAtomLoss.label(),
            PerfVariant::RstimInterpreted.label(),
        ),
        PerfComparisonKind::AnalyzerCompiledVsFlattened => {
            ("rstim-analyzer-compiled", "rstim-analyzer-flattened")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfWorkload {
    Sample,
    Detect,
    AnalyzeErrors,
}

impl PerfWorkload {
    pub fn as_str(&self) -> &'static str {
        match self {
            PerfWorkload::Sample => "sample",
            PerfWorkload::Detect => "detect",
            PerfWorkload::AnalyzeErrors => "analyze_errors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfVariant {
    StimCli,
    RstimInterpreted,
    RstimCompiled,
    RstimInterpretedAtomLoss,
    RstimAnalyzerFlattened,
    RstimAnalyzerCompiled,
}

impl PerfVariant {
    pub fn label(&self) -> &'static str {
        match self {
            PerfVariant::StimCli => "stim-cli",
            PerfVariant::RstimInterpreted => "rstim-interpreted",
            PerfVariant::RstimCompiled => "rstim-compiled",
            PerfVariant::RstimInterpretedAtomLoss => "rstim-interpreted-atom-loss",
            PerfVariant::RstimAnalyzerFlattened => "rstim-analyzer-flattened",
            PerfVariant::RstimAnalyzerCompiled => "rstim-analyzer-compiled",
        }
    }
}

pub fn expected_variant_labels(case: PerfBenchmarkCase) -> Vec<&'static str> {
    let mut variants = vec![PerfVariant::StimCli.label()];
    match case.workload {
        PerfWorkload::Sample | PerfWorkload::Detect => {
            variants.push(PerfVariant::RstimInterpreted.label());
            if case.requires_compiled {
                variants.push(PerfVariant::RstimCompiled.label());
            }
            if case.atom_loss_variant.is_some() {
                variants.push(PerfVariant::RstimInterpretedAtomLoss.label());
            }
        }
        PerfWorkload::AnalyzeErrors => {
            variants.push(PerfVariant::RstimAnalyzerFlattened.label());
            if case.requires_compiled {
                variants.push(PerfVariant::RstimAnalyzerCompiled.label());
            }
        }
    }
    variants
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerfNoiseMetadata {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfCircuitSource {
    Generator {
        code: &'static str,
        task: &'static str,
        distance: usize,
        rounds: usize,
        noise: f64,
    },
    Fixture {
        case_id: &'static str,
        canonical_input_path: &'static str,
        noise: PerfNoiseMetadata,
    },
    Inline {
        text: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfAtomLossVariant {
    pub source: PerfCircuitSource,
    pub per_event_probability: f64,
    pub aggregate_error_probability: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfBenchmarkCase {
    pub label: &'static str,
    pub workload: PerfWorkload,
    pub source: PerfCircuitSource,
    pub atom_loss_variant: Option<PerfAtomLossVariant>,
    pub shots: Option<usize>,
    pub tier: PerfCaseTier,
    pub requires_compiled: bool,
    pub requires_fallback: bool,
    pub comparisons: &'static [PerfComparisonKind],
}

const SAMPLER_COMPARE: &[PerfComparisonKind] = &[PerfComparisonKind::SamplerCompiledVsInterpreted];
const ANALYZER_COMPARE: &[PerfComparisonKind] = &[PerfComparisonKind::AnalyzerCompiledVsFlattened];
const NO_COMPARE: &[PerfComparisonKind] = &[];
const STIM_SURFACE_D11_R100_FIXTURE_PATH: &str =
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim";
const STIM_SURFACE_D11_R100_ATOM_LOSS_FIXTURE_PATH: &str = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim";
const STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY: f64 = 0.0003334445062;
const STIM_STYLE_SURFACE_NOISE: PerfNoiseMetadata = PerfNoiseMetadata {
    before_round_data_depolarization: 0.0,
    after_clifford_depolarization: 0.001,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
};
const STIM_STYLE_SURFACE_ATOM_LOSS_NOISE: PerfNoiseMetadata = PerfNoiseMetadata {
    before_round_data_depolarization: 0.0,
    after_clifford_depolarization: STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY,
    before_measure_flip_probability: 0.001,
    after_reset_flip_probability: 0.001,
};
const STIM_SURFACE_COMPARISONS: &[PerfComparisonKind] = &[
    PerfComparisonKind::SamplerCompiledVsInterpreted,
    PerfComparisonKind::SamplerAtomLossVsInterpreted,
];

pub fn benchmark_variants() -> Vec<PerfVariant> {
    vec![
        PerfVariant::StimCli,
        PerfVariant::RstimInterpreted,
        PerfVariant::RstimCompiled,
        PerfVariant::RstimInterpretedAtomLoss,
        PerfVariant::RstimAnalyzerFlattened,
        PerfVariant::RstimAnalyzerCompiled,
    ]
}

pub fn benchmark_case_variants(
    case: PerfBenchmarkCase,
    instrs: &[StimInstr],
) -> Result<Vec<PerfVariant>, String> {
    let compiled = compile_circuit(instrs)?;
    let variants = match case.workload {
        PerfWorkload::Sample | PerfWorkload::Detect => {
            let mut variants = vec![PerfVariant::StimCli, PerfVariant::RstimInterpreted];
            if choose_sampler_path(&compiled) == SamplerPathDecision::FastPath {
                variants.push(PerfVariant::RstimCompiled);
            }
            if case.atom_loss_variant.is_some() {
                variants.push(PerfVariant::RstimInterpretedAtomLoss);
            }
            variants
        }
        PerfWorkload::AnalyzeErrors => {
            let mut variants = vec![PerfVariant::StimCli, PerfVariant::RstimAnalyzerFlattened];
            if choose_analyzer_path(&compiled) == CompiledPathDecision::FastPath {
                variants.push(PerfVariant::RstimAnalyzerCompiled);
            }
            variants
        }
    };
    Ok(variants)
}

pub fn effective_repeat_count(instrs: &[StimInstr]) -> usize {
    effective_repeat_count_with_multiplier(instrs, 1)
}

fn effective_repeat_count_with_multiplier(instrs: &[StimInstr], multiplier: usize) -> usize {
    let mut total = 0usize;
    for instr in instrs {
        if let StimInstr::Repeat { count, body } = instr {
            let scaled = multiplier.saturating_mul(*count as usize);
            total = total.saturating_add(scaled);
            total = total.saturating_add(effective_repeat_count_with_multiplier(body, scaled));
        }
    }
    total
}

pub fn benchmark_cases() -> Vec<PerfBenchmarkCase> {
    vec![
        PerfBenchmarkCase {
            label: "rep-sample-d13-r13",
            workload: PerfWorkload::Sample,
            source: PerfCircuitSource::Generator {
                code: "repetition_code",
                task: "memory",
                distance: 13,
                rounds: 13,
                noise: 0.001,
            },
            atom_loss_variant: None,
            shots: Some(20_000),
            tier: PerfCaseTier::Gating,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: SAMPLER_COMPARE,
        },
        PerfBenchmarkCase {
            label: "surface-detect-d13-r13",
            workload: PerfWorkload::Detect,
            source: PerfCircuitSource::Generator {
                code: "surface_code",
                task: "rotated_memory_x",
                distance: 13,
                rounds: 13,
                noise: 0.001,
            },
            atom_loss_variant: None,
            shots: Some(10_000),
            tier: PerfCaseTier::Gating,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: SAMPLER_COMPARE,
        },
        PerfBenchmarkCase {
            label: "repeat-analyze-large",
            workload: PerfWorkload::AnalyzeErrors,
            source: PerfCircuitSource::Inline {
                text: "REPEAT 4096 {\n    X_ERROR(0.001) 0\n    MR 0\n    DETECTOR rec[-1]\n}\n",
            },
            atom_loss_variant: None,
            shots: None,
            tier: PerfCaseTier::Gating,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: ANALYZER_COMPARE,
        },
        PerfBenchmarkCase {
            label: "loss-protection-sample",
            workload: PerfWorkload::Sample,
            source: PerfCircuitSource::Inline {
                text: "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n",
            },
            atom_loss_variant: None,
            shots: Some(128),
            tier: PerfCaseTier::Gating,
            requires_compiled: false,
            requires_fallback: true,
            comparisons: NO_COMPARE,
        },
        PerfBenchmarkCase {
            label: "repeat-analyze-stress-report",
            workload: PerfWorkload::AnalyzeErrors,
            source: PerfCircuitSource::Inline {
                text: "REPEAT 8192 {\n    X_ERROR(0.001) 0\n    MR 0\n    DETECTOR rec[-1]\n}\n",
            },
            atom_loss_variant: None,
            shots: None,
            tier: PerfCaseTier::ReportOnly,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: ANALYZER_COMPARE,
        },
        PerfBenchmarkCase {
            label: "stim-style-surface-sample-d11-r100-b1024",
            workload: PerfWorkload::Sample,
            source: PerfCircuitSource::Fixture {
                case_id: "stim_surface_d11_r100",
                canonical_input_path: STIM_SURFACE_D11_R100_FIXTURE_PATH,
                noise: STIM_STYLE_SURFACE_NOISE,
            },
            atom_loss_variant: Some(PerfAtomLossVariant {
                source: PerfCircuitSource::Fixture {
                    case_id: "stim_surface_d11_r100_atom_loss",
                    canonical_input_path: STIM_SURFACE_D11_R100_ATOM_LOSS_FIXTURE_PATH,
                    noise: STIM_STYLE_SURFACE_ATOM_LOSS_NOISE,
                },
                per_event_probability: STIM_STYLE_ATOM_LOSS_EVENT_PROBABILITY,
                aggregate_error_probability: 0.001,
            }),
            shots: Some(1024),
            tier: PerfCaseTier::ReportOnly,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: STIM_SURFACE_COMPARISONS,
        },
    ]
}
