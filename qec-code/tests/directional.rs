use qec_code::binary::try_in_row_span;
use qec_code::codes::directional::{
    parse_directional_route_support, DirectionalAncillaCoset, DirectionalConnectivity,
    DirectionalCssSpec, DirectionalLayoutSpec, DirectionalTorusSpec,
};
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality, CssFamilySpec,
    RequestedFamilyId,
};

fn fixture(name: &str) -> serde_json::Value {
    let text = match name {
        "square_ne2n_8x6.json" => include_str!("fixtures/directional/square_ne2n_8x6.json"),
        "hex_ne3n_18x4.json" => include_str!("fixtures/directional/hex_ne3n_18x4.json"),
        _ => panic!("unknown directional fixture: {name}"),
    };
    serde_json::from_str(text).expect("directional fixture should be valid JSON")
}

fn fixture_rows(fixture: &serde_json::Value, name: &str) -> Vec<Vec<usize>> {
    serde_json::from_value(fixture["checks"][name].clone())
        .expect("fixture checks should be sparse rows")
}

fn directional_spec(
    period_x: usize,
    period_y: usize,
    vertical_period_x_shift: usize,
    route: &str,
    connectivity: DirectionalConnectivity,
) -> DirectionalCssSpec {
    DirectionalCssSpec {
        torus: DirectionalTorusSpec {
            period_x,
            period_y,
            vertical_period_x_shift,
        },
        route: route.to_owned(),
        layout: DirectionalLayoutSpec {
            x_ancilla_coset: DirectionalAncillaCoset::OddEven,
            z_ancilla_coset: DirectionalAncillaCoset::EvenOdd,
        },
        connectivity,
    }
}

fn dense_rows(n: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; n];
            for &column in row {
                dense[column] = 1;
            }
            dense
        })
        .collect()
}

fn has_component_logical(
    candidate: &[u8],
    kernel_checks: &[Vec<u8>],
    stabilizers: &[Vec<u8>],
) -> bool {
    kernel_checks.iter().all(|check| {
        check
            .iter()
            .zip(candidate)
            .fold(0_u8, |parity, (&entry, &bit)| parity ^ (entry & bit))
            == 0
    }) && !try_in_row_span(stabilizers, candidate).expect("fixture rows should be binary")
}

fn search_supports(
    n: usize,
    weight: usize,
    next: usize,
    candidate: &mut [u8],
    kernel_checks: &[Vec<u8>],
    stabilizers: &[Vec<u8>],
) -> bool {
    if weight == 0 {
        return has_component_logical(candidate, kernel_checks, stabilizers);
    }
    for column in next..=n - weight {
        candidate[column] = 1;
        if search_supports(
            n,
            weight - 1,
            column + 1,
            candidate,
            kernel_checks,
            stabilizers,
        ) {
            return true;
        }
        candidate[column] = 0;
    }
    false
}

fn exact_component_distance(
    n: usize,
    kernel_checks: &[Vec<usize>],
    stabilizers: &[Vec<usize>],
    maximum_distance: usize,
) -> usize {
    let kernel_checks = dense_rows(n, kernel_checks);
    let stabilizers = dense_rows(n, stabilizers);
    let mut candidate = vec![0; n];
    for weight in 1..=maximum_distance {
        if search_supports(n, weight, 0, &mut candidate, &kernel_checks, &stabilizers) {
            return weight;
        }
    }
    panic!("no component logical support found up to distance {maximum_distance}");
}

fn assert_fixture_matches(spec: DirectionalCssSpec, fixture: serde_json::Value) {
    let expected_h_x = fixture_rows(&fixture, "h_x");
    let expected_h_z = fixture_rows(&fixture, "h_z");
    let parsed = parse_css_construction_json(
        &serde_json::to_string(&fixture["request"]).expect("fixture request is serializable"),
    )
    .expect("fixture request should parse");
    assert_eq!(parsed, CssFamilySpec::Directional(spec.clone()).into());

    let result = construct_css(CssFamilySpec::Directional(spec).into())
        .expect("fixture directional construction should succeed");
    assert_eq!(result.checks.h_x, expected_h_x);
    assert_eq!(result.checks.h_z, expected_h_z);
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::Directional)
    );
    assert_eq!(
        result.stats.n,
        fixture["stats"]["n"].as_u64().unwrap() as usize
    );
    assert_eq!(
        result.stats.m_x,
        fixture["stats"]["m_x"].as_u64().unwrap() as usize
    );
    assert_eq!(
        result.stats.m_z,
        fixture["stats"]["m_z"].as_u64().unwrap() as usize
    );
    assert_eq!(
        result.stats.rank_x,
        fixture["stats"]["rank_x"].as_u64().unwrap() as usize
    );
    assert_eq!(
        result.stats.rank_z,
        fixture["stats"]["rank_z"].as_u64().unwrap() as usize
    );
    assert_eq!(
        result.stats.k,
        fixture["stats"]["k"].as_u64().unwrap() as usize
    );
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z)
        .expect("fixture checks should be orthogonal");
    assert_eq!(
        exact_component_distance(
            result.stats.n,
            &result.checks.h_z,
            &result.checks.h_x,
            fixture["distances"]["d_x"].as_u64().unwrap() as usize,
        ),
        fixture["distances"]["d_x"].as_u64().unwrap() as usize,
    );
    assert_eq!(
        exact_component_distance(
            result.stats.n,
            &result.checks.h_x,
            &result.checks.h_z,
            fixture["distances"]["d_z"].as_u64().unwrap() as usize,
        ),
        fixture["distances"]["d_z"].as_u64().unwrap() as usize,
    );

    let repeated = construct_css(parsed).expect("parsed fixture construction should succeed");
    assert_eq!(
        serde_json::to_string(&result.normalized_parameters).unwrap(),
        serde_json::to_string(&repeated.normalized_parameters).unwrap(),
        "normalized directional metadata should be deterministic"
    );
}

#[test]
fn directional_square_ne2n_matches_fixture() {
    assert_eq!(
        parse_directional_route_support("NE2N").unwrap(),
        vec![(0, 1), (1, 2), (3, 2), (4, 3)]
    );
    assert_fixture_matches(
        directional_spec(8, 6, 4, "NE2N", DirectionalConnectivity::Square),
        fixture("square_ne2n_8x6.json"),
    );
}

#[test]
fn directional_hex_ne3n_matches_fixture() {
    assert_fixture_matches(
        directional_spec(18, 4, 0, "NE3N", DirectionalConnectivity::Hex),
        fixture("hex_ne3n_18x4.json"),
    );
}

#[test]
fn directional_rejects_incompatible_routes() {
    assert!(construct_css(
        CssFamilySpec::Directional(directional_spec(
            8,
            6,
            4,
            "NE2N",
            DirectionalConnectivity::Hex
        ))
        .into()
    )
    .is_err());
    assert!(construct_css(
        CssFamilySpec::Directional(directional_spec(
            8,
            6,
            4,
            "NE",
            DirectionalConnectivity::Square
        ))
        .into()
    )
    .is_err());
    assert!(construct_css(
        CssFamilySpec::Directional(directional_spec(
            4,
            2,
            0,
            "NE2N",
            DirectionalConnectivity::Square
        ))
        .into()
    )
    .is_err());
}
