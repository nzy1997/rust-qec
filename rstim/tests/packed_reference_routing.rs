use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::CompiledMeasurementSampler;
use rstim::compiled::{
    SamplerPathDecision, SamplingFallbackReason, choose_sampler_path, compile_circuit,
};
use rstim::data_path::{
    ReferenceSampleDecision, ReferenceSampleMode, build_reference_sample_with_decision,
    build_reference_sample_with_sweep_bits_and_decision,
};
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use rstim::sampler::{
    BatchOutput, SampleBatchDecision, SampleOptions, SampleOutputMode, SamplingBackend,
    sample_batch_with_options_and_decision, sample_batch_with_options_sweep_bits_and_decision,
};
use rstim::sim::bit_table::BitTable;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);
const SURFACE_D11_R100_BYTES: &[u8] = include_bytes!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);
const SURFACE_D11_R100_SHA256: &str =
    "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229";
const SWEEP_TRUE: &[bool] = &[true];

#[derive(Clone, Copy, Debug)]
enum ExpectedReason {
    Loss,
    MeasurementRecordFeedback,
    SweepDependent,
    UnsupportedOperation(&'static str),
}

struct FallbackCase {
    name: &'static str,
    circuit: &'static str,
    sweep_bits: Option<&'static [bool]>,
    expected_bits: &'static [bool],
    expected_reason: ExpectedReason,
}

fn fallback_cases() -> [FallbackCase; 5] {
    [
        FallbackCase {
            name: "loss",
            circuit: "LOSS(0) 0\nM 0\n",
            sweep_bits: None,
            expected_bits: &[false],
            expected_reason: ExpectedReason::Loss,
        },
        FallbackCase {
            name: "measurement record feedback",
            circuit: "X 0\nM 0\nCX rec[-1] 1\nM 1\n",
            sweep_bits: None,
            expected_bits: &[true, true],
            expected_reason: ExpectedReason::MeasurementRecordFeedback,
        },
        FallbackCase {
            name: "measurement record feedback in mixed pair list",
            circuit: "X 0\nM 0\nCX rec[-1] 1 2 3\nM 1 3\n",
            sweep_bits: None,
            expected_bits: &[true, true, false],
            expected_reason: ExpectedReason::MeasurementRecordFeedback,
        },
        FallbackCase {
            name: "sweep dependent",
            circuit: "X 1\nCX sweep[0] 1\nM 1\n",
            sweep_bits: Some(SWEEP_TRUE),
            expected_bits: &[false],
            expected_reason: ExpectedReason::SweepDependent,
        },
        FallbackCase {
            name: "unsupported operation",
            circuit: "H 1\nX 0\nCZ 0 1\nH 1\nM 0 1\n",
            sweep_bits: None,
            expected_bits: &[true, true],
            expected_reason: ExpectedReason::UnsupportedOperation("CZ"),
        },
    ]
}

fn parse_circuit(source: &str) -> Vec<StimInstr> {
    parse_lines(source).expect("test circuit parses")
}

fn build_reference_for_case(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> (Vec<bool>, ReferenceSampleDecision) {
    let result = match sweep_bits {
        Some(bits) => build_reference_sample_with_sweep_bits_and_decision(instrs, Some(bits))
            .expect("reference sample with sweep bits builds"),
        None => build_reference_sample_with_decision(instrs).expect("reference sample builds"),
    };
    (result.bits, result.decision)
}

fn sample_auto_for_case(
    instrs: &[StimInstr],
    shots: usize,
    sweep_bits: Option<&[bool]>,
) -> (BatchOutput, SampleBatchDecision) {
    let mut rng = StdRng::seed_from_u64(458);
    let options = SampleOptions {
        backend: SamplingBackend::Auto,
        output_mode: SampleOutputMode::MeasurementsOnly,
        ..SampleOptions::default()
    };

    match sweep_bits {
        Some(bits) => sample_batch_with_options_sweep_bits_and_decision(
            instrs,
            shots,
            &mut rng,
            options,
            Some(bits),
        )
        .expect("auto sampler with sweep bits succeeds"),
        None => sample_batch_with_options_and_decision(instrs, shots, &mut rng, options)
            .expect("auto sampler succeeds"),
    }
}

fn assert_sampling_reason(actual: &SamplingFallbackReason, expected: ExpectedReason) {
    match (actual, expected) {
        (SamplingFallbackReason::Loss, ExpectedReason::Loss) => {}
        (
            SamplingFallbackReason::MeasurementRecordFeedback,
            ExpectedReason::MeasurementRecordFeedback,
        ) => {}
        (SamplingFallbackReason::SweepDependent, ExpectedReason::SweepDependent) => {}
        (
            SamplingFallbackReason::UnsupportedOperation(name),
            ExpectedReason::UnsupportedOperation(expected_name),
        ) => {
            assert_eq!(name.to_string(), expected_name);
        }
        _ => panic!("expected {expected:?}, got {actual:?}"),
    }
}

fn assert_legacy_reference_decision(actual: &ReferenceSampleDecision, expected: ExpectedReason) {
    match actual {
        ReferenceSampleDecision::LegacyFallback(reason) => {
            assert_sampling_reason(reason, expected);
        }
        ReferenceSampleDecision::PackedInverse => {
            panic!("expected legacy reference fallback decision");
        }
    }
}

fn assert_interpreted_legacy_decision(actual: &SampleBatchDecision, expected: ExpectedReason) {
    match actual {
        SampleBatchDecision::InterpretedLegacy(reason) => {
            assert_sampling_reason(reason, expected);
        }
        _ => panic!("expected interpreted legacy sample-batch decision"),
    }
}

fn assert_packed_reference_decision(actual: &ReferenceSampleDecision) {
    match actual {
        ReferenceSampleDecision::PackedInverse => {}
        ReferenceSampleDecision::LegacyFallback(_) => {
            panic!("expected packed inverse reference decision");
        }
    }
}

fn assert_measurements_match_bits(
    output: &BatchOutput,
    expected_bits: &[bool],
    shots: usize,
    label: &str,
) {
    assert_bit_table_matches_rows(&output.measurements, expected_bits, shots, label);
    assert_eq!(output.output_mode, SampleOutputMode::MeasurementsOnly);
}

fn assert_bit_table_matches_rows(
    table: &BitTable,
    expected_bits: &[bool],
    shots: usize,
    label: &str,
) {
    assert_eq!(
        table.num_major(),
        expected_bits.len(),
        "{label}: measurement count"
    );
    assert_eq!(table.num_minor(), shots, "{label}: shot count");

    for shot in 0..shots {
        for (measurement, expected_bit) in expected_bits.iter().copied().enumerate() {
            assert_eq!(
                table.get(measurement, shot),
                expected_bit,
                "{label}: measurement {measurement}, shot {shot}"
            );
        }
    }
}

fn assert_all_false(bits: &[bool], label: &str) {
    assert!(
        bits.iter().all(|bit| !*bit),
        "{label}: expected every reference bit to be false"
    );
}

#[test]
fn noiseless_sweep_targets_do_not_force_sweep_fallback() {
    let instrs = parse_circuit(
        "X_ERROR(0) sweep[0]\nY_ERROR(0) sweep[1]\nZ_ERROR(0) sweep[2]\nDEPOLARIZE1(0) sweep[3]\nDEPOLARIZE2(0) sweep[4] sweep[5]\nM 0\n",
    );
    let compiled = compile_circuit(&instrs).expect("compiled circuit builds");
    assert_eq!(
        choose_sampler_path(&compiled),
        SamplerPathDecision::FastPath
    );

    let reference = build_reference_sample_with_decision(&instrs)
        .expect("reference sample skips noiseless sweep-target noise");
    assert_packed_reference_decision(&reference.decision);
    assert_eq!(reference.bits, vec![false]);
}

#[test]
fn packed_reference_covers_y_and_batched_reset_branches() {
    let y_reference = build_reference_sample_with_decision(&parse_circuit("Y 0\nM 0\n"))
        .expect("Y reference sample builds");
    assert_packed_reference_decision(&y_reference.decision);
    assert_eq!(y_reference.bits, vec![true]);

    let measure_reset_reference =
        build_reference_sample_with_decision(&parse_circuit("X 0 1\nMR 0 1\nM 0 1\n"))
            .expect("batched measure-reset reference sample builds");
    assert_packed_reference_decision(&measure_reset_reference.decision);
    assert_eq!(measure_reset_reference.bits, vec![true, true, false, false]);

    let reset_reference =
        build_reference_sample_with_decision(&parse_circuit("X 0 1\nR 0 1\nM 0 1\n"))
            .expect("batched reset reference sample builds");
    assert_packed_reference_decision(&reset_reference.decision);
    assert_eq!(reset_reference.bits, vec![false, false]);
}

#[test]
fn packed_reference_invalid_targets_report_legacy_errors() {
    let odd_target_err = build_reference_sample_with_decision(&parse_circuit("CX 0\n"))
        .expect_err("odd pair count should fail legacy construction too");
    assert!(
        odd_target_err.contains("odd"),
        "unexpected odd target error: {odd_target_err}"
    );

    let non_qubit_pair_err = build_reference_sample_with_decision(&parse_circuit("CX 0 rec[-1]\n"))
        .expect_err("non-qubit pair should fail legacy construction too");
    assert!(
        non_qubit_pair_err.contains("expected qubit target in pair"),
        "unexpected target error: {non_qubit_pair_err}"
    );
}

#[test]
fn auto_full_output_uses_sweep_table_for_interpreted_legacy_recovery() {
    let instrs =
        parse_circuit("CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
    let mut rng = StdRng::seed_from_u64(458);
    let options = SampleOptions {
        backend: SamplingBackend::Auto,
        output_mode: SampleOutputMode::Full,
        ..SampleOptions::default()
    };

    let (output, decision) = sample_batch_with_options_sweep_bits_and_decision(
        &instrs,
        2,
        &mut rng,
        options,
        Some(SWEEP_TRUE),
    )
    .expect("auto sweep fallback with full output succeeds");

    assert_interpreted_legacy_decision(&decision, ExpectedReason::SweepDependent);
    assert_bit_table_matches_rows(&output.measurements, &[true], 2, "sweep full output");
    assert_bit_table_matches_rows(&output.detections, &[false], 2, "sweep detector output");
    assert_bit_table_matches_rows(
        &output.observable_flips,
        &[false],
        2,
        "sweep observable output",
    );
}

#[test]
fn auto_feedback_recovery_handles_cy_and_cz_pair_lists() {
    let cy_instrs = parse_circuit("X 0\nM 0\nCY rec[-1] 1\nM 1\n");
    let (cy_output, cy_decision) = sample_auto_for_case(&cy_instrs, 2, None);
    assert_interpreted_legacy_decision(&cy_decision, ExpectedReason::MeasurementRecordFeedback);
    assert_measurements_match_bits(&cy_output, &[true, true], 2, "CY feedback");

    let cz_instrs = parse_circuit("X 0\nM 0\nH 1\nCZ rec[-1] 1\nMX 1\n");
    let (cz_output, cz_decision) = sample_auto_for_case(&cz_instrs, 2, None);
    assert_interpreted_legacy_decision(&cz_decision, ExpectedReason::MeasurementRecordFeedback);
    assert_measurements_match_bits(&cz_output, &[true, true], 2, "CZ feedback");
}

#[test]
fn sampler_fallback_reason_messages_remain_stable() {
    assert_eq!(
        SamplingFallbackReason::SweepDependent.to_string(),
        "sweep-dependent instructions require the interpreted path"
    );
}

#[test]
fn fallback_routing_reports_typed_reason_at_each_layer() {
    for case in fallback_cases() {
        let instrs = parse_circuit(case.circuit);

        let (reference_bits, reference_decision) =
            build_reference_for_case(&instrs, case.sweep_bits);
        assert_eq!(
            reference_bits, case.expected_bits,
            "{} reference",
            case.name
        );
        assert_legacy_reference_decision(&reference_decision, case.expected_reason);

        let compile_err = CompiledMeasurementSampler::compile_with_decision(
            &instrs,
            ReferenceSampleMode::SimulateNoiseless,
        )
        .err()
        .expect("direct compiled sampler construction should reject fallback circuit");
        assert_sampling_reason(&compile_err, case.expected_reason);

        let shots = 3;
        let (auto_output, auto_decision) = sample_auto_for_case(&instrs, shots, case.sweep_bits);
        assert_measurements_match_bits(&auto_output, &reference_bits, shots, case.name);
        assert_interpreted_legacy_decision(&auto_decision, case.expected_reason);
    }
}

#[test]
fn supported_reference_construction_uses_packed_inverse() {
    assert_eq!(sha256_hex(SURFACE_D11_R100_BYTES), SURFACE_D11_R100_SHA256);

    let surface_instrs = parse_circuit(SURFACE_D11_R100);
    let surface_reference = build_reference_sample_with_decision(&surface_instrs)
        .expect("surface fixture reference sample builds");
    let surface_bits = surface_reference.bits;
    let surface_decision = surface_reference.decision;
    assert_packed_reference_decision(&surface_decision);
    assert_eq!(surface_bits.len(), 12_121);
    assert_all_false(&surface_bits, "surface fixture");

    let nested_instrs = parse_circuit(
        "REPEAT 2 {\n  H 0\n  REPEAT 3 {\n    M 0\n    X_ERROR(0) 0\n    Y_ERROR(0) 0\n    Z_ERROR(0) 0\n    DEPOLARIZE1(0) 0\n    DEPOLARIZE2(0) 0 1\n    PAULI_CHANNEL_1(0,0,0) 0\n    PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\n    CORRELATED_ERROR(0) X0\n    E(0) Z0\n    ELSE_CORRELATED_ERROR(0) Y0\n    TICK\n    DETECTOR rec[-1]\n    OBSERVABLE_INCLUDE(0) rec[-1]\n  }\n  H 0\n}\n",
    );
    let nested_reference = build_reference_sample_with_decision(&nested_instrs)
        .expect("nested repeat reference sample builds");
    let nested_bits = nested_reference.bits;
    let nested_decision = nested_reference.decision;
    assert_packed_reference_decision(&nested_decision);
    assert_eq!(rstim::stats::num_measurements(&nested_instrs), 6);
    assert_eq!(nested_bits.len(), 6);
    assert_all_false(&nested_bits, "nested repeat");

    println!("PASS packed reference routing");
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = sha256(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("write hex");
    }
    out
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h = H0;
    let bit_len = (bytes.len() as u64) * 8;
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let start = i * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for i in 16..64 {
            w[i] = small_sigma1(w[i - 2])
                .wrapping_add(w[i - 7])
                .wrapping_add(small_sigma0(w[i - 15]))
                .wrapping_add(w[i - 16]);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let t1 = hh
                .wrapping_add(big_sigma1(e))
                .wrapping_add(ch(e, f, g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = big_sigma0(a).wrapping_add(maj(a, b, c));
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

fn big_sigma0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

fn big_sigma1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

fn small_sigma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

fn small_sigma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}
