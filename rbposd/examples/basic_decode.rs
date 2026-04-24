use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 5, vec![vec![0]])?;
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        DecoderConfig::default(),
    )?;
    let syndrome = Syndrome::from(vec![true]);
    let result = decoder.decode(&syndrome)?;

    println!("used_osd={}", result.used_osd);
    println!("bp_iterations={}", result.bp_iterations);
    println!("correction={:?}", result.correction.as_slice());
    println!("valid={}", pcm.multiply(&result.correction) == syndrome);
    Ok(())
}
