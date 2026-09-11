use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: export_decoder_oracle DATASET OUTPUT.json"
    );
    let data = rustqec_cli::export_decoder_oracle_dataset(Path::new(&args[1]))
        .expect("public benchmark dataset");
    std::fs::write(&args[2], serde_json::to_vec(&data).unwrap()).unwrap();
}
