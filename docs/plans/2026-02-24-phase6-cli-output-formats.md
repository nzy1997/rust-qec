# Phase 6: CLI and Output Formats Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a usable command-line interface (`rstim sample`, `rstim detect`, `rstim analyze_errors`, `rstim sample_dem`) and multiple output formats (`01`, `b8`, `r8`, `hits`, `dets`) so that rstim can be used as a drop-in replacement for Stim's core CLI commands.

**Architecture:** The CLI uses `clap` for argument parsing, with subcommands mirroring Stim's interface. Output formats are implemented as a standalone module (`src/output.rs`) with `write_*` functions that stream results to any `Write` destination. Each subcommand reads a circuit/DEM from `--in` (or stdin), runs the appropriate simulation, and writes results in the specified format to `--out` (or stdout). The `BitTable` already stores data in the row-major bit-packed layout needed for efficient format conversion.

**Tech Stack:** Rust, `clap` (derive API), existing `rstim` library modules (`parser`, `sampler`, `error_analyzer`, `dem`)

---

## Task 1: Output Formats Module

**Files:**
- Create: `src/output.rs`
- Modify: `src/lib.rs` (add `pub mod output;`)
- Test: `tests/output_formats.rs`

### Step 1: Write the failing test

Create `tests/output_formats.rs`:

```rust
use rstim::output::{OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets};
use rstim::sim::bit_table::BitTable;

#[test]
fn format_01_simple() {
    let mut table = BitTable::new(2, 3); // 2 bits, 3 shots
    table.set(0, 0, true);  // shot 0: bit 0 = 1
    table.set(1, 0, true);  // shot 0: bit 1 = 1
    table.set(0, 1, false); // shot 1: bit 0 = 0
    table.set(1, 1, true);  // shot 1: bit 1 = 1
    // shot 2: all false
    let mut buf = Vec::new();
    write_shots_01(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "11\n01\n00\n");
}

#[test]
fn format_01_empty() {
    let table = BitTable::new(0, 3);
    let mut buf = Vec::new();
    write_shots_01(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "\n\n\n");
}

#[test]
fn format_b8_simple() {
    // 10 bits, 1 shot: bits 0,1,2,3 set = 0x0F, bits 4-9 clear = 0x00
    let mut table = BitTable::new(10, 1);
    for i in 0..4 { table.set(i, 0, true); }
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x0F, 0x00]); // ceil(10/8) = 2 bytes
}

#[test]
fn format_b8_bit_order() {
    // bits 0 and 7 set in a single byte
    let mut table = BitTable::new(8, 1);
    table.set(0, 0, true);
    table.set(7, 0, true);
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x81]); // bit 0 = 1s place, bit 7 = 128s place
}

#[test]
fn format_r8_no_hits() {
    let table = BitTable::new(5, 1); // all zeros, 5 bits
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    // Terminator at position 5: run of 5 zeros then implicit True
    assert_eq!(buf, vec![5]);
}

#[test]
fn format_r8_first_bit_set() {
    let mut table = BitTable::new(3, 1);
    table.set(0, 0, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    // bit 0 set: run of 0 before first True, then run of 2 before terminator True
    assert_eq!(buf, vec![0, 2]);
}

#[test]
fn format_r8_long_run() {
    // Single bit set at position 300
    let mut table = BitTable::new(301, 1);
    table.set(300, 0, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    // 300 = 255 + 45, so: 255 (no True), 45 (True at pos 300), 0 (terminator at pos 301)
    assert_eq!(buf, vec![255, 45, 0]);
}

#[test]
fn format_hits_simple() {
    let mut table = BitTable::new(5, 2);
    table.set(1, 0, true);
    table.set(3, 0, true);
    // shot 1: no hits
    let mut buf = Vec::new();
    write_shots_hits(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "1,3\n\n");
}

#[test]
fn format_dets_simple() {
    let mut dets = BitTable::new(3, 2);
    dets.set(1, 0, true);
    let mut obs = BitTable::new(2, 2);
    obs.set(0, 1, true);
    let mut buf = Vec::new();
    write_shots_dets(&dets, &obs, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "shot D1\nshot L0\n");
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test output_formats`
Expected: FAIL (module doesn't exist)

### Step 3: Write minimal implementation

Create `src/output.rs`:

```rust
use std::io::Write;
use crate::sim::bit_table::BitTable;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Format01,
    B8,
    R8,
    Hits,
    Dets,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "01" => Ok(Self::Format01),
            "b8" => Ok(Self::B8),
            "r8" => Ok(Self::R8),
            "hits" => Ok(Self::Hits),
            "dets" => Ok(Self::Dets),
            _ => Err(format!("unknown output format: {s}")),
        }
    }
}

/// Write shots in 01 (dense text) format.
/// BitTable layout: rows = bits per shot, columns = shots.
pub fn write_shots_01(table: &BitTable, w: &mut impl Write) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        for bit in 0..n_bits {
            write!(w, "{}", if table.get(bit, shot) { '1' } else { '0' })?;
        }
        writeln!(w)?;
    }
    Ok(())
}

/// Write shots in b8 (dense binary) format.
pub fn write_shots_b8(table: &BitTable, w: &mut impl Write) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    let bytes_per_shot = (n_bits + 7) / 8;
    for shot in 0..n_shots {
        for byte_idx in 0..bytes_per_shot {
            let mut byte: u8 = 0;
            for bit_in_byte in 0..8 {
                let bit = byte_idx * 8 + bit_in_byte;
                if bit < n_bits && table.get(bit, shot) {
                    byte |= 1u8 << bit_in_byte;
                }
            }
            w.write_all(&[byte])?;
        }
    }
    Ok(())
}

/// Write shots in r8 (sparse binary run-length) format.
pub fn write_shots_r8(table: &BitTable, w: &mut impl Write) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        let mut gap: usize = 0;
        // Process data bits, then an implicit terminator True
        for bit in 0..=n_bits {
            let is_true = if bit < n_bits { table.get(bit, shot) } else { true };
            if is_true {
                while gap >= 255 {
                    w.write_all(&[255])?;
                    gap -= 255;
                }
                w.write_all(&[gap as u8])?;
                gap = 0;
            } else {
                gap += 1;
            }
        }
    }
    Ok(())
}

/// Write shots in hits (sparse text, comma-separated indices) format.
pub fn write_shots_hits(table: &BitTable, w: &mut impl Write) -> std::io::Result<()> {
    let n_bits = table.num_major();
    let n_shots = table.num_minor();
    for shot in 0..n_shots {
        let mut first = true;
        for bit in 0..n_bits {
            if table.get(bit, shot) {
                if !first { write!(w, ",")?; }
                write!(w, "{}", bit)?;
                first = false;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}

/// Write shots in dets (sparse text with D/L prefixes) format.
pub fn write_shots_dets(
    detections: &BitTable,
    observable_flips: &BitTable,
    w: &mut impl Write,
) -> std::io::Result<()> {
    let n_dets = detections.num_major();
    let n_obs = observable_flips.num_major();
    let n_shots = detections.num_minor();
    for shot in 0..n_shots {
        write!(w, "shot")?;
        for d in 0..n_dets {
            if detections.get(d, shot) {
                write!(w, " D{d}")?;
            }
        }
        for o in 0..n_obs {
            if observable_flips.get(o, shot) {
                write!(w, " L{o}")?;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}
```

Add to `src/lib.rs`:
```rust
pub mod output;
```

### Step 4: Run test to verify it passes

Run: `cargo test --test output_formats`
Expected: PASS

### Step 5: Commit

```bash
git add src/output.rs src/lib.rs tests/output_formats.rs
git commit -m "feat: output formats module (01, b8, r8, hits, dets)"
```

---

## Task 2: CLI Framework with `rstim sample`

**Files:**
- Modify: `Cargo.toml` (add `clap` dependency)
- Modify: `src/main.rs` (replace with clap CLI)
- Test: `tests/cli_sample.rs`

### Step 1: Write the failing test

Create `tests/cli_sample.rs`:

```rust
use std::process::Command;

fn rstim_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn sample_01_format() {
    let output = rstim_bin()
        .args(["sample", "--shots", "3", "--out_format", "01"])
        .write_stdin("R 0\nM 0")
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3);
    for line in &lines {
        assert_eq!(line.len(), 1);
        assert!(line == &"0" || line == &"1");
    }
}

#[test]
fn sample_default_format_is_01() {
    let output = rstim_bin()
        .args(["sample", "--shots", "1"])
        .write_stdin("R 0\nM 0")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim() == "0" || stdout.trim() == "1");
}

#[test]
fn sample_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let circuit_path = dir.path().join("test.stim");
    std::fs::write(&circuit_path, "R 0\nX 0\nM 0").unwrap();
    let output = rstim_bin()
        .args(["sample", "--shots", "1", "--in", circuit_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
}

#[test]
fn sample_hits_format() {
    let output = rstim_bin()
        .args(["sample", "--shots", "1", "--out_format", "hits"])
        .write_stdin("R 0\nX 0\nM 0")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "0");
}

#[test]
fn sample_b8_format() {
    let output = rstim_bin()
        .args(["sample", "--shots", "1", "--out_format", "b8"])
        .write_stdin("R 0\nX 0\nM 0")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0x01]); // bit 0 set
}

#[test]
fn sample_seed_deterministic() {
    let out1 = rstim_bin()
        .args(["sample", "--shots", "10", "--seed", "42"])
        .write_stdin("H 0\nM 0")
        .output().unwrap();
    let out2 = rstim_bin()
        .args(["sample", "--shots", "10", "--seed", "42"])
        .write_stdin("H 0\nM 0")
        .output().unwrap();
    assert_eq!(out1.stdout, out2.stdout);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test cli_sample`
Expected: FAIL

### Step 3: Write minimal implementation

Add to `Cargo.toml`:
```toml
[dependencies]
rand = "0.8"
clap = { version = "4", features = ["derive"] }
```

Also add `tempfile` as a dev dependency for tests:
```toml
[dev-dependencies]
tempfile = "3"
```

Replace `src/main.rs`:

```rust
use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstim::output::{OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits};
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

#[derive(Parser)]
#[command(name = "rstim", version, about = "Rust stabilizer circuit simulator")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sample measurement results from a circuit
    Sample {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long, default_value = "01")]
        out_format: String,
        #[arg(long)]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Sample { shots, out_format, r#in, out, seed }) => {
            cmd_sample(shots.unwrap_or(1), &out_format, r#in.as_deref(), out.as_deref(), seed)
        }
        None => {
            println!("rstim {}", rstim::version());
            Ok(())
        }
    }
}

fn read_circuit(path: Option<&str>) -> Result<String, String> {
    match path {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("failed to read {p}: {e}")),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).map_err(|e| format!("failed to read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    }
}

fn open_output(path: Option<&str>) -> Result<Box<dyn Write>, String> {
    match path {
        Some(p) => {
            let f = std::fs::File::create(p).map_err(|e| format!("failed to create {p}: {e}"))?;
            Ok(Box::new(io::BufWriter::new(f)))
        }
        None => Ok(Box::new(io::BufWriter::new(io::stdout().lock()))),
    }
}

fn cmd_sample(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let circuit_text = read_circuit(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots as usize, &mut rng)?;
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Format01 => write_shots_01(&result.measurements, &mut out),
        OutputFormat::B8 => write_shots_b8(&result.measurements, &mut out),
        OutputFormat::R8 => write_shots_r8(&result.measurements, &mut out),
        OutputFormat::Hits => write_shots_hits(&result.measurements, &mut out),
        OutputFormat::Dets => return Err("dets format not applicable to sample command; use detect".to_string()),
    }.map_err(|e| format!("write error: {e}"))
}
```

### Step 4: Run test to verify it passes

Run: `cargo test --test cli_sample`
Expected: PASS

### Step 5: Commit

```bash
git add Cargo.toml src/main.rs tests/cli_sample.rs
git commit -m "feat: CLI framework with rstim sample command"
```

---

## Task 3: `rstim detect` Command

**Files:**
- Modify: `src/main.rs` (add Detect subcommand)
- Test: `tests/cli_detect.rs`

### Step 1: Write the failing test

Create `tests/cli_detect.rs`:

```rust
use std::process::Command;

fn rstim_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn detect_01_noiseless() {
    let output = rstim_bin()
        .args(["detect", "--shots", "3"])
        .write_stdin("R 0\nM 0\nDETECTOR rec[-1]")
        .output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    for line in s.trim().split('\n') {
        assert_eq!(line, "0");
    }
}

#[test]
fn detect_dets_format() {
    let output = rstim_bin()
        .args(["detect", "--shots", "1", "--out_format", "dets"])
        .write_stdin("R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert_eq!(s.trim(), "shot D0");
}

#[test]
fn detect_with_observable() {
    let output = rstim_bin()
        .args(["detect", "--shots", "1", "--out_format", "dets"])
        .write_stdin("R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn detect_append_observables() {
    let output = rstim_bin()
        .args(["detect", "--shots", "1", "--append_observables"])
        .write_stdin("R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    // 1 detector + 1 observable appended = "11"
    assert_eq!(s.trim(), "11");
}

#[test]
fn detect_seed_deterministic() {
    let circuit = "H 0\nM 0\nDETECTOR rec[-1]";
    let out1 = rstim_bin()
        .args(["detect", "--shots", "10", "--seed", "42"])
        .write_stdin(circuit)
        .output().unwrap();
    let out2 = rstim_bin()
        .args(["detect", "--shots", "10", "--seed", "42"])
        .write_stdin(circuit)
        .output().unwrap();
    assert_eq!(out1.stdout, out2.stdout);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test cli_detect`
Expected: FAIL

### Step 3: Write minimal implementation

Add to `src/main.rs` the `Detect` variant in the `Commands` enum:

```rust
    /// Sample detection events and observable flips from a circuit
    Detect {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long, default_value = "01")]
        out_format: String,
        #[arg(long)]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        append_observables: bool,
    },
```

Add `cmd_detect` function:

```rust
fn cmd_detect(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
    append_observables: bool,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let circuit_text = read_circuit(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots as usize, &mut rng)?;
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, &mut out)
                .map_err(|e| format!("write error: {e}"))
        }
        _ => {
            if append_observables {
                let merged = merge_detections_observables(&result.detections, &result.observable_flips);
                write_format(fmt, &merged, &mut out)
            } else {
                write_format(fmt, &result.detections, &mut out)
            }
        }
    }
}
```

Add helper functions `merge_detections_observables` and `write_format`:

```rust
fn merge_detections_observables(dets: &BitTable, obs: &BitTable) -> BitTable {
    let n_dets = dets.num_major();
    let n_obs = obs.num_major();
    let n_shots = dets.num_minor();
    let mut merged = BitTable::new(n_dets + n_obs, n_shots);
    for row in 0..n_dets {
        for shot in 0..n_shots {
            if dets.get(row, shot) { merged.set(row, shot, true); }
        }
    }
    for row in 0..n_obs {
        for shot in 0..n_shots {
            if obs.get(row, shot) { merged.set(n_dets + row, shot, true); }
        }
    }
    merged
}

fn write_format(fmt: OutputFormat, table: &BitTable, out: &mut impl Write) -> Result<(), String> {
    match fmt {
        OutputFormat::Format01 => write_shots_01(table, out),
        OutputFormat::B8 => write_shots_b8(table, out),
        OutputFormat::R8 => write_shots_r8(table, out),
        OutputFormat::Hits => write_shots_hits(table, out),
        OutputFormat::Dets => return Err("use write_shots_dets directly".to_string()),
    }.map_err(|e| format!("write error: {e}"))
}
```

Import `write_shots_dets` and `BitTable` in main.rs.

### Step 4: Run test to verify it passes

Run: `cargo test --test cli_detect`
Expected: PASS

### Step 5: Commit

```bash
git add src/main.rs tests/cli_detect.rs
git commit -m "feat: rstim detect command with dets/append_observables support"
```

---

## Task 4: `rstim analyze_errors` Command

**Files:**
- Modify: `src/main.rs` (add AnalyzeErrors subcommand)
- Test: `tests/cli_analyze.rs`

### Step 1: Write the failing test

Create `tests/cli_analyze.rs`:

```rust
use std::process::Command;

fn rstim_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn analyze_errors_basic() {
    let output = rstim_bin()
        .args(["analyze_errors"])
        .write_stdin("R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]")
        .output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("error(0.1)"));
    assert!(s.contains("D0"));
}

#[test]
fn analyze_errors_with_observable() {
    let output = rstim_bin()
        .args(["analyze_errors"])
        .write_stdin("R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn analyze_errors_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.stim");
    std::fs::write(&path, "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let output = rstim_bin()
        .args(["analyze_errors", "--in", path.to_str().unwrap()])
        .output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("error"));
}

#[test]
fn analyze_errors_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("out.dem");
    let output = rstim_bin()
        .args(["analyze_errors", "--out", out_path.to_str().unwrap()])
        .write_stdin("R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    let dem = std::fs::read_to_string(&out_path).unwrap();
    assert!(dem.contains("error"));
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test cli_analyze`
Expected: FAIL

### Step 3: Write minimal implementation

Add `AnalyzeErrors` variant to `Commands` enum:

```rust
    /// Convert a circuit into a detector error model
    AnalyzeErrors {
        #[arg(long)]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
```

Add `cmd_analyze_errors`:

```rust
fn cmd_analyze_errors(
    in_path: Option<&str>,
    out_path: Option<&str>,
) -> Result<(), String> {
    let circuit_text = read_circuit(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs)?;
    let dem_str = dem.to_string();
    let mut out = open_output(out_path)?;
    out.write_all(dem_str.as_bytes()).map_err(|e| format!("write error: {e}"))
}
```

Import `ErrorAnalyzer`.

### Step 4: Run test to verify it passes

Run: `cargo test --test cli_analyze`
Expected: PASS

### Step 5: Commit

```bash
git add src/main.rs tests/cli_analyze.rs
git commit -m "feat: rstim analyze_errors command"
```

---

## Task 5: `rstim sample_dem` Command

**Files:**
- Modify: `src/main.rs` (add SampleDem subcommand)
- Test: `tests/cli_sample_dem.rs`

### Step 1: Write the failing test

Create `tests/cli_sample_dem.rs`:

```rust
use std::process::Command;

fn rstim_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn sample_dem_01_format() {
    let output = rstim_bin()
        .args(["sample_dem", "--shots", "3"])
        .write_stdin("error(1) D0 L0")
        .output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    for line in s.trim().split('\n') {
        assert_eq!(line, "1");
    }
}

#[test]
fn sample_dem_dets_format() {
    let output = rstim_bin()
        .args(["sample_dem", "--shots", "1", "--out_format", "dets"])
        .write_stdin("error(1) D0 D1")
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert_eq!(s.trim(), "shot D0 D1");
}

#[test]
fn sample_dem_seed_deterministic() {
    let dem = "error(0.5) D0";
    let out1 = rstim_bin()
        .args(["sample_dem", "--shots", "10", "--seed", "42"])
        .write_stdin(dem).output().unwrap();
    let out2 = rstim_bin()
        .args(["sample_dem", "--shots", "10", "--seed", "42"])
        .write_stdin(dem).output().unwrap();
    assert_eq!(out1.stdout, out2.stdout);
}

#[test]
fn sample_dem_obs_out() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = rstim_bin()
        .args(["sample_dem", "--shots", "1", "--obs_out", obs_path.to_str().unwrap()])
        .write_stdin("error(1) D0 L0")
        .output().unwrap();
    assert!(output.status.success());
    let det_out = String::from_utf8(output.stdout).unwrap();
    assert_eq!(det_out.trim(), "1"); // D0
    let obs_out = std::fs::read_to_string(&obs_path).unwrap();
    assert_eq!(obs_out.trim(), "1"); // L0
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test cli_sample_dem`
Expected: FAIL

### Step 3: Write minimal implementation

Add `SampleDem` variant:

```rust
    /// Sample detection events from a detector error model
    SampleDem {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long, default_value = "01")]
        out_format: String,
        #[arg(long)]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        obs_out: Option<String>,
        #[arg(long, default_value = "01")]
        obs_out_format: String,
    },
```

Add `cmd_sample_dem`:

```rust
fn cmd_sample_dem(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
    obs_out: Option<&str>,
    obs_out_format: &str,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let dem_text = read_circuit(in_path)?; // same reader for DEM text
    let dem = DetectorErrorModel::parse(&dem_text)?;
    let mut rng = make_rng(seed);
    let result = dem.sample_batch(shots as usize, &mut rng);
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, &mut out)
                .map_err(|e| format!("write error: {e}"))?;
        }
        _ => {
            write_format(fmt, &result.detections, &mut out)?;
        }
    }
    if let Some(obs_path) = obs_out {
        let obs_fmt = OutputFormat::from_str(obs_out_format)?;
        let mut obs_out = open_output(Some(obs_path))?;
        write_format(obs_fmt, &result.observable_flips, &mut obs_out)?;
    }
    Ok(())
}
```

Import `DetectorErrorModel`.

### Step 4: Run test to verify it passes

Run: `cargo test --test cli_sample_dem`
Expected: PASS

### Step 5: Commit

```bash
git add src/main.rs tests/cli_sample_dem.rs
git commit -m "feat: rstim sample_dem command"
```

---

## Task 6: Integration Smoke Tests

**Files:**
- Test: `tests/cli_integration.rs`

### Step 1: Write the test

Create `tests/cli_integration.rs`:

```rust
use std::process::Command;

fn rstim_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn version_prints() {
    let output = rstim_bin().output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("rstim"));
}

#[test]
fn sample_r8_format() {
    let output = rstim_bin()
        .args(["sample", "--shots", "1", "--out_format", "r8"])
        .write_stdin("R 0\nX 0\nM 0")
        .output().unwrap();
    assert!(output.status.success());
    // bit 0 set: r8 = [0 (run before bit 0), 0 (run before terminator)]
    assert_eq!(output.stdout, vec![0, 0]);
}

#[test]
fn detect_r8_format() {
    let output = rstim_bin()
        .args(["detect", "--shots", "1", "--out_format", "r8"])
        .write_stdin("R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]")
        .output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0, 0]); // D0 fired
}

#[test]
fn pipeline_analyze_then_sample_dem() {
    // analyze_errors produces DEM, then sample_dem consumes it
    let analyze_out = rstim_bin()
        .args(["analyze_errors"])
        .write_stdin("R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]")
        .output().unwrap();
    assert!(analyze_out.status.success());

    let dem_text = String::from_utf8(analyze_out.stdout).unwrap();
    let sample_out = rstim_bin()
        .args(["sample_dem", "--shots", "1", "--out_format", "dets"])
        .write_stdin(&dem_text)
        .output().unwrap();
    assert!(sample_out.status.success());
    let s = String::from_utf8(sample_out.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn invalid_subcommand_fails() {
    let output = rstim_bin()
        .args(["nonexistent"])
        .output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn sample_invalid_format_fails() {
    let output = rstim_bin()
        .args(["sample", "--shots", "1", "--out_format", "unknown"])
        .write_stdin("R 0\nM 0")
        .output().unwrap();
    assert!(!output.status.success());
}
```

### Step 2: Run test to verify it passes

Run: `cargo test --test cli_integration`
Expected: PASS (all commands already implemented)

### Step 3: Commit

```bash
git add tests/cli_integration.rs
git commit -m "test: CLI integration smoke tests with pipeline test"
```
