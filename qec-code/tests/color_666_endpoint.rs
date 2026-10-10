//! Regression for the bounded C19 endpoint used by circuit clients.
use qec_code::Pauli;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::css_endpoint::{CssEndpointDecoder, EndpointLimits, EndpointScore, PauliFrame};
use qec_code::family_contract::{Color666FamilySpec, Color666Layout, CssFamilySpec, construct_css};
use qec_code::phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup};

fn c19() -> CssEndpointDecoder {
    let construction = construct_css(
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into(),
    )
    .unwrap();
    let hx = SparseRowsMatrix::new(19, construction.checks.h_x)
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(19, construction.checks.h_z)
        .unwrap()
        .to_dense_rows();
    let css = CssCode::from_hx_hz(hx.clone(), hz.clone()).unwrap();
    let checks = hx
        .into_iter()
        .map(|x| Pauli::from_xz_bits(x, vec![0; 19]).unwrap())
        .chain(
            hz.into_iter()
                .map(|z| Pauli::from_xz_bits(vec![0; 19], z).unwrap()),
        )
        .map(|p| PhasedPauli::new(p, Phase::PlusOne))
        .collect();
    let group = SignedStabilizerGroup::new(19, checks).unwrap();
    let basis = css.code().canonical_logical_basis().unwrap();
    let basis = group
        .validate_logical_basis(
            basis
                .logical_x
                .into_iter()
                .map(|p| PhasedPauli::new(p, Phase::PlusOne))
                .collect(),
            basis
                .logical_z
                .into_iter()
                .map(|p| PhasedPauli::new(p, Phase::PlusOne))
                .collect(),
        )
        .unwrap();
    CssEndpointDecoder::new(
        &group,
        &basis,
        2,
        EndpointLimits {
            max_physical_qubits: 19,
            max_masks_per_plane: 191,
        },
    )
    .unwrap()
}

#[test]
fn c19_corrects_two_site_paulis_but_retains_a_weight_three_logical_failure() {
    let decoder = c19();
    // Literal geometric logical Z: qubits 0,1,4,7,14, independent of decoder recovery.
    let logical_z = 0x4093;
    let signature = decoder
        .signature(PauliFrame { x: 0, z: logical_z })
        .unwrap();
    assert_eq!((signature.syndrome, signature.logical), (0, 1));
    let mut frames = vec![PauliFrame { x: 0, z: 0 }];
    for q in 0..19 {
        for a in 1..=3 {
            frames.push(PauliFrame {
                x: (a & 1) << q,
                z: ((a >> 1) & 1) << q,
            });
        }
        for r in q + 1..19 {
            for a in 1..=3 {
                for b in 1..=3 {
                    frames.push(PauliFrame {
                        x: ((a & 1) << q) | ((b & 1) << r),
                        z: (((a >> 1) & 1) << q) | (((b >> 1) & 1) << r),
                    });
                }
            }
        }
    }
    assert_eq!(frames.len(), 1597);
    for frame in frames {
        assert!(matches!(
            decoder.score(frame).unwrap(),
            EndpointScore::Covered { correct: true, .. }
        ));
    }
    // Z0 Z1 Z4 receives Z7 Z14 and becomes the literal logical Z above.
    let EndpointScore::Covered {
        correction,
        residual,
        correct,
    } = decoder.score(PauliFrame { x: 0, z: 0x13 }).unwrap()
    else {
        panic!("declared counterexample must be covered")
    };
    assert_eq!(correction, PauliFrame { x: 0, z: 0x4080 });
    assert_eq!(
        (residual.syndrome, residual.logical, correct),
        (0, 1, false)
    );
}
