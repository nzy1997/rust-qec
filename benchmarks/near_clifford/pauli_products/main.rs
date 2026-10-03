//! Direct probability-query control; preparation and physics checks are untimed.
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn prepare(topology: &str, width: usize) -> Result<(ActiveState, String), String> {
    if !["star", "chain"].contains(&topology) || width < 2 {
        return Err("expected star/chain and width >= 2".into());
    }
    let mut state = ActiveState::new(width, 1);
    state.apply_clifford(CliffordGate::H(0))?;
    state.t(0)?;
    let mut circuit = "H 0\nT 0\n".to_string();
    for q in 1..width {
        let control = if topology == "chain" { q - 1 } else { 0 };
        state.apply_clifford(CliffordGate::CX(control, q))?;
        circuit.push_str(&format!("CX {control} {q}\n"));
    }
    Ok((state, circuit))
}

fn physics(topology: &str, width: usize) -> Result<Value, String> {
    let mut witnesses = Vec::new();
    for seed in [739u64, 20261005] {
        let (mut state, _) = prepare(topology, width)?;
        let mut rng = StdRng::seed_from_u64(seed);
        let mut phase = std::f64::consts::FRAC_PI_4;
        let mut steps = Vec::new();
        for q in (1..width).rev() {
            let (basis, name) = if (q + seed as usize) % 2 == 0 {
                (MeasurementBasis::X, "X")
            } else {
                (MeasurementBasis::Y, "Y")
            };
            let (zero, one) = state.measurement_probabilities(q, basis)?;
            if (zero - 0.5).abs() > 1e-10 || (one - 0.5).abs() > 1e-10 {
                return Err("GHZ intermediate conditional probability differs".into());
            }
            let outcome = state.measure(q, basis, &mut rng)?;
            steps.push(json!({"q":q,"basis":name,"zero":zero,"one":one,"outcome":outcome}));
            if basis == MeasurementBasis::Y {
                phase -= std::f64::consts::FRAC_PI_2;
            }
            if outcome {
                phase += std::f64::consts::PI;
            }
            phase = phase.rem_euclid(2.0 * std::f64::consts::PI);
        }
        let mut final_probabilities = Vec::new();
        for (basis, name, expected) in [
            (MeasurementBasis::X, "X", (1.0 + phase.cos()) / 2.0),
            (MeasurementBasis::Y, "Y", (1.0 + phase.sin()) / 2.0),
            (MeasurementBasis::Z, "Z", 0.5),
        ] {
            let (zero, one) = state.measurement_probabilities(0, basis)?;
            if (zero - expected).abs() > 1e-10 || (one - (1.0 - expected)).abs() > 1e-10 {
                return Err("GHZ final conditional probability differs".into());
            }
            final_probabilities.push(json!({"basis":name,"zero":zero,"one":one}));
        }
        witnesses.push(json!({"seed":seed,"steps":steps,"final":final_probabilities,"continuation":rng.next_u64()}));
    }
    Ok(json!(witnesses))
}

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let topology = args.first().ok_or("expected topology")?;
    let width: usize = args
        .get(1)
        .ok_or("expected width")?
        .parse()
        .map_err(|_| "invalid width")?;
    let mode = args.get(2).ok_or("expected verify/diagnose/time")?;
    let (state, circuit) = prepare(topology, width)?;
    let probability = state.measurement_probabilities(0, MeasurementBasis::Y)?;
    match mode.as_str() {
        "verify" => println!(
            "{}",
            json!({"topology":topology,"width":width,"circuit":circuit,"probability":probability,"physics":physics(topology,width)?})
        ),
        "diagnose" => {
            #[cfg(diagnostics)]
            {
                rstim::near_clifford::benchmark_pauli_reset();
                black_box(state.measurement_probabilities(0, MeasurementBasis::Y)?);
                println!(
                    "{}",
                    json!({"topology":topology,"width":width,"rank":state.active_rank(),"row_work":rstim::near_clifford::benchmark_pauli_work()})
                );
            }
            #[cfg(not(diagnostics))]
            return Err("diagnose requires the separate diagnostic build".into());
        }
        "time" => {
            let calls: usize = args
                .get(3)
                .ok_or("expected calls")?
                .parse()
                .map_err(|_| "invalid calls")?;
            let repetitions: usize = args
                .get(4)
                .ok_or("expected repetitions")?
                .parse()
                .map_err(|_| "invalid repetitions")?;
            if calls == 0 || repetitions == 0 {
                return Err("calls/repetitions must be positive".into());
            }
            for _ in 0..32 {
                black_box(state.measurement_probabilities(0, MeasurementBasis::Y)?);
            }
            let mut raw_ns = Vec::new();
            for _ in 0..repetitions {
                let start = Instant::now();
                for _ in 0..calls {
                    black_box(
                        black_box(&state).measurement_probabilities(
                            black_box(0),
                            black_box(MeasurementBasis::Y),
                        )?,
                    );
                }
                raw_ns.push(start.elapsed().as_nanos());
            }
            let mut sorted = raw_ns.clone();
            sorted.sort_unstable();
            println!(
                "{}",
                json!({"topology":topology,"width":width,"circuit":circuit,"calls":calls,"warmup_calls":32,"raw_ns":raw_ns,"median_ns":sorted[sorted.len()/2]})
            );
        }
        _ => return Err("expected verify/diagnose/time".into()),
    }
    Ok(())
}
