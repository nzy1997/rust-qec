# Phase 10: convert + m2d CLI Subcommands Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `rstim convert` (reformat shot data between all 6 output formats) and `rstim m2d` (convert raw measurement bits to detection events given a circuit).

**Architecture:** `convert` reads a `BitTable` from one format and writes it in another; it needs `--bits` (or `--circuit`/`--dem`) to know the shot width. `m2d` runs a noiseless reference simulation to get expected measurement values, XORs with actual measurements to produce detection events, then writes them. Both live in `src/cli.rs` as new `Commands` variants; format reading lives in `src/output.rs`.

**Tech Stack:** Rust, existing `rstim` output/executor/sampler modules, `clap`

---

## Task 1: Format Readers in output.rs

**Files:**
- Modify: `src/output.rs`
- Test: `tests/convert.rs`

### Step 1: Write the failing tests

Create `tests/convert.rs`:

```rust
use rstim::output::{read_shots_01, read_shots_b8, write_shots_01, write_shots_b8};
use rstim::sim::bit_table::BitTable;

fn make_table(bits: usize, shots: usize, pattern: impl Fn(usize, usize) -> bool) -> BitTable {
    let mut t = BitTable::new(bits, shots);
    for b in 0..bits {
        for s in 0..shots {
            t.set(b, s, pattern(b, s));
        }
    }
    t
}

#[test]
fn roundtrip_01() {
    let orig = make_table(4, 3, |b, s| (b + s) % 2 == 0);
    let mut buf = Vec::new();
    write_shots_01(&orig, &mut buf).unwrap();
    let recovered = read_shots_01(&buf, 4).unwrap();
    for b in 0..4 {
        for s in 0..3 {
            assert_eq!(orig.get(b, s), recovered.get(b, s));
        }
    }
}

#[test]
fn roundtrip_b8() {
    let orig = make_table(5, 2, |b, s| b == s);
    let mut buf = Vec::new();
    write_shots_b8(&orig, &mut buf).unwrap();
    let recovered = read_shots_b8(&buf, 5).unwrap();
    for b in 0..5 {
        for s in 0..2 {
            assert_eq!(orig.get(b, s), recovered.get(b, s));
        }
    }
}

#[test]
fn roundtrip_r8() {
    use rstim::output::{read_shots_r8, write_shots_r8};
    let orig = make_table(6, 3, |b, s| b % 3 == s);
    let mut buf = Vec::new();
    write_shots_r8(&orig, &mut buf).unwrap();
    let recovered = read_shots_r8(&buf, 6).unwrap();
    for b in 0..6 {
        for s in 0..3 {
            assert_eq!(orig.get(b, s), recovered.get(b, s));
        }
    }
}

#[test]
fn roundtrip_hits() {
    use rstim::output::{read_shots_hits, write_shots_hits};
    let orig = make_table(4, 2, |b, s| b == 1 && s == 0);
    let mut buf = Vec::new();
    write_shots_hits(&orig, &mut buf).unwrap();
    let recovered = read_shots_hits(&buf, 4).unwrap();
    for b in 0..4 {
        for s in 0..2 {
            assert_eq!(orig.get(b, s), recovered.get(b, s));
        }
    }
}

#[test]
fn roundtrip_ptb64() {
    use rstim::output::{read_shots_ptb64, write_shots_ptb64};
    let orig = make_table(3, 70, |b, s| (b * 7 + s) % 5 == 0);
    let mut buf = Vec::new();
    write_shots_ptb64(&orig, &mut buf).unwrap();
    let recovered = read_shots_ptb64(&buf, 3, 70).unwrap();
    for b in 0..3 {
        for s in 0..70 {
            assert_eq!(orig.get(b, s), recovered.get(b, s), "b={b} s={s}");
        }
    }
}
```

### Step 2: Run test to verify it fails

```
cargo test --test convert
```
Expected: compile error — reader functions not found.

### Step 3: Implement format readers

In `src/output.rs`, add reader functions:

