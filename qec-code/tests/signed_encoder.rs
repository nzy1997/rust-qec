//! Independent, tiny literal state-vector oracle; no production phase replay.
use qec_code::Pauli;
use qec_code::phased_pauli::{Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup};
use qec_code::signed_encoder::{
    EncoderError, EncoderGate, EncoderLimits, MAX_ENCODER_GENERATORS, MAX_ENCODER_QUBITS,
    SignedCliffordEncoder, synthesize_signed_encoder,
};

#[derive(Clone, Copy, Debug, Default)]
struct C(f64, f64);
impl std::ops::Add for C {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0, self.1 + rhs.1)
    }
}
impl std::ops::Mul for C {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(
            self.0 * rhs.0 - self.1 * rhs.1,
            self.0 * rhs.1 + self.1 * rhs.0,
        )
    }
}

fn word(s: &str, negative: bool) -> PhasedPauli {
    let x = s
        .bytes()
        .map(|b| u8::from(b == b'X' || b == b'Y'))
        .collect();
    let z = s
        .bytes()
        .map(|b| u8::from(b == b'Z' || b == b'Y'))
        .collect();
    let phase = s.bytes().filter(|&b| b == b'Y').count() as u8 + 2 * u8::from(negative);
    PhasedPauli::new(
        Pauli::from_xz_bits(x, z).unwrap(),
        Phase::from_exponent(phase),
    )
}

fn basis(n: usize, index: usize) -> Vec<C> {
    assert!(n <= 4, "test oracle is deliberately limited to four wires");
    let mut v = vec![C::default(); 1 << n];
    v[index] = C(1., 0.);
    v
}

fn gates(mut v: Vec<C>, circuit: &[EncoderGate]) -> Vec<C> {
    for &gate in circuit {
        match gate {
            EncoderGate::H(q) => {
                for i in 0..v.len() {
                    if i & (1 << q) == 0 {
                        let j = i | (1 << q);
                        let (a, b) = (v[i], v[j]);
                        v[i] = (a + b) * C(std::f64::consts::FRAC_1_SQRT_2, 0.);
                        v[j] = (a + b * C(-1., 0.)) * C(std::f64::consts::FRAC_1_SQRT_2, 0.);
                    }
                }
            }
            EncoderGate::S(q) | EncoderGate::Sdg(q) => {
                let phase = if matches!(gate, EncoderGate::S(_)) {
                    C(0., 1.)
                } else {
                    C(0., -1.)
                };
                for (i, a) in v.iter_mut().enumerate() {
                    if i & (1 << q) != 0 {
                        *a = *a * phase;
                    }
                }
            }
            EncoderGate::Cx { control, target } => {
                for i in 0..v.len() {
                    if i & (1 << control) != 0 && i & (1 << target) == 0 {
                        v.swap(i, i | (1 << target));
                    }
                }
            }
        }
    }
    v
}

fn pauli(v: &[C], p: &PhasedPauli) -> Vec<C> {
    let phase = [C(1., 0.), C(0., 1.), C(-1., 0.), C(0., -1.)][p.phase().exponent() as usize];
    let mut out = vec![C::default(); v.len()];
    for (i, &a) in v.iter().enumerate() {
        let mut target = i;
        let mut sign = 1.;
        for q in 0..p.n() {
            if p.support().z_bits()[q] != 0 && i & (1 << q) != 0 {
                sign = -sign;
            }
            if p.support().x_bits()[q] != 0 {
                target ^= 1 << q;
            }
        }
        out[target] = a * phase * C(sign, 0.);
    }
    out
}

fn close(a: &[C], b: &[C]) -> bool {
    a.iter()
        .zip(b)
        .all(|(a, b)| (a.0 - b.0).abs() < 1e-11 && (a.1 - b.1).abs() < 1e-11)
}

fn standard(n: usize, q: usize, axis: char) -> PhasedPauli {
    let mut s = vec!['I'; n];
    s[q] = axis;
    word(&s.into_iter().collect::<String>(), false)
}

fn images_match(encoder: &SignedCliffordEncoder, circuit: &[EncoderGate]) -> bool {
    let n = encoder.n();
    for i in 0..1 << n {
        let input = basis(n, i);
        let output = gates(input.clone(), circuit);
        for (axis, images) in [
            ('X', encoder.input_x_images()),
            ('Z', encoder.input_z_images()),
        ] {
            for (q, image) in images.iter().enumerate() {
                if !close(
                    &gates(pauli(&input, &standard(n, q, axis)), circuit),
                    &pauli(&output, image),
                ) {
                    return false;
                }
            }
        }
    }
    true
}

