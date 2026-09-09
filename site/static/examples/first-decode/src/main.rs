use rmatching::Matching;
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let dem = fs::read_to_string("model.dem")?;
    let mut matching = Matching::from_dem(&dem)?;
    let rows = fs::read_to_string("detectors.01")?;
    for (index, row) in rows.lines().enumerate() {
        // This example's model has exactly two detectors, D0 and D1.
        if row.len() != 2 || !row.bytes().all(|bit| matches!(bit, b'0' | b'1')) {
            return Err(format!("row {}: expected two detector bits, e.g. 11", index + 1).into());
        }
        let syndrome: Vec<u8> = row.bytes().map(|bit| bit - b'0').collect();
        let prediction = matching.decode(&syndrome);
        println!("row {}: predicted L0={}", index + 1, prediction[0]);
    }
    Ok(())
}