```rust
pub fn read_shots_01(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let bytes_per_shot = bits + 1; // bits + newline
    if data.len() % bytes_per_shot != 0 {
        return Err(format!("01 data length {} not divisible by {}", data.len(), bytes_per_shot));
    }
    let shots = data.len() / bytes_per_shot;
    let mut table = BitTable::new(bits, shots);
    for shot in 0..shots {
        for bit in 0..bits {
            let ch = data[shot * bytes_per_shot + bit];
            if ch == b'1' {
                table.set(bit, shot, true);
            } else if ch != b'0' {
                return Err(format!("unexpected byte {ch} in 01 format"));
            }
        }
    }
    Ok(table)
}

pub fn read_shots_b8(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let bytes_per_shot = (bits + 7) / 8;
    if bytes_per_shot == 0 { return Ok(BitTable::new(0, 0)); }
    if data.len() % bytes_per_shot != 0 {
        return Err(format!("b8 data length {} not divisible by {}", data.len(), bytes_per_shot));
    }
    let shots = data.len() / bytes_per_shot;
    let mut table = BitTable::new(bits, shots);
    for shot in 0..shots {
        for byte_idx in 0..bytes_per_shot {
            let byte = data[shot * bytes_per_shot + byte_idx];
            for bit_in_byte in 0..8 {
                let bit = byte_idx * 8 + bit_in_byte;
                if bit < bits && (byte >> bit_in_byte) & 1 == 1 {
                    table.set(bit, shot, true);
                }
            }
        }
    }
    Ok(table)
}

pub fn read_shots_r8(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let mut table_rows: Vec<Vec<bool>> = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let mut shot = vec![false; bits];
        let mut bit = 0usize;
        loop {
            if pos >= data.len() {
                return Err("r8: unexpected end of data".to_string());
            }
            let run = data[pos] as usize;
            pos += 1;
            bit += run;
            if run < 255 {
                if bit < bits { shot[bit] = true; }
                bit += 1;
                if bit > bits { break; } // terminator past end
                if bit == bits + 1 { break; }
            }
            if bit >= bits { break; }
        }
        table_rows.push(shot);
    }
    let shots = table_rows.len();
    let mut table = BitTable::new(bits, shots);
    for (shot, row) in table_rows.iter().enumerate() {
        for (bit, &val) in row.iter().enumerate() {
            if val { table.set(bit, shot, true); }
        }
    }
    Ok(table)
}

pub fn read_shots_hits(data: &[u8], bits: usize) -> Result<BitTable, String> {
    let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = text.lines().collect();
    let shots = lines.len();
    let mut table = BitTable::new(bits, shots);
    for (shot, line) in lines.iter().enumerate() {
        if line.is_empty() { continue; }
        for token in line.split(',') {
            let bit: usize = token.trim().parse().map_err(|_| format!("bad hits token: {token}"))?;
            if bit < bits { table.set(bit, shot, true); }
        }
    }
    Ok(table)
}

pub fn read_shots_ptb64(data: &[u8], bits: usize, shots: usize) -> Result<BitTable, String> {
    let chunks = (shots + 63) / 64;
    if data.len() != chunks * bits * 8 {
        return Err(format!("ptb64: expected {} bytes, got {}", chunks * bits * 8, data.len()));
    }
    let mut table = BitTable::new(bits, shots);
    let mut offset = 0;
    let mut chunk_start = 0;
    while chunk_start < shots {
        let chunk_end = (chunk_start + 64).min(shots);
        for bit in 0..bits {
            let word = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            offset += 8;
            for k in 0..(chunk_end - chunk_start) {
                if (word >> k) & 1 == 1 {
                    table.set(bit, chunk_start + k, true);
                }
            }
        }
        chunk_start += 64;
    }
    Ok(table)
}
```

### Step 4: Run tests

```
cargo test --test convert
```
Expected: all pass.

### Step 5: Commit

```bash
git add src/output.rs tests/convert.rs
git commit -m "feat: add format readers (01, b8, r8, hits, ptb64) to output.rs"
```

---

## Task 2: convert CLI Subcommand

**Files:**
- Modify: `src/cli.rs`
- Test: `tests/convert.rs` (extend)

### Step 1: Write the failing test

Add to `tests/convert.rs`:

```rust
#[test]
fn convert_01_to_b8_via_lib() {
    use rstim::output::{read_shots_01, write_shots_b8};
    let input = b"1010\n0101\n";
    let table = read_shots_01(input, 4).unwrap();
    let mut out = Vec::new();
    write_shots_b8(&table, &mut out).unwrap();
    // shot0: bits 0,2 set → byte = 0b0101 = 5
    // shot1: bits 1,3 set → byte = 0b1010 = 10
    assert_eq!(out, vec![5u8, 10u8]);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test convert convert_01_to_b8
```
Expected: FAIL (logic error or compile error).

