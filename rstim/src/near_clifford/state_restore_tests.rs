use super::*;

fn bits(values: &[ComplexAmp]) -> Vec<(u64, u64)> {
    values
        .iter()
        .map(|v| (v.re.to_bits(), v.im.to_bits()))
        .collect()
}

#[test]
fn restores_are_reused_only_while_the_arithmetic_buffer_is_unchanged() {
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(
            "H 0\nT 0\nMX 0\n",
            arithmetic,
        )
        .unwrap();
        let mut sampler = plan.prepare_sampler().unwrap();
        let start = sampler.cache.as_ref().unwrap().start;
        let expected = bits(&plan.initial_coefficients);
        sampler.load_state(start).unwrap();
        sampler.load_state(start).unwrap();
        assert_eq!(
            sampler.state_restores, 1,
            "the same unchanged input needs one restore"
        );
        assert_eq!(bits(&sampler.coefficients), expected);

        let (p, expand, dagger) = plan
            .operations
            .iter()
            .find_map(|op| match op {
                PlanOp::Rotate {
                    pauli,
                    expand,
                    dagger,
                } => Some((pauli, *expand, *dagger)),
                _ => None,
            })
            .unwrap();
        sampler.rotate_signed(p, expand, dagger, false).unwrap();
        assert_ne!(bits(&sampler.coefficients), expected);
        sampler.load_state(start).unwrap();
        assert_eq!(sampler.state_restores, 2);
        assert_eq!(bits(&sampler.coefficients), expected);

        let m = plan
            .operations
            .iter()
            .find_map(|op| match op {
                PlanOp::Measure(m) if matches!(m.projection, Projection::Active { .. }) => Some(m),
                _ => None,
            })
            .unwrap();
        let Projection::Active {
            index, y, offset, ..
        } = m.projection
        else {
            unreachable!()
        };
        sampler.project(&m.pauli, index, y, offset).unwrap();
        sampler.load_state(start).unwrap();
        assert_eq!(sampler.state_restores, 3);
        assert_eq!(bits(&sampler.coefficients), expected);
        sampler.load_state(0).unwrap();
        assert_eq!(bits(&sampler.coefficients), vec![(1.0f64.to_bits(), 0)]);
        sampler.load_state(start).unwrap();
        assert_eq!(
            sampler.state_restores, 5,
            "a different loaded state must restore the old input"
        );
        assert_eq!(bits(&sampler.coefficients), expected);
    }
}

#[test]
fn restored_cache_bits_include_signed_zeros_and_nan_payloads() {
    let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMX 0\n").unwrap();
    let mut sampler = plan.prepare_sampler().unwrap();
    let values = [
        ComplexAmp::new(-0.0, 0.0),
        ComplexAmp::new(f64::from_bits(0x7ff8_0000_0000_1234), -0.0),
        ComplexAmp::new(f64::from_bits(1), f64::NEG_INFINITY),
    ];
    let id = sampler.cache.as_mut().unwrap().store(31, &values).unwrap();
    sampler.load_state(id).unwrap();
    sampler.load_state(id).unwrap();
    assert_eq!(sampler.state_restores, 1);
    assert_eq!(bits(&sampler.coefficients), bits(&values));
    sampler.load_state(0).unwrap();
    sampler.load_state(id).unwrap();
    assert_eq!(sampler.state_restores, 3);
    assert_eq!(bits(&sampler.coefficients), bits(&values));
}

#[test]
fn a_failed_projection_does_not_leave_a_materialized_state_hint() {
    let plan = CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nMX 0\n").unwrap();
    let mut sampler = plan.prepare_sampler().unwrap();
    let values = [ComplexAmp::new(1., 0.), ComplexAmp::new(0., -0.)];
    let id = sampler.cache.as_mut().unwrap().store(31, &values).unwrap();
    let m = plan
        .operations
        .iter()
        .find_map(|op| match op {
            PlanOp::Measure(m) if matches!(m.projection, Projection::Active { .. }) => Some(m),
            _ => None,
        })
        .unwrap();
    let Projection::Active { index, y, .. } = m.projection else {
        unreachable!()
    };
    assert_eq!((m.pauli.x, m.pauli.z, index, y), (0, 1, 0, false));
    sampler.load_state(id).unwrap();
    assert_eq!(
        sampler.project(&m.pauli, index, y, true).unwrap_err(),
        "compiled near-Clifford zero-probability measurement"
    );
    assert_eq!(sampler.loaded_state, usize::MAX);
    sampler.load_state(id).unwrap();
    assert_eq!(sampler.state_restores, 2);
    assert_eq!(bits(&sampler.coefficients), bits(&values));
}
