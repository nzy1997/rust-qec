use rstim::cli::run_export_decoder_dataset;
use rstim::decoder_dataset::{
    DecoderDatasetArtifacts, DecoderDatasetMode, DecoderDatasetSummary,
    ExportDecoderDatasetConfig, ExportDecoderDatasetLogicalFlipConfig, LogicalFlip, LogicalPauli,
    export_decoder_dataset, export_decoder_dataset_with_logical_flip,
    generate_decoder_dataset_artifacts,
};
use std::path::PathBuf;

#[test]
fn released_x_only_rust_api_remains_source_compatible() {
    let legacy = ExportDecoderDatasetConfig {
        circuit_text: "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n".to_string(),
        shots: 1,
        mode: DecoderDatasetMode::Detectors,
        logical_x_qubits: Vec::new(),
        public_out: PathBuf::from("public-unused"),
        private_out: PathBuf::from("private-unused"),
        seed: Some(1),
    };

    let _: fn(ExportDecoderDatasetConfig) -> Result<DecoderDatasetSummary, String> =
        export_decoder_dataset;
    let _: fn(&ExportDecoderDatasetConfig) -> Result<DecoderDatasetArtifacts, String> =
        generate_decoder_dataset_artifacts;
    let _: fn(
        &str,
        u64,
        &str,
        Option<&str>,
        &str,
        &str,
        Option<u64>,
    ) -> Result<(), String> = run_export_decoder_dataset;

    let artifacts = generate_decoder_dataset_artifacts(&legacy).unwrap();
    assert_eq!(artifacts.public_row_kind, "detectors");
    let generalized = ExportDecoderDatasetLogicalFlipConfig::from(&legacy);
    assert_eq!(generalized.logical_flip, None);
}

#[test]
fn generalized_rust_api_accepts_a_logical_z_flip() {
    let config = ExportDecoderDatasetLogicalFlipConfig {
        circuit_text: "RX 0\n# RSTIM_LOGICAL_FLIP_POINT\nMX 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
            .to_string(),
        shots: 1,
        mode: DecoderDatasetMode::MeasurementsBlinded,
        logical_flip: Some(LogicalFlip {
            pauli: LogicalPauli::Z,
            qubits: vec![0],
        }),
        public_out: PathBuf::from("public-unused"),
        private_out: PathBuf::from("private-unused"),
        seed: Some(1),
    };

    let _: fn(ExportDecoderDatasetLogicalFlipConfig) -> Result<DecoderDatasetSummary, String> =
        export_decoder_dataset_with_logical_flip;
    assert_eq!(config.logical_flip.unwrap().pauli, LogicalPauli::Z);
}
