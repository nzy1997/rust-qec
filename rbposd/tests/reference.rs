use std::fs;
use std::path::PathBuf;

use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

struct ReferenceCase {
    name: &'static str,
    pcm: ParityCheckMatrix,
    channel: ChannelModel,
    syndrome: Syndrome,
    expect_osd: bool,
    max_bp_iterations: Option<usize>,
}

fn repetition_pcm() -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(4, 5, vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]])
        .unwrap()
}

fn reference_cases() -> Vec<ReferenceCase> {
    vec![
        ReferenceCase {
            name: "bp repetition single flip",
            pcm: repetition_pcm(),
            channel: ChannelModel::Bsc { error_rate: 0.05 },
            syndrome: Syndrome::from(vec![true, false, false, false]),
            expect_osd: false,
            max_bp_iterations: None,
        },
        ReferenceCase {
            name: "osd fallback small sparse code",
            pcm: ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap(),
            channel: ChannelModel::BitFlipProbabilities(vec![0.1, 0.2, 0.3]),
            syndrome: Syndrome::from(vec![true, false]),
            expect_osd: true,
            max_bp_iterations: Some(0),
        },
    ]
}

#[test]
fn reference_contract_loop_matches_plan_cases() {
    for case in reference_cases() {
        let mut config = DecoderConfig::default();
        if let Some(max_bp_iterations) = case.max_bp_iterations {
            config.max_bp_iterations = max_bp_iterations;
        }

        let decoder = BpOsdDecoder::new(case.pcm.clone(), case.channel.clone(), config).unwrap();
        let result = decoder.decode(&case.syndrome).unwrap();

        assert_eq!(result.used_osd, case.expect_osd, "case={}", case.name);
        assert_eq!(
            case.pcm.multiply(&result.correction),
            case.syndrome,
            "case={}",
            case.name
        );
    }
}

#[test]
fn task_6_documentation_surfaces_exist() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basic_example = crate_root.join("examples/basic_decode.rs");
    let profile_example = crate_root.join("examples/profile_repetition.rs");
    let lib_rs = crate_root.join("src/lib.rs");

    assert!(basic_example.exists(), "missing {}", basic_example.display());
    assert!(profile_example.exists(), "missing {}", profile_example.display());

    let lib_contents = fs::read_to_string(&lib_rs).unwrap();
    assert!(
        lib_contents.contains("use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};"),
        "missing crate-level usage example in {}",
        lib_rs.display()
    );
}