fn check(
    encoder: &SignedCliffordEncoder,
    group: &SignedStabilizerGroup,
    xs: &[PhasedPauli],
    zs: &[PhasedPauli],
) {
    let (n, k) = (encoder.n(), encoder.k());
    assert_eq!(&encoder.input_x_images()[..k], xs);
    assert_eq!(&encoder.input_z_images()[..k], zs);
    assert_eq!(encoder.stabilizer_witnesses().len(), n - k);
    for (image, witness) in encoder.input_z_images()[k..]
        .iter()
        .zip(encoder.stabilizer_witnesses())
    {
        assert_eq!(
            group
                .product_from_indices(witness.generator_indices())
                .unwrap(),
            *image
        );
    }
    // Check every exported signed X/Z image on the ENTIRE tiny physical space.
    assert!(images_match(encoder, encoder.gates()));
    for i in 0..1 << n {
        let input = basis(n, i);
        let output = gates(input.clone(), encoder.gates());
        assert!(close(&gates(output, &encoder.inverse_gates()), &input));
    }
    // Test the actual encoding subspace, retaining every logical input column.
    for i in 0..1 << k {
        let output = gates(basis(n, i), encoder.gates());
        for generator in group.generators() {
            assert!(close(&output, &pauli(&output, generator)));
        }
    }
}

#[test]
fn independent_oracle_rejects_phase_direction_and_missing_gate_mutations() {
    let group = SignedStabilizerGroup::new(3, vec![word("XZI", true), word("IZZ", false)]).unwrap();
    let e = synthesize_signed_encoder(
        &group,
        &[word("ZYX", true)],
        &[word("XII", false)],
        EncoderLimits::default(),
    )
    .unwrap();
    let mut phase = e.gates().to_vec();
    let p = phase
        .iter()
        .position(|g| matches!(g, EncoderGate::S(_) | EncoderGate::Sdg(_)))
        .unwrap();
    phase[p] = phase[p].inverse();
    assert!(!images_match(&e, &phase));
    let mut direction = e.gates().to_vec();
    let p = direction
        .iter()
        .position(|g| matches!(g, EncoderGate::Cx { .. }))
        .unwrap();
    if let EncoderGate::Cx { control, target } = direction[p] {
        direction[p] = EncoderGate::Cx {
            control: target,
            target: control,
        };
    }
    assert!(!images_match(&e, &direction));
    let mut missing = e.gates().to_vec();
    missing.remove(0);
    assert!(!images_match(&e, &missing));
}

#[test]
fn mixed_y_signed_code_preserves_unknown_logical_input_and_full_images() {
    let group = SignedStabilizerGroup::new(3, vec![word("XZI", true), word("IZZ", false)]).unwrap();
    let xs = [word("ZYX", true)];
    let zs = [word("XII", false)];
    let e = synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()).unwrap();
    check(&e, &group, &xs, &zs);
    assert_eq!(
        e.gates(),
        synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default())
            .unwrap()
            .gates()
    );
    // A wrong target sign is observable; no per-column phase equivalence is used.
    let output = gates(basis(3, 0), e.gates());
    assert!(!close(
        &pauli(&output, &xs[0]),
        &pauli(&output, &word("ZYX", false))
    ));
}

#[test]
fn all_logical_ports_and_redundant_generators_are_retained() {
    let group = SignedStabilizerGroup::new(
        4,
        vec![
            word("IIII", false),
            word("ZZII", true),
            word("ZZII", true),
            word("IIZZ", false),
            word("ZZZZ", true),
        ],
    )
    .unwrap();
    let xs = [word("XXII", true), word("IIXX", false)];
    let zs = [word("ZIII", false), word("IIZI", true)];
    let e = synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()).unwrap();
    assert_eq!(e.k(), 2);
    check(&e, &group, &xs, &zs);
}

