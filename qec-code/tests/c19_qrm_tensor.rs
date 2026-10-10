//! Regression for the frozen mixed-code export, using an independent code reference.
use qec_code::Pauli;
use qec_code::phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup};
use serde_json::Value;

fn word(s: &str) -> PhasedPauli {
    let x = s
        .bytes()
        .map(|b| u8::from(b == b'X' || b == b'Y'))
        .collect();
    let z = s
        .bytes()
        .map(|b| u8::from(b == b'Z' || b == b'Y'))
        .collect();
    PhasedPauli::new(
        Pauli::from_xz_bits(x, z).unwrap(),
        Phase::from_exponent((s.bytes().filter(|b| *b == b'Y').count() % 4) as u8),
    )
}

fn axis(support: u128, x_axis: bool) -> PhasedPauli {
    let bits = (0..83)
        .map(|q| ((support >> q) & 1) as u8)
        .collect::<Vec<_>>();
    let zeros = vec![0; 83];
    PhasedPauli::new(
        Pauli::from_xz_bits(
            if x_axis { bits.clone() } else { zeros.clone() },
            if x_axis { zeros } else { bits },
        )
        .unwrap(),
        Phase::PlusOne,
    )
}

fn monomial(vars: &[usize]) -> u128 {
    (0..64)
        .filter(|q| vars.iter().all(|v| q & (1 << v) != 0))
        .fold(0, |m, q| m | (1u128 << (19 + q)))
}

#[test]
fn c19_qrm_export_matches_literal_color_and_reed_muller_reference() {
    let candidate: Value =
        serde_json::from_str(include_str!("fixtures/c19_qrm83_joint.json")).unwrap();
    let read = |key: &str| {
        candidate[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| word(v.as_str().unwrap()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (
            candidate["n"].as_u64(),
            candidate["rank"].as_u64(),
            candidate["k"].as_u64()
        ),
        (Some(83), Some(62), Some(21))
    );
    let checks = read("checks");
    let group = SignedStabilizerGroup::new(83, checks.clone()).unwrap();
    let basis = group
        .validate_logical_basis(read("logical_x"), read("logical_z"))
        .unwrap();
    assert_eq!((group.rank(), basis.k()), (62, 21));

    // Reference is independently declared: literal color geometry and RM(2,6)
    // Boolean evaluations, not the exporter's constructor or its tensor helper.
    let color: Value =
        serde_json::from_str(include_str!("fixtures/css/color_666_d5_hx.json")).unwrap();
    let mut supports = color["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .fold(0u128, |mask, q| mask | (1u128 << q.as_u64().unwrap()))
        })
        .collect::<Vec<_>>();
    supports.push(monomial(&[]));
    for a in 0..6 {
        supports.push(monomial(&[a]));
        for b in a + 1..6 {
            supports.push(monomial(&[a, b]));
        }
    }
    assert_eq!(supports.len(), 31);
    let reference_checks = supports
        .iter()
        .map(|m| axis(*m, true))
        .chain(supports.iter().map(|m| axis(*m, false)))
        .collect::<Vec<_>>();
    let reference = SignedStabilizerGroup::new(83, reference_checks.clone()).unwrap();
    assert_eq!(reference.rank(), 62);
    for p in reference_checks {
        assert!(group.contains_with_witness(&p).unwrap().is_some());
    }
    for p in &checks {
        assert!(reference.contains_with_witness(p).unwrap().is_some());
    }

    // Literal geometric controller logical, independent of exported columns.
    let gamma = axis(0x4093, false);
    assert!(checks.iter().all(|g| gamma.commutes_with(g).unwrap()));
    assert_eq!(basis.logical_z()[0], gamma);
    for (i, lx) in basis.logical_x().iter().enumerate() {
        assert_eq!(!gamma.commutes_with(lx).unwrap(), i == 0);
    }
    let rows = candidate["columns_X_then_Z"].as_array().unwrap();
    assert_eq!(rows.len(), 166);
    for plane in 0..2 {
        for q in 0..83 {
            let p = axis(1u128 << q, plane == 0);
            let syndrome = checks.iter().enumerate().fold(0u128, |v, (i, g)| {
                v | (u128::from(!p.commutes_with(g).unwrap()) << i)
            });
            let logical = basis
                .logical_x()
                .iter()
                .chain(basis.logical_z())
                .enumerate()
                .fold(0u128, |v, (i, g)| {
                    v | (u128::from(!p.commutes_with(g).unwrap()) << i)
                });
            assert_eq!(
                rows[plane * 83 + q][0].as_str().unwrap(),
                format!("{syndrome:x}")
            );
            assert_eq!(
                rows[plane * 83 + q][1].as_str().unwrap(),
                format!("{logical:x}")
            );
        }
    }
}
