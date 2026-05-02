use rmatching::Matching;
use rstim::codegen::{NoiseParams, rotated_memory_z_with_params};
use rstim::error_analyzer::ErrorAnalyzer;

const BLOSSOM_PANIC_FIRED_DETECTORS: &[usize] = &[
    3, 5, 24, 27, 29, 32, 36, 37, 38, 39, 41, 43, 46, 50, 55, 84, 85, 86, 90, 91, 103, 118,
    120, 128, 133, 138, 146, 155, 166, 182, 195, 202, 203, 204, 209, 210, 213, 214, 229, 236,
    242, 255, 262, 263, 275, 287, 293, 306, 316, 340, 341, 347, 348, 352, 388, 393, 400, 405,
    419, 431, 441, 446, 451, 466, 474, 477, 478, 483, 484, 487, 490, 491, 494, 495, 505, 507,
    512, 514, 516, 520, 524, 528, 529, 531, 535, 547, 552, 553, 555, 557, 559, 564, 568, 569,
    570, 571, 572, 573, 576, 592, 600, 601, 603, 607, 611, 619, 621, 624, 628, 630, 636, 642,
    643, 651, 659, 660, 667, 668, 669, 676, 688, 699, 711, 712, 719, 731, 734, 736, 739, 740,
    741, 742, 753, 780, 781, 786, 792, 794, 796, 797, 800, 813, 819, 820, 821, 827, 828, 833,
    834, 835, 842, 845, 861, 867, 876, 890, 908, 915, 926, 927, 933, 956, 962, 974, 994, 997,
];

fn make_surface_code_blossom_panic_case() -> (String, Vec<u8>) {
    // Captured by rsinter/examples/find_blossom_bug_case.rs from:
    // d=7, rounds=21, p=0.012, seed=1, shot=2097 within sample_batch(..., 4096, ...).
    let circuit = rotated_memory_z_with_params(7, 21, NoiseParams::uniform(0.012));
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)
        .expect("surface-code circuit should convert into a decomposed DEM");
    assert_eq!(
        dem.num_detectors(),
        1008,
        "fixture detector count changed; this repro likely needs to be recaptured"
    );

    let mut syndrome = vec![0u8; dem.num_detectors()];
    for &det in BLOSSOM_PANIC_FIRED_DETECTORS {
        syndrome[det] = 1;
    }

    (dem.to_string(), syndrome)
}

#[test]
fn blossom_panic_fixture_still_builds() {
    let (dem_text, syndrome) = make_surface_code_blossom_panic_case();
    let _matching = Matching::from_dem(&dem_text).expect("fixture DEM should parse into Matching");

    assert_eq!(BLOSSOM_PANIC_FIRED_DETECTORS.len(), 166);
    assert_eq!(syndrome.len(), 1008);
    assert_eq!(
        syndrome.iter().map(|&b| usize::from(b)).sum::<usize>(),
        BLOSSOM_PANIC_FIRED_DETECTORS.len()
    );
}

#[test]
fn surface_code_d7_p012_seed1_shot2097_decodes_without_blossom_panic() {
    let (dem_text, syndrome) = make_surface_code_blossom_panic_case();
    let mut matching = Matching::from_dem(&dem_text).expect("fixture DEM should parse into Matching");
    let prediction = matching.decode(&syndrome);

    assert_eq!(prediction.len(), 1);
}

#[test]
#[ignore = "debugger entry point for the fixed surface-code regression fixture"]
fn surface_code_d7_p012_seed1_shot2097_debug_fixture() {
    let (dem_text, syndrome) = make_surface_code_blossom_panic_case();
    let mut matching = Matching::from_dem(&dem_text).expect("fixture DEM should parse into Matching");
    let _ = matching.decode(&syndrome);
}