#[test]
fn three_arbitrary_logical_ports_and_explicit_wire_swaps_are_supported() {
    let group = SignedStabilizerGroup::new(4, vec![word("XXXX", false)]).unwrap();
    let xs = [word("XIII", true), word("IXII", false), word("IIXI", true)];
    let zs = [word("ZIIZ", false), word("IZIZ", true), word("IIZZ", false)];
    let e = synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()).unwrap();
    assert_eq!(e.k(), 3);
    check(&e, &group, &xs, &zs);

    // The only logical port is physically on wire1, so no free layout swap.
    let group = SignedStabilizerGroup::new(2, vec![word("ZI", false)]).unwrap();
    let xs = [word("IX", false)];
    let zs = [word("IZ", false)];
    let e = synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()).unwrap();
    assert_eq!(e.gates().len(), 3);
    assert!(
        e.gates()
            .iter()
            .all(|g| matches!(g, EncoderGate::Cx { .. }))
    );
    check(&e, &group, &xs, &zs);
}

#[test]
fn signed_single_wire_frames_and_zero_logical_states() {
    for (x, z) in [
        ("X", "Z"),
        ("Y", "Z"),
        ("Z", "X"),
        ("X", "Y"),
        ("Y", "X"),
        ("Z", "Y"),
    ] {
        for sx in [false, true] {
            for sz in [false, true] {
                let group = SignedStabilizerGroup::new(1, vec![]).unwrap();
                let xs = [word(x, sx)];
                let zs = [word(z, sz)];
                let e =
                    synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()).unwrap();
                check(&e, &group, &xs, &zs);
            }
        }
    }
    let group = SignedStabilizerGroup::new(2, vec![word("YI", true), word("IX", false)]).unwrap();
    let e = synthesize_signed_encoder(&group, &[], &[], EncoderLimits::default()).unwrap();
    check(&e, &group, &[], &[]);
    let empty = SignedStabilizerGroup::new(0, vec![]).unwrap();
    let e = synthesize_signed_encoder(&empty, &[], &[], EncoderLimits { max_gates: 0 }).unwrap();
    check(&e, &empty, &[], &[]);
    assert!(e.gates().is_empty());
}

#[test]
fn supplied_logicals_are_revalidated_against_the_actual_group() {
    let group = SignedStabilizerGroup::new(2, vec![word("ZI", false)]).unwrap();
    for (xs, zs) in [
        (vec![], vec![]),
        (vec![word("X", false)], vec![word("Z", false)]),
        (vec![word("IX", false)], vec![word("IX", false)]),
        (vec![word("XI", false)], vec![word("IZ", false)]),
    ] {
        assert!(matches!(
            synthesize_signed_encoder(&group, &xs, &zs, EncoderLimits::default()),
            Err(EncoderError::InvalidFrame(_))
        ));
    }
    let anti_hermitian = PhasedPauli::new(word("IX", false).into_support(), Phase::PlusI);
    assert!(matches!(
        synthesize_signed_encoder(
            &group,
            &[anti_hermitian],
            &[word("IZ", false)],
            EncoderLimits::default()
        ),
        Err(EncoderError::InvalidFrame(
            PhaseAlgebraError::NonHermitianLogical { .. }
        ))
    ));
    assert!(SignedStabilizerGroup::new(1, vec![word("X", false), word("X", true)]).is_err());
    assert!(SignedStabilizerGroup::new(1, vec![word("X", false), word("Z", false)]).is_err());
}

#[test]
fn limits_are_enforced_before_export_and_exact_gate_budget_is_accepted() {
    let wide = SignedStabilizerGroup::new(MAX_ENCODER_QUBITS + 1, vec![]).unwrap();
    assert!(matches!(
        synthesize_signed_encoder(
            &wide,
            &[],
            &[],
            EncoderLimits {
                max_gates: usize::MAX
            }
        ),
        Err(EncoderError::QubitLimit { .. })
    ));
    let redundant =
        SignedStabilizerGroup::new(1, vec![word("Z", false); MAX_ENCODER_GENERATORS + 1]).unwrap();
    assert!(matches!(
        synthesize_signed_encoder(&redundant, &[], &[], EncoderLimits::default()),
        Err(EncoderError::GeneratorLimit { .. })
    ));
    let group = SignedStabilizerGroup::new(1, vec![word("Y", true)]).unwrap();
    let e = synthesize_signed_encoder(&group, &[], &[], EncoderLimits::default()).unwrap();
    let count = e.gates().len();
    assert!(count > 0);
    assert!(matches!(
        synthesize_signed_encoder(
            &group,
            &[],
            &[],
            EncoderLimits {
                max_gates: count - 1
            }
        ),
        Err(EncoderError::GateLimit { .. })
    ));
    assert_eq!(
        e.gates(),
        synthesize_signed_encoder(&group, &[], &[], EncoderLimits { max_gates: count })
            .unwrap()
            .gates()
    );
}
