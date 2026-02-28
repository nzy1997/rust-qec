# Phase 8: sweep[] Targets + ptb64 Output Format Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `sweep[k]` classical-control target support to the IR/parser/executor, and add the `ptb64` partially-transposed bit-packed binary output format.

**Architecture:** `sweep[k]` is a new `StimTarget::Sweep(u32)` variant; in the tableau path it is treated as always-0 (no-op); in the frame path it is also treated as 0 (no sweep table provided). `ptb64` writes bits transposed: for each group of 64 shots, write one u64 word per detector (bit k = shot k within the group), iterating detectors in order.

**Tech Stack:** Rust, existing `rstim` library modules, `clap` for CLI

---

## Task 1: sweep[] IR + Parser

**Files:**
- Modify: `src/ir.rs`
- Modify: `src/parser.rs`
- Test: `tests/sweep.rs`

### Step 1: Write the failing test

Create `tests/sweep.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::ir::StimTarget;

#[test]
fn parse_sweep_target() {
    let instrs = parse_lines("CX sweep[0] 1").unwrap();
    assert_eq!(instrs.len(), 1);
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets[0], StimTarget::Sweep(0));
    assert_eq!(targets[1], StimTarget::Qubit(1));
}

#[test]
fn parse_sweep_large_index() {
    let instrs = parse_lines("M sweep[99]").unwrap();
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets[0], StimTarget::Sweep(99));
}

#[test]
fn sweep_roundtrip() {
    use rstim::ir::circuit_to_string;
    let src = "CX sweep[0] 1\n";
    let instrs = parse_lines(src).unwrap();
    assert_eq!(circuit_to_string(&instrs), src);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test sweep
```
Expected: compile error — `StimTarget::Sweep` does not exist.

### Step 3: Add `Sweep` variant to IR

In `src/ir.rs`, add to `StimTarget` enum:

```rust
Sweep(u32),
```

In `circuit_to_string` / `write_instrs`, add to the target match arm:

```rust
StimTarget::Sweep(k) => write!(s, "sweep[{k}]").unwrap(),
```

In `qubit_index()` impl, add:

```rust
StimTarget::Sweep(_) => None,
```

### Step 4: Add sweep parsing to parser

In `src/parser.rs`, in `parse_target`, add before the final `Err`:

```rust
if token.starts_with("sweep[") && token.ends_with(']') {
    let inner = &token[6..token.len() - 1];
    let k: u32 = inner.parse().map_err(|_| format!("bad sweep target {token}"))?;
    return Ok(Some(StimTarget::Sweep(k)));
}
```

### Step 5: Run tests

```
cargo test --test sweep
```
Expected: all 3 tests pass.

### Step 6: Commit

```bash
git add src/ir.rs src/parser.rs tests/sweep.rs
git commit -m "feat: add sweep[] target to IR and parser"
```

---

## Task 2: sweep[] in Executor (tableau + frame)

**Files:**
- Modify: `src/executor.rs`
- Modify: `src/sim/frame.rs`
- Test: `tests/sweep.rs` (extend)

### Step 1: Write the failing test

Add to `tests/sweep.rs`:

```rust
use rstim::executor::Executor;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[test]
fn sweep_target_executes_without_error() {
    // sweep[k] treated as 0 — CX with sweep[0]=0 is identity on qubit 1
    let instrs = parse_lines("CX sweep[0] 1\nM 1").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    assert_eq!(out.measurements, vec![false]);
}

#[test]
fn sweep_in_frame_sim() {
    use rstim::executor::sample_batch;
    let instrs = parse_lines("CX sweep[0] 1\nM 1").unwrap();
    let result = sample_batch(&instrs, 4, Some(0)).unwrap();
    // all shots: qubit 1 unmeasured = 0
    for shot in 0..4 {
        assert!(!result.measurements.get(0, shot));
    }
}
```

### Step 2: Run test to verify it fails

```
cargo test --test sweep sweep_target_executes
```
Expected: FAIL — executor errors on unknown target type.

### Step 3: Handle Sweep in executor tableau path

In `src/executor.rs`, in `for_each_qubit` or wherever qubit targets are iterated, `Sweep` targets should be skipped (treated as qubit index 0 is wrong — they should be ignored entirely for the no-sweep-table case).

Find the `for_each_qubit` helper and add a guard, or handle `StimTarget::Sweep` explicitly in the measurement arms. The simplest approach: in the measurement ops (`M`, `MX`, `MY`, `MR`, etc.), skip `Sweep` targets silently. For gate ops, skip them too.

In `src/executor.rs`, update `for_each_qubit`:

```rust
fn for_each_qubit(
    targets: &[StimTarget],
    mut f: impl FnMut(u32) -> Result<(), String>,
) -> Result<(), String> {
    for t in targets {
        match t {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => f(*q)?,
            StimTarget::Sweep(_) => {} // treated as 0, skip
            _ => {}
        }
    }
    Ok(())
}
```

For measurement instructions that iterate targets directly, add `StimTarget::Sweep(_) => { /* no-op */ }` match arms.

### Step 4: Handle Sweep in frame simulator

In `src/sim/frame.rs`, in measurement instruction handling, add `StimTarget::Sweep(_) => {}` arms wherever targets are matched.

### Step 5: Run tests

```
cargo test --test sweep
```
Expected: all tests pass.

### Step 6: Commit

```bash
git add src/executor.rs src/sim/frame.rs tests/sweep.rs
git commit -m "feat: handle sweep[] targets in executor and frame simulator (no-op)"
```

---

## Task 3: ptb64 Output Format

**Files:**
- Modify: `src/output.rs`
- Modify: `src/cli.rs`
- Test: `tests/output_formats.rs` (extend or create)

### Step 1: Write the failing test

Create `tests/output_formats.rs` (or add to existing):

```rust
use rstim::output::{OutputFormat, write_shots_ptb64};
use rstim::sim::bit_table::BitTable;

#[test]
fn ptb64_format_parses() {
    assert!(OutputFormat::from_str("ptb64").is_ok());
}

#[test]
fn ptb64_single_shot_single_bit() {
    // 1 bit, 1 shot, bit=1 → one u64 word = 0x0000000000000001
    let mut table = BitTable::new(1, 1);
    table.set(0, 0, true);
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 8);
    assert_eq!(u64::from_le_bytes(out[..8].try_into().unwrap()), 1u64);
}

#[test]
fn ptb64_two_bits_two_shots() {
    // 2 bits, 2 shots
    // shot0: bit0=1, bit1=0
    // shot1: bit0=0, bit1=1
    // ptb64 layout: for each 64-shot chunk, write one u64 per bit
    // bit0 word: shot0=1, shot1=0 → 0b01 = 1
    // bit1 word: shot0=0, shot1=1 → 0b10 = 2
    let mut table = BitTable::new(2, 2);
    table.set(0, 0, true);
    table.set(1, 1, true);
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 16); // 2 words × 8 bytes
    let w0 = u64::from_le_bytes(out[0..8].try_into().unwrap());
    let w1 = u64::from_le_bytes(out[8..16].try_into().unwrap());
    assert_eq!(w0, 1u64); // bit0: shot0 set
    assert_eq!(w1, 2u64); // bit1: shot1 set
}

#[test]
fn ptb64_more_than_64_shots() {
    // 1 bit, 65 shots, all set → two chunks
    // chunk0: 64 shots all set → 0xFFFFFFFFFFFFFFFF
    // chunk1: 1 shot set → 0x0000000000000001
    let mut table = BitTable::new(1, 65);
    for s in 0..65 { table.set(0, s, true); }
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 16);
    let w0 = u64::from_le_bytes(out[0..8].try_into().unwrap());
    let w1 = u64::from_le_bytes(out[8..16].try_into().unwrap());
    assert_eq!(w0, u64::MAX);
    assert_eq!(w1, 1u64);
}
```

### Step 2: Run test to verify it fails

```
cargo test --test output_formats
```
Expected: compile error — `write_shots_ptb64` not found, `OutputFormat::Ptb64` not found.

### Step 3: Implement ptb64

In `src/output.rs`:

Add `Ptb64` to `OutputFormat` enum:

```rust
pub enum OutputFormat {
    Format01,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}
```

Add `"ptb64"` to `from_str`:

```rust
"ptb64" => Ok(Self::Ptb64),
```

Add the writer function:

```rust
/// Partially-transposed bit-packed binary.
/// Shots are grouped in chunks of 64. For each chunk, write one u64 per bit:
/// bit k of the word = value of that bit in shot (chunk_start + k).
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
```

### Step 4: Wire ptb64 into CLI dispatch

In `src/cli.rs`, find where `OutputFormat::from_str` is called and where formats are dispatched to writers. Add `ptb64` to the accepted format strings. In the `sample` and `detect` dispatch, handle `OutputFormat::Ptb64` by calling `write_shots_ptb64`.

### Step 5: Run tests

```
cargo test --test output_formats
cargo test
```
Expected: all pass.

### Step 6: Commit

```bash
git add src/output.rs src/cli.rs tests/output_formats.rs
git commit -m "feat: add ptb64 output format"
```
