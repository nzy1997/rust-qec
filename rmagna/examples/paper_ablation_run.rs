//! Matched, bounded work observations for the four paper evidence ablations.

use rmagna::{
    circuit::{ParseOptions, parse_circuit},
    instrument::{CompileOptions, compile_unitary},
    noise::{RandomSource, SplitMix64},
    probability::OccurrenceThreshold,
    runtime::{CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler, RuntimeOptions},
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::CompilerStrategy,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::error::Error;

const PHYSICAL: &str = "H 0 1\nCX 1 2\nT 2\nCX 1 2\nH 1\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nPOSTSELECT det[0]";
const TARGET: &str = "I 0";
const HISTORY_DRAWS: usize = 64;
const LANE_SEED: u64 = 2026100801;
const CUTOFFS: [u64; 6] = [0, 1, 1 << 63, 0x5555_5555_5555_5555, u64::MAX - 1, u64::MAX];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn scalar(value: &ExactScalar) -> Value {
    json!({
        "coefficients": value.coefficients().each_ref().map(ToString::to_string),
        "power": value.power()
    })
}

struct RepeatedWord(u64);
impl RandomSource for RepeatedWord {
    fn next_u64(&mut self) -> u64 {
        self.0
    }
}

fn run_histories(
    name: &str,
    strategy: CausalStrategy,
    compiler: CompilerStrategy,
    sampler: HistorySampler,
) -> Result<Value, Box<dyn Error>> {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry.register("copy", "1", |bits: &[bool]| Ok(vec![bits[0]]))?;
    let source = parse_circuit(
        PHYSICAL,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )?;
    let target = compile_unitary(
        &parse_circuit(TARGET, &ParseOptions::default())?,
        &CompileOptions::default(),
        &a,
    )?;
    let program = CausalProgram::new(
        &source,
        &registry,
        RuntimeOptions {
            strategy,
            history_sampler: sampler,
            compile: CompileOptions {
                compiler_strategy: compiler,
                ..Default::default()
            },
            ..Default::default()
        },
        &a,
    )?;

    let half = a.omega(0, -2)?;
    let accepted_error = a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -3)?;
    let mut rows = Vec::with_capacity(HISTORY_DRAWS);
    let mut report_counts = [0, 0];
    for index in 0..HISTORY_DRAWS {
        // Both actual samplers see the same pair of tapes. Compare complete
        // branch contributions keyed by the reported bit, even if their tape
        // bit convention permutes the two equally probable histories.
        let word = if index % 2 == 0 {
            0
        } else {
            0xc000_0000_0000_0001
        };
        let draw = program.sample(&[1], &target, &mut RepeatedWord(word), &a)?;
        let [call] = draw.controller_calls.as_slice() else {
            return Err("unexpected number of actual controller calls".into());
        };
        let [reported] = call.inputs.as_slice() else {
            return Err("unexpected reported syndrome width".into());
        };
        let reported = *reported;
        let zero = ExactScalar::zero();
        let expected_accepted = if reported { &zero } else { &half };
        let expected_error = if reported { &zero } else { &accepted_error };
        if call.name != "copy"
            || call.outputs != [reported]
            || &draw.masses.accepted.value != expected_accepted
            || &draw.masses.error != expected_error
            || draw.history_mass != half
            || draw
                .history_proposal_mass
                .as_ref()
                .is_some_and(|mass| mass != &half)
            || (sampler == HistorySampler::AffineSupport) != draw.history_proposal_mass.is_some()
            || draw.noise_probability.to_string() != "1"
            || draw.noise_likelihood_ratio.to_string() != "1"
        {
            return Err(
                format!("changed exact masses or callback for {name} on tape {index}").into(),
            );
        }
        report_counts[usize::from(reported)] += 1;
        rows.push(json!({
            "index": index, "tape": format!("{word:016x}"), "reported": reported, "callback": call.outputs,
            "history": scalar(&draw.history_mass), "proposal": draw.history_proposal_mass.as_ref().map(scalar),
            "accepted": scalar(&draw.masses.accepted.value), "error": scalar(&draw.masses.error),
            "history_queries": draw.history_queries, "network_compilations": draw.network_compilations,
            "history_words": draw.history_entropy_words, "noise_words": draw.noise_entropy_words
        }));
    }
    if report_counts != [HISTORY_DRAWS / 2; 2] {
        return Err(
            format!("tapes did not cover both branches for {name}: {report_counts:?}").into(),
        );
    }
    let shape = program.compilation();
    if shape.strategy != compiler || program.strategy() != strategy {
        return Err(format!("changed compiler or causal route for {name}").into());
    }
    Ok(json!({
        "name": name, "causal_strategy": format!("{strategy:?}"),
        "compiler": format!("{compiler:?}"), "history_sampler": format!("{sampler:?}"),
        "reference_compilations": program.reference_compilations(),
        "path_variables": shape.path_variables,
        "clifford_gates": shape.clifford_gates,
        "phase_ancillas": shape.phase_ancillas,
        "shared_phase_coordinates": shape.shared_phase_coordinates,
        "rows": rows
    }))
}