### Step 3: Add Convert command to CLI

In `src/cli.rs`, add to `Commands` enum:

```rust
/// Convert shot data between output formats
#[command(name = "convert")]
Convert {
    #[arg(long = "in_format", default_value = "01")]
    in_format: String,
    #[arg(long = "out_format", default_value = "01")]
    out_format: String,
    /// Number of bits per shot (required unless --circuit or --dem provided)
    #[arg(long)]
    bits: Option<usize>,
    /// Infer bits from circuit file
    #[arg(long)]
    circuit: Option<String>,
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    /// Number of shots (required for ptb64 input)
    #[arg(long)]
    shots: Option<usize>,
},
```

Add dispatch in `run`:

```rust
Some(Commands::Convert { in_format, out_format, bits, circuit, r#in, out, shots }) => {
    let data = read_input_bytes(r#in.as_deref())?;
    let mut w = open_output(out.as_deref())?;
    run_convert(&data, &in_format, &out_format, bits, circuit.as_deref(), shots, &mut w)
}
```

Add `run_convert` function:

```rust
pub fn run_convert(
    data: &[u8],
    in_format: &str,
    out_format: &str,
    bits: Option<usize>,
    circuit: Option<&str>,
    shots: Option<usize>,
    out: &mut dyn Write,
) -> Result<(), String> {
    use crate::output::*;
    // Resolve bits count
    let n_bits = if let Some(b) = bits {
        b
    } else if let Some(circ_text) = circuit {
        let instrs = crate::parser::parse_lines(circ_text)?;
        crate::stats::num_measurements(&instrs)
    } else {
        return Err("--bits or --circuit required for convert".to_string());
    };

    let table = match in_format {
        "01" => read_shots_01(data, n_bits)?,
        "b8" => read_shots_b8(data, n_bits)?,
        "r8" => read_shots_r8(data, n_bits)?,
        "hits" => read_shots_hits(data, n_bits)?,
        "ptb64" => {
            let n_shots = shots.ok_or("--shots required for ptb64 input")?;
            read_shots_ptb64(data, n_bits, n_shots)?
        }
        _ => return Err(format!("unknown in_format: {in_format}")),
    };

    match out_format {
        "01" => write_shots_01(&table, out),
        "b8" => write_shots_b8(&table, out),
        "r8" => write_shots_r8(&table, out),
        "hits" => write_shots_hits(&table, out),
        "ptb64" => write_shots_ptb64(&table, out),
        _ => return Err(format!("unknown out_format: {out_format}")),
    }.map_err(|e| e.to_string())
}
```

Also add `read_input_bytes` helper (reads stdin or file as raw bytes):

```rust
fn read_input_bytes(path: Option<&str>) -> Result<Vec<u8>, String> {
    use std::io::Read;
    match path {
        Some(p) => std::fs::read(p).map_err(|e| e.to_string()),
        None => {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf).map_err(|e| e.to_string())?;
            Ok(buf)
        }
    }
}
```

### Step 4: Run tests

```
cargo test
```
Expected: all pass.

### Step 5: Commit

```bash
git add src/cli.rs
git commit -m "feat: add rstim convert subcommand"
```

---

## Task 3: m2d CLI Subcommand

**Files:**
- Create: `src/m2d.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli.rs`
- Test: `tests/m2d.rs`

### Step 1: Write the failing tests

