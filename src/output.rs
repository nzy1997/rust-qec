use std::io::Write;
use crate::sim::bit_table::BitTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Format01,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "01" => Ok(Self::Format01),
            "b8" => Ok(Self::B8),
            "r8" => Ok(Self::R8),
            "hits" => Ok(Self::Hits),
            "dets" => Ok(Self::Dets),
            "ptb64" => Ok(Self::Ptb64),
            _ => Err(format!("unknown output format: {:?}", s)),
        }
    }
}

/// Dense text: one line per shot, '0'/'1' per bit.
pub fn write_shots_01(table: &BitTable, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        for bit in 0..n_bits {
            let ch = if table.get(bit, shot) { b'1' } else { b'0' };
            w.write_all(&[ch])?;
        }
        w.write_all(b"\n")?;
    }
    Ok(())
}

/// Dense binary: ceil(n_bits/8) bytes per shot, LSB-first bit packing.
pub fn write_shots_b8(table: &BitTable, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    let bytes_per_shot = (n_bits + 7) / 8;
    for shot in 0..n_shots {
        for byte_idx in 0..bytes_per_shot {
            let mut byte_val: u8 = 0;
            for bit_in_byte in 0..8 {
                let bit = byte_idx * 8 + bit_in_byte;
                if bit < n_bits && table.get(bit, shot) {
                    byte_val |= 1 << bit_in_byte;
                }
            }
            w.write_all(&[byte_val])?;
        }
    }
    Ok(())
}

/// Sparse binary run-length encoding.
/// Each byte = length of run of 0s before next 1. 255 = 255 zeros without a 1.
/// Each shot terminated by an implicit True bit past the end.
pub fn write_shots_r8(table: &BitTable, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        let mut run: usize = 0;
        for bit in 0..n_bits {
            if table.get(bit, shot) {
                while run >= 255 {
                    w.write_all(&[255])?;
                    run -= 255;
                }
                w.write_all(&[run as u8])?;
                run = 0;
            } else {
                run += 1;
            }
        }
        // Terminator: remaining zeros before implicit True past end
        while run >= 255 {
            w.write_all(&[255])?;
            run -= 255;
        }
        w.write_all(&[run as u8])?;
    }
    Ok(())
}

/// Sparse text: comma-separated indices of set bits, one line per shot.
pub fn write_shots_hits(table: &BitTable, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        let mut first = true;
        for bit in 0..n_bits {
            if table.get(bit, shot) {
                if !first {
                    w.write_all(b",")?;
                }
                write!(w, "{}", bit)?;
                first = false;
            }
        }
        w.write_all(b"\n")?;
    }
    Ok(())
}

/// Sparse text with "shot D# L#" format.
/// Detections prefixed with D, observable flips prefixed with L.
pub fn write_shots_dets(
    detections: &BitTable,
    observable_flips: &BitTable,
    w: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    let n_shots = detections.num_minor();
    let n_dets = detections.num_major();
    let n_obs = observable_flips.num_major();
    for shot in 0..n_shots {
        w.write_all(b"shot")?;
        for d in 0..n_dets {
            if detections.get(d, shot) {
                write!(w, " D{}", d)?;
            }
        }
        for l in 0..n_obs {
            if observable_flips.get(l, shot) {
                write!(w, " L{}", l)?;
            }
        }
        w.write_all(b"\n")?;
    }
    Ok(())
}

/// Partially-transposed bit-packed binary (ptb64).
/// Shots are grouped in chunks of 64. For each chunk, one little-endian u64
/// word is written per bit/detector: bit k of the word = value of that bit
/// in shot (chunk_start + k).
pub fn write_shots_ptb64(table: &BitTable, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    let mut chunk_start = 0;
    while chunk_start < n_shots {
        let chunk_end = (chunk_start + 64).min(n_shots);
        for bit in 0..n_bits {
            let mut word: u64 = 0;
            for (k, shot) in (chunk_start..chunk_end).enumerate() {
                if table.get(bit, shot) {
                    word |= 1u64 << k;
                }
            }
            w.write_all(&word.to_le_bytes())?;
        }
        chunk_start += 64;
    }
    Ok(())
}
