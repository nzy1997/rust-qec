use std::time::Instant;

use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pcm = ParityCheckMatrix::from_sparse_rows(
        4,
        5,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
    )?;
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )?;

    let syndromes = [
        Syndrome::from(vec![true, false, false, false]),
        Syndrome::from(vec![false, true, false, false]),
        Syndrome::from(vec![false, false, true, false]),
        Syndrome::from(vec![false, false, false, true]),
    ];

    let mut total_ns = 0u128;
    let mut total_iterations = 0usize;
    let mut osd_uses = 0usize;

    for syndrome in syndromes.iter().cycle().take(200) {
        let start = Instant::now();
        let result = decoder.decode(syndrome)?;
        total_ns += start.elapsed().as_nanos();
        total_iterations += result.bp_iterations;
        osd_uses += usize::from(result.used_osd);
    }

    println!("shots=200");
    println!("avg_ns={}", total_ns / 200);
    println!("avg_iterations={:.2}", total_iterations as f64 / 200.0);
    println!("osd_uses={}", osd_uses);
    Ok(())
}