fn finite_occurrence() -> Result<Vec<Value>, Box<dyn Error>> {
    let mut random = SplitMix64::new(LANE_SEED);
    let mut rows = Vec::new();
    for (cutoff_index, cutoff) in CUTOFFS.into_iter().enumerate() {
        for mask in [u64::MAX, 0xa55a_5aa5_f00f_6996] {
            let mut words: [u64; 64] = std::array::from_fn(|_| random.next_u64());
            words[..4].copy_from_slice(&[0, cutoff.wrapping_sub(1), cutoff, u64::MAX]);
            let mut planes = [0_u64; 64];
            for (lane, word) in words.iter().enumerate() {
                for (bit, plane) in planes.iter_mut().enumerate() {
                    *plane |= (word >> (63 - bit) & 1) << lane;
                }
            }

            // Both representations use identical conceptual words and the
            // same finite threshold. The operation counts exclude transposition.
            let threshold = OccurrenceThreshold::Below(cutoff);
            let mut scalar_calls = 0;
            let mut scalar = 0;
            for (lane, word) in words.iter().enumerate() {
                if (mask >> lane) & 1 != 0
                    && threshold.draw_with(&mut || {
                        scalar_calls += 1;
                        *word
                    })
                {
                    scalar |= 1_u64 << lane;
                }
            }
            let mut packed_calls = 0;
            let packed = threshold.draw_bits_with(mask, &mut || {
                let word = planes[packed_calls];
                packed_calls += 1;
                word
            });
            if scalar != packed {
                return Err("scalar and packed occurrence decisions differ".into());
            }
            rows.push(json!({
                "cutoff_index": cutoff_index, "cutoff": format!("{cutoff:016x}"),
                "live": format!("{mask:016x}"),
                "words": words.iter().map(|word| format!("{word:016x}")).collect::<Vec<_>>(),
                "scalar": format!("{scalar:016x}"), "packed": format!("{packed:016x}"),
                "scalar_entropy_words": scalar_calls, "packed_entropy_planes": packed_calls
            }));
        }
    }
    Ok(rows)
}

fn main() -> Result<(), Box<dyn Error>> {
    use CausalStrategy::{ColdPrefix, FixedTemplate};
    use CompilerStrategy::{ParityCopies, SharedCommutingLayer};
    use HistorySampler::{AffineSupport, ExactMarginal};

    if cfg!(debug_assertions) {
        return Err("build the paper work probe in release mode".into());
    }
    let histories = [
        run_histories(
            "baseline",
            FixedTemplate,
            SharedCommutingLayer,
            ExactMarginal,
        )?,
        run_histories("cold", ColdPrefix, SharedCommutingLayer, ExactMarginal)?,
        run_histories("parity_copies", FixedTemplate, ParityCopies, ExactMarginal)?,
        run_histories("affine", FixedTemplate, SharedCommutingLayer, AffineSupport)?,
    ];
    let executable = std::fs::read(std::env::current_exe()?)?;
    println!(
        "{}",
        json!({
            "schema": "rmagna.paper-ablation-run.v1", "status": "complete",
            "rust_source_sha256": env!("RMAGNA_PAPER_SOURCE_SHA256"),
            "executable_sha256": sha(&executable),
            "build": {"os": std::env::consts::OS, "architecture": std::env::consts::ARCH},
            "physical_sha256": sha(PHYSICAL.as_bytes()), "target_sha256": sha(TARGET.as_bytes()),
            "lane_seed": LANE_SEED, "histories": histories, "finite_occurrence": finite_occurrence()?
        })
    );
    Ok(())
}
