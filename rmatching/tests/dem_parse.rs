use rmatching::driver::dem_parse::parse_dem;

#[test]
fn parse_simple_dem() {
    let dem = "error(0.1) D0 D1 L0";
    let g = parse_dem(dem).unwrap();
    assert_eq!(g.edges.len(), 1);
    let e = &g.edges[0];
    assert_eq!(e.node1, 0);
    assert_eq!(e.node2, 1);
    assert_eq!(e.observable_indices, vec![0]);
    assert!((e.error_probability - 0.1).abs() < 1e-9);
}

#[test]
fn parse_boundary_dem() {
    let dem = "error(0.1) D0 L0";
    let g = parse_dem(dem).unwrap();
    assert_eq!(g.edges.len(), 1);
    let e = &g.edges[0];
    assert_eq!(e.node1, 0);
    assert_eq!(e.node2, usize::MAX); // boundary sentinel
    assert_eq!(e.observable_indices, vec![0]);
}

#[test]
fn parse_repeat_dem() {
    // repeat 3 iterations, each with shift_detectors 2
    let dem = "\
repeat 3 {
    error(0.1) D0 D1 L0
    shift_detectors 2
}";
    let g = parse_dem(dem).unwrap();
    // 3 iterations → 3 edges
    assert_eq!(g.edges.len(), 3);
    // Iteration 0: D0-D1, iteration 1: D2-D3, iteration 2: D4-D5
    assert_eq!(g.edges[0].node1, 0);
    assert_eq!(g.edges[0].node2, 1);
    assert_eq!(g.edges[1].node1, 2);
    assert_eq!(g.edges[1].node2, 3);
    assert_eq!(g.edges[2].node1, 4);
    assert_eq!(g.edges[2].node2, 5);
}

#[test]
fn parse_dem_roundtrip() {
    let dem = "\
error(0.1) D0 D1 L0
error(0.05) D1 D2
error(0.1) D0 L1
detector D0
detector D1
detector D2
";
    let g = parse_dem(dem).unwrap();
    // 2 normal edges + 1 boundary edge
    assert_eq!(g.edges.len(), 3);
    assert_eq!(g.get_num_nodes(), 3); // D0, D1, D2
    assert_eq!(g.num_observables, 2); // L0 and L1
}

#[test]
fn declarations_and_zero_probability_errors_preserve_dimensions() {
    let g = parse_dem("detector D8\nlogical_observable L9\nerror(0) D10 L11\n").unwrap();

    assert_eq!(g.get_num_detectors(), 11);
    assert_eq!(g.num_observables, 12);
    assert!(g.edges.is_empty(), "unexpected edges: {:?}", g.edges);
}

#[test]
fn extreme_declared_indices_return_errors_without_panicking() {
    let cases = [
        format!("logical_observable L{}\n", usize::MAX),
        format!("error(0) D{}\n", usize::MAX),
        format!("shift_detectors {}\ndetector D1\n", usize::MAX),
    ];

    for dem in cases {
        let outcome = std::panic::catch_unwind(|| parse_dem(&dem));
        assert!(outcome.is_ok(), "parser panicked for `{dem}`");
        let error = match outcome.unwrap() {
            Ok(_) => panic!("extreme index unexpectedly parsed: `{dem}`"),
            Err(error) => error,
        };
        assert!(
            error.contains("too large")
                || error.contains("supported graph capacity")
                || error.contains("overflow"),
            "unexpected error for `{dem}`: {error}"
        );
        assert!(error.contains("while parsing"), "missing context: {error}");
    }
}

#[test]
fn invalid_probability_is_rejected_before_large_dimension_allocation() {
    let dem = "error(1) D100000000\n";
    let outcome = std::panic::catch_unwind(|| parse_dem(dem));

    assert!(outcome.is_ok());
    let error = match outcome.unwrap() {
        Ok(_) => panic!("invalid probability unexpectedly parsed"),
        Err(error) => error,
    };
    assert!(error.contains("finite error probabilities in the range 0 <= p < 1"));
    assert!(error.contains("while parsing"));
}