Create `tests/m2d.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::m2d::measurements_to_detections;
use rstim::sim::bit_table::BitTable;

fn bool_table(bits: usize, shots: usize, vals: &[bool]) -> BitTable {
    let mut t = BitTable::new(bits, shots);
    for (i, &v) in vals.iter().enumerate() {
        let bit = i % bits;
        let shot = i / bits;
        if v { t.set(bit, shot, true); }
    }
    t
}

#[test]
fn m2d_repetition_code_no_errors() {
    // Simple 2-qubit circuit: M 0, M 1, DETECTOR rec[-2] rec[-1]
    // With no errors, both measurements = 0, detector = 0 XOR 0 = 0
    let circuit = "R 0 1\nCX 0 1\nM 0 1\nDETECTOR rec[-2] rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    // Reference: M0=0, M1=0 (after CX from |0>: qubit1 flipped → M0=0, M1=1)
    // Actually CX 0 1 with both in |0>: control=0, no flip → M0=0, M1=0
    // Detector = rec[-2] XOR rec[-1] = 0 XOR 0 = 0
    let meas = bool_table(2, 1, &[false, false]);
    let (dets, _obs) = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(dets.num_major(), 1); // 1 detector
    assert!(!dets.get(0, 0)); // detector not fired
}

#[test]
fn m2d_detector_fires_on_unexpected_measurement() {
    // Same circuit, but measurement 0 is flipped (error)
    let circuit = "R 0 1\nCX 0 1\nM 0 1\nDETECTOR rec[-2] rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    // Reference: M0=0, M1=0. Actual: M0=1, M1=0 → detector = 1 XOR 0 = 1
    let meas = bool_table(2, 1, &[true, false]);
    let (dets, _obs) = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(dets.get(0, 0)); // detector fired
}
```

### Step 2: Run test to verify it fails

```
cargo test --test m2d
```
Expected: compile error — `rstim::m2d` not found.

### Step 3: Implement m2d

Create `src/m2d.rs`:

```rust
use crate::ir::StimInstr;
use crate::executor::reference_sample;
use crate::sim::bit_table::BitTable;

/// Convert a table of raw measurement bits to detection events.
///
/// `meas_table`: BitTable with shape (num_measurements, num_shots).
/// Returns (detections, observable_flips) as BitTables.
pub fn measurements_to_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<(BitTable, BitTable), String> {
    // Get reference (noiseless) measurement values
    let reference = reference_sample(instrs)?;
    let n_meas = reference.len();
    let n_shots = meas_table.num_minor();

    if meas_table.num_major() != n_meas {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(), n_meas
        ));
    }

    // Collect detector and observable definitions by re-running the executor
    // to extract which rec[] offsets each DETECTOR/OBSERVABLE_INCLUDE uses.
    let det_obs = collect_det_obs(instrs, n_meas)?;
    let n_dets = det_obs.detectors.len();
    let n_obs = det_obs.observables.len();

    let mut dets = BitTable::new(n_dets, n_shots);
    let mut obs = BitTable::new(n_obs, n_shots);

    for shot in 0..n_shots {
        // XOR actual measurements with reference to get flips
        let flips: Vec<bool> = (0..n_meas)
            .map(|i| meas_table.get(i, shot) ^ reference[i])
            .collect();

        for (d, rec_offsets) in det_obs.detectors.iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val { dets.set(d, shot, true); }
        }
        for (o, rec_offsets) in det_obs.observables.iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val { obs.set(o, shot, true); }
        }
    }

    Ok((dets, obs))
}

struct DetObsDef {
    detectors: Vec<Vec<usize>>,   // each detector: list of absolute measurement indices
    observables: Vec<Vec<usize>>, // each observable: list of absolute measurement indices
}

fn collect_det_obs(instrs: &[StimInstr], n_meas: usize) -> Result<DetObsDef, String> {
    let mut detectors = Vec::new();
    let mut observables: Vec<Vec<usize>> = Vec::new();
    let mut meas_count = 0usize;
    collect_det_obs_instrs(instrs, &mut meas_count, &mut detectors, &mut observables)?;
    Ok(DetObsDef { detectors, observables })
}

fn collect_det_obs_instrs(
    instrs: &[StimInstr],
    meas_count: &mut usize,
    detectors: &mut Vec<Vec<usize>>,
    observables: &mut Vec<Vec<usize>>,
) -> Result<(), String> {
    use crate::ir::StimTarget;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                // Count measurements
                let meas_added = count_measurements_op(name, targets);
                match name.as_str() {
                    "DETECTOR" => {
                        let indices: Vec<usize> = targets.iter().filter_map(|t| {
                            if let StimTarget::Rec(r) = t {
                                Some((*meas_count as i64 + *r as i64) as usize)
                            } else { None }
                        }).collect();
                        detectors.push(indices);
                    }
                    "OBSERVABLE_INCLUDE" => {
                        let idx = instr.args().and_then(|a| a.first()).copied().unwrap_or(0.0) as usize;
                        while observables.len() <= idx { observables.push(Vec::new()); }
                        for t in targets {
                            if let StimTarget::Rec(r) = t {
                                let abs = (*meas_count as i64 + *r as i64) as usize;
                                observables[idx].push(abs);
                            }
                        }
                    }
                    _ => {}
                }
                *meas_count += meas_added;
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    collect_det_obs_instrs(body, meas_count, detectors, observables)?;
                }
            }
        }
    }
    Ok(())
}

fn count_measurements_op(name: &str, targets: &[crate::ir::StimTarget]) -> usize {
    use crate::ir::StimTarget;
    match name {
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" | "MZ" | "MRZ" => {
            targets.iter().filter(|t| matches!(t, StimTarget::Qubit(_) | StimTarget::QubitInv(_))).count()
        }
        "MXX" | "MYY" | "MZZ" => targets.len() / 2,
        "MPP" => targets.iter().filter(|t| matches!(t, StimTarget::Combiner)).count() + 1,
        "MPAD" => targets.len(),
        _ => 0,
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod m2d;
```

