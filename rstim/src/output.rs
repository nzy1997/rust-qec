use crate::sim::bit_table::BitTable;
use std::io::Write;

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

/// Read 01 format: each shot is `bits` ASCII '0'/'1' chars followed by '\n'.
pub fn read_shots_01(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let line_len = bits
        .checked_add(1)
        .ok_or_else(|| "01 bit count overflows line length".to_string())?; // bits chars + newline
    if data.len() % line_len != 0 {
        return Err(format!(
            "01 data length {} not divisible by line length {}",
            data.len(),
            line_len
        ));
    }
    let shots = data.len() / line_len;
    let mut table = alloc_bit_table(bits, shots)?;
    for shot in 0..shots {
        let line = &data[shot * line_len..shot * line_len + bits];
        for (bit, &ch) in line.iter().enumerate() {
            match ch {
                b'0' => {}
                b'1' => table.set(bit, shot, true),
                _ => return Err(format!("unexpected byte 0x{:02x} in 01 data", ch)),
            }
        }
    }
    Ok(table)
}

/// Read b8 format: each shot is `ceil(bits/8)` bytes, LSB-first.
pub fn read_shots_b8(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let bytes_per_shot = bits
        .checked_add(7)
        .ok_or_else(|| "b8 bit count overflows bytes_per_shot".to_string())?
        / 8;
    if bytes_per_shot == 0 {
        return alloc_bit_table(bits, 0);
    }
    if data.len() % bytes_per_shot != 0 {
        return Err(format!(
            "b8 data length {} not divisible by bytes_per_shot {}",
            data.len(),
            bytes_per_shot
        ));
    }
    let shots = data.len() / bytes_per_shot;
    let mut table = alloc_bit_table(bits, shots)?;
    for shot in 0..shots {
        let chunk = &data[shot * bytes_per_shot..(shot + 1) * bytes_per_shot];
        for byte_idx in 0..bytes_per_shot {
            for bit_in_byte in 0..8usize {
                let bit = byte_idx * 8 + bit_in_byte;
                if bit < bits && (chunk[byte_idx] >> bit_in_byte) & 1 == 1 {
                    table.set(bit, shot, true);
                }
            }
        }
    }
    Ok(table)
}

/// Read r8 format: run-length encoding, one terminator byte per shot.
pub fn read_shots_r8(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let mut shots_data: Vec<Vec<bool>> = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let mut shot_bits = vec![false; bits];
        let mut current_bit: usize = 0;
        loop {
            if pos >= data.len() {
                return Err("r8 data ended unexpectedly mid-shot".to_string());
            }
            let run = data[pos] as usize;
            pos += 1;
            if run == 255 {
                // 255 zeros, no 1 follows
                current_bit += 255;
            } else {
                // `run` zeros then a 1 (or terminator past end)
                current_bit += run;
                if current_bit < bits {
                    shot_bits[current_bit] = true;
                    current_bit += 1;
                } else {
                    // terminator: implicit 1 past end, shot is done
                    break;
                }
            }
        }
        shots_data.push(shot_bits);
    }
    let shots = shots_data.len();
    let mut table = alloc_bit_table(bits, shots)?;
    for (shot, shot_bits) in shots_data.iter().enumerate() {
        for (bit, &val) in shot_bits.iter().enumerate() {
            if val {
                table.set(bit, shot, true);
            }
        }
    }
    Ok(table)
}

/// Read hits format: comma-separated bit indices per line.
pub fn read_shots_hits(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let text =
        std::str::from_utf8(data).map_err(|e| format!("hits data not valid UTF-8: {}", e))?;
    let shot_lines: Vec<&str> = {
        let mut v: Vec<&str> = text.split('\n').collect();
        if v.last() == Some(&"") {
            v.pop();
        }
        v
    };
    let shots = shot_lines.len();
    let mut table = alloc_bit_table(bits, shots)?;
    for (shot, line) in shot_lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        for token in line.split(',') {
            let idx: usize = token
                .trim()
                .parse()
                .map_err(|_| format!("invalid hit index {:?} in shot {}", token, shot))?;
            if idx >= bits {
                return Err(format!("hit index {} out of range (bits={})", idx, bits));
            }
            table.set(idx, shot, true);
        }
    }
    Ok(table)
}

/// Read ptb64 format: inverse of write_shots_ptb64. Needs explicit shot count.
pub fn read_shots_ptb64(data: &[u8], bits: usize, shots: usize) -> Result<BitTable, String> {
    let n_chunks = shots
        .checked_add(63)
        .ok_or_else(|| "ptb64 shot count overflows chunk count".to_string())?
        / 64;
    let expected = n_chunks
        .checked_mul(bits)
        .and_then(|words| words.checked_mul(8))
        .ok_or_else(|| "ptb64 expected byte count overflows".to_string())?;
    if data.len() != expected {
        return Err(format!(
            "ptb64 data length {} != expected {} (bits={}, shots={})",
            data.len(),
            expected,
            bits,
            shots
        ));
    }
    let mut table = alloc_bit_table(bits, shots)?;
    let mut chunk_start = 0;
    let mut data_pos = 0;
    for _chunk in 0..n_chunks {
        let chunk_end = (chunk_start + 64).min(shots);
        for bit in 0..bits {
            let word = u64::from_le_bytes(data[data_pos..data_pos + 8].try_into().unwrap());
            data_pos += 8;
            for (k, shot) in (chunk_start..chunk_end).enumerate() {
                if (word >> k) & 1 == 1 {
                    table.set(bit, shot, true);
                }
            }
        }
        chunk_start += 64;
    }
    Ok(table)
}

fn alloc_bit_table(num_major: usize, num_minor: usize) -> Result<BitTable, String> {
    BitTable::try_new(num_major, num_minor)
        .map_err(|err| format!("BitTable allocation failed: {err:?}"))
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