#[test]
fn detector_capacity_overflow_returns_an_error_with_instruction_context() {
    // The node count fits usize, but exceeds Vec's isize::MAX capacity limit.
    // This exercises capacity rejection without attempting a large allocation.
    let detector = usize::MAX - 1;
    let dem = format!("detector D{detector}");
    let outcome = std::panic::catch_unwind(|| parse_dem(&dem));
    let error = match outcome.expect("capacity overflow must not panic") {
        Ok(_) => panic!("unsupported graph capacity was accepted"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        format!(
            "detector index {detector} exceeds supported graph capacity; while parsing `{dem}`"
        )
    );
}

#[test]
fn parse_correlated_segments_from_single_error_instruction() {
    let dem = "error(0.1) D0 D1 L0 ^ D2 L1 ^ D3 D4";
    let g = parse_dem(dem).unwrap();

    assert_eq!(g.edges.len(), 3);

    assert_eq!(g.edges[0].node1, 0);
    assert_eq!(g.edges[0].node2, 1);
    assert_eq!(g.edges[0].observable_indices, vec![0]);

    assert_eq!(g.edges[1].node1, 2);
    assert_eq!(g.edges[1].node2, usize::MAX);
    assert_eq!(g.edges[1].observable_indices, vec![1]);

    assert_eq!(g.edges[2].node1, 3);
    assert_eq!(g.edges[2].node2, 4);
    assert!(g.get_num_nodes() >= 5);
}

#[test]
fn reject_non_graphlike_error_component() {
    let dem = "error(0.1) D0 D1 D2 L0";
    let error = match parse_dem(dem) {
        Ok(_) => panic!("expected a non-graphlike DEM to be rejected"),
        Err(error) => error,
    };

    assert!(error.contains("requires a graphlike DEM"));
    assert!(error.contains("3 detectors"));
    assert!(error.contains(dem));
}

#[test]
fn reject_non_graphlike_correlated_component() {
    let dem = "error(0.1) D0 D1 ^ D2 D3 D4 L0";
    let error = match parse_dem(dem) {
        Ok(_) => panic!("expected a non-graphlike DEM to be rejected"),
        Err(error) => error,
    };

    assert!(error.contains("requires a graphlike DEM"));
    assert!(error.contains("3 detectors"));
    assert!(error.contains(dem));
}

#[test]
fn detector_like_tokens_in_inline_comments_are_ignored() {
    let g = parse_dem("error(0.1) D0 D1 L0 # D2 D3").unwrap();

    assert_eq!(g.edges.len(), 1);
    assert_eq!((g.edges[0].node1, g.edges[0].node2), (0, 1));
    assert_eq!(g.edges[0].observable_indices, vec![0]);
}

#[test]
fn hash_inside_instruction_tag_is_not_treated_as_a_comment() {
    let g = parse_dem("error[tag#detail](0.1) D0 D1 L0 # D2 D3").unwrap();

    assert_eq!(g.edges.len(), 1);
    assert_eq!((g.edges[0].node1, g.edges[0].node2), (0, 1));
    assert_eq!(g.edges[0].observable_indices, vec![0]);
}

#[test]
fn braces_in_inline_comments_do_not_change_repeat_structure() {
    let dem =
        "repeat 2 { # } ignored\n    error(0.1) D0 D1 # D2\n    shift_detectors 2\n} # { ignored";
    let g = parse_dem(dem).unwrap();

    assert_eq!(g.edges.len(), 2);
    assert_eq!((g.edges[0].node1, g.edges[0].node2), (0, 1));
    assert_eq!((g.edges[1].node1, g.edges[1].node2), (2, 3));
}

#[test]
fn reject_non_graphlike_component_inside_repeat() {
    let dem = "repeat 2 {\n    error(0.1) D0 D1 D2 L0\n    shift_detectors 3\n}";
    let error = match parse_dem(dem) {
        Ok(_) => panic!("expected a repeated non-graphlike DEM to be rejected"),
        Err(error) => error,
    };

    assert!(error.contains("requires a graphlike DEM"));
    assert!(error.contains("3 detectors"));
    assert!(error.contains("error(0.1) D0 D1 D2 L0"));
}

#[test]
fn parse_repeat_with_coordinate_shift_and_detector_shift() {
    let dem = "\
repeat 2 {
    error(0.1) D0 D1
    shift_detectors(0, 0, 1) 0
    shift_detectors 2
}";
    let g = parse_dem(dem).unwrap();

    assert_eq!(g.edges.len(), 2);
    assert_eq!((g.edges[0].node1, g.edges[0].node2), (0, 1));
    assert_eq!((g.edges[1].node1, g.edges[1].node2), (2, 3));
}