### Step 4: Add m2d CLI command

In `src/cli.rs`, add to `Commands`:

```rust
/// Convert measurement results to detection events
#[command(name = "m2d")]
M2d {
    #[arg(long = "in_format", default_value = "01")]
    in_format: String,
    #[arg(long = "out_format", default_value = "dets")]
    out_format: String,
    #[arg(long)]
    circuit: Option<String>,
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long = "append_observables")]
    append_observables: bool,
    #[arg(long)]
    shots: Option<usize>,
},
```

Add dispatch:

```rust
Some(Commands::M2d { in_format, out_format, circuit, r#in, out, append_observables, shots }) => {
    let circ_text = read_input(circuit.as_deref())?;
    let data = read_input_bytes(r#in.as_deref())?;
    let mut w = open_output(out.as_deref())?;
    run_m2d(&circ_text, &data, &in_format, &out_format, append_observables, shots, &mut w)
}
```

Add `run_m2d`:

```rust
pub fn run_m2d(
    circuit_text: &str,
    data: &[u8],
    in_format: &str,
    out_format: &str,
    append_observables: bool,
    shots: Option<usize>,
    out: &mut dyn Write,
) -> Result<(), String> {
    use crate::output::*;
    let instrs = crate::parser::parse_lines(circuit_text)?;
    let n_meas = crate::stats::num_measurements(&instrs);
    let table = match in_format {
        "01" => read_shots_01(data, n_meas)?,
        "b8" => read_shots_b8(data, n_meas)?,
        "r8" => read_shots_r8(data, n_meas)?,
        "hits" => read_shots_hits(data, n_meas)?,
        "ptb64" => {
            let n = shots.ok_or("--shots required for ptb64 input")?;
            read_shots_ptb64(data, n_meas, n)?
        }
        _ => return Err(format!("unknown in_format: {in_format}")),
    };
    let (dets, obs) = crate::m2d::measurements_to_detections(&instrs, &table)?;
    match out_format {
        "dets" => write_shots_dets(&dets, &obs, out).map_err(|e| e.to_string()),
        "01" => {
            if append_observables {
                // concatenate dets and obs horizontally — build combined table
                let n_dets = dets.num_major();
                let n_obs = obs.num_major();
                let n_shots = dets.num_minor();
                let mut combined = BitTable::new(n_dets + n_obs, n_shots);
                for s in 0..n_shots {
                    for d in 0..n_dets { if dets.get(d, s) { combined.set(d, s, true); } }
                    for o in 0..n_obs { if obs.get(o, s) { combined.set(n_dets + o, s, true); } }
                }
                write_shots_01(&combined, out).map_err(|e| e.to_string())
            } else {
                write_shots_01(&dets, out).map_err(|e| e.to_string())
            }
        }
        _ => Err(format!("unsupported out_format for m2d: {out_format}")),
    }
}
```

### Step 5: Run tests

```
cargo test
```
Expected: all pass.

### Step 6: Commit

```bash
git add src/m2d.rs src/lib.rs src/cli.rs tests/m2d.rs
git commit -m "feat: add m2d module and rstim m2d CLI subcommand"
```
